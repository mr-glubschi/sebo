//! Passwortprüfung: Klartext aus `PASSWORD` oder argon2-Hash aus `PASSWORD_HASH`.

use std::process::ExitCode;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use subtle::ConstantTimeEq;

use crate::crypto::sha256;

pub const MIN_LENGTH: usize = 12;

pub enum PasswordCheck {
    /// SHA-256 des Klartext-Passworts, Vergleich in konstanter Zeit.
    Plain([u8; 32]),
    /// argon2-Hash im PHC-Format (`$argon2id$...`).
    Argon2(String),
}

impl PasswordCheck {
    pub fn from_env_values(plain: Option<String>, hash: Option<String>) -> Result<Self, String> {
        match (plain, hash) {
            (Some(_), Some(_)) => Err(
                "PASSWORD und PASSWORD_HASH sind beide gesetzt. Bitte nur eins von beiden verwenden."
                    .into(),
            ),
            (Some(plain), None) => {
                if plain.chars().count() < MIN_LENGTH {
                    return Err(format!(
                        "PASSWORD ist zu kurz. Es muss mindestens {MIN_LENGTH} Zeichen lang sein."
                    ));
                }
                Ok(Self::Plain(sha256(plain.as_bytes())))
            }
            (None, Some(hash)) => {
                let hash = hash.trim().to_string();
                PasswordHash::new(&hash).map_err(|_| {
                    "PASSWORD_HASH ist kein gültiger argon2-Hash. Erzeugen mit: docker exec -it sebo hash-password"
                        .to_string()
                })?;
                Ok(Self::Argon2(hash))
            }
            (None, None) => Err(
                "Kein Passwort gesetzt. Bitte die Umgebungsvariable PASSWORD (mindestens 12 Zeichen) oder PASSWORD_HASH setzen."
                    .into(),
            ),
        }
    }

    /// Prüft ein eingegebenes Passwort. Bei argon2 rechenintensiv, daher
    /// aus async-Code über `spawn_blocking` aufrufen.
    pub fn verify(&self, input: &str) -> bool {
        match self {
            Self::Plain(expected) => sha256(input.as_bytes()).ct_eq(expected).into(),
            Self::Argon2(hash) => Argon2::default()
                .verify_password(input.as_bytes(), hash.as_str())
                .is_ok(),
        }
    }
}

/// Fingerabdruck des eingestellten Passworts in `settings`. So lässt sich beim
/// Start erkennen, ob es geändert wurde.
const KEY_FINGERPRINT: &str = "password_fingerprint";

#[derive(Debug, PartialEq)]
pub enum PasswordSync {
    /// Erster Start mit dieser Funktion: Fingerabdruck gespeichert, niemand abgemeldet.
    Recorded,
    Unchanged,
    /// Passwort geändert: alle Sitzungen gelöscht.
    Changed,
}

/// Vergleicht das eingestellte Passwort mit dem beim letzten Start und meldet
/// bei einer Änderung alle Sitzungen ab. Erwartet die bereits über
/// `from_env_values` geprüften Werte.
///
/// Bei `PASSWORD` wird ein argon2-Hash gespeichert, kein schneller SHA-256:
/// Aus einer entwendeten Datenbank soll sich das Passwort nicht leicht
/// ermitteln lassen. `PASSWORD_HASH` steht ohnehin in der Umgebung, dort
/// reicht SHA-256 des Hashes. Ein Wechsel zwischen beiden gilt als Änderung.
pub fn sync_sessions(
    conn: &mut rusqlite::Connection,
    plain: Option<&str>,
    hash_value: Option<&str>,
) -> Result<PasswordSync, String> {
    let db_error = |error: rusqlite::Error| format!("Datenbankfehler: {error}");
    let stored = crate::db::get_setting(conn, KEY_FINGERPRINT).map_err(db_error)?;

    let unchanged = match (&stored, plain, hash_value) {
        (Some(stored), Some(plain), _) => stored
            .strip_prefix("plain:")
            .is_some_and(|stored| PasswordCheck::Argon2(stored.to_string()).verify(plain)),
        (Some(stored), None, Some(hash_value)) => *stored == hash_fingerprint(hash_value),
        _ => false,
    };
    if unchanged {
        return Ok(PasswordSync::Unchanged);
    }

    let fingerprint = match (plain, hash_value) {
        (Some(plain), _) => format!("plain:{}", hash(plain)?),
        (None, Some(hash_value)) => hash_fingerprint(hash_value),
        (None, None) => return Err("Kein Passwort gesetzt".into()),
    };
    let tx = conn.transaction().map_err(db_error)?;
    if stored.is_some() {
        tx.execute("DELETE FROM sessions", []).map_err(db_error)?;
    }
    crate::db::set_setting(&tx, KEY_FINGERPRINT, &fingerprint).map_err(db_error)?;
    tx.commit().map_err(db_error)?;
    Ok(if stored.is_some() {
        PasswordSync::Changed
    } else {
        PasswordSync::Recorded
    })
}

