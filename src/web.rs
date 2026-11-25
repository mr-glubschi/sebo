use axum::{
    Router,
    body::Body,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;
use tokio::net::TcpListener;

use crate::config::Config;

/// Das gebaute Svelte-Frontend, eingebettet ins Programm.
/// Beim Entwickeln (Debug-Build) wird direkt von der Platte gelesen.
#[derive(Embed)]
#[folder = "web/dist"]
#[allow_missing = true]
struct Assets;

pub async fn run(config: Config) -> std::io::Result<()> {
    let app = router();

    let listener = TcpListener::bind(("0.0.0.0", config.port)).await?;
    tracing::info!(
        "SEBO {} läuft auf Port {}, Daten in {}",
        env!("CARGO_PKG_VERSION"),
        config.port,
        config.data_dir.display()
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("SEBO beendet");
    Ok(())
}

fn router() -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .fallback(static_file)
}

/// Liefert Dateien des Frontends aus. Unbekannte Pfade ohne Dateiendung
/// bekommen `index.html`, damit das Routing im Browser funktioniert.
async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = Assets::get(path) {
        return file_response(path, file);
    }

    let looks_like_file = path
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'));
    if !looks_like_file
        && !path.starts_with("api/")
        && let Some(index) = Assets::get("index.html")
    {
        return file_response("index.html", index);
    }

    StatusCode::NOT_FOUND.into_response()
}

fn file_response(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite versieht Dateien unter assets/ mit einem Hash im Namen, die
    // dürfen dauerhaft gecacht werden. Alles andere wird immer neu geprüft.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    let mut response = Response::new(Body::from(file.data.into_owned()));
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// Wartet auf Strg+C oder SIGTERM (`docker stop`), damit laufende Anfragen
/// sauber abgeschlossen werden.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("Beende SEBO …");
}
