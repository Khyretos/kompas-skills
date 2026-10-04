-- One-time pairing codes for the one-line runner install (F6). Only the hash is kept.
CREATE TABLE pair_codes (
    code_hash  TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
