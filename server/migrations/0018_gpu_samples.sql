-- M6-01: the GPU ledger every 10 s, kept 24 h, for the trace timeline (M6-04).
CREATE TABLE gpu_sample (
    gpu_id       TEXT NOT NULL,
    at           TEXT NOT NULL,
    used_mib     INTEGER,          -- measured (NULL when the GPU didn't report it)
    reserved_mib INTEGER NOT NULL, -- loaded holders at their peak (weights + KV cache)
    other_mib    INTEGER NOT NULL, -- in use by something the ledger doesn't know
    holdings     TEXT NOT NULL     -- JSON: [{name, kind, nowMib, peakMib, busy}]
);
CREATE INDEX gpu_sample_by_gpu ON gpu_sample(gpu_id, at);
