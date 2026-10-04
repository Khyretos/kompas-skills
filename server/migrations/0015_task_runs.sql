-- W2: a task runs on a computer, in a folder, checked by a command, in its own chat.
ALTER TABLE tasks ADD COLUMN machine_id TEXT;
ALTER TABLE tasks ADD COLUMN folder TEXT;
ALTER TABLE tasks ADD COLUMN check_cmd TEXT;
ALTER TABLE tasks ADD COLUMN chat_id TEXT;
