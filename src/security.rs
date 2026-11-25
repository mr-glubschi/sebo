//! Sicherheits-Header, CSRF-Schutz und Wartezeit nach Fehlversuchen.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{api::ApiError, state::AppState};

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
    connect-src 'self'; font-src 'self'; manifest-src 'self'; object-src 'none'; \
    base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Setzt Sicherheits-Header für alle Antworten.
pub async fn headers(request: Request, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    if is_api {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// CSRF-Schutz: Ändernde Anfragen müssen von derselben Herkunft kommen.
pub async fn csrf(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return next.run(request).await;
    }

    let headers = request.headers();
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let host = expected_host(headers, state.config.trust_proxy);

    if origin_matches(origin, host) {
        next.run(request).await
    } else {
        tracing::warn!(
            "Anfrage abgewiesen: Herkunft {:?} passt nicht zu Host {:?}",
            origin,
            host
        );
        ApiError::new(StatusCode::FORBIDDEN, "Ungültige Herkunft der Anfrage").into_response()
    }
}

fn expected_host(headers: &HeaderMap, trust_proxy: bool) -> Option<&str> {
    let forwarded = trust_proxy
        .then(|| headers.get("x-forwarded-host"))
        .flatten()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim);
    forwarded.or_else(|| headers.get(header::HOST).and_then(|v| v.to_str().ok()))
}

fn origin_matches(origin: Option<&str>, host: Option<&str>) -> bool {
    let (Some(origin), Some(host)) = (origin, host) else {
        return false;
    };
    origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .is_some_and(|authority| authority.eq_ignore_ascii_case(host))
}

/// Kam die Anfrage per HTTPS? Nur hinter einem vertrauenswürdigen Proxy erkennbar.
pub fn is_https(headers: &HeaderMap, trust_proxy: bool) -> bool {
    trust_proxy
        && headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"))
}

/// Optionale Liste erlaubter Hostnamen (`ALLOWED_HOSTS`). Schützt vor
/// DNS-Rebinding: Eine fremde Domain, die auf SEBO im Heimnetz zeigt,
/// wird abgewiesen. `/healthz` bleibt immer erreichbar (Docker-Healthcheck).
pub async fn allowed_host(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let allowed = &state.config.allowed_hosts;
    if allowed.is_empty() || request.uri().path() == "/healthz" {
        return next.run(request).await;
    }
    let host = expected_host(request.headers(), state.config.trust_proxy);
    if host.is_some_and(|host| host_allowed(host, allowed)) {
        next.run(request).await
    } else {
        tracing::warn!("Anfrage abgewiesen: Host {host:?} steht nicht in ALLOWED_HOSTS");
        ApiError::new(StatusCode::MISDIRECTED_REQUEST, "Unbekannter Host").into_response()
    }
}

/// Ein Eintrag passt auf den Host mit oder ohne Port.
fn host_allowed(host: &str, allowed: &[String]) -> bool {
    let host = host.to_ascii_lowercase();
    let without_port = strip_port(&host);
    allowed
        .iter()
        .any(|entry| *entry == host || *entry == without_port)
}

fn strip_port(host: &str) -> &str {
    if host.starts_with('[') {
        // IPv6: [::1]:8080
        return host
            .split_once("]:")
            .map_or(host, |(ip, _)| &host[..=ip.len()]);
    }
    match host.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    }
}

/// Globale, ansteigende Wartezeit nach fehlgeschlagenen Logins. Bewusst
/// nicht pro IP: Bei einem Benutzer ist das robust und nicht durch
/// gefälschte Proxy-Header zu umgehen.
///
/// Zusätzlich werden Fehlversuche mit richtigem Passwort, aber falschem
/// Code über 24 Stunden gezählt. Ein erfolgreicher Login setzt diese
/// Zählung nicht zurück, damit jemand, der das Passwort kennt, den
/// TOTP-Code nicht langsam durchprobieren kann.
#[derive(Debug, Default)]
pub struct Throttle {
    failures: u32,
    code_failures: VecDeque<Instant>,
    blocked_until: Option<Instant>,
}

pub const MAX_DELAY: Duration = Duration::from_secs(300);
pub const MAX_CODE_DELAY: Duration = Duration::from_secs(60 * 60);
const CODE_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

impl Throttle {
    pub fn wait_time(&self, now: Instant) -> Option<Duration> {
        self.blocked_until
            .and_then(|until| until.checked_duration_since(now))
            .filter(|wait| !wait.is_zero())
    }

    /// Falsches Passwort.
    pub fn record_failure(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        self.update_block(now);
    }

