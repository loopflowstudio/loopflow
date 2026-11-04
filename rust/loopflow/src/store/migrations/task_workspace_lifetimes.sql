-- Desktop shell receipts are identities, not a second process ledger.
CREATE TABLE task_shells (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    checkout TEXT NOT NULL,
    owner TEXT REFERENCES processes(id),
    created_at INTEGER NOT NULL,
    jobs_settled INTEGER NOT NULL DEFAULT 0,
    close_requested INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX task_shells_task ON task_shells(task_id);
CREATE UNIQUE INDEX task_shells_owner ON task_shells(owner);
CREATE TRIGGER store_revision_task_shells_insert AFTER INSERT ON task_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_shells_update AFTER UPDATE ON task_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_shells_delete AFTER DELETE ON task_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;

-- Native hosts retain Work without storing their layouts here. A stale lease
-- is unknown observation, never evidence that a window was closed.
CREATE TABLE task_host_occurrences (
    task_id TEXT NOT NULL REFERENCES tasks(id),
    occurrence TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('retained','away','closed','unknown')),
    lease_until INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    PRIMARY KEY(task_id,occurrence)
);
CREATE TABLE task_retirements (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id),
    completion TEXT NOT NULL,
    checkout TEXT NOT NULL,
    branch TEXT NOT NULL,
    due_at INTEGER,
    phase TEXT NOT NULL CHECK(phase IN ('waiting','closing','retired')),
    reason TEXT
);
CREATE TRIGGER store_revision_task_host_occurrences_insert AFTER INSERT ON task_host_occurrences BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_host_occurrences_update AFTER UPDATE ON task_host_occurrences BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_host_occurrences_delete AFTER DELETE ON task_host_occurrences BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_insert AFTER INSERT ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_update AFTER UPDATE ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_delete AFTER DELETE ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
-- Reopen, replacement checkout, and a different completion revoke queued work.
CREATE TRIGGER cancel_task_retirement AFTER UPDATE OF planning_completed,planning_completed_at,worktree,branch ON tasks
WHEN NEW.planning_completed=0 OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN
    DELETE FROM task_retirements WHERE task_id=NEW.id;
END;
