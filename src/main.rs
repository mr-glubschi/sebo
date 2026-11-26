mod api;
mod backup;
mod config;
mod crypto;
mod db;
mod images;
mod library;
mod library_api;
mod meta;
mod net;
mod netscape;
mod password;
mod security;
mod session;
mod state;
mod two_factor;
mod web;
mod worker;

use std::{process::ExitCode, sync::Arc};

use config::{Config, env};
use password::PasswordCheck;
use state::AppState;

const USAGE: &str = "Verwendung: sebo [serve | reset-2fa | hash-password]";

fn main() -> ExitCode {
    let command = std::env::args().nth(1);

    match command.as_deref() {
        None | Some("serve") => serve(),
        Some("reset-2fa") => reset_2fa(),
        Some("hash-password") => password::hash_password_command(),
        Some("-h") | Some("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("Unbekannter Befehl: {other}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("Fehler: {message}");
    ExitCode::FAILURE
}

fn serve() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => return fail(message),
    };
    init_tracing(&config.log_level);

    let plain = env("PASSWORD");
    let hash = env("PASSWORD_HASH");
    let password = match PasswordCheck::from_env_values(plain.clone(), hash.clone()) {
        Ok(password) => password,
        Err(message) => return fail(message),
    };

    // Datenbank vorbereiten (synchron, vor dem Start des Servers).
    let mut conn = match db::prepare(&config.data_dir) {
        Ok(conn) => conn,
        Err(message) => return fail(message),
    };
    match two_factor::pending_setup_url(&conn) {
        Ok(Some(url)) => two_factor::print_setup(&url),
        Ok(None) => tracing::info!("Zwei-Faktor-Authentifizierung ist eingerichtet"),
        Err(message) => return fail(message),
    }
    match password::sync_sessions(&mut conn, plain.as_deref(), hash.as_deref()) {
        Ok(password::PasswordSync::Changed) => {
            tracing::warn!("Passwort geändert, alle Sitzungen abgemeldet");
        }
        Ok(_) => {}
        Err(message) => return fail(message),
    }
    drop((plain, hash));
    if let Err(error) = session::purge_expired(&conn) {
        return fail(format!(
            "Abgelaufene Sitzungen konnten nicht gelöscht werden: {error}"
        ));
    }
    drop(conn);

    let images_dir = config.data_dir.join("images");
    if let Err(error) = std::fs::create_dir_all(&images_dir) {
        return fail(format!(
            "Bildordner {} kann nicht angelegt werden: {error}",
            images_dir.display()
        ));
    }
    restrict_data_dir(&config.data_dir);
    let fetcher = match net::Fetcher::new(config.allow_private_urls) {
        Ok(fetcher) => fetcher,
        Err(message) => return fail(message),
    };
    if config.allow_private_urls {
        tracing::info!("ALLOW_PRIVATE_URLS ist aktiv: auch interne Adressen werden abgerufen");
    }

    // Ein Benutzer, wenig Last: wenige Threads reichen, auch auf Servern mit
    // vielen Kernen (Standard wäre ein Thread pro Kern).
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4));
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .max_blocking_threads(32)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return fail(format!(
                "Laufzeitumgebung konnte nicht gestartet werden: {error}"
            ));
        }
    };

    let result = runtime.block_on(async {
        let db = db::Db::open(db::db_path(&config.data_dir))
            .await
            .map_err(|error| format!("Datenbank kann nicht geöffnet werden: {error}"))?;
        db.call(|conn| db::configure(conn))
            .await
            .map_err(|error| format!("Datenbank kann nicht eingerichtet werden: {error}"))?;

        let state = AppState {
            db,
            config: Arc::new(config),
            password: Arc::new(password),
            login: Default::default(),
            fetcher: Arc::new(fetcher),
            worker: Default::default(),
            images_dir,
        };
        worker::spawn(state.clone());
        backup::spawn(state.clone(), state.config.data_dir.clone());
        web::run(state).await.map_err(|error| error.to_string())
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!("Server beendet mit Fehler: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Befehl `reset-2fa`: neues Geheimnis, alle Sitzungen abmelden.
/// Der laufende Server liest den Zustand bei jeder Anfrage aus der
/// Datenbank, ein Neustart ist daher nicht nötig.
fn reset_2fa() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => return fail(message),
    };
    init_tracing(&config.log_level);

    let mut conn = match db::prepare(&config.data_dir) {
        Ok(conn) => conn,
        Err(message) => return fail(message),
    };
    match two_factor::reset(&mut conn) {
        Ok(url) => {
            println!("Die Zwei-Faktor-Authentifizierung wurde zurückgesetzt.");
            println!("Alle Sitzungen wurden abgemeldet. Ein Neustart ist nicht nötig.");
            two_factor::print_setup(&url);
            ExitCode::SUCCESS
        }
        Err(message) => fail(message),
    }
}

/// Datenbank und Sicherungen enthalten das 2FA-Geheimnis: den Datenordner
/// nur für den App-Benutzer zugänglich machen (gilt auch für ältere Dateien).
fn restrict_data_dir(data_dir: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) =
            std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
        {
            tracing::warn!(
                "Rechte von {} nicht einschränkbar: {error}",
                data_dir.display()
            );
        }
    }
    #[cfg(not(unix))]
    let _ = data_dir;
}

fn init_tracing(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_new(format!("sebo={level}"))
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sebo=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
}
