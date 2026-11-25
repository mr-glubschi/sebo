use std::path::PathBuf;

/// Einstellungen aus den Umgebungsvariablen (ohne Passwort, das liest
/// `password::PasswordCheck` separat, damit es nie in Debug-Ausgaben landet).
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub log_level: String,
    /// `X-Forwarded-*` von einem Reverse Proxy auswerten.
    pub trust_proxy: bool,
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

        let trust_proxy = bool_env("TRUST_PROXY")?;

        Ok(Self {
            port,
            data_dir,
            log_level,
            trust_proxy,
        })
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            port: 0,
            data_dir: PathBuf::from("."),
            log_level: "info".into(),
            trust_proxy: false,
        }
    }
}

/// Liest eine Umgebungsvariable; leere Werte gelten als nicht gesetzt.
pub fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn bool_env(name: &str) -> Result<bool, String> {
    match env(name)
        .map(|value| value.trim().to_lowercase())
        .as_deref()
    {
        None | Some("false" | "0" | "no" | "nein") => Ok(false),
        Some("true" | "1" | "yes" | "ja") => Ok(true),
        Some(other) => Err(format!("{name} muss true oder false sein, nicht „{other}“")),
    }
}
