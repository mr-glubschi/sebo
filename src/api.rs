//! JSON-API unter `/api`.

use std::{net::SocketAddr, time::Instant};

use axum::{
    Json, Router,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    security::is_https,
    session,
    state::AppState,
    two_factor::{self, CodeCheck},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .merge(crate::library_api::router())
        .fallback(|| async { ApiError::new(StatusCode::NOT_FOUND, "Nicht gefunden") })
}

/// Fehlerantwort als JSON: `{ "error": "…" }`.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!("Interner Fehler: {error}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Interner Fehler")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = match self.retry_after {
            Some(seconds) => json!({ "error": self.message, "retry_after": seconds }),
            None => json!({ "error": self.message }),
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after
            && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        response
    }
}

impl<E: std::fmt::Display> From<tokio_rusqlite::Error<E>> for ApiError {
    fn from(error: tokio_rusqlite::Error<E>) -> Self {
        Self::internal(error)
    }
}

/// IP des Clients fürs Log (hinter Proxy nur mit `TRUST_PROXY`).
struct ClientIp(String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let forwarded = state
            .config
            .trust_proxy
            .then(|| parts.headers.get("x-forwarded-for"))
            .flatten()
            .and_then(|v| v.to_str().ok())
            // Der letzte Eintrag stammt vom eigenen Proxy, die vorderen
            // kann der Client selbst setzen.
            .and_then(|v| v.rsplit(',').next())
            .map(|v| v.trim().to_string());
        let direct = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip().to_string());
        Ok(Self(
            forwarded.or(direct).unwrap_or_else(|| "unbekannt".into()),
        ))
    }
}

#[derive(Serialize)]
struct StatusResponse {
    authenticated: bool,
    setup_required: bool,
}

