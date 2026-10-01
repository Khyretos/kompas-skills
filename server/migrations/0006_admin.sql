-- Admins, each user's light/dark choice, and admin settings (key -> JSON value).
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN theme TEXT NOT NULL DEFAULT 'system'
    CHECK (theme IN ('system', 'light', 'dark'));
-- Machines tab refresh in seconds; 1 = live (pushed over the event stream).
ALTER TABLE users ADD COLUMN machines_refresh INTEGER NOT NULL DEFAULT 5;
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
