//! Zwei-Faktor-Authentifizierung: TOTP-Einrichtung, Prüfung, Wiederherstellungscodes.

use qrcode::{QrCode, render::unicode::Dense1x2};
use rusqlite::{Connection, params};
use subtle::ConstantTimeEq;
use totp_rs::{Builder, Secret, Totp};

use crate::{
    crypto::{random_bytes, sha256_hex},
    db::{get_setting, now_secs, set_setting},
};

const KEY_SECRET: &str = "totp_secret";
const KEY_CONFIRMED: &str = "totp_confirmed";
const KEY_LAST_STEP: &str = "totp_last_step";

pub const RECOVERY_CODE_COUNT: usize = 10;
/// Crockford-Base32: ohne I, L, O, U. 32 Zeichen = 5 Bit pro Zeichen,
/// 12 Zeichen = 60 Bit pro Code.
const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const RECOVERY_LENGTH: usize = 12;

fn build_totp(secret_base32: &str) -> Result<Totp, String> {
    let secret = Secret::try_from_base32(secret_base32)
        .map_err(|_| "Gespeichertes 2FA-Geheimnis ist ungültig".to_string())?;
    Builder::new()
        .with_secret(secret)
        .with_issuer(Some("SEBO"))
        .with_account_name("SEBO")
        .build()
        .map_err(|error| format!("2FA konnte nicht erstellt werden: {error:?}"))
}

pub fn is_confirmed(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(get_setting(conn, KEY_CONFIRMED)?.as_deref() == Some("1"))
}

/// Sorgt dafür, dass es während der Einrichtung ein (noch unbestätigtes)
/// Geheimnis gibt. Gibt die `otpauth://`-Adresse zurück, solange die
/// Einrichtung nicht bestätigt ist, sonst `None`.
pub fn pending_setup_url(conn: &Connection) -> Result<Option<String>, String> {
    let db_error = |error: rusqlite::Error| format!("Datenbankfehler: {error}");

    if is_confirmed(conn).map_err(db_error)? {
        return Ok(None);
    }

    let secret = match get_setting(conn, KEY_SECRET).map_err(db_error)? {
        Some(secret) => secret,
        None => {
            let secret = Secret::generate().to_base32();
            set_setting(conn, KEY_SECRET, &secret).map_err(db_error)?;
            secret
        }
    };

    let url = build_totp(&secret)?
        .to_url()
        .map_err(|error| format!("otpauth-Adresse konnte nicht erstellt werden: {error:?}"))?;
    Ok(Some(url))
}

/// Gibt QR-Code und Adresse für die Einrichtung aus (direkt auf stdout,
/// damit der QR-Code nicht durch das Log-Format zerstört wird).
pub fn print_setup(url: &str) {
    let qr = QrCode::new(url.as_bytes())
        .map(|code| {
            code.render::<Dense1x2>()
                .dark_color(Dense1x2::Light)
                .light_color(Dense1x2::Dark)
                .quiet_zone(true)
                .build()
        })
        .unwrap_or_else(|_| "(QR-Code konnte nicht erzeugt werden)".into());

    println!();
    println!("================ SEBO: Zwei-Faktor-Einrichtung ================");
    println!();
    println!("Diesen QR-Code mit einer Authenticator-App scannen:");
    println!();
    println!("{qr}");
    println!();
    println!("Falls der QR-Code verzerrt ist, diese Adresse manuell eintragen:");
    println!("{url}");
    println!();
    println!("Danach in SEBO mit Passwort und Code aus der App anmelden.");
    println!("Nach der Bestätigung wird dieser Code nie wieder angezeigt.");
    println!("================================================================");
    println!();
}

pub enum CodeCheck {
    /// Gültiger TOTP-Code bzw. gültiger Wiederherstellungscode.
    Valid,
    /// Einrichtung abgeschlossen; die Wiederherstellungscodes werden
    /// genau einmal zurückgegeben.
    SetupCompleted {
        recovery_codes: Vec<String>,
    },
    Invalid,
}

