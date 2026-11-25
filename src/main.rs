mod config;
mod web;

use std::process::ExitCode;

use config::Config;

const USAGE: &str = "Verwendung: sebo [serve | reset-2fa | hash-password]";

fn main() -> ExitCode {
    let command = std::env::args().nth(1);

    match command.as_deref() {
        None | Some("serve") => serve(),
        Some("reset-2fa") | Some("hash-password") => {
            eprintln!("Dieser Befehl ist noch nicht umgesetzt.");
            ExitCode::FAILURE
        }
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

fn serve() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("Konfigurationsfehler: {message}");
            return ExitCode::FAILURE;
        }
    };

    init_tracing(&config.log_level);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Laufzeitumgebung konnte nicht gestartet werden: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(web::run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("Server beendet mit Fehler: {error}");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_new(format!("sebo={level},tower_http={level}"))
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sebo=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
}
