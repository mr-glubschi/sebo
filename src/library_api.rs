//! API für Ordner, Bookmarks und Bilder. Alle Routen brauchen eine Sitzung.

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    api::ApiError,
    library::{self, BookmarkUpdate, DeleteContents, LibraryError},
    session::Authenticated,
    state::AppState,
    worker,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/library", get(load))
        .route("/folders", post(create_folder))
        .route("/folders/{id}", patch(update_folder).delete(delete_folder))
        .route("/bookmarks", post(create_bookmark))
        .route(
            "/bookmarks/{id}",
            patch(update_bookmark).delete(delete_bookmark),
        )
        .route("/bookmarks/{id}/refresh", post(refresh_bookmark))
        .route("/bookmarks/{id}/image", delete(remove_image))
        .route("/preview", post(preview))
        .route("/images/{name}", get(image))
}

impl From<LibraryError> for ApiError {
    fn from(error: LibraryError) -> Self {
        match error {
            LibraryError::NotFound => ApiError::new(StatusCode::NOT_FOUND, "Nicht gefunden"),
            LibraryError::Invalid(message) => ApiError::new(StatusCode::BAD_REQUEST, message),
            LibraryError::Db(message) => ApiError::internal(message),
        }
    }
}

/// Führt eine Bibliotheksfunktion in der Datenbank aus.
async fn with_db<T, F>(state: &AppState, function: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> library::Result<T> + Send + 'static,
{
    match state.db.call(function).await {
        Ok(value) => Ok(value),
        Err(tokio_rusqlite::Error::Error(error)) => Err(error.into()),
        Err(error) => Err(ApiError::internal(error)),
    }
}

/// Unterscheidet „Feld fehlt“ (`None`) von „Feld ist null“ (`Some(None)`).
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

async fn load(
    _: Authenticated,
    State(state): State<AppState>,
) -> Result<Json<library::Library>, ApiError> {
    Ok(Json(with_db(&state, |conn| library::load(conn)).await?))
}

#[derive(Deserialize)]
struct CreateFolder {
    name: String,
    parent_id: Option<i64>,
}

async fn create_folder(
    _: Authenticated,
    State(state): State<AppState>,
    Json(input): Json<CreateFolder>,
) -> Result<(StatusCode, Json<library::Folder>), ApiError> {
    let folder = with_db(&state, move |conn| {
        library::create_folder(conn, &input.name, input.parent_id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

#[derive(Deserialize)]
struct UpdateFolder {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    parent_id: Option<Option<i64>>,
}

async fn update_folder(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateFolder>,
) -> Result<Json<library::Folder>, ApiError> {
    let folder = with_db(&state, move |conn| {
        library::update_folder(conn, id, input.name.as_deref(), input.parent_id)
    })
    .await?;
    Ok(Json(folder))
}

#[derive(Deserialize)]
struct DeleteFolderQuery {
    /// `delete`: Inhalt mitlöschen, `move`: in den übergeordneten Ordner.
    contents: String,
}

async fn delete_folder(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<DeleteFolderQuery>,
) -> Result<StatusCode, ApiError> {
    let contents = match query.contents.as_str() {
        "delete" => DeleteContents::Delete,
        "move" => DeleteContents::MoveUp,
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "contents muss delete oder move sein",
            ));
        }
    };
    let deleted = with_db(&state, move |conn| {
        library::delete_folder(conn, id, contents)
    })
    .await?;
    library::remove_image_files(&state.images_dir, &deleted);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CreateBookmark {
    url: String,
    title: Option<String>,
    folder_id: Option<i64>,
}

async fn create_bookmark(
    _: Authenticated,
    State(state): State<AppState>,
    Json(input): Json<CreateBookmark>,
) -> Result<(StatusCode, Json<library::Bookmark>), ApiError> {
    let bookmark = with_db(&state, move |conn| {
        library::create_bookmark(conn, &input.url, input.title.as_deref(), input.folder_id)
    })
    .await?;
    state.worker.notify_one();
    Ok((StatusCode::CREATED, Json(bookmark)))
}

#[derive(Deserialize)]
struct UpdateBookmark {
    url: Option<String>,
    title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    folder_id: Option<Option<i64>>,
}

async fn update_bookmark(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateBookmark>,
) -> Result<Json<library::Bookmark>, ApiError> {
    let (bookmark, refetch) = with_db(&state, move |conn| {
        let update = BookmarkUpdate {
            url: input.url.as_deref(),
            title: input.title.as_deref(),
            folder_id: input.folder_id,
        };
        library::update_bookmark(conn, id, update)
    })
    .await?;
    if refetch {
        state.worker.notify_one();
    }
    Ok(Json(bookmark))
}

async fn delete_bookmark(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    with_db(&state, move |conn| library::delete_bookmark(conn, id)).await?;
    library::remove_image_files(&state.images_dir, &[id]);
    Ok(StatusCode::NO_CONTENT)
}

async fn refresh_bookmark(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<library::Bookmark>, ApiError> {
    let bookmark = with_db(&state, move |conn| library::mark_for_refresh(conn, id)).await?;
    state.worker.notify_one();
    Ok(Json(bookmark))
}

async fn remove_image(
    _: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<library::Bookmark>, ApiError> {
    let bookmark = with_db(&state, move |conn| library::remove_image(conn, id)).await?;
    let _ = tokio::fs::remove_file(library::image_path(&state.images_dir, id)).await;
    Ok(Json(bookmark))
}

#[derive(Deserialize)]
struct PreviewRequest {
    url: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
struct Preview {
    /// Normalisierte URL.
    url: String,
    title: Option<String>,
}

/// Titelvorschlag für den „+“-Dialog.
async fn preview(
    _: Authenticated,
    State(state): State<AppState>,
    Json(input): Json<PreviewRequest>,
) -> Result<Json<Preview>, ApiError> {
    let url = library::parse_url(&input.url)?;
    let title = worker::fetch_title(&state.fetcher, &url).await;
    Ok(Json(Preview {
        url: url.to_string(),
        title,
    }))
}

/// Liefert ein gespeichertes Vorschaubild oder Favicon aus.
async fn image(
    _: Authenticated,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let not_found = || ApiError::new(StatusCode::NOT_FOUND, "Nicht gefunden");

    // Nur exakt <zahl>-og.jpg oder <zahl>-icon.png, kein Pfad.
    let (id, kind) = name.split_once('-').ok_or_else(not_found)?;
    let content_type = match kind {
        "og.jpg" => "image/jpeg",
        "icon.png" => "image/png",
        _ => return Err(not_found()),
    };
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) || id.len() > 19 {
        return Err(not_found());
    }

    let bytes = tokio::fs::read(state.images_dir.join(&name))
        .await
        .map_err(|_| not_found())?;
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    // Die Adresse enthält ?v=<Änderungszeit>, daher darf lange gecacht werden.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    Ok(response.into_response())
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use axum::{body::Body, http::Request, response::Redirect};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::{
        api::tests::{HOST, send, test_state},
        library::MetaStatus,
        net::Fetcher,
        session,
    };

    const ORIGIN: &str = "http://sebo.test";

    async fn login(state: &AppState) -> String {
        let token = state.db.call(|conn| session::create(conn)).await.unwrap();
        format!("sebo_session={token}")
    }

    fn request(method: &str, path: &str, cookie: &str, body: Option<Value>) -> Request<Body> {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST)
            .header(header::ORIGIN, ORIGIN)
            .header(header::COOKIE, cookie);
        match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    async fn call(
        state: &AppState,
        method: &str,
        path: &str,
        cookie: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, _, json) = send(state, request(method, path, cookie, body)).await;
        (status, json)
    }

    async fn stored(state: &AppState, id: i64) -> library::Bookmark {
        state
            .db
            .call(move |conn| library::get_bookmark(conn, id).map_err(|e| e.to_string()))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn ohne_anmeldung_gesperrt() {
        let state = test_state().await;
        for (method, path) in [
            ("GET", "/api/library"),
            ("POST", "/api/bookmarks"),
            ("GET", "/api/images/1-og.jpg"),
            ("POST", "/api/preview"),
        ] {
            let (status, _) = call(&state, method, path, "sebo_session=x", Some(json!({}))).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        }
    }

    #[tokio::test]
    async fn ordner_und_bookmarks_ueber_die_api() {
        let state = test_state().await;
        let cookie = login(&state).await;

        let (status, folder) = call(
            &state,
            "POST",
            "/api/folders",
            &cookie,
            Some(json!({ "name": "Lesen" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let folder_id = folder["id"].as_i64().unwrap();

        let (status, sub) = call(
            &state,
            "POST",
            "/api/folders",
            &cookie,
            Some(json!({ "name": "Später", "parent_id": folder_id })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(sub["parent_id"], folder_id);

        let (status, bookmark) = call(
            &state,
            "POST",
            "/api/bookmarks",
            &cookie,
            Some(json!({ "url": "example.org/artikel", "folder_id": folder_id })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(bookmark["url"], "https://example.org/artikel");
        assert_eq!(bookmark["title"], "example.org");
        assert_eq!(bookmark["meta_status"], "pending");
        let id = bookmark["id"].as_i64().unwrap();

        let (status, error) = call(
            &state,
            "POST",
            "/api/bookmarks",
            &cookie,
            Some(json!({ "url": "javascript:alert(1)" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error["error"].as_str().unwrap().contains("http"));

        // Titel setzen und nach „Unsortiert“ verschieben (folder_id: null).
        let path = format!("/api/bookmarks/{id}");
        let (status, updated) = call(
            &state,
            "PATCH",
            &path,
            &cookie,
            Some(json!({ "title": "Guter Artikel", "folder_id": null })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["title"], "Guter Artikel");
        assert_eq!(updated["folder_id"], Value::Null);

        // Ein fehlendes Feld lässt den Ordner unverändert.
        call(
            &state,
            "PATCH",
            &path,
            &cookie,
            Some(json!({ "folder_id": folder_id })),
        )
        .await;
        let (_, updated) = call(
            &state,
            "PATCH",
            &path,
            &cookie,
            Some(json!({ "title": "Noch besser" })),
        )
        .await;
        assert_eq!(updated["folder_id"], folder_id);

        let (status, all) = call(&state, "GET", "/api/library", &cookie, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(all["folders"].as_array().unwrap().len(), 2);
        assert_eq!(all["bookmarks"].as_array().unwrap().len(), 1);

        let folder_path = format!("/api/folders/{folder_id}");
        let (status, _) = call(&state, "DELETE", &folder_path, &cookie, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "contents fehlt");
        let (status, _) = call(
            &state,
            "DELETE",
            &format!("{folder_path}?contents=delete"),
            &cookie,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, all) = call(&state, "GET", "/api/library", &cookie, None).await;
        assert!(all["folders"].as_array().unwrap().is_empty());
        assert!(all["bookmarks"].as_array().unwrap().is_empty());

        let (status, _) = call(&state, "DELETE", &path, &cookie, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn bilddateinamen_werden_geprueft() {
        let state = test_state().await;
        let cookie = login(&state).await;
        std::fs::write(state.images_dir.join("geheim.txt"), "x").unwrap();
        for name in [
            "geheim.txt",
            "..%2Fapp.db",
            "1-og.png",
            "-og.jpg",
            "abc-og.jpg",
            "1-og.jpg",
        ] {
            let path = format!("/api/images/{name}");
            let (status, _) = call(&state, "GET", &path, &cookie, None).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{name}");
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 120, 90, 255]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    const TEST_PAGE: &str = "<html><head><title>Testseite</title>\
        <meta property=\"og:image\" content=\"/og.png\">\
        <link rel=\"icon\" href=\"/icon.png\"></head></html>";

    /// Kleiner Webserver als Gegenstelle. Zählt die Anfragen.
    async fn test_site() -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let og = png(800, 400);
        let icon = png(32, 32);
        let app = Router::new()
            .route("/", get(|| async { axum::response::Html(TEST_PAGE) }))
            .route("/weiter", get(|| async { Redirect::temporary("/") }))
            .route(
                "/og.png",
                get(move || async move { ([(header::CONTENT_TYPE, "image/png")], og) }),
            )
            .route(
                "/icon.png",
                get(move || async move { ([(header::CONTENT_TYPE, "image/png")], icon) }),
            )
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    counter.fetch_add(1, Ordering::SeqCst);
                    next.run(request)
                },
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), hits)
    }

    #[tokio::test]
    async fn abruf_von_titel_und_bildern() {
        let state = test_state().await;
        let cookie = login(&state).await;
        let (site, _) = test_site().await;

        let (_, bookmark) = call(
            &state,
            "POST",
            "/api/bookmarks",
            &cookie,
            Some(json!({ "url": format!("{site}/weiter") })),
        )
        .await;
        let id = bookmark["id"].as_i64().unwrap();
        crate::worker::process(&state, id).await;

        let bookmark = stored(&state, id).await;
        assert_eq!(bookmark.meta_status, MetaStatus::Done);
        assert_eq!(bookmark.title, "Testseite");
        assert!(bookmark.icon.is_some(), "Favicon");
        let image_url = bookmark.image.expect("Vorschaubild");

        let response = crate::web::router(state.clone())
            .oneshot(request("GET", &image_url, &cookie, None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/jpeg");
        let cache = headers
            .get(header::CACHE_CONTROL)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cache.contains("immutable"), "{cache}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let saved = image::load_from_memory(&bytes).unwrap();
        assert_eq!((saved.width(), saved.height()), (600, 300));

        // Bild entfernen und Bookmark löschen räumt die Dateien auf.
        let path = format!("/api/bookmarks/{id}");
        let (_, without) = call(&state, "DELETE", &format!("{path}/image"), &cookie, None).await;
        assert_eq!(without["image"], Value::Null);
        assert!(!library::image_path(&state.images_dir, id).exists());
        call(&state, "DELETE", &path, &cookie, None).await;
        assert!(!library::icon_path(&state.images_dir, id).exists());

        // Titelvorschlag für den „+“-Dialog.
        let (status, preview) = call(
            &state,
            "POST",
            "/api/preview",
            &cookie,
            Some(json!({ "url": site })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preview["title"], "Testseite");
    }

    #[tokio::test]
    async fn interne_adressen_werden_nicht_abgerufen() {
        let mut state = test_state().await;
        state.fetcher = Arc::new(Fetcher::new(false).unwrap());
        let cookie = login(&state).await;
        let (site, hits) = test_site().await;

        // Speichern geht, abgerufen wird nichts.
        let (status, bookmark) = call(
            &state,
            "POST",
            "/api/bookmarks",
            &cookie,
            Some(json!({ "url": format!("{site}/") })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = bookmark["id"].as_i64().unwrap();
        crate::worker::process(&state, id).await;

        let bookmark = stored(&state, id).await;
        assert_eq!(bookmark.meta_status, MetaStatus::Skipped);
        assert_eq!(bookmark.image, None);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            0,
            "keine Anfrage an die interne Adresse"
        );
    }
}
