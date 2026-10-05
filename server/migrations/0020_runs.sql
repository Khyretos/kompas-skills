-- W2 runs and their report (item 6): one row per "Run it on a computer".
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    chat_id TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    check_cmd TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    step TEXT,
    plan TEXT NOT NULL DEFAULT '[]',
    rounds TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX runs_by_task ON runs(task_id, started_at);

ALTER TABLE pc_actions ADD COLUMN run_id TEXT;
ALTER TABLE pc_actions ADD COLUMN ended_at TEXT;

ALTER TABLE calls ADD COLUMN run_id TEXT;

CREATE INDEX pc_actions_by_run ON pc_actions(run_id);
CREATE INDEX calls_by_run ON calls(run_id);

-- The runner version from each computer's last report (runner 0.4.5 and newer).
ALTER TABLE machines ADD COLUMN runner_version TEXT;
