-- Task descriptions, order, change history; projects get a kind (internal or
-- windshift) for the Windshift sync.
ALTER TABLE tasks ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN position REAL NOT NULL DEFAULT 0;
-- Changed in Kompanion and not yet written to Windshift.
ALTER TABLE tasks ADD COLUMN sync_dirty INTEGER NOT NULL DEFAULT 0;
-- Windshift's updated_at at the last sync, to spot changes on both sides.
ALTER TABLE tasks ADD COLUMN remote_updated_at TEXT;

ALTER TABLE projects ADD COLUMN kind TEXT NOT NULL DEFAULT 'internal'
    CHECK (kind IN ('internal', 'windshift'));
UPDATE projects SET kind = 'windshift' WHERE source LIKE 'windshift:%';

-- Imported tasks keep their Windshift order for now.
UPDATE tasks SET position = CAST(substr(id, length(rtrim(id, '0123456789')) + 1) AS REAL)
    WHERE source LIKE 'windshift:%';

CREATE TABLE task_events (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    at      TEXT NOT NULL,
    kind    TEXT NOT NULL,
    detail  TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX task_events_task ON task_events (task_id, id);

-- Deleted here, still to be closed in Windshift.
CREATE TABLE sync_deletes (
    task_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source  TEXT,
    at      TEXT NOT NULL
);
