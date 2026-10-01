-- Every user has their own projects, chats, tasks, call log and roles.
-- Data from before this change goes to the first account.
ALTER TABLE projects ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE chats    ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE tasks    ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE calls    ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE CASCADE;

UPDATE projects SET user_id = (SELECT id FROM users ORDER BY created_at LIMIT 1);
UPDATE chats    SET user_id = (SELECT id FROM users ORDER BY created_at LIMIT 1);
UPDATE tasks    SET user_id = (SELECT id FROM users ORDER BY created_at LIMIT 1);
UPDATE calls    SET user_id = (SELECT id FROM users ORDER BY created_at LIMIT 1);

CREATE INDEX projects_user ON projects (user_id);
CREATE INDEX chats_user ON chats (user_id);
CREATE INDEX tasks_user ON tasks (user_id);
CREATE INDEX calls_user ON calls (user_id, at);

CREATE TABLE user_roles (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('orchestrator', 'worker', 'reviewer')),
    provider_id TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    PRIMARY KEY (user_id, role)
);
INSERT INTO user_roles (user_id, role, provider_id, model_id)
    SELECT u.id, r.role, r.provider_id, r.model_id
    FROM roles r, (SELECT id FROM users ORDER BY created_at LIMIT 1) u;
DROP TABLE roles;
