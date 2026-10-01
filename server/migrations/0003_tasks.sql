-- Tasks belong to a project. Imported tasks keep a pointer to where they came from.
ALTER TABLE projects ADD COLUMN source TEXT;
CREATE TABLE tasks (
    id         TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    state      TEXT NOT NULL DEFAULT 'queued'
               CHECK (state IN ('queued','waiting_resources','running','needs_input','in_review','done','failed')),
    progress   REAL NOT NULL DEFAULT 0,
    step       TEXT NOT NULL DEFAULT '',
    role       TEXT NOT NULL DEFAULT 'worker',
    model      TEXT NOT NULL DEFAULT '',
    source     TEXT,
    updated_at TEXT NOT NULL
);
CREATE INDEX tasks_project ON tasks (project_id);
