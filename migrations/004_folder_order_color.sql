-- Eigene Reihenfolge der Ordner innerhalb ihrer Ebene. Gleiche Werte (z. B. 0 für
-- alle bestehenden Ordner) werden nach Namen sortiert, neue Ordner kommen ans Ende.
ALTER TABLE folders ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

-- Farbe des Ordner-Symbols (Name aus der festen Palette im Frontend). NULL = Akzentfarbe.
ALTER TABLE folders ADD COLUMN color TEXT;
