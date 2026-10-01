-- Users and sessions (session tokens are stored only as SHA-256 hashes).
CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    updated_at  TEXT NOT NULL
);

CREATE TABLE chats (
    id         TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    title      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id      TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    author  TEXT NOT NULL CHECK (author IN ('user', 'orchestrator')),
    text    TEXT NOT NULL,
    at      TEXT NOT NULL
);
CREATE INDEX messages_chat ON messages(chat_id, at);

-- Which model fills which role. Skills belong to roles, not to models.
CREATE TABLE roles (
    role        TEXT PRIMARY KEY CHECK (role IN ('orchestrator', 'worker', 'reviewer')),
    provider_id TEXT NOT NULL,
    model_id    TEXT NOT NULL
);

-- Every model call, stored in full: what was asked, why, and what it cost.
CREATE TABLE calls (
    id          TEXT PRIMARY KEY,
    chat_id     TEXT REFERENCES chats(id) ON DELETE SET NULL,
    task_id     TEXT,
    role        TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    reason      TEXT NOT NULL,
    request     TEXT NOT NULL, -- JSON, secrets never included
    response    TEXT NOT NULL,
    tokens_in   INTEGER,
    tokens_out  INTEGER,
    ms          INTEGER NOT NULL,
    error       TEXT,
    at          TEXT NOT NULL
);
CREATE INDEX calls_at ON calls(at);
