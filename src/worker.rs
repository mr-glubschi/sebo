//! Hintergrund-Abruf von Titel, Vorschaubild und Favicon.
//!
//! Offene Bookmarks stehen mit `meta_status = 'pending'` in der Datenbank.
//! Dadurch geht nach einem Neustart nichts verloren. Höchstens `PARALLEL`
//! Bookmarks werden gleichzeitig abgerufen. Jeder Versuch wird gezählt;
//! nach `library::MAX_FETCH_ATTEMPTS` (z. B. weil der Container beim
//! Verarbeiten abgestürzt ist) gilt das Bookmark als fehlgeschlagen.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::{sync::Semaphore, task::JoinSet, time::timeout};
use url::Url;

use crate::{
    images,
    library::{self, FetchResult, MetaStatus},
    meta,
    net::{FetchError, Fetcher},
    state::AppState,
};

const PARALLEL: usize = 4;
const BATCH: usize = 16;
pub const MAX_HTML_BYTES: usize = 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_ICON_ATTEMPTS: usize = 3;
const IDLE_CHECK: Duration = Duration::from_secs(60);
/// Gesamtfrist für Seite, Vorschaubild und Favicons eines Bookmarks.
const FETCH_DEADLINE: Duration = Duration::from_secs(30);
/// Frist für rechenintensive Arbeit (HTML-Parsing, Bilder).
const CPU_DEADLINE: Duration = Duration::from_secs(10);

/// Begrenzt rechen- und speicherintensive Bildverarbeitung auf zwei
/// gleichzeitige Aufgaben, unabhängig von der Zahl paralleler Abrufe.
static CPU: Semaphore = Semaphore::const_new(2);

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        // Laufende Abrufe: Task-ID → Bookmark-ID. Frei werdende Plätze werden
        // sofort nachgefüllt, ein langsamer Abruf hält die anderen nicht auf.
        let mut tasks = JoinSet::new();
        let mut running: HashMap<tokio::task::Id, i64> = HashMap::new();
        loop {
            let free = PARALLEL - tasks.len();
            if free > 0 {
                let busy: HashSet<i64> = running.values().copied().collect();
                let ids = state
                    .db
                    .call(move |conn| {
                        library::fail_exhausted(conn)?;
                        library::pending_ids(conn, BATCH + PARALLEL)
                    })
                    .await;
                match ids {
                    Ok(ids) => {
                        for id in ids.into_iter().filter(|id| !busy.contains(id)).take(free) {
                            let task_state = state.clone();
                            let handle = tasks.spawn(async move { process(&task_state, id).await });
                            running.insert(handle.id(), id);
                        }
                    }
                    Err(error) => {
                        tracing::error!("Offene Bookmarks nicht lesbar: {error}");
                        if tasks.is_empty() {
                            tokio::time::sleep(IDLE_CHECK).await;
                            continue;
                        }
                    }
                }
            }

            if tasks.is_empty() {
                tokio::select! {
                    () = state.worker.notified() => {}
                    () = tokio::time::sleep(IDLE_CHECK) => {}
                }
                continue;
            }

            tokio::select! {
                Some(finished) = tasks.join_next_with_id() => {
                    let id = match finished {
                        Ok((id, ())) => id,
                        Err(error) => {
                            tracing::error!("Abruf abgebrochen: {error}");
                            error.id()
                        }
                    };
                    running.remove(&id);
                }
                // Neue Bookmarks: freie Plätze sofort belegen.
                () = state.worker.notified(), if tasks.len() < PARALLEL => {}
            }
        }
    });
}

pub(crate) async fn process(state: &AppState, id: i64) {
    // Zählt den Versuch, bevor gearbeitet wird.
    let url = match state
        .db
        .call(move |conn| library::begin_fetch(conn, id))
        .await
    {
        Ok(Some(url)) => url,
        Ok(None) => return,
        Err(error) => {
            tracing::error!("Bookmark {id} nicht lesbar: {error}");
            return;
        }
    };

    let mut files = NewFiles::default();
    let result = match Url::parse(&url) {
        Ok(parsed) => {
            let work = fetch_all(&state.fetcher, &parsed, &state.images_dir, id, &mut files);
            match timeout(FETCH_DEADLINE, work).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::info!("Abruf von {url} hat zu lange gedauert");
                    files.discard().await;
                    FetchResult::failed()
                }
            }
        }
        Err(_) => FetchResult::failed(),
    };
    tracing::debug!("Bookmark {id}: {:?}", result.status);

    let stored = state
        .db
        .call(move |conn| library::store_fetch_result(conn, id, &url, &result))
        .await;
    match stored {
        Ok(true) => files.commit().await,
        // Bookmark wurde inzwischen gelöscht oder die URL geändert:
        // die neuen Bilder gehören nicht mehr dazu.
        Ok(false) => {
            tracing::debug!("Bookmark {id} hat sich während des Abrufs geändert");
            files.discard().await;
        }
        Err(error) => {
            tracing::error!("Ergebnis für Bookmark {id} nicht speicherbar: {error}");
            files.discard().await;
        }
    }
}

