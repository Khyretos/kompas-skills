-- Assets, milestone 3: AI tags and captions (A770 OVMS), Kees's own tags, category review.
-- Off by default: nothing runs until an admin turns it on (settings key 'assets.ai').
CREATE TABLE asset_tagname (
    id   INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,                 -- style | mood | setting | custom
    name TEXT NOT NULL,
    UNIQUE (kind, name)
);
CREATE TABLE asset_tag (
    asset_id INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES asset_tagname(id) ON DELETE CASCADE,
    by       TEXT NOT NULL,             -- ai | kees
    PRIMARY KEY (asset_id, tag_id)
);
CREATE INDEX asset_tag_by_tag ON asset_tag(tag_id);

ALTER TABLE asset ADD COLUMN ai_state TEXT;        -- NULL: to do | ok | error
ALTER TABLE asset ADD COLUMN ai_error TEXT;
ALTER TABLE asset ADD COLUMN ai_caption TEXT;      -- one sentence
ALTER TABLE asset ADD COLUMN ai_subject TEXT;      -- a few words: "mossy rock", "sword hit"
ALTER TABLE asset ADD COLUMN ai_category TEXT;     -- set only when the AI disagrees with the category
ALTER TABLE asset ADD COLUMN ai_transcript TEXT;   -- voice lines (Whisper)
ALTER TABLE asset ADD COLUMN ai_model TEXT;
ALTER TABLE asset ADD COLUMN ai_at TEXT;
ALTER TABLE asset ADD COLUMN category_by TEXT NOT NULL DEFAULT 'rule'; -- rule | kees (a scan never overrides kees)
CREATE INDEX asset_ai_review ON asset(ai_category) WHERE ai_category IS NOT NULL;

-- Search gets a fifth column with the AI caption, transcript and tags; refilled right here so
-- search keeps working between the deploy and the next scan.
DROP TABLE asset_fts;
CREATE VIRTUAL TABLE asset_fts USING fts5(name, path, pack, category, ai, tokenize = 'unicode61');
INSERT INTO asset_fts (rowid, name, path, pack, category, ai)
SELECT a.id, a.name, CASE a.container WHEN '' THEN a.path ELSE a.container || '/' || a.path END, p.name, a.category, ''
FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.missing_since IS NULL;
