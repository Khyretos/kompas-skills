-- The ID token of a single sign-on session, used as id_token_hint to sign out
-- of the provider too.
ALTER TABLE sessions ADD COLUMN id_token TEXT;
