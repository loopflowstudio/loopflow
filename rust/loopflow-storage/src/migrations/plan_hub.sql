-- The plan of a repository without Linear or Asana lives on one hub Machine
-- (LOO-938). Three tables, one per side of the exchange.
--
-- `plan_changes`: on every Machine, the newest change of each Wave, Project,
-- Task and comment. `seq` only grows (a change replaces its owner's row), so
-- a hub answers "what changed since N" from one index range. Seeded with every
-- owner, so any Machine can become a hub.
CREATE TABLE plan_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    repo TEXT NOT NULL,
    owner_table TEXT NOT NULL CHECK(owner_table IN ('waves','projects','tasks','task_comments')),
    owner_id TEXT NOT NULL,
    UNIQUE(owner_table,owner_id)
);
CREATE INDEX plan_changes_repo ON plan_changes(repo,seq);

INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT repo,'waves',id FROM waves;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT w.repo,'projects',p.id FROM projects p JOIN waves w ON w.id=p.wave_id;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT r.repo,'tasks',t.id FROM tasks t JOIN repository_plans r ON r.id=t.repository_id;
INSERT INTO plan_changes(repo,owner_table,owner_id)
SELECT r.repo,'task_comments',c.id FROM task_comments c
JOIN tasks t ON t.id=c.task_id JOIN repository_plans r ON r.id=t.repository_id;

-- `hub_plans`: on a Machine whose plan is on a hub, one row per repository it
-- has exchanged with that hub. `seq` is the hub's `plan_changes` watermark;
-- NULL until the first read (first contact). A row is what makes the writer
-- queue `hub_sends`.
CREATE TABLE hub_plans (
    repo TEXT PRIMARY KEY,
    hub_machine_id TEXT NOT NULL,
    seq INTEGER,
    error TEXT,
    failing_since INTEGER
);

-- `hub_sends`: each value saved here that the hub has not confirmed, newest
-- per field. A save replaces the field's row (a new `seq`); a confirmed send
-- deletes the rows it carried.
CREATE TABLE hub_sends (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    repo TEXT NOT NULL,
    owner_table TEXT NOT NULL CHECK(owner_table IN ('waves','projects','tasks','task_comments')),
    target TEXT NOT NULL,
    field TEXT NOT NULL,
    edit_json TEXT NOT NULL CHECK(json_valid(edit_json)),
    sent_at INTEGER,
    UNIQUE(target,field)
);
CREATE INDEX hub_sends_repo ON hub_sends(repo,seq);

CREATE TRIGGER store_revision_hub_sends_insert AFTER INSERT ON hub_sends
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_hub_sends_update AFTER UPDATE ON hub_sends
WHEN NEW.seq IS NOT OLD.seq OR NEW.repo IS NOT OLD.repo OR
    NEW.owner_table IS NOT OLD.owner_table OR NEW.target IS NOT OLD.target OR
    NEW.field IS NOT OLD.field OR NEW.edit_json IS NOT OLD.edit_json OR
    NEW.sent_at IS NOT OLD.sent_at
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_hub_sends_delete AFTER DELETE ON hub_sends
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_hub_plans_insert AFTER INSERT ON hub_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_hub_plans_update AFTER UPDATE ON hub_plans
WHEN NEW.repo IS NOT OLD.repo OR NEW.hub_machine_id IS NOT OLD.hub_machine_id OR
    NEW.seq IS NOT OLD.seq OR NEW.error IS NOT OLD.error OR
    NEW.failing_since IS NOT OLD.failing_since
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_hub_plans_delete AFTER DELETE ON hub_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

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
