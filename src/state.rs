use std::{path::PathBuf, sync::Arc};

use tokio::sync::{Mutex, Notify};

use crate::{config::Config, db::Db, net::Fetcher, password::PasswordCheck, security::Throttle};

/// Gemeinsamer Zustand aller Anfragen.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub config: Arc<Config>,
    pub password: Arc<PasswordCheck>,
    /// Hält die Wartezeit nach Fehlversuchen. Logins laufen nacheinander,
    /// damit parallele Versuche die Wartezeit nicht umgehen.
    pub login: Arc<Mutex<Throttle>>,
    /// Abruf fremder Seiten mit SSRF-Schutz.
    pub fetcher: Arc<Fetcher>,
    /// Weckt den Hintergrund-Abruf, wenn es neue Bookmarks gibt.
    pub worker: Arc<Notify>,
    /// Ablage der Vorschaubilder und Favicons.
    pub images_dir: PathBuf,
}
