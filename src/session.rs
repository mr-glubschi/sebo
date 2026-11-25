//! Sitzungen: zufälliges Token im Cookie, in der Datenbank nur dessen Hash.

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, StatusCode, header, request::Parts},
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    api::ApiError,
    crypto::{hex, random_bytes, sha256_hex},
    db::{Db, now_secs},
    security::is_https,
    state::AppState,
};

/// Extractor für Routen, die eine gültige Sitzung brauchen.
pub struct Authenticated;

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let https = is_https(&parts.headers, state.config.trust_proxy);
        if is_authenticated(&state.db, &parts.headers, https).await? {
            Ok(Self)
        } else {
            Err(ApiError::new(StatusCode::UNAUTHORIZED, "Nicht angemeldet"))
        }
    }
}

/// Nach 30 Tagen ohne Nutzung läuft eine Sitzung ab.
pub const IDLE_TIMEOUT_SECS: i64 = 30 * 24 * 60 * 60;
/// `last_seen_at` wird höchstens einmal pro Stunde aktualisiert.
const TOUCH_INTERVAL_SECS: i64 = 60 * 60;

const COOKIE_SECURE: &str = "__Host-sebo_session";
const COOKIE_PLAIN: &str = "sebo_session";

/// Erzeugt ein neues Token (für das Cookie) und legt die Sitzung an.
pub fn create(conn: &Connection) -> rusqlite::Result<String> {
    let token = hex(&random_bytes::<32>());
    let now = now_secs();
    conn.execute(
        "INSERT INTO sessions (token_hash, created_at, last_seen_at) VALUES (?1, ?2, ?2)",
        params![sha256_hex(token.as_bytes()), now],
    )?;
    Ok(token)
}

/// Prüft, ob das Token zu einer gültigen Sitzung gehört, und verlängert sie.
pub fn validate(conn: &Connection, token: &str, now: i64) -> rusqlite::Result<bool> {
    let hash = sha256_hex(token.as_bytes());
    let last_seen: Option<i64> = conn
        .query_row(
            "SELECT last_seen_at FROM sessions WHERE token_hash = ?1",
            params![hash],
            |row| row.get(0),
        )
        .optional()?;

    let Some(last_seen) = last_seen else {
        return Ok(false);
    };
    if now - last_seen > IDLE_TIMEOUT_SECS {
        conn.execute("DELETE FROM sessions WHERE token_hash = ?1", params![hash])?;
        return Ok(false);
    }
    if now - last_seen > TOUCH_INTERVAL_SECS {
        conn.execute(
            "UPDATE sessions SET last_seen_at = ?2 WHERE token_hash = ?1",
            params![hash, now],
        )?;
    }
    Ok(true)
}

pub fn delete(conn: &Connection, token: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM sessions WHERE token_hash = ?1",
        params![sha256_hex(token.as_bytes())],
    )?;
    Ok(())
}

pub fn purge_expired(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM sessions WHERE last_seen_at < ?1",
        params![now_secs() - IDLE_TIMEOUT_SECS],
    )
}

/// Ist die Sitzung zum Token in den Headern gültig?
pub async fn is_authenticated(
    db: &Db,
    headers: &HeaderMap,
    https: bool,
) -> Result<bool, tokio_rusqlite::Error> {
    let Some(token) = token_from_headers(headers, https) else {
        return Ok(false);
    };
    let now = now_secs();
    db.call(move |conn| validate(conn, &token, now)).await
}

/// Liest das Sitzungs-Token aus dem Cookie-Header. Bei HTTPS zählt nur das
/// `__Host-`-Cookie, das andere Subdomains nicht setzen können. Ungültige
/// Werte werden übersprungen, statt das gültige Cookie zu verdecken.
pub fn token_from_headers(headers: &HeaderMap, https: bool) -> Option<String> {
    let wanted = if https { COOKIE_SECURE } else { COOKIE_PLAIN };
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(name, _)| *name == wanted)
        .map(|(_, value)| value)
        .find(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_string)
}

/// `Set-Cookie` für eine neue Sitzung. Bei HTTPS mit `__Host-`-Präfix und `Secure`.
/// Die Laufzeit im Browser ist lang, abgelaufen wird serverseitig nach Inaktivität.
pub fn set_cookie(token: &str, https: bool) -> String {
    let name = if https { COOKIE_SECURE } else { COOKIE_PLAIN };
    let secure = if https { "; Secure" } else { "" };
    format!("{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=34560000{secure}")
}

pub fn clear_cookies(https: bool) -> Vec<String> {
    let secure = if https { "; Secure" } else { "" };
    let mut cookies = vec![format!(
        "{COOKIE_PLAIN}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"
    )];
    if https {
        cookies.push(format!(
            "{COOKIE_SECURE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Secure"
        ));
    }
    cookies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;

    #[test]
    fn sitzung_anlegen_pruefen_ablaufen() {
        let conn = open_test();
        let token = create(&conn).unwrap();
        let now = now_secs();
        assert!(validate(&conn, &token, now).unwrap());
        assert!(!validate(&conn, "ff".repeat(32).as_str(), now).unwrap());
        // Nach Ablauf der Inaktivitätszeit ungültig und gelöscht.
        assert!(!validate(&conn, &token, now + IDLE_TIMEOUT_SECS + 10).unwrap());
        assert!(!validate(&conn, &token, now).unwrap());
    }

    #[test]
    fn cookie_lesen() {
        let mut headers = HeaderMap::new();
        let token = "ab".repeat(32);
        headers.insert(
            header::COOKIE,
            format!("andere=1; sebo_session={token}").parse().unwrap(),
        );
        assert_eq!(token_from_headers(&headers, false), Some(token.clone()));
        // Bei HTTPS zählt nur das __Host-Cookie.
        assert_eq!(token_from_headers(&headers, true), None);

        // Ein untergeschobenes ungültiges Cookie verdeckt das gültige nicht.
        headers.insert(
            header::COOKIE,
            format!("sebo_session=kaputt; sebo_session={token}")
                .parse()
                .unwrap(),
        );
        assert_eq!(token_from_headers(&headers, false), Some(token));

        headers.insert(header::COOKIE, "sebo_session=kaputt".parse().unwrap());
        assert_eq!(token_from_headers(&headers, false), None);
    }
}
