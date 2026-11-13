-- Each ownership entry names the Machine that wrote it, so every Machine
-- stores the same row. Entries this Machine wrote name it; entries released
-- lf took in from planning name no writer.
-- `written_here` stays filled for released lf until the contract drops it.
ALTER TABLE task_ownership ADD COLUMN writer_machine_id TEXT;
-- The person, as a `people` id, where this Machine knew one when it wrote.
ALTER TABLE task_ownership ADD COLUMN person_id TEXT;
DROP TRIGGER task_ownership_append_only;
UPDATE task_ownership SET writer_machine_id=(SELECT id FROM machines WHERE is_local=1)
WHERE written_here=1;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label
    AND NEW.writer_machine_id IS OLD.writer_machine_id AND NEW.person_id IS OLD.person_id)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- Machine-local: each move of a Task this Machine sent or received. A move
-- is `leaving` from the claim naming its target until it is sent: it keeps
-- that claim and, once its Graph stopped, the step it stopped, so running the
-- same move again carries on leaving. A sent move keeps what it sent until it
-- arrives, so running it again delivers it without stopping, committing or
-- pushing again.
CREATE TABLE task_moves (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_machine_id TEXT NOT NULL,
    to_machine_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('leaving', 'sent', 'arrived')),
    -- When the move began, while leaving or sent; when it arrived, once arrived.
    at INTEGER NOT NULL,
    -- The ownership entry claiming the Task for the target, while leaving.
    claim_id TEXT,
    -- The Graph the move stopped, while leaving.
    stopped TEXT CHECK (stopped IS NULL OR json_valid(stopped)),
    -- The move as sent, while it has not arrived.
    payload TEXT CHECK (payload IS NULL OR json_valid(payload)),
    CHECK ((state = 'leaving') = (claim_id IS NOT NULL)),
    CHECK (state = 'leaving' OR stopped IS NULL),
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
-- The retirement sweep finds abandoned Tasks that still have a checkout here.
CREATE INDEX tasks_abandoned ON tasks(id) WHERE abandoned_at IS NOT NULL;

-- Machine-local: the conversation this Machine opens for a Task. A Task with
-- no checkout here can still have one (it runs in the repository), so it is
-- not a checkout column. `tasks.primary_session_id` stays for released lf
-- until the contract drops it.
CREATE TABLE task_primary_sessions (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL UNIQUE REFERENCES agent_sessions(id) ON DELETE CASCADE
);
INSERT OR IGNORE INTO task_primary_sessions(task_id,session_id)
SELECT t.id,t.primary_session_id FROM tasks t
JOIN agent_sessions s ON s.id=t.primary_session_id;
CREATE TRIGGER store_revision_task_primary_sessions_insert AFTER INSERT ON task_primary_sessions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_primary_sessions_update AFTER UPDATE ON task_primary_sessions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_primary_sessions_delete AFTER DELETE ON task_primary_sessions
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
-- A Task's conversation resumes wherever its Task runs (the ownership log),
-- so it no longer names the Machine it last moved to.
UPDATE agent_sessions SET moved_to_machine_id=NULL
WHERE moved_to_machine_id IS NOT NULL
  AND (task_id IS NOT NULL OR id IN (SELECT session_id FROM task_primary_sessions));

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

-- Machine-local: a Task's Auto-Ship log, in the order it happened. `approved`
-- hands the Task off (its Process carries it), `carried` names a later
-- carrier, `restarted` counts a restart after a failed carrier and
-- `progressed` resets that count; `stopped` is a person's stop, which stands
-- until the next approval, and `withdrawn` ends an approval without one.
-- `tasks.shipped_at` stays the planning fact peers share. The Processes may be
-- pruned, so no key. The six `tasks.ship_*` columns stay for released lf
-- until the contract drops them.
CREATE TABLE task_ship_events (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN
        ('approved','carried','restarted','progressed','stopped','withdrawn')),
    at INTEGER NOT NULL,
    lf_process_id TEXT,
    -- The person named at a stop no Process made.
    person TEXT
);
CREATE INDEX task_ship_events_task ON task_ship_events(task_id, id);
CREATE INDEX task_ship_events_process ON task_ship_events(lf_process_id)
WHERE lf_process_id IS NOT NULL;
CREATE TRIGGER task_ship_events_append_only BEFORE UPDATE ON task_ship_events
BEGIN SELECT RAISE(ABORT, 'Task ship events are append-only'); END;
CREATE TRIGGER store_revision_task_ship_events_insert AFTER INSERT ON task_ship_events
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_ship_events_delete AFTER DELETE ON task_ship_events
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
-- A stop that stood beside an approval came first: the approval lifted it.
INSERT INTO task_ship_events(task_id,kind,at,lf_process_id,person)
SELECT id,'stopped',ship_stopped_at,ship_stopped_lf_process_id,ship_stopped_person
FROM tasks WHERE ship_stopped_at IS NOT NULL;
INSERT INTO task_ship_events(task_id,kind,at,lf_process_id)
SELECT id,'approved',shipped_at,ship_approved_lf_process_id FROM tasks
WHERE shipped_at IS NOT NULL;
INSERT INTO task_ship_events(task_id,kind,at,lf_process_id)
SELECT id,'carried',shipped_at,ship_lf_process_id FROM tasks
WHERE shipped_at IS NOT NULL AND ship_lf_process_id IS NOT NULL
    AND ship_lf_process_id IS NOT ship_approved_lf_process_id;
WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<64)
INSERT INTO task_ship_events(task_id,kind,at)
SELECT t.id,'restarted',t.shipped_at FROM tasks t JOIN n ON n.i<=t.ship_restarts
WHERE t.shipped_at IS NOT NULL;

-- An owned machine's pool, the cost committed when it was added and when it
-- stops taking new work. A rented machine has no row. `machines.pool`,
-- `cost_usd` and `expires_at` stay for released lf until the contract drops them.
CREATE TABLE pool_machines (
    machine_id TEXT PRIMARY KEY REFERENCES machines(id) ON DELETE CASCADE,
    pool TEXT REFERENCES pools(name),
    cost_usd TEXT,
    expires_at INTEGER
);
INSERT INTO pool_machines(machine_id,pool,cost_usd,expires_at)
SELECT id,pool,cost_usd,expires_at FROM machines
WHERE pool IS NOT NULL OR cost_usd IS NOT NULL OR expires_at IS NOT NULL;
CREATE INDEX pool_machines_pool ON pool_machines(pool);

-- One row per person, keyed by the planning provider's identity for them
-- (`linear:<user id>`, or `journal:<planning user key>` under the Git journal),
-- with the name to show. Ownership entries refer to it by `person_id`; a
-- Machine's `person_id` is who it belongs to.
CREATE TABLE people (
    id TEXT PRIMARY KEY CHECK (id GLOB 'linear:?*' OR id GLOB 'journal:?*'),
    name TEXT
);
ALTER TABLE machines ADD COLUMN person_id TEXT REFERENCES people(id);
INSERT INTO people(id) SELECT 'journal:' || user_key FROM planning_user;
UPDATE machines SET person_id=(SELECT id FROM people) WHERE is_local=1
    AND (SELECT count(*) FROM people)=1;
CREATE TRIGGER store_revision_people_insert AFTER INSERT ON people
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_people_update AFTER UPDATE ON people
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_people_delete AFTER DELETE ON people
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
