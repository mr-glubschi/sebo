//! Abruf fremder URLs mit SSRF-Schutz.
//!
//! Jede Verbindung läuft über einen eigenen DNS-Resolver, der nur erlaubte
//! IP-Adressen zurückgibt. reqwest verbindet sich genau mit diesen Adressen,
//! dadurch ist die geprüfte IP auch die verwendete (kein DNS-Rebinding
//! zwischen Prüfung und Verbindung). Weiterleitungen werden einzeln verfolgt
//! und jedes Ziel neu geprüft. IP-Adressen direkt in der URL werden vor dem
//! Abruf geprüft, weil sie den Resolver nicht durchlaufen.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use reqwest::{
    StatusCode,
    dns::{Addrs, Name, Resolve, Resolving},
    header, redirect,
};
use url::{Host, Url};

pub const MAX_REDIRECTS: usize = 5;
const TIMEOUT: Duration = Duration::from_secs(5);
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; SEBO/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/mr-glubschi/sebo)"
);

#[derive(Debug)]
pub enum FetchError {
    /// Ziel ist eine interne Adresse und `ALLOW_PRIVATE_URLS` ist aus.
    Blocked,
    TooLarge,
    Status(StatusCode),
    Other(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked => write!(f, "interne Adresse gesperrt"),
            Self::TooLarge => write!(f, "Antwort zu groß"),
            Self::Status(status) => write!(f, "HTTP {status}"),
            Self::Other(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Debug)]
struct BlockedError;

impl std::fmt::Display for BlockedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "interne Adresse gesperrt")
    }
}

impl std::error::Error for BlockedError {}

impl From<reqwest::Error> for FetchError {
    fn from(error: reqwest::Error) -> Self {
        // Kam der Fehler von unserem Resolver?
        let mut source: Option<&dyn std::error::Error> = Some(&error);
        while let Some(current) = source {
            if current.is::<BlockedError>() {
                return Self::Blocked;
            }
            source = current.source();
        }
        if error.is_timeout() {
            Self::Other("Zeitüberschreitung".into())
        } else if error.is_connect() {
            Self::Other("Verbindung fehlgeschlagen".into())
        } else {
            Self::Other("Abruf fehlgeschlagen".into())
        }
    }
}

pub struct Response {
    pub url: Url,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct Fetcher {
    client: reqwest::Client,
    allow_private: bool,
}

impl Fetcher {
    pub fn new(allow_private: bool) -> Result<Self, String> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("TLS-Einrichtung fehlgeschlagen: {error}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();

        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .dns_resolver(GuardedResolver { allow_private })
            .redirect(redirect::Policy::none())
            .no_proxy()
            .timeout(TIMEOUT)
            .connect_timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| format!("HTTP-Client konnte nicht erstellt werden: {error}"))?;

        Ok(Self {
            client,
            allow_private,
        })
    }

    /// Ruft `url` ab und folgt Weiterleitungen. Höchstens `max_bytes` werden
    /// gelesen; bei `truncate` wird danach abgeschnitten, sonst ist es ein Fehler.
    pub async fn get(
        &self,
        url: &Url,
        accept: &str,
        max_bytes: usize,
        truncate: bool,
    ) -> Result<Response, FetchError> {
        let mut current = url.clone();
        for _ in 0..=MAX_REDIRECTS {
            check_url(&current, self.allow_private)?;

            let mut response = self
                .client
                .get(current.clone())
                .header(header::ACCEPT, accept)
                .send()
                .await?;

            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| FetchError::Other("Weiterleitung ohne Ziel".into()))?;
                current = current
                    .join(location)
                    .map_err(|_| FetchError::Other("ungültiges Weiterleitungsziel".into()))?;
                continue;
            }
            if !response.status().is_success() {
                return Err(FetchError::Status(response.status()));
            }
            if !truncate
                && response
                    .content_length()
                    .is_some_and(|length| length > max_bytes as u64)
            {
                return Err(FetchError::TooLarge);
            }

            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_ascii_lowercase());

            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                let remaining = max_bytes - body.len();
                if chunk.len() > remaining {
                    if truncate {
                        body.extend_from_slice(&chunk[..remaining]);
                        break;
                    }
                    return Err(FetchError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }

            return Ok(Response {
                url: current,
                content_type,
                body,
            });
        }
        Err(FetchError::Other("zu viele Weiterleitungen".into()))
    }
}

/// Prüft Schema und (bei IP-Literalen) die Adresse.
pub fn check_url(url: &Url, allow_private: bool) -> Result<(), FetchError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::Other("nur http und https".into()));
    }
    let ip = match url.host() {
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
        Some(Host::Domain(_)) => return Ok(()),
        None => return Err(FetchError::Other("URL ohne Host".into())),
    };
    if !allow_private && is_forbidden(ip) {
        return Err(FetchError::Blocked);
    }
    Ok(())
}

