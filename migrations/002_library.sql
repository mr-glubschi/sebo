-- Ordner, beliebig tief verschachtelt. parent_id NULL = oberste Ebene.
CREATE TABLE folders (
    id         INTEGER PRIMARY KEY,
    parent_id  INTEGER REFERENCES folders (id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX folders_parent ON folders (parent_id);

-- Bookmarks. folder_id NULL = „Unsortiert“.
-- title_auto = 1: Titel darf durch den abgerufenen Seitentitel ersetzt werden.
-- has_image / has_icon: Datei liegt unter /data/images/<id>-og.jpg bzw. <id>-icon.png.
CREATE TABLE bookmarks (
    -- AUTOINCREMENT: IDs gelöschter Bookmarks werden nie wiederverwendet
    -- (Bilddateien sind nach der ID benannt).
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id   INTEGER REFERENCES folders (id) ON DELETE CASCADE,
    url         TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    title_auto  INTEGER NOT NULL DEFAULT 1,
    has_image   INTEGER NOT NULL DEFAULT 0,
    has_icon    INTEGER NOT NULL DEFAULT 0,
    meta_status TEXT    NOT NULL DEFAULT 'pending'
        CHECK (meta_status IN ('pending', 'done', 'failed', 'skipped')),
    -- Abgebrochene Abrufversuche seit dem letzten Ergebnis.
    fetch_attempts INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
) STRICT;

CREATE INDEX bookmarks_folder ON bookmarks (folder_id);
CREATE INDEX bookmarks_url ON bookmarks (url);
CREATE INDEX bookmarks_pending ON bookmarks (id) WHERE meta_status = 'pending';