fn hash_fingerprint(hash_value: &str) -> String {
    format!(
        "hash:{}",
        crate::crypto::sha256_hex(hash_value.trim().as_bytes())
    )
}

pub fn hash(password: &str) -> Result<String, String> {
    let hash: PasswordHash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|error| format!("Hash konnte nicht erzeugt werden: {error}"))?;
    Ok(hash.to_string())
}

/// Befehl `hash-password`: fragt das Passwort zweimal ab und gibt den Hash aus.
pub fn hash_password_command() -> ExitCode {
    let first = match rpassword::prompt_password("Neues Passwort: ") {
        Ok(value) => value,
        Err(_) => {
            eprintln!(
                "Passwort konnte nicht gelesen werden. Bitte mit Terminal aufrufen: docker exec -it sebo hash-password"
            );
            return ExitCode::FAILURE;
        }
    };
    if first.chars().count() < MIN_LENGTH {
        eprintln!("Das Passwort muss mindestens {MIN_LENGTH} Zeichen lang sein.");
        return ExitCode::FAILURE;
    }
    let second = rpassword::prompt_password("Passwort wiederholen: ").unwrap_or_default();
    if first != second {
        eprintln!("Die Passwörter stimmen nicht überein.");
        return ExitCode::FAILURE;
    }

    match hash(&first) {
        Ok(hash) => {
            println!(
                "\nDiesen Wert als Variable PASSWORD_HASH eintragen und PASSWORD entfernen:\n"
            );
            println!("{hash}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klartext_passwort() {
        let check = PasswordCheck::from_env_values(Some("richtig-langes-pw".into()), None).unwrap();
        assert!(check.verify("richtig-langes-pw"));
        assert!(!check.verify("falsch-langes-pw!"));
    }

    #[test]
    fn zu_kurz_und_doppelt_und_fehlend() {
        assert!(PasswordCheck::from_env_values(Some("kurz".into()), None).is_err());
        assert!(PasswordCheck::from_env_values(Some("a".repeat(12)), Some("x".into())).is_err());
        assert!(PasswordCheck::from_env_values(None, None).is_err());
        assert!(PasswordCheck::from_env_values(None, Some("kein-hash".into())).is_err());
    }

    #[test]
    fn passwortwechsel_meldet_ab() {
        let mut conn = crate::db::open_test();
        let sessions = |conn: &rusqlite::Connection| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
                .unwrap()
        };
        let login = |conn: &rusqlite::Connection| {
            crate::session::create(conn).unwrap();
        };

        // Erster Start: nur merken, niemand abmelden.
        login(&conn);
        let first = Some("erstes-langes-passwort");
        assert_eq!(
            sync_sessions(&mut conn, first, None),
            Ok(PasswordSync::Recorded)
        );
        assert_eq!(sessions(&conn), 1);
        // Kein Klartext und kein schneller Hash in der Datenbank.
        let stored = crate::db::get_setting(&conn, KEY_FINGERPRINT)
            .unwrap()
            .unwrap();
        assert!(stored.starts_with("plain:$argon2"));
        assert!(!stored.contains("erstes"));

        // Gleiches Passwort: bleibt angemeldet.
        assert_eq!(
            sync_sessions(&mut conn, first, None),
            Ok(PasswordSync::Unchanged)
        );
        assert_eq!(sessions(&conn), 1);

        // Anderes Passwort: alle abgemeldet.
        let second = Some("zweites-langes-passwort");
        assert_eq!(
            sync_sessions(&mut conn, second, None),
            Ok(PasswordSync::Changed)
        );
        assert_eq!(sessions(&conn), 0);

        // Wechsel auf PASSWORD_HASH (auch mit gleichem Passwort) gilt als Änderung.
        login(&conn);
        let hashed = hash("zweites-langes-passwort").unwrap();
        assert_eq!(
            sync_sessions(&mut conn, None, Some(&hashed)),
            Ok(PasswordSync::Changed)
        );
        assert_eq!(sessions(&conn), 0);
        login(&conn);
        assert_eq!(
            sync_sessions(&mut conn, None, Some(&format!(" {hashed}\n"))),
            Ok(PasswordSync::Unchanged)
        );
        assert_eq!(sessions(&conn), 1);
        let other = hash("drittes-langes-passwort").unwrap();
        assert_eq!(
            sync_sessions(&mut conn, None, Some(&other)),
            Ok(PasswordSync::Changed)
        );
        assert_eq!(sessions(&conn), 0);
    }

    #[test]
    fn argon2_hash() {
        let hash = hash("geheimes-passwort").unwrap();
        let check = PasswordCheck::from_env_values(None, Some(hash)).unwrap();
        assert!(check.verify("geheimes-passwort"));
        assert!(!check.verify("anderes-passwort"));
    }
}
