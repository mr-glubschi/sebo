-- Favoriten: einzelnes Merkmal pro Bookmark (kein Tagging).
ALTER TABLE bookmarks ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;

-- Symbol eines Ordners (Name aus der festen Auswahl im Frontend). NULL = Standard.
ALTER TABLE folders ADD COLUMN icon TEXT;
