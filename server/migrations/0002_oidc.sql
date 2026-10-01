-- Single sign-on: an account can be linked to one OIDC subject.
-- Accounts created by single sign-on have an empty password_hash, which never verifies.
ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ADD COLUMN oidc_issuer TEXT;
ALTER TABLE users ADD COLUMN oidc_subject TEXT;
CREATE UNIQUE INDEX users_oidc ON users (oidc_issuer, oidc_subject) WHERE oidc_subject IS NOT NULL;