/// Neu geschriebene Bilder. Sie liegen zunächst unter einem temporären
/// Namen und werden erst nach dem Speichern in der Datenbank übernommen.
#[derive(Default)]
struct NewFiles(Vec<(PathBuf, PathBuf)>);

impl NewFiles {
    async fn write(&mut self, target: PathBuf, bytes: &[u8]) -> bool {
        let temporary = target.with_extension("new");
        match tokio::fs::write(&temporary, bytes).await {
            Ok(()) => {
                self.0.push((temporary, target));
                true
            }
            Err(error) => {
                tracing::error!("Bild {} nicht speicherbar: {error}", target.display());
                let _ = tokio::fs::remove_file(&temporary).await;
                false
            }
        }
    }

    async fn commit(self) {
        for (temporary, target) in self.0 {
            if let Err(error) = tokio::fs::rename(&temporary, &target).await {
                tracing::error!("Bild {} nicht übernehmbar: {error}", target.display());
                let _ = tokio::fs::remove_file(&temporary).await;
            }
        }
    }

    async fn discard(&mut self) {
        for (temporary, _) in self.0.drain(..) {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
    }
}

/// Holt Seite, Vorschaubild und Favicon.
async fn fetch_all(
    fetcher: &Fetcher,
    url: &Url,
    images_dir: &Path,
    id: i64,
    files: &mut NewFiles,
) -> FetchResult {
    let mut result = FetchResult {
        status: MetaStatus::Done,
        title: None,
        has_image: false,
        has_icon: false,
    };

    let page = match fetch_page(fetcher, url).await {
        Ok(page) => page,
        Err(FetchError::Blocked) => {
            result.status = MetaStatus::Skipped;
            return result;
        }
        Err(error) => {
            tracing::info!("Seite {url} nicht abrufbar: {error}");
            result.status = MetaStatus::Failed;
            // Favicon trotzdem versuchen, viele Seiten sperren nur Bots.
            meta::PageMeta {
                icons: url.join("/favicon.ico").into_iter().collect(),
                ..Default::default()
            }
        }
    };
    result.title = page.title;

    if let Some(image_url) = page.image
        && let Some(bytes) = fetch_processed(fetcher, &image_url, images::process_og).await
    {
        result.has_image = files
            .write(library::image_path(images_dir, id), &bytes)
            .await;
    }

    for icon_url in page.icons.iter().take(MAX_ICON_ATTEMPTS) {
        if let Some(bytes) = fetch_processed(fetcher, icon_url, images::process_icon).await {
            result.has_icon = files
                .write(library::icon_path(images_dir, id), &bytes)
                .await;
            break;
        }
    }
    result
}

/// Ruft nur den Titel einer Seite ab (für den „+“-Dialog).
pub async fn fetch_title(fetcher: &Fetcher, url: &Url) -> Option<String> {
    timeout(FETCH_DEADLINE, fetch_page(fetcher, url))
        .await
        .ok()?
        .ok()
        .and_then(|page| page.title)
}

async fn fetch_page(fetcher: &Fetcher, url: &Url) -> Result<meta::PageMeta, FetchError> {
    let response = fetcher
        .get(
            url,
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5",
            MAX_HTML_BYTES,
            true,
        )
        .await?;
    let is_html = response
        .content_type
        .as_deref()
        .is_none_or(|kind| kind.contains("html"));
    if !is_html {
        // Direkter Link auf PDF, Bild usw.: kein HTML auszuwerten.
        return Ok(meta::PageMeta {
            icons: response.url.join("/favicon.ico").into_iter().collect(),
            ..Default::default()
        });
    }
    let html = String::from_utf8_lossy(&response.body).into_owned();
    let final_url = response.url;
    run_limited(move || Ok(meta::parse(&html, &final_url)))
        .await
        .map_err(FetchError::Other)
}

async fn fetch_processed(
    fetcher: &Fetcher,
    url: &Url,
    process: fn(&[u8]) -> Result<Vec<u8>, String>,
) -> Option<Vec<u8>> {
    let response = fetcher
        .get(url, "image/*,*/*;q=0.5", MAX_IMAGE_BYTES, false)
        .await
        .map_err(|error| tracing::debug!("Bild {url} nicht abrufbar: {error}"))
        .ok()?;
    run_limited(move || process(&response.body))
        .await
        .map_err(|error| tracing::debug!("Bild {url} nicht verwendbar: {error}"))
        .ok()
}

/// Führt rechenintensive Arbeit in einem eigenen Thread aus, begrenzt in
/// Anzahl und Dauer. Läuft die Arbeit zu lange, wird nicht mehr darauf
/// gewartet; der Abruf geht ohne Ergebnis weiter. Der Platz in `CPU` bleibt
/// belegt, bis der Thread wirklich fertig ist (er lässt sich nicht abbrechen),
/// damit nie mehr als zwei solche Aufgaben gleichzeitig rechnen.
async fn run_limited<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let permit = CPU.acquire().await.map_err(|error| error.to_string())?;
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    });
    match timeout(CPU_DEADLINE, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(format!("Verarbeitung abgebrochen: {error}")),
        Err(_) => Err("Verarbeitung dauerte zu lange".into()),
    }
}
