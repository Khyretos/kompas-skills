-- Assets, milestone 4: licences per pack (linked by Kees, never guessed), games with a
-- profile and a needs list, and the AI's picks per need.

CREATE TABLE asset_licence (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,      -- "Synty Standard EULA", "CC0", "CC-BY 4.0"
    commercial  INTEGER NOT NULL DEFAULT 0, -- may ship in a game that is sold
    attribution INTEGER NOT NULL DEFAULT 0, -- the credits must name the author
    url         TEXT,
    notes       TEXT
);
ALTER TABLE asset_pack ADD COLUMN licence_id INTEGER REFERENCES asset_licence(id) ON DELETE SET NULL;

-- A game: found in a game repo (kk-engine games/<name>/game.json) or added by hand.
CREATE TABLE asset_game (
    id            INTEGER PRIMARY KEY,
    key           TEXT NOT NULL UNIQUE,    -- "kk-engine/showcase", or "own/<n>" for one added by hand
    name          TEXT NOT NULL,
    source        TEXT NOT NULL,           -- repo name, or 'own'
    path          TEXT,                    -- its folder inside the mounted repo
    about         TEXT NOT NULL DEFAULT '', -- the repo's own description (game.json)
    genre         TEXT NOT NULL DEFAULT '',
    art_style     TEXT NOT NULL DEFAULT '',
    setting       TEXT NOT NULL DEFAULT '',
    commercial    INTEGER NOT NULL DEFAULT 0, -- will be sold: only packs whose licence allows it
    profile_by    TEXT NOT NULL DEFAULT 'none', -- none | ai (a draft to confirm) | kees
    missing_since TEXT,
    created_at    TEXT NOT NULL
);

-- What a game needs: "footsteps on grass", "calm ambient loop".
CREATE TABLE asset_need (
    id         INTEGER PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES asset_game(id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    category   TEXT,                       -- only this category, or any
    by         TEXT NOT NULL DEFAULT 'kees', -- ai (draft) | kees
    position   INTEGER NOT NULL DEFAULT 0,
    picked_at  TEXT,
    pick_error TEXT
);
CREATE INDEX asset_need_by_game ON asset_need(game_id, position);

-- Picks for a need: the AI suggests (with a reason), Kees keeps one as a candidate or
-- rejects it. A rejected asset is never suggested to that game again.
CREATE TABLE asset_pick (
    need_id  INTEGER NOT NULL REFERENCES asset_need(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    status   TEXT NOT NULL,                -- suggested | candidate | rejected
    reason   TEXT,
    rank     INTEGER NOT NULL DEFAULT 0,
    by       TEXT NOT NULL,                -- ai | kees
    at       TEXT NOT NULL,
    PRIMARY KEY (need_id, asset_id)
);
CREATE INDEX asset_pick_by_asset ON asset_pick(asset_id);
