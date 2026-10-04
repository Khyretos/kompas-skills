-- Assets, milestone 2: previews (images and audio, made on the server's CPU) and probe data.
ALTER TABLE asset ADD COLUMN preview_state TEXT;   -- NULL: to do | ok | none (can't preview) | error
ALTER TABLE asset ADD COLUMN preview_kind TEXT;    -- image | audio
ALTER TABLE asset ADD COLUMN preview_v INTEGER NOT NULL DEFAULT 0; -- bumped on every new preview (cache key)
ALTER TABLE asset ADD COLUMN preview_error TEXT;
ALTER TABLE asset ADD COLUMN width INTEGER;
ALTER TABLE asset ADD COLUMN height INTEGER;
ALTER TABLE asset ADD COLUMN has_alpha INTEGER;
ALTER TABLE asset ADD COLUMN duration_s REAL;
ALTER TABLE asset ADD COLUMN sample_rate INTEGER;
ALTER TABLE asset ADD COLUMN channels INTEGER;
ALTER TABLE asset ADD COLUMN peaks TEXT;           -- 64 levels, one base-36 character each
CREATE INDEX asset_preview_todo ON asset(ext) WHERE preview_state IS NULL;