    /// Richtiges Passwort, falscher Code.
    pub fn record_code_failure(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        self.code_failures.push_back(now);
        self.update_block(now);
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.blocked_until = None;
    }

    fn update_block(&mut self, now: Instant) {
        while self
            .code_failures
            .front()
            .is_some_and(|first| now.duration_since(*first) > CODE_WINDOW)
        {
            self.code_failures.pop_front();
        }
        let delay = delay_for(self.failures, MAX_DELAY)
            .max(delay_for(self.code_failures.len() as u32, MAX_CODE_DELAY));
        self.blocked_until = (!delay.is_zero()).then(|| now + delay);
    }
}

/// Die ersten 3 Fehlversuche ohne Wartezeit, danach 2, 4, 8 … Sekunden bis `max`.
fn delay_for(failures: u32, max: Duration) -> Duration {
    if failures <= 3 {
        return Duration::ZERO;
    }
    let exponent = (failures - 3).min(16);
    Duration::from_secs(2u64.pow(exponent)).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_fehlversuche_bleiben_nach_erfolg() {
        let now = Instant::now();
        let mut throttle = Throttle::default();
        // Angreifer kennt das Passwort: 3 falsche Codes, dann ein echter Login.
        for _ in 0..3 {
            throttle.record_code_failure(now);
        }
        throttle.record_success();
        // Der nächste falsche Code führt trotzdem sofort zu einer Wartezeit.
        throttle.record_code_failure(now);
        assert_eq!(throttle.wait_time(now), Some(Duration::from_secs(2)));
        // Nach 24 Stunden zählen alte Fehlversuche nicht mehr.
        let later = now + CODE_WINDOW + Duration::from_secs(1);
        throttle.record_success();
        throttle.record_code_failure(later);
        assert_eq!(throttle.wait_time(later), None);
    }

    #[test]
    fn erlaubte_hosts() {
        let allowed = vec!["sebo.example.com".to_string(), "192.168.1.5".to_string()];
        assert!(host_allowed("sebo.example.com", &allowed));
        assert!(host_allowed("SEBO.example.com:443", &allowed));
        assert!(host_allowed("192.168.1.5:8080", &allowed));
        assert!(!host_allowed("boese.example", &allowed));
        assert!(!host_allowed("sebo.example.com.boese.example", &allowed));
        assert_eq!(strip_port("[::1]:8080"), "[::1]");
        assert_eq!(strip_port("[::1]"), "[::1]");
        assert_eq!(strip_port("host"), "host");
    }

    #[test]
    fn wartezeit_steigt_an() {
        assert_eq!(delay_for(1, MAX_DELAY), Duration::ZERO);
        assert_eq!(delay_for(3, MAX_DELAY), Duration::ZERO);
        assert_eq!(delay_for(4, MAX_DELAY), Duration::from_secs(2));
        assert_eq!(delay_for(5, MAX_DELAY), Duration::from_secs(4));
        assert_eq!(delay_for(20, MAX_DELAY), MAX_DELAY);
        assert_eq!(delay_for(u32::MAX, MAX_DELAY), MAX_DELAY);
        assert_eq!(delay_for(20, MAX_CODE_DELAY), MAX_CODE_DELAY);

        let now = Instant::now();
        let mut throttle = Throttle::default();
        for _ in 0..4 {
            throttle.record_failure(now);
        }
        assert_eq!(throttle.wait_time(now), Some(Duration::from_secs(2)));
        assert_eq!(throttle.wait_time(now + Duration::from_secs(3)), None);
        throttle.record_success();
        assert_eq!(throttle.wait_time(now), None);
    }

    #[test]
    fn herkunft_pruefen() {
        assert!(origin_matches(
            Some("http://192.168.1.5:8080"),
            Some("192.168.1.5:8080")
        ));
        assert!(origin_matches(
            Some("https://sebo.example.com"),
            Some("sebo.example.com")
        ));
        assert!(!origin_matches(
            Some("https://boese.example"),
            Some("sebo.example.com")
        ));
        assert!(!origin_matches(Some("null"), Some("sebo.example.com")));
        assert!(!origin_matches(None, Some("sebo.example.com")));
    }

    #[test]
    fn proxy_header_nur_mit_trust_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "intern:8080".parse().unwrap());
        headers.insert("x-forwarded-host", "sebo.example.com".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(expected_host(&headers, false), Some("intern:8080"));
        assert_eq!(expected_host(&headers, true), Some("sebo.example.com"));
        assert!(!is_https(&headers, false));
        assert!(is_https(&headers, true));
    }
}
