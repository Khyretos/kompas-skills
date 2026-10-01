-- Chat menu: pin to the top, archive (hidden from the list, kept).
ALTER TABLE chats ADD COLUMN pinned   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
