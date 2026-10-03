-- Per-user task notifications by mail.
CREATE TABLE notification_prefs (
    user_id        TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email          TEXT NOT NULL DEFAULT '',
    on_needs_input INTEGER NOT NULL DEFAULT 1,
    on_failed      INTEGER NOT NULL DEFAULT 1,
    on_done        INTEGER NOT NULL DEFAULT 0,
    daily_summary  INTEGER NOT NULL DEFAULT 0,
    last_daily     TEXT
);
