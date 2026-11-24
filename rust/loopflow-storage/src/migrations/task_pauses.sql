CREATE TABLE task_pauses (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    paused_at INTEGER NOT NULL,
    paused_by TEXT NOT NULL CHECK (json_valid(paused_by)),
    stopping_sessions TEXT CHECK (stopping_sessions IS NULL OR json_valid(stopping_sessions))
) STRICT;

CREATE TRIGGER store_revision_task_pauses_insert AFTER INSERT ON task_pauses
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_pauses_update AFTER UPDATE ON task_pauses
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_pauses_delete AFTER DELETE ON task_pauses
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
