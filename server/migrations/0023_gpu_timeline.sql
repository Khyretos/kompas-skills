-- M6-04 trace timeline: watts per GPU sample, and events that explain gaps (role switches,
-- OVMS restarts). Both are kept 24 hours, like the samples.
ALTER TABLE gpu_sample ADD COLUMN watts REAL;
CREATE TABLE gpu_event (
    id     INTEGER PRIMARY KEY,
    gpu_id TEXT NOT NULL,
    at     TEXT NOT NULL,
    kind   TEXT NOT NULL,          -- role | restart | note
    detail TEXT NOT NULL DEFAULT ''
);
CREATE INDEX gpu_event_by_gpu ON gpu_event (gpu_id, at);
