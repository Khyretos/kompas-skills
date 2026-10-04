-- Assets, milestone 5: "Used in", filled from the game repos' scene files (kk-engine
-- *.scene.json: "packs" and objects[].asset). Rebuilt by every scan; never edited by hand.

-- An asset placed in a game's scene.
CREATE TABLE asset_use (
    game_id  INTEGER NOT NULL REFERENCES asset_game(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    scene    TEXT NOT NULL,              -- the scene file, relative to its repo
    count    INTEGER NOT NULL DEFAULT 1, -- objects placing it (a grid counts once)
    PRIMARY KEY (game_id, asset_id, scene)
);
CREATE INDEX asset_use_by_asset ON asset_use(asset_id);

-- A pack a scene loads ("packs"), found in the library.
CREATE TABLE asset_pack_use (
    game_id INTEGER NOT NULL REFERENCES asset_game(id) ON DELETE CASCADE,
    pack_id INTEGER NOT NULL REFERENCES asset_pack(id) ON DELETE CASCADE,
    scene   TEXT NOT NULL,
    PRIMARY KEY (game_id, pack_id, scene)
);
CREATE INDEX asset_pack_use_by_pack ON asset_pack_use(pack_id);

-- What a scene names that isn't in the library (a pack or an asset).
CREATE TABLE asset_scene_miss (
    game_id INTEGER NOT NULL REFERENCES asset_game(id) ON DELETE CASCADE,
    scene   TEXT NOT NULL,
    kind    TEXT NOT NULL,               -- pack | asset
    name    TEXT NOT NULL,
    PRIMARY KEY (game_id, scene, kind, name)
);
