-- Lessons a task run's reviewer proposes; the user accepts, edits or dismisses each in the project thread.
CREATE TABLE IF NOT EXISTS lessons (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    chat_id TEXT NOT NULL,
    run_id TEXT,
    task_id TEXT,
    card TEXT NOT NULL,
    text TEXT NOT NULL,
    finding TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'proposed' CHECK (state IN ('proposed', 'accepted', 'dismissed')),
    created_at TEXT NOT NULL,
    decided_at TEXT
);

CREATE INDEX IF NOT EXISTS lessons_by_chat ON lessons(chat_id, created_at);