/// Prüft einen eingegebenen Code. Während der Einrichtung schließt ein
/// gültiger TOTP-Code die Einrichtung ab. Danach werden TOTP-Codes
/// (jeder nur einmal) und Wiederherstellungscodes angenommen.
///
/// Muss innerhalb einer Transaktion aufgerufen werden, die der Aufrufer
/// nur bei Erfolg bestätigt (zusammen mit dem Anlegen der Sitzung).
pub fn check_code(tx: &Connection, input: &str, now: u64) -> Result<CodeCheck, String> {
    let db_error = |error: rusqlite::Error| format!("Datenbankfehler: {error}");

    let Some(secret) = get_setting(tx, KEY_SECRET).map_err(db_error)? else {
        return Ok(CodeCheck::Invalid);
    };
    let confirmed = is_confirmed(tx).map_err(db_error)?;
    let code: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        let totp = build_totp(&secret)?;
        let Some(step) = totp.check(&code, now) else {
            return Ok(CodeCheck::Invalid);
        };
        let last_step: Option<u64> = get_setting(tx, KEY_LAST_STEP)
            .map_err(db_error)?
            .and_then(|value| value.parse().ok());
        if last_step.is_some_and(|last| step <= last) {
            // Code wurde schon einmal verwendet (oder ein älterer).
            return Ok(CodeCheck::Invalid);
        }
        set_setting(tx, KEY_LAST_STEP, &step.to_string()).map_err(db_error)?;

        let result = if confirmed {
            CodeCheck::Valid
        } else {
            set_setting(tx, KEY_CONFIRMED, "1").map_err(db_error)?;
            let recovery_codes = replace_recovery_codes(tx).map_err(db_error)?;
            CodeCheck::SetupCompleted { recovery_codes }
        };
        return Ok(result);
    }

    // Wiederherstellungscodes gibt es erst nach der Einrichtung.
    if !confirmed {
        return Ok(CodeCheck::Invalid);
    }
    let Some(normalized) = normalize_recovery_code(&code) else {
        return Ok(CodeCheck::Invalid);
    };
    if consume_recovery_code(tx, &normalized).map_err(db_error)? {
        Ok(CodeCheck::Valid)
    } else {
        Ok(CodeCheck::Invalid)
    }
}

/// Ungefähr gleich teure Arbeit wie `check_code`, ohne etwas zu ändern.
/// Wird bei falschem Passwort ausgeführt, damit die Antwortzeit nicht
/// verrät, ob das Passwort stimmte.
pub fn dummy_check(conn: &Connection, input: &str, now: u64) {
    let secret = get_setting(conn, KEY_SECRET).ok().flatten();
    let _ = is_confirmed(conn);
    let _ = get_setting(conn, KEY_LAST_STEP);
    if let Some(totp) = secret.and_then(|secret| build_totp(&secret).ok()) {
        let _ = totp.check(input.trim(), now);
    }
}

