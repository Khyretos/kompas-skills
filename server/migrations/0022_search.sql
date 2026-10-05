-- Global search (Ctrl+K): one FTS5 index over tasks, chats, chat messages and projects,
-- kept up to date by triggers. search_doc gives every item a stable rowid in the index.
-- Triggers use INSERT ... WHERE NOT EXISTS, never INSERT OR IGNORE: the conflict policy of an
-- outer statement (an upsert) overrides the one inside a trigger.

-- Create the mapping table for the FTS index
CREATE TABLE search_doc (
    rowid INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    ref TEXT NOT NULL,
    UNIQUE (kind, ref)
);

-- Create the virtual FTS5 table
CREATE VIRTUAL TABLE search_fts USING fts5(
    title,
    body,
    kind UNINDEXED,
    ref UNINDEXED,
    parent UNINDEXED,
    user_id UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- ==========================================
-- TASKS
-- ==========================================

-- Insert into search_doc for existing tasks
INSERT INTO search_doc (kind, ref)
SELECT 'task', id FROM tasks;

-- Populate search_fts from existing tasks
INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
SELECT d.rowid, COALESCE(t.title, ''), COALESCE(t.description, ''), 'task', t.id, t.project_id, t.user_id
FROM tasks t
JOIN search_doc d ON d.kind = 'task' AND d.ref = t.id;

-- Trigger: insert/update on tasks
CREATE TRIGGER search_tasks_ins
AFTER INSERT ON tasks
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'task', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'task' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'task' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'task' AND ref = NEW.id),
            COALESCE(NEW.title, ''),
            COALESCE(NEW.description, ''),
            'task',
            NEW.id,
            NEW.project_id,
            NEW.user_id);
END;

CREATE TRIGGER search_tasks_upd
AFTER UPDATE OF title, description, user_id, project_id ON tasks
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'task', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'task' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'task' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'task' AND ref = NEW.id),
            COALESCE(NEW.title, ''),
            COALESCE(NEW.description, ''),
            'task',
            NEW.id,
            NEW.project_id,
            NEW.user_id);
END;

CREATE TRIGGER search_tasks_del
AFTER DELETE ON tasks
BEGIN
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'task' AND ref = OLD.id);
    DELETE FROM search_doc WHERE kind = 'task' AND ref = OLD.id;
END;

-- ==========================================
-- CHATS
-- ==========================================

-- Insert into search_doc for existing chats
INSERT INTO search_doc (kind, ref)
SELECT 'chat', id FROM chats;

-- Populate search_fts from existing chats
INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
SELECT d.rowid, COALESCE(c.title, ''), '', 'chat', c.id, NULL, c.user_id
FROM chats c
JOIN search_doc d ON d.kind = 'chat' AND d.ref = c.id;

-- Trigger: insert/update on chats
CREATE TRIGGER search_chats_ins
AFTER INSERT ON chats
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'chat', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'chat' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'chat' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'chat' AND ref = NEW.id),
            COALESCE(NEW.title, ''),
            '',
            'chat',
            NEW.id,
            NULL,
            NEW.user_id);
END;

CREATE TRIGGER search_chats_upd
AFTER UPDATE OF title, user_id ON chats
BEGIN
    -- Its messages show the chat's title and belong to its owner.
    UPDATE search_fts SET title = COALESCE(NEW.title, ''), user_id = NEW.user_id
    WHERE rowid IN (SELECT d.rowid FROM messages m JOIN search_doc d ON d.kind = 'message' AND d.ref = m.id WHERE m.chat_id = NEW.id);
    INSERT INTO search_doc (kind, ref) SELECT 'chat', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'chat' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'chat' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'chat' AND ref = NEW.id),
            COALESCE(NEW.title, ''),
            '',
            'chat',
            NEW.id,
            NULL,
            NEW.user_id);
END;

