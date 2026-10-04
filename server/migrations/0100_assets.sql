-- Assets section: an index of the game asset library (mounted read-only).
-- Numbered 0100 so it never collides with the app's own 00xx migrations.

-- A pack: one zip or Unity package (anywhere in the library), one top-level folder
-- of loose files, or the loose files at the library root (key '').
CREATE TABLE asset_pack (
    id            INTEGER PRIMARY KEY,
    key           TEXT NOT NULL UNIQUE,   -- library path of the pack file, or the folder name
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL,          -- zip | unitypackage | folder | loose
    size          INTEGER NOT NULL DEFAULT 0, -- bytes on disk
    mtime         INTEGER NOT NULL DEFAULT 0,
    duplicate_of  INTEGER REFERENCES asset_pack(id) ON DELETE SET NULL, -- " (1)" re-download
    error         TEXT,                   -- why its contents could not be listed
    missing_since TEXT,                   -- gone from the library (rows are never deleted by a scan)
    seen_scan     INTEGER NOT NULL DEFAULT 0
);

-- One file: loose in the library, or an entry inside a pack file (never unpacked).
CREATE TABLE asset (
    id            INTEGER PRIMARY KEY,
    pack_id       INTEGER NOT NULL REFERENCES asset_pack(id) ON DELETE CASCADE,
    container     TEXT NOT NULL DEFAULT '', -- library path of the zip/unitypackage, '' when loose
    path          TEXT NOT NULL,            -- inside the container, else relative to the library
    name          TEXT NOT NULL,
    ext           TEXT NOT NULL,
    size          INTEGER NOT NULL,         -- unpacked bytes
    mtime         INTEGER,                  -- loose files only
    entry_offset  INTEGER,                  -- zip local header, for reading one entry later
    category      TEXT NOT NULL,
    rule          TEXT NOT NULL,            -- why it got that category
    is_meta       INTEGER NOT NULL DEFAULT 0, -- licence, readme, credits
    dup_of        INTEGER,                  -- same size and name as that asset: shown once
    missing_since TEXT,
    seen_scan     INTEGER NOT NULL DEFAULT 0,
    UNIQUE (container, path)
);
CREATE INDEX asset_by_pack ON asset(pack_id, path);
CREATE INDEX asset_by_category ON asset(category);

-- Search over names, paths, pack names and categories (file contents are never indexed).
CREATE VIRTUAL TABLE asset_fts USING fts5(name, path, pack, category, tokenize = 'unicode61');

CREATE TABLE asset_scan (
    id          INTEGER PRIMARY KEY,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    rules       INTEGER NOT NULL DEFAULT 0, -- classifier version the scan used
    files       INTEGER NOT NULL DEFAULT 0, -- loose files on disk
    entries     INTEGER NOT NULL DEFAULT 0, -- files inside zips
    unity       INTEGER NOT NULL DEFAULT 0, -- files inside Unity packages
    packs_read  INTEGER NOT NULL DEFAULT 0, -- pack files (re)listed; unchanged ones are skipped
    errors      TEXT NOT NULL DEFAULT '[]', -- JSON list, first 50
    took_ms     INTEGER
);
