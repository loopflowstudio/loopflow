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

-- Machine-local: the checkout this Machine has for a Task. A row exists only
-- on the Machine that has the checkout, so no path from another Machine is
-- stored here. Planning never carried a path, so a checkout that names no
-- Machine was written here. `tasks.worktree`, `branch`, `base_commit`, `parent_pr_id` and
-- `checkout_machine_id` stay for released lf until the contract drops them.
CREATE TABLE task_checkouts (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    worktree TEXT NOT NULL CHECK (worktree != ''),
    branch TEXT NOT NULL DEFAULT '',
    base_commit TEXT NOT NULL DEFAULT '',
    parent_pr_id TEXT REFERENCES task_prs(id)
);
INSERT INTO task_checkouts(task_id,worktree,branch,base_commit,parent_pr_id)
SELECT id,worktree,branch,base_commit,parent_pr_id FROM tasks
WHERE worktree IS NOT NULL AND worktree != ''
    AND (checkout_machine_id IS NULL
        OR checkout_machine_id=(SELECT id FROM machines WHERE is_local=1));
CREATE UNIQUE INDEX task_checkouts_worktree ON task_checkouts(worktree);
CREATE INDEX task_checkouts_root ON task_checkouts(rtrim(worktree,'/'));
CREATE INDEX task_checkouts_branch ON task_checkouts(branch);

-- The current PR follows the checkout's branch, base and parent.
CREATE TRIGGER task_checkout_to_pr_insert AFTER INSERT ON task_checkouts
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.task_id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;
CREATE TRIGGER task_checkout_to_pr_update AFTER UPDATE OF branch,base_commit,parent_pr_id ON task_checkouts
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.task_id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;

CREATE TRIGGER store_revision_task_checkouts_insert AFTER INSERT ON task_checkouts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain IN ('planning','task_updates'); END;
CREATE TRIGGER store_revision_task_checkouts_update AFTER UPDATE ON task_checkouts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain IN ('planning','task_updates'); END;
CREATE TRIGGER store_revision_task_checkouts_delete AFTER DELETE ON task_checkouts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain IN ('planning','task_updates'); END;

-- A current PR matches this Machine's checkout; with no checkout here it
-- carries no placement.
DROP TRIGGER IF EXISTS task_pr_placement_insert;
DROP TRIGGER IF EXISTS task_pr_placement_update;
CREATE TRIGGER task_pr_placement_insert BEFORE INSERT ON task_prs WHEN NEW.historical=0
AND NOT EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=NEW.task_id AND c.branch=NEW.branch
    AND c.base_commit=NEW.base_commit AND c.parent_pr_id IS NEW.parent_pr_id)
AND (EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=NEW.task_id)
    OR NEW.branch!='' OR NEW.base_commit!='' OR NEW.parent_pr_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;
CREATE TRIGGER task_pr_placement_update BEFORE UPDATE OF branch,base_commit,parent_pr_id ON task_prs
WHEN NEW.historical=0
AND NOT EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=NEW.task_id AND c.branch=NEW.branch
    AND c.base_commit=NEW.base_commit AND c.parent_pr_id IS NEW.parent_pr_id)
AND (EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=NEW.task_id)
    OR NEW.branch!='' OR NEW.base_commit!='' OR NEW.parent_pr_id IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;

-- A replaced or removed checkout revokes queued retirement, as reopening does
-- through `cancel_task_retirement`.
CREATE TRIGGER cancel_task_retirement_checkout_update
AFTER UPDATE OF worktree,branch ON task_checkouts
WHEN NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN DELETE FROM task_retirements WHERE task_id=NEW.task_id; END;
CREATE TRIGGER cancel_task_retirement_checkout_delete AFTER DELETE ON task_checkouts
BEGIN DELETE FROM task_retirements WHERE task_id=OLD.task_id; END;
