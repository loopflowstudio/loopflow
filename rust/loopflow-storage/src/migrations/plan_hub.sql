-- The plan of a repository without Linear or Asana is the same on every
-- Machine that holds it (LOO-938): one stamp per field, the newest wins, and
-- carriers move stamped records between Machines.
--
-- `plan_changes`: the local order in which Waves, Projects, Tasks and comments
-- last changed, by a save here or an arriving value that won. `seq` only grows
-- (a change replaces its record's row), so "what changed since N" is one index
-- range. Seeded with every record, so a first exchange sends the whole plan.
CREATE TABLE plan_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    repo TEXT NOT NULL,
    owner_table TEXT NOT NULL CHECK(owner_table IN ('waves','projects','tasks','task_comments')),
    owner_id TEXT NOT NULL,
    UNIQUE(owner_table,owner_id)
);
CREATE INDEX plan_changes_repo ON plan_changes(repo,seq);

-- A carrier's unsent count is read from here, so views follow it.
CREATE TRIGGER store_revision_plan_changes_insert AFTER INSERT ON plan_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_plan_changes_update AFTER UPDATE ON plan_changes
WHEN NEW.seq IS NOT OLD.seq OR NEW.repo IS NOT OLD.repo OR
    NEW.owner_table IS NOT OLD.owner_table OR NEW.owner_id IS NOT OLD.owner_id
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_plan_changes_delete AFTER DELETE ON plan_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT repo,'waves',id FROM waves;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT w.repo,'projects',p.id FROM projects p JOIN waves w ON w.id=p.wave_id;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT r.repo,'tasks',t.id FROM tasks t JOIN repository_plans r ON r.id=t.repository_id;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT r.repo,'task_comments',c.id FROM task_comments c
JOIN tasks t ON t.id=c.task_id JOIN repository_plans r ON r.id=t.repository_id;

-- `plan_stamps`: when and where each field's value was written, `at` in
-- milliseconds and never behind any stamp this Machine has seen. A field with
-- no row reads as (0, this Machine), so any save beats it.
CREATE TABLE plan_stamps (
    owner_id TEXT NOT NULL,
    field TEXT NOT NULL,
    at INTEGER NOT NULL,
    machine_id TEXT NOT NULL,
    PRIMARY KEY(owner_id,field)
) WITHOUT ROWID;
CREATE INDEX plan_stamps_at ON plan_stamps(at);

-- `plan_carriers`: one row per repository and way its plan travels. `sent_seq`
-- is how far into this Machine's `plan_changes` the carrier has taken, so the
-- records past it are unsent; `seen_seq` is a hub's own watermark read to.
-- `not_taken` counts the records the carrier's last read brought and this
-- Machine could not write, and `not_taken_reason` says why the first was not;
-- both NULL when it took everything.
CREATE TABLE plan_carriers (
    repo TEXT NOT NULL,
    carrier TEXT NOT NULL CHECK(carrier IN ('git','hub')),
    peer TEXT NOT NULL,
    sent_seq INTEGER NOT NULL,
    seen_seq INTEGER,
    error TEXT,
    failing_since INTEGER,
    not_taken INTEGER,
    not_taken_reason TEXT,
    PRIMARY KEY(repo,carrier)
) WITHOUT ROWID;

CREATE TRIGGER store_revision_plan_carriers_insert AFTER INSERT ON plan_carriers
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_plan_carriers_update AFTER UPDATE ON plan_carriers
WHEN NEW.repo IS NOT OLD.repo OR NEW.carrier IS NOT OLD.carrier OR
    NEW.peer IS NOT OLD.peer OR NEW.sent_seq IS NOT OLD.sent_seq OR
    NEW.seen_seq IS NOT OLD.seen_seq OR NEW.error IS NOT OLD.error OR
    NEW.failing_since IS NOT OLD.failing_since OR NEW.not_taken IS NOT OLD.not_taken OR
    NEW.not_taken_reason IS NOT OLD.not_taken_reason
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_plan_carriers_delete AFTER DELETE ON plan_carriers
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- `plan_refs`: the last commit of each other Machine's plan ref
-- (`refs/loopflow/plan/<machine id>`) this store has taken in, so the next
-- fetch reads only the files that changed. Kept per store, not per clone.
CREATE TABLE plan_refs (
    repo TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    tip TEXT NOT NULL,
    PRIMARY KEY(repo,machine_id)
) WITHOUT ROWID;

-- Claims are first-wins: an ownership entry names the entry in effect when it
-- was written (itself when there was none), and a claim whose writer had not
-- seen the claim holding the Task loses to it. NULL on entries written before
-- this, and with Linear or Asana; those never lose.
ALTER TABLE task_ownership ADD COLUMN seen TEXT;
DROP TRIGGER task_ownership_append_only;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label
    AND NEW.writer_machine_id IS OLD.writer_machine_id AND NEW.person_id IS OLD.person_id
    AND NEW.seen IS OLD.seen)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- The Git planning ref, its peer journal and destinations are gone; released
-- lf still writes them until the staging window passes.
-- leaving: git_tracker_changes LOO-938
-- leaving: git_tracker_conflicts LOO-938
-- leaving: git_tracker_destinations LOO-938
-- leaving: git_tracker_imports LOO-938
-- leaving: git_tracker_routes LOO-938
-- leaving: git_tracker_seen LOO-938
-- leaving: planning_destinations LOO-938
-- leaving: planning_peer_changes LOO-938
-- leaving: planning_peer_conflicts LOO-938
-- leaving: planning_peer_imports LOO-938
-- leaving: planning_peer_observed LOO-938
-- leaving: planning_peer_routes LOO-938
-- leaving: planning_user LOO-938
-- A Task hold was a Git journal value; nothing reads the column.
-- leaving: tasks.paused LOO-938
