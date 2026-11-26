//! SQLite: Öffnen, Einstellungen, Migrationen und Sicherung vor Migrationen.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};

/// Asynchroner Zugriff auf die eine Datenbankverbindung.
pub type Db = tokio_rusqlite::Connection;

const MIGRATIONS: &[M<'static>] = &[
    M::up(include_str!("../migrations/001_auth.sql")),
    M::up(include_str!("../migrations/002_library.sql")),
    M::up(include_str!("../migrations/003_favorites_icons.sql")),
];

fn migrations() -> Migrations<'static> {
    Migrations::from_slice(MIGRATIONS)
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("app.db")
}

/// Einstellungen, die für jede Verbindung gelten.
pub fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Bringt die Datenbank auf den neuesten Stand.
pub fn migrate(conn: &mut Connection) -> Result<(), String> {
    migrations()
        .to_latest(conn)
        .map_err(|error| format!("Datenbank-Migration fehlgeschlagen: {error}"))
}

/// Legt Datenverzeichnis und Datenbank an, sichert sie vor anstehenden
/// Migrationen und migriert. Gibt eine fertige, synchrone Verbindung zurück.
pub fn prepare(data_dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(data_dir).map_err(|error| {
        format!(
            "Datenverzeichnis {} kann nicht angelegt werden: {error}",
            data_dir.display()
        )
    })?;

    let path = db_path(data_dir);
    let mut conn = Connection::open(&path).map_err(|error| {
        format!(
            "Datenbank {} kann nicht geöffnet werden: {error}",
            path.display()
        )
    })?;
    configure(&conn)
        .map_err(|error| format!("Datenbank kann nicht eingerichtet werden: {error}"))?;

    let current: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("Datenbankversion nicht lesbar: {error}"))?;
    let pending = migrations()
        .pending_migrations(&conn)
        .map_err(|error| format!("Datenbankversion nicht lesbar: {error}"))?;

    if current > 0 && pending > 0 {
        backup_before_migration(&conn, data_dir, current)?;
    }

    migrate(&mut conn)?;
    Ok(conn)
}

fn backup_before_migration(conn: &Connection, data_dir: &Path, version: i64) -> Result<(), String> {
    let dir = data_dir.join("backups");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Sicherungsordner kann nicht angelegt werden: {error}"))?;
    let target = dir.join(format!("sebo-vor-migration-v{version}-{}.db", now_secs()));
    conn.execute("VACUUM INTO ?1", params![target.to_string_lossy()])
        .map_err(|error| format!("Sicherung vor der Migration fehlgeschlagen: {error}"))?;
    tracing::info!(
        "Sicherung vor der Datenbank-Migration: {}",
        target.display()
    );
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Aktuelle Zeit in Sekunden seit 1970.
/// Datum (UTC) als JJJJ-MM-TT, ohne Datumsbibliothek.
pub fn date_string(secs: i64) -> String {
    // Algorithmus „civil_from_days“ von Howard Hinnant
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
pub fn open_test() -> Connection {
    let mut conn = Connection::open_in_memory().expect("In-Memory-Datenbank");
    configure(&conn).expect("Datenbank einrichten");
    migrate(&mut conn).expect("Migration");
    conn
}

#[cfg(test)]
mod tests {
    #[test]
    fn datum() {
        assert_eq!(super::date_string(0), "1970-01-01");
        assert_eq!(super::date_string(951_782_400), "2000-02-29");
        assert_eq!(super::date_string(1_795_651_200), "2026-11-26");
    }

    #[test]
    fn migrationen_sind_gueltig() {
        super::migrations().validate().expect("Migrationen gültig");
    }
}