CREATE TRIGGER search_chats_del
AFTER DELETE ON chats
BEGIN
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'chat' AND ref = OLD.id);
    DELETE FROM search_doc WHERE kind = 'chat' AND ref = OLD.id;
END;

-- ==========================================
-- MESSAGES
-- ==========================================

-- Insert into search_doc for existing messages
INSERT INTO search_doc (kind, ref)
SELECT 'message', id FROM messages;

-- Populate search_fts from existing messages (join with chats for title and user_id)
INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
SELECT d.rowid, COALESCE(c.title, ''), COALESCE(m.text, ''), 'message', m.id, m.chat_id, c.user_id
FROM messages m
JOIN chats c ON c.id = m.chat_id
JOIN search_doc d ON d.kind = 'message' AND d.ref = m.id;

-- Trigger: insert only on messages
CREATE TRIGGER search_messages_ins
AFTER INSERT ON messages
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'message', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'message' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'message' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    SELECT (SELECT rowid FROM search_doc WHERE kind = 'message' AND ref = NEW.id),
           COALESCE(c.title, ''),
           COALESCE(NEW.text, ''),
           'message',
           NEW.id,
           NEW.chat_id,
           c.user_id
    FROM chats c
    WHERE c.id = NEW.chat_id;
END;

CREATE TRIGGER search_messages_upd
AFTER UPDATE ON messages
BEGIN
    -- Messages are never updated per schema, but trigger exists for completeness
    INSERT INTO search_doc (kind, ref) SELECT 'message', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'message' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'message' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    SELECT (SELECT rowid FROM search_doc WHERE kind = 'message' AND ref = NEW.id),
           COALESCE(c.title, ''),
           COALESCE(NEW.text, ''),
           'message',
           NEW.id,
           NEW.chat_id,
           c.user_id
    FROM chats c
    WHERE c.id = NEW.chat_id;
END;

CREATE TRIGGER search_messages_del
AFTER DELETE ON messages
BEGIN
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'message' AND ref = OLD.id);
    DELETE FROM search_doc WHERE kind = 'message' AND ref = OLD.id;
END;

-- ==========================================
-- PROJECTS
-- ==========================================

-- Insert into search_doc for existing projects
INSERT INTO search_doc (kind, ref)
SELECT 'project', id FROM projects;

-- Populate search_fts from existing projects
INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
SELECT d.rowid, COALESCE(p.name, ''), COALESCE(p.description, ''), 'project', p.id, NULL, p.user_id
FROM projects p
JOIN search_doc d ON d.kind = 'project' AND d.ref = p.id;

-- Trigger: insert/update on projects
CREATE TRIGGER search_projects_ins
AFTER INSERT ON projects
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'project', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'project' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'project' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'project' AND ref = NEW.id),
            COALESCE(NEW.name, ''),
            COALESCE(NEW.description, ''),
            'project',
            NEW.id,
            NULL,
            NEW.user_id);
END;

CREATE TRIGGER search_projects_upd
AFTER UPDATE OF name, description, user_id ON projects
BEGIN
    INSERT INTO search_doc (kind, ref) SELECT 'project', NEW.id WHERE NOT EXISTS (SELECT 1 FROM search_doc WHERE kind = 'project' AND ref = NEW.id);
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'project' AND ref = NEW.id);
    INSERT INTO search_fts (rowid, title, body, kind, ref, parent, user_id)
    VALUES ((SELECT rowid FROM search_doc WHERE kind = 'project' AND ref = NEW.id),
            COALESCE(NEW.name, ''),
            COALESCE(NEW.description, ''),
            'project',
            NEW.id,
            NULL,
            NEW.user_id);
END;

CREATE TRIGGER search_projects_del
AFTER DELETE ON projects
BEGIN
    DELETE FROM search_fts WHERE rowid = (SELECT rowid FROM search_doc WHERE kind = 'project' AND ref = OLD.id);
    DELETE FROM search_doc WHERE kind = 'project' AND ref = OLD.id;
END;
