-- Steps Kompanion wants to run on a paired PC from a chat (F6). Each one waits
-- for the user's decision in an approval card; the runner still checks grants.
CREATE TABLE pc_actions (
    id         TEXT PRIMARY KEY,
    chat_id    TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL,
    machine_id TEXT NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    tool       TEXT NOT NULL,           -- the runner tool call, JSON
    summary    TEXT NOT NULL,           -- one line for the card
    state      TEXT NOT NULL,           -- pending | approved | denied | done | failed | refused
    result     TEXT,
    created_at TEXT NOT NULL,
    decided_at TEXT
);
CREATE INDEX pc_actions_chat ON pc_actions(chat_id, created_at);
