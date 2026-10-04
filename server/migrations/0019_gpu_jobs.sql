-- M6-02: GPU jobs. A job reserves VRAM (and host RAM) on one GPU from start to end.
-- Its owner beats every 30 s; a job whose owner stopped beating is dropped, so a crashed
-- worker never holds a GPU.
CREATE TABLE gpu_job (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,            -- chat | code | asset
    what       TEXT NOT NULL,            -- for people: "ComfyUI: fox sprites"
    gpus       TEXT NOT NULL,            -- JSON: GPU ids in preference order
    vram_mib   INTEGER NOT NULL,
    ram_mib    INTEGER NOT NULL,
    tonight    INTEGER NOT NULL DEFAULT 0,
    state      TEXT NOT NULL,            -- queued | running | done | failed | dropped
    gpu        TEXT,                     -- where it runs
    created_at TEXT NOT NULL,
    started_at TEXT,
    ended_at   TEXT,
    beat_at    TEXT NOT NULL,
    error      TEXT
);
CREATE INDEX gpu_job_by_state ON gpu_job(state, created_at);