fn normalize_recovery_code(input: &str) -> Option<String> {
    // Verwechselbare Zeichen wie bei Crockford-Base32 korrigieren.
    let normalized: String = input
        .chars()
        .filter(|c| *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    (normalized.len() == RECOVERY_LENGTH
        && normalized.bytes().all(|b| RECOVERY_ALPHABET.contains(&b)))
    .then_some(normalized)
}

fn consume_recovery_code(conn: &Connection, normalized: &str) -> rusqlite::Result<bool> {
    let wanted = sha256_hex(normalized.as_bytes());
    // Alle Hashes laden und in konstanter Zeit vergleichen.
    let mut stmt = conn.prepare("SELECT code_hash FROM recovery_codes")?;
    let hashes: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let found = hashes
        .iter()
        .find(|hash| bool::from(hash.as_bytes().ct_eq(wanted.as_bytes())));
    match found {
        Some(hash) => {
            conn.execute(
                "DELETE FROM recovery_codes WHERE code_hash = ?1",
                params![hash],
            )?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn replace_recovery_codes(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    conn.execute("DELETE FROM recovery_codes", [])?;
    let mut codes = Vec::with_capacity(RECOVERY_CODE_COUNT);
    for _ in 0..RECOVERY_CODE_COUNT {
        let bytes = random_bytes::<RECOVERY_LENGTH>();
        // 256 ist durch 32 teilbar, daher gleichverteilt.
        let raw: String = bytes
            .iter()
            .map(|b| RECOVERY_ALPHABET[(b & 31) as usize] as char)
            .collect();
        conn.execute(
            "INSERT INTO recovery_codes (code_hash) VALUES (?1)",
            params![sha256_hex(raw.as_bytes())],
        )?;
        codes.push(format!("{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12]));
    }
    Ok(codes)
}

/// Setzt 2FA vollständig zurück: neues Geheimnis, keine Wiederherstellungs-
/// codes, alle Sitzungen abgemeldet. Gibt die neue `otpauth://`-Adresse zurück.
pub fn reset(conn: &mut Connection) -> Result<String, String> {
    let db_error = |error: rusqlite::Error| format!("Datenbankfehler: {error}");
    let tx = conn.transaction().map_err(db_error)?;
    tx.execute(
        "DELETE FROM settings WHERE key IN (?1, ?2, ?3)",
        params![KEY_SECRET, KEY_CONFIRMED, KEY_LAST_STEP],
    )
    .map_err(db_error)?;
    tx.execute("DELETE FROM recovery_codes", [])
        .map_err(db_error)?;
    tx.execute("DELETE FROM sessions", []).map_err(db_error)?;
    let url = pending_setup_url(&tx)?.ok_or("2FA-Zurücksetzen fehlgeschlagen")?;
    tx.commit().map_err(db_error)?;
    Ok(url)
}

/// Aktuelle Zeit für die TOTP-Prüfung.
pub fn now() -> u64 {
    now_secs().max(0) as u64
}

#[cfg(test)]
pub fn current_code(conn: &Connection, time: u64) -> String {
    let secret = get_setting(conn, KEY_SECRET).unwrap().unwrap();
    build_totp(&secret).unwrap().generate(time).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;

    /// Prüfung in eigener Transaktion, wie im Login.
    fn check(conn: &mut Connection, code: &str, time: u64) -> Result<CodeCheck, String> {
        let tx = conn.transaction().unwrap();
        let result = check_code(&tx, code, time);
        tx.commit().unwrap();
        result
    }

    #[test]
    fn einrichtung_replay_und_wiederherstellung() {
        let mut conn = open_test();
        let url = pending_setup_url(&conn)
            .unwrap()
            .expect("Einrichtung offen");
        assert!(url.starts_with("otpauth://totp/"));
        // Gleiches Geheimnis bei erneutem Aufruf (z. B. Neustart).
        assert_eq!(pending_setup_url(&conn).unwrap().unwrap(), url);

        let t = 2_000_000_000;

        // Falscher Code schließt die Einrichtung nicht ab.
        let wrong = if current_code(&conn, t) == "000000" {
            "111111"
        } else {
            "000000"
        };
        assert!(matches!(
            check(&mut conn, wrong, t).unwrap(),
            CodeCheck::Invalid
        ));
        assert!(!is_confirmed(&conn).unwrap());
        // Wiederherstellungscodes gibt es vor der Einrichtung nicht.
        assert!(matches!(
            check(&mut conn, "AAAA-AAAA-AAAA", t).unwrap(),
            CodeCheck::Invalid
        ));

        let code = current_code(&conn, t);
        let CodeCheck::SetupCompleted { recovery_codes } = check(&mut conn, &code, t).unwrap()
        else {
            panic!("Einrichtung sollte abgeschlossen werden");
        };
        assert_eq!(recovery_codes.len(), RECOVERY_CODE_COUNT);
        assert!(pending_setup_url(&conn).unwrap().is_none());

        // Derselbe Code darf nicht zweimal funktionieren.
        assert!(matches!(
            check(&mut conn, &code, t).unwrap(),
            CodeCheck::Invalid
        ));
        // Der nächste Code schon.
        let next = current_code(&conn, t + 30);
        assert!(matches!(
            check(&mut conn, &next, t + 30).unwrap(),
            CodeCheck::Valid
        ));

        // Wiederherstellungscode: klein geschrieben und ohne Striche erlaubt, aber nur einmal.
        let recovery = recovery_codes[0].replace('-', "").to_lowercase();
        assert!(matches!(
            check(&mut conn, &recovery, t).unwrap(),
            CodeCheck::Valid
        ));
        assert!(matches!(
            check(&mut conn, &recovery, t).unwrap(),
            CodeCheck::Invalid
        ));
        assert!(matches!(
            check(&mut conn, &recovery_codes[1], t).unwrap(),
            CodeCheck::Valid
        ));
    }

    #[test]
    fn zuruecksetzen() {
        let mut conn = open_test();
        let first = pending_setup_url(&conn).unwrap().unwrap();
        let t = 2_000_000_000;
        let code = current_code(&conn, t);
        check(&mut conn, &code, t).unwrap();
        conn.execute("INSERT INTO sessions VALUES ('x', 0, 0)", [])
            .unwrap();

        let second = reset(&mut conn).unwrap();
        assert_ne!(first, second);
        assert!(!is_confirmed(&conn).unwrap());
        let sessions: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        let codes: i64 = conn
            .query_row("SELECT COUNT(*) FROM recovery_codes", [], |r| r.get(0))
            .unwrap();
        assert_eq!((sessions, codes), (0, 0));
    }
}
