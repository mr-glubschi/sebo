use std::path::PathBuf;

/// Einstellungen aus den Umgebungsvariablen.
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let port = match env("PORT") {
            Some(value) => value
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| {
                    format!("PORT muss eine Zahl zwischen 1 und 65535 sein, nicht „{value}“")
                })?,
            None => 8080,
        };

        let data_dir = env("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/data".into());

        let log_level = env("LOG_LEVEL")
            .unwrap_or_else(|| "info".into())
            .to_lowercase();
        if !matches!(
            log_level.as_str(),
            "error" | "warn" | "info" | "debug" | "trace"
        ) {
            return Err(format!(
                "LOG_LEVEL muss error, warn, info, debug oder trace sein, nicht „{log_level}“"
            ));
        }

        Ok(Self {
            port,
            data_dir,
            log_level,
        })
    }
}

/// Liest eine Umgebungsvariable; leere Werte gelten als nicht gesetzt.
fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}