struct GuardedResolver {
    allow_private: bool,
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_private = self.allow_private;
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|address| allow_private || !is_forbidden(address.ip()))
                .collect();
            if addresses.is_empty() {
                return Err(Box::new(BlockedError) as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

/// Adressen, die SEBO ohne `ALLOW_PRIVATE_URLS` nicht abrufen darf:
/// alles, was nicht eindeutig öffentlich ist.
pub fn is_forbidden(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_forbidden_v4(ip),
        IpAddr::V6(ip) => is_forbidden_v6(ip),
    }
}

fn is_forbidden_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local() // 169.254.0.0/16, auch 169.254.169.254
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0 // 0.0.0.0/8
        || (a == 100 && (64..128).contains(&b)) // 100.64.0.0/10 CGNAT
        || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24
        || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15
        || a >= 240 // 240.0.0.0/4 reserviert
}

fn is_forbidden_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    // Eingebettete IPv4-Adressen nach den IPv4-Regeln prüfen.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_forbidden_v4(v4);
    }
    let embedded_v4 = |high: u16, low: u16| {
        Ipv4Addr::new((high >> 8) as u8, high as u8, (low >> 8) as u8, low as u8)
    };
    if segments[..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
        // NAT64 64:ff9b::/96
        return is_forbidden_v4(embedded_v4(segments[6], segments[7]));
    }
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 0x0001 {
        // Lokale NAT64-Präfixe 64:ff9b:1::/48 (RFC 8215), Lage der IPv4 variabel.
        return true;
    }
    if segments[0] == 0x2002 {
        // 6to4 2002::/16
        return is_forbidden_v4(embedded_v4(segments[1], segments[2]));
    }
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || segments[..6] == [0, 0, 0, 0, 0, 0] // IPv4-kompatibel ::/96
        || (segments[0] & 0xfe00) == 0xfc00 // fc00::/7 Unique Local
        || (segments[0] & 0xffc0) == 0xfe80 // fe80::/10 Link-Local
        || (segments[0] & 0xffc0) == 0xfec0 // fec0::/10 Site-Local
        || (segments[0] == 0x2001 && segments[1] == 0x0db8) // Dokumentation
        || (segments[0] == 0x2001 && segments[1] == 0x0000) // Teredo
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forbidden(ip: &str) -> bool {
        is_forbidden(ip.parse().unwrap())
    }

    #[test]
    fn interne_adressen_gesperrt() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.178.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.1",
            "::ffff:127.0.0.1",
            "64:ff9b::c0a8:0101",
            "2002:c0a8:0101::1",
            "::192.168.1.1",
            "64:ff9b:1::808:808",
        ] {
            assert!(forbidden(ip), "{ip} sollte gesperrt sein");
        }
    }

    #[test]
    fn oeffentliche_adressen_erlaubt() {
        for ip in [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
            "64:ff9b::0808:0808",
        ] {
            assert!(!forbidden(ip), "{ip} sollte erlaubt sein");
        }
    }

    #[test]
    fn ip_literale_in_urls() {
        let check = |url: &str, allow| check_url(&Url::parse(url).unwrap(), allow);
        assert!(matches!(
            check("http://192.168.1.1/", false),
            Err(FetchError::Blocked)
        ));
        assert!(matches!(
            check("http://[::1]:8080/", false),
            Err(FetchError::Blocked)
        ));
        assert!(matches!(
            check("http://2130706433/", false),
            Err(FetchError::Blocked)
        ));
        assert!(check("http://192.168.1.1/", true).is_ok());
        assert!(check("https://example.com/", false).is_ok());
        assert!(check("ftp://example.com/", false).is_err());
    }

    /// Braucht Internet: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn echter_abruf_ueber_https() {
        let fetcher = Fetcher::new(false).unwrap();
        let url = Url::parse("https://www.rust-lang.org/").unwrap();
        let response = fetcher
            .get(&url, "text/html", 1024 * 1024, true)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&response.body);
        assert!(html.contains("<title>"), "kein HTML erhalten");
    }

    #[tokio::test]
    async fn resolver_sperrt_localhost() {
        let fetcher = Fetcher::new(false).unwrap();
        let url = Url::parse("http://localhost:9/").unwrap();
        let result = fetcher.get(&url, "*/*", 1000, true).await;
        assert!(
            matches!(result, Err(FetchError::Blocked)),
            "{:?}",
            result.err()
        );
    }
}
