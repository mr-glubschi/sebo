//! Ordner und Bookmarks: Datenmodell, Prüfung der Eingaben, Datenbankzugriff.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use url::Url;

use crate::{db::now_secs, meta::clean_title};

pub const MAX_URL_LENGTH: usize = 2048;
pub const MAX_FOLDER_NAME_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Folder {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: i64,
    #[cfg_attr(test, ts(type = "number | null"))]
    pub parent_id: Option<i64>,
    pub name: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum MetaStatus {
    Pending,
    Done,
    Failed,
    Skipped,
}

impl MetaStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "done" => Self::Done,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Bookmark {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: i64,
    #[cfg_attr(test, ts(type = "number | null"))]
    pub folder_id: Option<i64>,
    pub url: String,
    pub title: String,
    /// Adresse des Vorschaubilds (vom eigenen Server), falls vorhanden.
    pub image: Option<String>,
    /// Adresse des Favicons (vom eigenen Server), falls vorhanden.
    pub icon: Option<String>,
    pub meta_status: MetaStatus,
    #[cfg_attr(test, ts(type = "number"))]
    pub created_at: i64,
    #[cfg_attr(test, ts(type = "number"))]
    pub updated_at: i64,
}

/// Alles, was der Browser zum Anzeigen und Durchsuchen braucht.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Library {
    pub folders: Vec<Folder>,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, PartialEq)]
pub enum LibraryError {
    NotFound,
    Invalid(String),
    Db(String),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "nicht gefunden"),
            Self::Invalid(message) | Self::Db(message) => write!(f, "{message}"),
        }
    }
}

impl From<rusqlite::Error> for LibraryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LibraryError>;

fn invalid(message: &str) -> LibraryError {
    LibraryError::Invalid(message.into())
}

// ---------------------------------------------------------------------------
// Eingaben prüfen

/// Prüft und normalisiert eine URL. Ohne Schema wird `https://` ergänzt.
pub fn parse_url(input: &str) -> Result<Url> {
    let input = input.trim();
    if input.is_empty() {
        return Err(invalid("Bitte eine URL angeben"));
    }
    if input.len() > MAX_URL_LENGTH {
        return Err(invalid("Die URL ist zu lang"));
    }
    let url = match Url::parse(input) {
        Ok(url) => url,
        Err(url::ParseError::RelativeUrlWithoutBase) => Url::parse(&format!("https://{input}"))
            .map_err(|_| invalid("Das ist keine gültige URL"))?,
        Err(_) => return Err(invalid("Das ist keine gültige URL")),
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("Nur http- und https-Adressen sind erlaubt"));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("Die URL braucht einen Host"));
    }
    if url.as_str().len() > MAX_URL_LENGTH {
        return Err(invalid("Die URL ist zu lang"));
    }
    Ok(url)
}

pub fn clean_folder_name(input: &str) -> Result<String> {
    let name: String = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_FOLDER_NAME_CHARS)
        .collect();
    if name.is_empty() {
        return Err(invalid("Bitte einen Ordnernamen angeben"));
    }
    Ok(name)
}

/// Platzhalter-Titel, solange die Seite nicht abgerufen ist: der Host.
pub fn fallback_title(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

// ---------------------------------------------------------------------------
// Bilder

pub fn image_path(images_dir: &Path, id: i64) -> PathBuf {
    images_dir.join(format!("{id}-og.jpg"))
}

pub fn icon_path(images_dir: &Path, id: i64) -> PathBuf {
    images_dir.join(format!("{id}-icon.png"))
}

/// Löscht die Bilddateien der angegebenen Bookmarks (Fehler werden ignoriert).
pub fn remove_image_files(images_dir: &Path, ids: &[i64]) {
    for id in ids {
        let _ = std::fs::remove_file(image_path(images_dir, *id));
        let _ = std::fs::remove_file(icon_path(images_dir, *id));
    }
}

// ---------------------------------------------------------------------------
// Lesen

const BOOKMARK_COLUMNS: &str =
    "id, folder_id, url, title, has_image, has_icon, meta_status, created_at, updated_at";

fn bookmark_from_row(row: &Row) -> rusqlite::Result<Bookmark> {
    let id: i64 = row.get(0)?;
    let updated_at: i64 = row.get(8)?;
    let has_image: bool = row.get(4)?;
    let has_icon: bool = row.get(5)?;
    let status: String = row.get(6)?;
    Ok(Bookmark {
        id,
        folder_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        // ?v= sorgt dafür, dass der Browser neue Bilder lädt, obwohl er
        // Bilder dauerhaft cachen darf.
        image: has_image.then(|| format!("/api/images/{id}-og.jpg?v={updated_at}")),
        icon: has_icon.then(|| format!("/api/images/{id}-icon.png?v={updated_at}")),
        meta_status: MetaStatus::parse(&status),
        created_at: row.get(7)?,
        updated_at,
    })
}

fn folder_from_row(row: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        created_at: row.get(3)?,
    })
}

