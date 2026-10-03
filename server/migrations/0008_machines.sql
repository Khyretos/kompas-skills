-- PCs paired with a runner. Only the SHA-256 of the runner's token is kept.
CREATE TABLE machines (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    last_seen  TEXT
);
CREATE INDEX machines_user ON machines (user_id);
