-- GPU panel: bars pinned per user, e.g. ["0000:10:00.0/power"].
ALTER TABLE users ADD COLUMN gpu_pins TEXT NOT NULL DEFAULT '[]';
