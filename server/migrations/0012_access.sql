-- F5: jobs for runners, a mirror of each machine's grants, and the access history.
CREATE TABLE machine_jobs (
    id         TEXT PRIMARY KEY,
    machine_id TEXT NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tool       TEXT NOT NULL,
    state      TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'sent', 'done', 'refused', 'failed')),
    result     TEXT,
    created_at TEXT NOT NULL,
    sent_at    TEXT,
    done_at    TEXT,
    task_id    TEXT
);
CREATE INDEX machine_jobs_queue ON machine_jobs (machine_id, state, created_at);
CREATE TABLE machine_grants (
    machine_id TEXT NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    target     TEXT NOT NULL,
    rights     TEXT NOT NULL,
    granted_by TEXT NOT NULL,
    granted_at TEXT NOT NULL,
    expires    TEXT,
    PRIMARY KEY (machine_id, target)
);
CREATE TABLE access_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    machine_id TEXT NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    at         TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('granted', 'revoked', 'used', 'refused')),
    target     TEXT,
    detail     TEXT
);
CREATE INDEX access_log_user ON access_log (user_id, id);
