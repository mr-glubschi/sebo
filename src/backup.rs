//! Tägliche Sicherung der Datenbank nach `/data/backups/sebo-JJJJMMTT.db`.
//!
//! `VACUUM INTO` schreibt eine konsistente Kopie, auch während SEBO läuft.
//! Die letzten `KEEP` Tagesstände bleiben erhalten. Bilder werden nicht
//! gesichert (sie lassen sich jederzeit neu abrufen).

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::params;

use crate::{db, state::AppState};

const KEEP: usize = 7;
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);

pub fn spawn(state: AppState, data_dir: PathBuf) {
    tokio::spawn(async move {
        let dir = data_dir.join("backups");
        loop {
            match run(&state, &dir).await {
                Ok(Some(path)) => tracing::info!("Sicherung erstellt: {}", path.display()),
                Ok(None) => {}
                Err(error) => tracing::error!("Sicherung fehlgeschlagen: {error}"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Erstellt die Sicherung für heute, falls es sie noch nicht gibt.
async fn run(state: &AppState, dir: &Path) -> Result<Option<PathBuf>, String> {
    let name = format!(
        "sebo-{}.db",
        db::date_string(db::now_secs()).replace('-', "")
    );
    let target = dir.join(&name);
    if tokio::fs::try_exists(&target).await.unwrap_or(false) {
        return Ok(None);
    }
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| format!("Ordner {} nicht anlegbar: {error}", dir.display()))?;

    // Erst unter temporärem Namen, damit nie eine halbe Sicherung liegen bleibt.
    let temporary = dir.join(format!("{name}.tmp"));
    let _ = tokio::fs::remove_file(&temporary).await;
    let path = temporary.to_string_lossy().into_owned();
    state
        .db
        .call(move |conn| conn.execute("VACUUM INTO ?1", params![path]))
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::rename(&temporary, &target)
        .await
        .map_err(|error| error.to_string())?;

    rotate(dir).await;
    Ok(Some(target))
}

/// Löscht alte Tagesstände, behält die neuesten `KEEP`.
async fn rotate(dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let mut daily = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_daily(&name) {
            daily.push(name);
        }
    }
    // Datumsformat JJJJMMTT: alphabetisch = zeitlich
    daily.sort();
    let excess = daily.len().saturating_sub(KEEP);
    for name in &daily[..excess] {
        if let Err(error) = tokio::fs::remove_file(dir.join(name)).await {
            tracing::warn!("Alte Sicherung {name} nicht löschbar: {error}");
        }
    }
}

/// `sebo-JJJJMMTT.db`, nicht die Sicherungen vor Migrationen.
fn is_daily(name: &str) -> bool {
    name.strip_prefix("sebo-")
        .and_then(|rest| rest.strip_suffix(".db"))
        .is_some_and(|date| date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::tests::test_state;

    #[test]
    fn erkennt_tagessicherungen() {
        assert!(is_daily("sebo-20261125.db"));
        assert!(!is_daily("sebo-vor-migration-v1-1700000000.db"));
        assert!(!is_daily("sebo-20261125.db.tmp"));
        assert!(!is_daily("sebo-2026112.db"));
    }

    #[tokio::test]
    async fn sichert_einmal_taeglich_und_rotiert() {
        let state = test_state().await;
        let dir = tempfile::tempdir().unwrap();
        // Alte Stände und eine Migrationssicherung
        tokio::fs::create_dir_all(dir.path()).await.unwrap();
        for day in 10..20 {
            std::fs::write(dir.path().join(format!("sebo-202001{day}.db")), b"alt").unwrap();
        }
        std::fs::write(dir.path().join("sebo-vor-migration-v1-1.db"), b"x").unwrap();

        let created = run(&state, dir.path()).await.unwrap().unwrap();
        assert!(created.exists());
        // Gültige SQLite-Datei
        let copy = rusqlite::Connection::open(&created).unwrap();
        let tables: i64 = copy
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'bookmarks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 1);

        // Zweiter Lauf am selben Tag: nichts
        assert!(run(&state, dir.path()).await.unwrap().is_none());

        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.iter().filter(|n| is_daily(n)).count(), KEEP);
        assert!(names.contains(&"sebo-vor-migration-v1-1.db".to_string()));
        assert!(!names.contains(&"sebo-20200110.db".to_string()));
    }
}