pub fn load(conn: &Connection) -> Result<Library> {
    let folders = conn
        .prepare(
            "SELECT id, parent_id, name, created_at FROM folders ORDER BY name COLLATE NOCASE, id",
        )?
        .query_map([], folder_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    let bookmarks = conn
        .prepare(&format!(
            "SELECT {BOOKMARK_COLUMNS} FROM bookmarks ORDER BY created_at DESC, id DESC"
        ))?
        .query_map([], bookmark_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Library { folders, bookmarks })
}

pub fn get_bookmark(conn: &Connection, id: i64) -> Result<Bookmark> {
    conn.query_row(
        &format!("SELECT {BOOKMARK_COLUMNS} FROM bookmarks WHERE id = ?1"),
        params![id],
        bookmark_from_row,
    )
    .optional()?
    .ok_or(LibraryError::NotFound)
}

pub fn get_folder(conn: &Connection, id: i64) -> Result<Folder> {
    conn.query_row(
        "SELECT id, parent_id, name, created_at FROM folders WHERE id = ?1",
        params![id],
        folder_from_row,
    )
    .optional()?
    .ok_or(LibraryError::NotFound)
}

fn ensure_folder(conn: &Connection, id: Option<i64>) -> Result<()> {
    match id {
        None => Ok(()),
        Some(id) => get_folder(conn, id)
            .map(|_| ())
            .map_err(|error| match error {
                LibraryError::NotFound => invalid("Der Ordner existiert nicht"),
                other => other,
            }),
    }
}

/// IDs des Ordners und aller Unterordner.
fn subtree(conn: &Connection, id: i64) -> Result<Vec<i64>> {
    Ok(conn
        .prepare(
            "WITH RECURSIVE tree(id) AS (
                 SELECT ?1
                 UNION ALL
                 SELECT folders.id FROM folders JOIN tree ON folders.parent_id = tree.id
             )
             SELECT id FROM tree",
        )?
        .query_map(params![id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------------
// Ordner ändern

pub fn create_folder(conn: &Connection, name: &str, parent_id: Option<i64>) -> Result<Folder> {
    let name = clean_folder_name(name)?;
    ensure_folder(conn, parent_id)?;
    conn.execute(
        "INSERT INTO folders (parent_id, name, created_at) VALUES (?1, ?2, ?3)",
        params![parent_id, name, now_secs()],
    )?;
    get_folder(conn, conn.last_insert_rowid())
}

pub fn update_folder(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    parent_id: Option<Option<i64>>,
) -> Result<Folder> {
    get_folder(conn, id)?;
    if let Some(name) = name {
        let name = clean_folder_name(name)?;
        conn.execute(
            "UPDATE folders SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
    }
    if let Some(parent_id) = parent_id {
        ensure_folder(conn, parent_id)?;
        if let Some(parent) = parent_id
            && subtree(conn, id)?.contains(&parent)
        {
            return Err(invalid(
                "Ein Ordner kann nicht in sich selbst oder einen seiner Unterordner verschoben werden",
            ));
        }
        conn.execute(
            "UPDATE folders SET parent_id = ?2 WHERE id = ?1",
            params![id, parent_id],
        )?;
    }
    get_folder(conn, id)
}

/// Was mit dem Inhalt eines gelöschten Ordners passiert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeleteContents {
    /// Unterordner und Bookmarks mitlöschen.
    Delete,
    /// Unterordner und Bookmarks in den übergeordneten Ordner verschieben.
    MoveUp,
}

/// Löscht einen Ordner. Gibt die IDs gelöschter Bookmarks zurück, damit
/// deren Bilder entfernt werden können.
pub fn delete_folder(conn: &mut Connection, id: i64, contents: DeleteContents) -> Result<Vec<i64>> {
    let folder = get_folder(conn, id)?;
    let tx = conn.transaction()?;
    let deleted = match contents {
        DeleteContents::MoveUp => {
            tx.execute(
                "UPDATE folders SET parent_id = ?2 WHERE parent_id = ?1",
                params![id, folder.parent_id],
            )?;
            tx.execute(
                "UPDATE bookmarks SET folder_id = ?2 WHERE folder_id = ?1",
                params![id, folder.parent_id],
            )?;
            Vec::new()
        }
        DeleteContents::Delete => {
            let folders = subtree(&tx, id)?;
            let mut ids = Vec::new();
            for folder_id in &folders {
                let mut stmt = tx.prepare("SELECT id FROM bookmarks WHERE folder_id = ?1")?;
                let found: Vec<i64> = stmt
                    .query_map(params![folder_id], |row| row.get(0))?
                    .collect::<rusqlite::Result<_>>()?;
                ids.extend(found);
            }
            ids
        }
    };
    // ON DELETE CASCADE entfernt Unterordner und deren Bookmarks.
    tx.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Bookmarks ändern

pub fn create_bookmark(
    conn: &Connection,
    url: &str,
    title: Option<&str>,
    folder_id: Option<i64>,
) -> Result<Bookmark> {
    let url = parse_url(url)?;
    ensure_folder(conn, folder_id)?;
    let title = title.and_then(clean_title);
    let title_auto = title.is_none();
    let title = title.unwrap_or_else(|| fallback_title(&url));
    let now = now_secs();
    conn.execute(
        "INSERT INTO bookmarks (folder_id, url, title, title_auto, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![folder_id, url.as_str(), title, title_auto, now],
    )?;
    get_bookmark(conn, conn.last_insert_rowid())
}

pub struct BookmarkUpdate<'a> {
    pub url: Option<&'a str>,
    /// Leerer Titel = wieder automatisch aus der Seite übernehmen.
    pub title: Option<&'a str>,
    pub folder_id: Option<Option<i64>>,
}

/// Ändert ein Bookmark. Gibt zurück, ob die Seite neu abgerufen werden muss.
pub fn update_bookmark(
    conn: &Connection,
    id: i64,
    update: BookmarkUpdate,
) -> Result<(Bookmark, bool)> {
    let current = get_bookmark(conn, id)?;
    let mut refetch = false;

    if let Some(url) = update.url {
        let url = parse_url(url)?;
        if url.as_str() != current.url {
            conn.execute(
                "UPDATE bookmarks SET url = ?2, meta_status = 'pending' WHERE id = ?1",
                params![id, url.as_str()],
            )?;
            refetch = true;
        }
    }
    if let Some(title) = update.title {
        match clean_title(title) {
            Some(title) => conn.execute(
                "UPDATE bookmarks SET title = ?2, title_auto = 0 WHERE id = ?1",
                params![id, title],
            )?,
            None => {
                refetch = true;
                conn.execute(
                    "UPDATE bookmarks SET title_auto = 1, meta_status = 'pending' WHERE id = ?1",
                    params![id],
                )?
            }
        };
    }
    if let Some(folder_id) = update.folder_id {
        ensure_folder(conn, folder_id)?;
        conn.execute(
            "UPDATE bookmarks SET folder_id = ?2 WHERE id = ?1",
            params![id, folder_id],
        )?;
    }
    conn.execute(
        "UPDATE bookmarks SET updated_at = ?2 WHERE id = ?1",
        params![id, now_secs()],
    )?;
    Ok((get_bookmark(conn, id)?, refetch))
}

pub fn delete_bookmark(conn: &Connection, id: i64) -> Result<()> {
    match conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])? {
        0 => Err(LibraryError::NotFound),
        _ => Ok(()),
    }
}

pub fn mark_for_refresh(conn: &Connection, id: i64) -> Result<Bookmark> {
    match conn.execute(
        "UPDATE bookmarks SET meta_status = 'pending' WHERE id = ?1",
        params![id],
    )? {
        0 => Err(LibraryError::NotFound),
        _ => get_bookmark(conn, id),
    }
}

pub fn remove_image(conn: &Connection, id: i64) -> Result<Bookmark> {
    match conn.execute(
        "UPDATE bookmarks SET has_image = 0, updated_at = ?2 WHERE id = ?1",
        params![id, now_secs()],
    )? {
        0 => Err(LibraryError::NotFound),
        _ => get_bookmark(conn, id),
    }
}

// ---------------------------------------------------------------------------
// Für den Hintergrund-Abruf

pub fn pending_ids(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<i64>> {
    conn.prepare("SELECT id FROM bookmarks WHERE meta_status = 'pending' ORDER BY id LIMIT ?1")?
        .query_map(params![limit as i64], |row| row.get(0))?
        .collect()
}

/// Was der Abruf ergeben hat.
pub struct FetchResult {
    pub status: MetaStatus,
    pub title: Option<String>,
    pub has_image: bool,
    pub has_icon: bool,
}

/// Speichert das Ergebnis, aber nur, wenn sich die URL inzwischen nicht
/// geändert hat. Ein Bild, das neu nicht gefunden wurde, bleibt erhalten.
pub fn store_fetch_result(
    conn: &Connection,
    id: i64,
    url: &str,
    result: &FetchResult,
) -> rusqlite::Result<bool> {
    let changed = conn.execute(
        "UPDATE bookmarks SET
             meta_status = ?3,
             title = CASE WHEN title_auto = 1 AND ?4 IS NOT NULL THEN ?4 ELSE title END,
             has_image = CASE WHEN ?5 THEN 1 ELSE has_image END,
             has_icon = CASE WHEN ?6 THEN 1 ELSE has_icon END,
             updated_at = ?7
         WHERE id = ?1 AND url = ?2",
        params![
            id,
            url,
            result.status.as_str(),
            result.title,
            result.has_image,
            result.has_icon,
            now_secs()
        ],
    )?;
    Ok(changed > 0)
}

pub fn bookmark_url(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT url FROM bookmarks WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;

    #[test]
    fn urls_pruefen() {
        assert_eq!(
            parse_url("example.com/a").unwrap().as_str(),
            "https://example.com/a"
        );
        assert_eq!(
            parse_url(" https://Example.com ").unwrap().as_str(),
            "https://example.com/"
        );
        assert_eq!(
            parse_url("http://192.168.1.10:8123").unwrap().as_str(),
            "http://192.168.1.10:8123/"
        );
        assert!(parse_url("javascript:alert(1)").is_err());
        assert!(parse_url("file:///etc/passwd").is_err());
        assert!(parse_url("").is_err());
        assert!(parse_url(&format!("https://example.com/{}", "a".repeat(3000))).is_err());
    }

    #[test]
    fn bookmark_anlegen_aendern_loeschen() {
        let conn = open_test();
        let folder = create_folder(&conn, "  Rust  ", None).unwrap();
        assert_eq!(folder.name, "Rust");

        let bookmark = create_bookmark(
            &conn,
            "https://www.rust-lang.org/learn",
            None,
            Some(folder.id),
        )
        .unwrap();
        assert_eq!(bookmark.title, "rust-lang.org");
        assert_eq!(bookmark.meta_status, MetaStatus::Pending);
        assert_eq!(bookmark.folder_id, Some(folder.id));

        // Abruf speichert Titel, weil er noch automatisch ist.
        let result = FetchResult {
            status: MetaStatus::Done,
            title: Some("Rust lernen".into()),
            has_image: true,
            has_icon: false,
        };
        assert!(store_fetch_result(&conn, bookmark.id, &bookmark.url, &result).unwrap());
        let stored = get_bookmark(&conn, bookmark.id).unwrap();
        assert_eq!(stored.title, "Rust lernen");
        assert!(
            stored
                .image
                .unwrap()
                .starts_with(&format!("/api/images/{}-og.jpg?v=", bookmark.id))
        );
        assert_eq!(stored.icon, None);

        // Eigener Titel wird vom Abruf nicht überschrieben.
        let update = BookmarkUpdate {
            url: None,
            title: Some("Mein Titel"),
            folder_id: Some(None),
        };
        let (updated, refetch) = update_bookmark(&conn, bookmark.id, update).unwrap();
        assert!(!refetch);
        assert_eq!(
            (updated.title.as_str(), updated.folder_id),
            ("Mein Titel", None)
        );
        store_fetch_result(&conn, bookmark.id, &bookmark.url, &result).unwrap();
        assert_eq!(
            get_bookmark(&conn, bookmark.id).unwrap().title,
            "Mein Titel"
        );

        // Neue URL: neu abrufen; ein alter Abruf für die alte URL wird verworfen.
        let update = BookmarkUpdate {
            url: Some("https://doc.rust-lang.org"),
            title: None,
            folder_id: None,
        };
        let (updated, refetch) = update_bookmark(&conn, bookmark.id, update).unwrap();
        assert!(refetch);
        assert_eq!(updated.meta_status, MetaStatus::Pending);
        assert!(!store_fetch_result(&conn, bookmark.id, &bookmark.url, &result).unwrap());

        delete_bookmark(&conn, bookmark.id).unwrap();
        assert_eq!(
            delete_bookmark(&conn, bookmark.id),
            Err(LibraryError::NotFound)
        );
    }

    #[test]
    fn ordner_verschieben_und_loeschen() {
        let mut conn = open_test();
        let a = create_folder(&conn, "A", None).unwrap();
        let b = create_folder(&conn, "B", Some(a.id)).unwrap();
        let c = create_folder(&conn, "C", Some(b.id)).unwrap();
        let in_b = create_bookmark(&conn, "https://b.example", None, Some(b.id)).unwrap();
        let in_c = create_bookmark(&conn, "https://c.example", None, Some(c.id)).unwrap();

        // Keine Kreise.
        assert!(matches!(
            update_folder(&conn, a.id, None, Some(Some(c.id))),
            Err(LibraryError::Invalid(_))
        ));
        assert!(matches!(
            update_folder(&conn, a.id, None, Some(Some(a.id))),
            Err(LibraryError::Invalid(_))
        ));
        assert!(matches!(
            create_folder(&conn, "X", Some(999)),
            Err(LibraryError::Invalid(_))
        ));
        assert!(matches!(
            create_folder(&conn, "   ", None),
            Err(LibraryError::Invalid(_))
        ));

        // B löschen, Inhalt nach oben: C und das Bookmark landen in A.
        assert!(
            delete_folder(&mut conn, b.id, DeleteContents::MoveUp)
                .unwrap()
                .is_empty()
        );
        assert_eq!(get_folder(&conn, c.id).unwrap().parent_id, Some(a.id));
        assert_eq!(get_bookmark(&conn, in_b.id).unwrap().folder_id, Some(a.id));

        // A mit Inhalt löschen: alles weg.
        let mut deleted = delete_folder(&mut conn, a.id, DeleteContents::Delete).unwrap();
        deleted.sort();
        assert_eq!(deleted, vec![in_b.id, in_c.id]);
        assert_eq!(get_folder(&conn, c.id), Err(LibraryError::NotFound));
        assert_eq!(get_bookmark(&conn, in_c.id), Err(LibraryError::NotFound));
        assert!(load(&conn).unwrap().bookmarks.is_empty());
    }
}
