-- RUN-01: what a task run protects, for its report
ALTER TABLE runs ADD COLUMN protected TEXT NOT NULL DEFAULT '[]';
ALTER TABLE runs ADD COLUMN tests_may_change INTEGER NOT NULL DEFAULT 0;
