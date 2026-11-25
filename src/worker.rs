//! Hintergrund-Abruf von Titel, Vorschaubild und Favicon.
//!
//! Offene Bookmarks stehen mit `meta_status = 'pending'` in der Datenbank.
//! Dadurch geht nach einem Neustart nichts verloren. Höchstens
//! `PARALLEL` Bookmarks werden gleichzeitig abgerufen.

use std::{path::Path, sync::Arc, time::Duration};

use tokio::{sync::Semaphore, task::JoinSet};
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

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(PARALLEL));
        loop {
            let ids = match state
                .db
                .call(|conn| library::pending_ids(conn, BATCH))
                .await
            {
                Ok(ids) => ids,
                Err(error) => {
                    tracing::error!("Offene Bookmarks nicht lesbar: {error}");
                    tokio::time::sleep(IDLE_CHECK).await;
                    continue;
                }
            };

            if ids.is_empty() {
                tokio::select! {
                    () = state.worker.notified() => {}
                    () = tokio::time::sleep(IDLE_CHECK) => {}
                }
                continue;
            }

            let mut tasks = JoinSet::new();
            for id in ids {
                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    return;
                };
                let state = state.clone();
                tasks.spawn(async move {
                    process(&state, id).await;
                    drop(permit);
                });
            }
            while tasks.join_next().await.is_some() {}
        }
    });
}

pub(crate) async fn process(state: &AppState, id: i64) {
    let url = match state
        .db
        .call(move |conn| library::bookmark_url(conn, id))
        .await
    {
        Ok(Some(url)) => url,
        Ok(None) => return,
        Err(error) => {
            tracing::error!("Bookmark {id} nicht lesbar: {error}");
            return;
        }
    };

    let result = match Url::parse(&url) {
        Ok(parsed) => fetch_all(&state.fetcher, &parsed, &state.images_dir, id).await,
        Err(_) => FetchResult {
            status: MetaStatus::Failed,
            title: None,
            has_image: false,
            has_icon: false,
        },
    };
    tracing::debug!("Bookmark {id}: {:?}", result.status);

    let stored = state
        .db
        .call(move |conn| library::store_fetch_result(conn, id, &url, &result))
        .await;
    match stored {
        Ok(true) => {}
        // URL wurde inzwischen geändert oder Bookmark gelöscht.
        Ok(false) => tracing::debug!("Bookmark {id} hat sich während des Abrufs geändert"),
        Err(error) => {
            tracing::error!("Ergebnis für Bookmark {id} nicht speicherbar: {error}");
            // Nicht endlos neu versuchen.
            let _ = state
                .db
                .call(move |conn| {
                    conn.execute(
                        "UPDATE bookmarks SET meta_status = 'failed' WHERE id = ?1",
                        [id],
                    )
                })
                .await;
        }
    }
}

/// Holt Seite, Vorschaubild und Favicon und legt die Bilder ab.
async fn fetch_all(fetcher: &Fetcher, url: &Url, images_dir: &Path, id: i64) -> FetchResult {
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
        result.has_image = write_file(&library::image_path(images_dir, id), &bytes).await;
    }

    for icon_url in page.icons.iter().take(MAX_ICON_ATTEMPTS) {
        if let Some(bytes) = fetch_processed(fetcher, icon_url, images::process_icon).await {
            result.has_icon = write_file(&library::icon_path(images_dir, id), &bytes).await;
            break;
        }
    }
    result
}

/// Ruft nur den Titel einer Seite ab (für den „+“-Dialog).
pub async fn fetch_title(fetcher: &Fetcher, url: &Url) -> Option<String> {
    fetch_page(fetcher, url)
        .await
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
    // HTML-Parsing kann bei großen Seiten etwas dauern.
    tokio::task::spawn_blocking(move || meta::parse(&html, &final_url))
        .await
        .map_err(|error| FetchError::Other(error.to_string()))
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
    tokio::task::spawn_blocking(move || process(&response.body))
        .await
        .ok()?
        .map_err(|error| tracing::debug!("Bild {url} nicht verwendbar: {error}"))
        .ok()
}

/// Schreibt über eine temporäre Datei, damit nie halbe Bilder ausgeliefert werden.
async fn write_file(path: &Path, bytes: &[u8]) -> bool {
    let temporary = path.with_extension("tmp");
    let result = async {
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, path).await
    }
    .await;
    if let Err(error) = result {
        tracing::error!("Bild {} nicht speicherbar: {error}", path.display());
        let _ = tokio::fs::remove_file(&temporary).await;
        return false;
    }
    true
}
