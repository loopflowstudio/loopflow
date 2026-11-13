-- Each ownership entry names the Machine that wrote it, so every Machine
-- stores the same row. Entries this Machine wrote name it; entries released
-- lf took in from planning name no writer.
-- `written_here` stays filled for released lf until the contract drops it.
ALTER TABLE task_ownership ADD COLUMN writer_machine_id TEXT;
DROP TRIGGER task_ownership_append_only;
UPDATE task_ownership SET writer_machine_id=(SELECT id FROM machines WHERE is_local=1)
WHERE written_here=1;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label
    AND NEW.writer_machine_id IS OLD.writer_machine_id)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- Machine-local: each move of a Task this Machine sent or received. A sent
-- move keeps what it sent until it arrives, so running the same move again
-- delivers it without stopping, committing or pushing again.
CREATE TABLE task_moves (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_machine_id TEXT NOT NULL,
    to_machine_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('sent', 'arrived')),
    -- When the move entered its state.
    at INTEGER NOT NULL,
    -- The move as sent, while it has not arrived.
    payload TEXT CHECK (payload IS NULL OR json_valid(payload)),
    CHECK ((state = 'sent') = (payload IS NOT NULL))
);
CREATE INDEX task_moves_task ON task_moves(task_id, id);

CREATE TRIGGER store_revision_task_moves_insert AFTER INSERT ON task_moves
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_moves_update AFTER UPDATE ON task_moves
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_moves_delete AFTER DELETE ON task_moves
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
