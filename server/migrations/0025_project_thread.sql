-- Add thread column and unique partial index for project thread chat
ALTER TABLE chats ADD COLUMN thread INTEGER NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX chats_one_thread ON chats(project_id) WHERE thread = 1;