async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StatusResponse>, ApiError> {
    let https = is_https(&headers, state.config.trust_proxy);
    let authenticated = session::is_authenticated(&state.db, &headers, https).await?;
    let setup_required = state
        .db
        .call(|conn| two_factor::is_confirmed(conn).map(|confirmed| !confirmed))
        .await?;
    Ok(Json(StatusResponse {
        authenticated,
        setup_required,
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    password: String,
    code: String,
}

#[derive(Serialize)]
struct LoginResponse {
    /// Nur direkt nach Abschluss der 2FA-Einrichtung gesetzt.
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_codes: Option<Vec<String>>,
}

async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    if request.password.len() > 1024 || request.code.len() > 64 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Eingabe zu lang"));
    }

    // Logins laufen nacheinander; die Sperre gilt bis zum Ende des Versuchs.
    let mut throttle = state.login.lock().await;
    if let Some(wait) = throttle.wait_time(Instant::now()) {
        let seconds = wait.as_secs().max(1);
        return Err(ApiError {
            retry_after: Some(seconds),
            ..ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                format!("Zu viele Fehlversuche. Bitte {seconds} Sekunden warten."),
            )
        });
    }

    let password = state.password.clone();
    let input = request.password;
    let password_ok = tokio::task::spawn_blocking(move || password.verify(&input))
        .await
        .unwrap_or(false);

    // Den Code nur prüfen, wenn das Passwort stimmt, damit ein Angreifer
    // keine TOTP-Schritte oder Wiederherstellungscodes verbrauchen kann.
    // Bei falschem Passwort läuft eine gleich teure Scheinprüfung, damit
    // die Antwortzeit nichts über das Passwort verrät.
    // Codeprüfung und Anlegen der Sitzung laufen in einer Transaktion, damit
    // ein gleichzeitiges `reset-2fa` keine Sitzung übrig lässt.
    let code = request.code;
    let outcome = state
        .db
        .call(move |conn| {
            let now = two_factor::now();
            if !password_ok {
                two_factor::dummy_check(conn, &code, now);
                return Ok::<_, String>((CodeCheck::Invalid, None));
            }
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|error| error.to_string())?;
            let outcome = two_factor::check_code(&tx, &code, now)?;
            if matches!(outcome, CodeCheck::Invalid) {
                return Ok((outcome, None));
            }
            let token = session::create(&tx).map_err(|error| error.to_string())?;
            tx.commit().map_err(|error| error.to_string())?;
            Ok((outcome, Some(token)))
        })
        .await?;

    let (recovery_codes, token) = match outcome {
        (CodeCheck::Valid, Some(token)) => (None, token),
        (CodeCheck::SetupCompleted { recovery_codes }, Some(token)) => {
            tracing::info!(
                "Zwei-Faktor-Einrichtung abgeschlossen. Hinweis: Der QR-Code steht noch in \n                 älteren Log-Einträgen. Container neu erstellen, um das Log zu leeren."
            );
            (Some(recovery_codes), token)
        }
        _ => {
            if password_ok {
                throttle.record_code_failure(Instant::now());
                tracing::warn!("Fehlgeschlagener Login von {ip}: Passwort richtig, Code falsch");
            } else {
                throttle.record_failure(Instant::now());
                tracing::warn!("Fehlgeschlagener Login von {ip}");
            }
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "Passwort oder Code falsch",
            ));
        }
    };
    throttle.record_success();
    drop(throttle);
    tracing::info!("Erfolgreicher Login von {ip}");

    let https = is_https(&headers, state.config.trust_proxy);
    let mut response = Json(LoginResponse { recovery_codes }).into_response();
    let cookie =
        HeaderValue::from_str(&session::set_cookie(&token, https)).map_err(ApiError::internal)?;
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let https = is_https(&headers, state.config.trust_proxy);
    if let Some(token) = session::token_from_headers(&headers, https) {
        state
            .db
            .call(move |conn| session::delete(conn, &token))
            .await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    for cookie in session::clear_cookies(https) {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    Ok(response)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::{config::Config, db, password::PasswordCheck, security::Throttle, two_factor, web};

    use super::*;

    const PASSWORD: &str = "ein-sicheres-passwort";
    pub(crate) const HOST: &str = "sebo.test";

    pub(crate) async fn test_state() -> AppState {
        let db = db::Db::open_in_memory().await.unwrap();
        db.call(|conn| {
            db::configure(conn).map_err(|error| error.to_string())?;
            db::migrate(conn)?;
            two_factor::pending_setup_url(conn)?;
            Ok::<_, String>(())
        })
        .await
        .unwrap();
        AppState {
            db,
            config: Arc::new(Config::for_tests()),
            password: Arc::new(
                PasswordCheck::from_env_values(Some(PASSWORD.into()), None).unwrap(),
            ),
            login: Arc::new(tokio::sync::Mutex::new(Throttle::default())),
            fetcher: Arc::new(crate::net::Fetcher::new(true).unwrap()),
            worker: Default::default(),
            images_dir: tempfile::tempdir().unwrap().keep(),
        }
    }

    async fn code(state: &AppState) -> String {
        let now = two_factor::now();
        state
            .db
            .call(move |conn| Ok::<_, rusqlite::Error>(two_factor::current_code(conn, now)))
            .await
            .unwrap()
    }

    pub(crate) async fn send(
        state: &AppState,
        request: Request<Body>,
    ) -> (StatusCode, HeaderMap, serde_json::Value) {
        let response = web::router(state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, headers, json)
    }

    fn login_request(password: &str, code: &str, origin: Option<&str>) -> Request<Body> {
        let mut builder = Request::post("/api/auth/login")
            .header(header::HOST, HOST)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        builder
            .body(Body::from(
                json!({ "password": password, "code": code }).to_string(),
            ))
            .unwrap()
    }

    fn status_request(cookie: Option<&str>) -> Request<Body> {
        let mut builder = Request::get("/api/auth/status").header(header::HOST, HOST);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        builder.body(Body::empty()).unwrap()
    }

    fn cookie_from(headers: &HeaderMap) -> String {
        let set_cookie = headers.get(header::SET_COOKIE).unwrap().to_str().unwrap();
        set_cookie.split(';').next().unwrap().to_string()
    }

    #[tokio::test]
    async fn kompletter_login_ablauf() {
        let state = test_state().await;
        let origin = Some("http://sebo.test");

        let (status, headers, body) = send(&state, status_request(None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({ "authenticated": false, "setup_required": true })
        );
        assert!(headers.get(header::CONTENT_SECURITY_POLICY).is_some());
        assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store");

        // Ohne passende Herkunft: abgewiesen (CSRF).
        let code = code(&state).await;
        let (status, _, _) = send(&state, login_request(PASSWORD, &code, None)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _, _) = send(
            &state,
            login_request(PASSWORD, &code, Some("https://boese.example")),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Falsches Passwort mit richtigem Code: abgewiesen, Code bleibt unverbraucht.
        let (status, _, _) = send(&state, login_request("falsches-passwort!", &code, origin)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Einrichtung abschließen.
        let (status, headers, body) = send(&state, login_request(PASSWORD, &code, origin)).await;
        assert_eq!(status, StatusCode::OK);
        let recovery: Vec<String> = serde_json::from_value(body["recovery_codes"].clone()).unwrap();
        assert_eq!(recovery.len(), 10);
        let cookie = cookie_from(&headers);
        assert!(
            headers
                .get(header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("HttpOnly")
        );

        let (_, _, body) = send(&state, status_request(Some(&cookie))).await;
        assert_eq!(
            body,
            json!({ "authenticated": true, "setup_required": false })
        );

        // Derselbe Code ein zweites Mal: abgewiesen.
        let (status, _, _) = send(&state, login_request(PASSWORD, &code, origin)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Wiederherstellungscode funktioniert genau einmal, ohne neue Codes zu liefern.
        let (status, _, body) = send(&state, login_request(PASSWORD, &recovery[0], origin)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("recovery_codes").is_none());

        // Abmelden: Sitzung ist danach ungültig.
        let logout = Request::post("/api/auth/logout")
            .header(header::HOST, HOST)
            .header(header::ORIGIN, "http://sebo.test")
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .unwrap();
        let (status, _, _) = send(&state, logout).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, _, body) = send(&state, status_request(Some(&cookie))).await;
        assert_eq!(body["authenticated"], false);
    }

    #[tokio::test]
    async fn wartezeit_nach_fehlversuchen() {
        let state = test_state().await;
        let origin = Some("http://sebo.test");
        for _ in 0..4 {
            let (status, _, _) = send(
                &state,
                login_request("falsches-passwort!", "123456", origin),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        // Jetzt muss gewartet werden, auch mit richtigen Daten.
        let code = code(&state).await;
        let (status, headers, body) = send(&state, login_request(PASSWORD, &code, origin)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(headers.get(header::RETRY_AFTER).is_some());
        assert!(body["retry_after"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn unbekannte_api_und_frontend_routen() {
        let state = test_state().await;
        let request = Request::get("/api/gibt-es-nicht")
            .body(Body::empty())
            .unwrap();
        let (status, _, body) = send(&state, request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Nicht gefunden");

        let request = Request::get("/healthz").body(Body::empty()).unwrap();
        let response = web::router(state.clone()).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
