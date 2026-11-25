//! Sitzungen: zufälliges Token im Cookie, in der Datenbank nur dessen Hash.

use axum::http::{HeaderMap, header};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    crypto::{hex, random_bytes, sha256_hex},
    db::{Db, now_secs},
};

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
pub async fn is_authenticated(db: &Db, headers: &HeaderMap) -> Result<bool, tokio_rusqlite::Error> {
    let Some(token) = token_from_headers(headers) else {
        return Ok(false);
    };
    let now = now_secs();
    db.call(move |conn| validate(conn, &token, now)).await
}

/// Liest das Sitzungs-Token aus dem Cookie-Header.
pub fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_SECURE || *name == COOKIE_PLAIN)
        .map(|(_, value)| value.to_string())
        .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
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
        assert_eq!(token_from_headers(&headers), Some(token));

        headers.insert(header::COOKIE, "sebo_session=kaputt".parse().unwrap());
        assert_eq!(token_from_headers(&headers), None);
    }
}
