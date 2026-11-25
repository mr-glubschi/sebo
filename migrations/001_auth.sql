-- Einstellungen als Schlüssel/Wert (2FA-Geheimnis, Status usw.)
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- Angemeldete Sitzungen. Gespeichert wird nur der SHA-256-Hash des Tokens.
CREATE TABLE sessions (
    token_hash   TEXT PRIMARY KEY,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
) STRICT;

-- Wiederherstellungscodes (SHA-256-Hash). Ein benutzter Code wird gelöscht.
CREATE TABLE recovery_codes (
    code_hash TEXT PRIMARY KEY
) STRICT;
