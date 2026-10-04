-- Project types (Kees, 2026-10-04), set by hand per project: chat (as before), game
-- (assets from the library attached to it), programming (a repo folder on a computer,
-- the default for W2 "Run it on a computer").
ALTER TABLE projects ADD COLUMN ptype TEXT NOT NULL DEFAULT 'chat';
ALTER TABLE projects ADD COLUMN repo_folder TEXT;
ALTER TABLE projects ADD COLUMN repo_machine_id TEXT;

-- Assets attached to a game project. asset(id) belongs to the asset library (0100 and up,
-- created after this file on a fresh install; SQLite checks the reference on use).
CREATE TABLE project_asset (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    asset_id   INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    added_at   TEXT NOT NULL,
    PRIMARY KEY (project_id, asset_id)
);
CREATE INDEX project_asset_by_asset ON project_asset(asset_id);
