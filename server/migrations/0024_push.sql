-- Android app: UnifiedPush endpoints per user (one per phone), from POST /api/push/register.

CREATE TABLE push_endpoints (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL,
    device TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    last_ok_at TEXT,
    UNIQUE (user_id, endpoint)
);

CREATE INDEX push_endpoints_user ON push_endpoints (user_id);
