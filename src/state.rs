use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{config::Config, db::Db, password::PasswordCheck, security::Throttle};

/// Gemeinsamer Zustand aller Anfragen.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub config: Arc<Config>,
    pub password: Arc<PasswordCheck>,
    /// Hält die Wartezeit nach Fehlversuchen. Logins laufen nacheinander,
    /// damit parallele Versuche die Wartezeit nicht umgehen.
    pub login: Arc<Mutex<Throttle>>,
}
