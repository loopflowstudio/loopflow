-- A Process records the Task it belongs to once, when it is written: the Task
-- its launch names, else the deepest checkout holding its directory, else its
-- caller's, else its parent's. Membership then reads this column instead of
-- matching paths, so a moved repository or a reused checkout path changes no
-- Task's Processes.
ALTER TABLE processes ADD COLUMN task_id TEXT;
CREATE INDEX processes_task ON processes(task_id) WHERE task_id IS NOT NULL;

-- Relocation receipts are the only membership a path cannot recover.
UPDATE processes SET task_id=r.task_id
FROM relocated_task_processes r WHERE r.process_id=processes.id;

-- Existing rows take their Task in the Machine server's maintenance pass, a
-- batch per write transaction: resolving every row here would hold the write
-- lock for tens of seconds at upgrade. Released lf keeps inserting rows without
-- a Task until it exits; the same pass reaches them past this cursor. The
-- table goes once no released lf writes Processes without a Task.
CREATE TABLE process_task_backfill (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    after_rowid INTEGER NOT NULL
);
INSERT INTO process_task_backfill(id, after_rowid) VALUES(1, 0);

-- A Session records its Task the same way, at creation: the Task it names,
-- else the deepest checkout holding its directory. A Task's primary
-- conversation names its Task even when it was chosen by path.
UPDATE agent_sessions SET task_id=chosen.task_id, wave_id=chosen.wave_id
FROM (SELECT tps.session_id, tps.task_id, p.wave_id FROM task_primary_sessions tps
    JOIN tasks t ON t.id=tps.task_id LEFT JOIN projects p ON p.id=t.project_id) chosen
WHERE chosen.session_id=agent_sessions.id AND agent_sessions.task_id IS NULL
    AND (agent_sessions.wave_id IS NULL OR agent_sessions.wave_id IS chosen.wave_id);

-- Every other Session that named nothing takes the deepest checkout holding
-- its directory, as membership read it by path until now. Each ancestor of
-- the directory is one index seek.
UPDATE agent_sessions SET task_id=inside.task_id, wave_id=p.wave_id,
    work_source=coalesce(agent_sessions.work_source,'checkout')
FROM (SELECT s.id, (WITH RECURSIVE up(path) AS (
            SELECT rtrim(s.cwd,'/')
            UNION ALL
            SELECT rtrim(rtrim(path,replace(path,'/','')),'/') FROM up WHERE path!='')
         SELECT c.task_id FROM up CROSS JOIN task_checkouts c INDEXED BY task_checkouts_root
            ON rtrim(c.worktree,'/')=up.path
         WHERE up.path!='' OR substr(s.cwd,1,1)='/' OR s.cwd=''
         ORDER BY length(up.path) DESC, c.task_id LIMIT 1) AS task_id
    FROM agent_sessions s
    WHERE s.task_id IS NULL AND s.wave_id IS NULL AND s.primary_scope IS NULL) inside
JOIN tasks t ON t.id=inside.task_id LEFT JOIN projects p ON p.id=t.project_id
WHERE inside.id=agent_sessions.id;

-- A Task's Auto-Ship carrier is a `task_ship_events` row since
-- task_machine_local copied it there. The leaving column's reference would
-- otherwise keep the carrier's Process from ever being forgotten.
UPDATE tasks SET ship_lf_process_id=NULL WHERE ship_lf_process_id IS NOT NULL;

-- leaving: relocated_task_processes LOO-877

-- A Task is completed when its state says so. Released lf still reads the old
-- boolean (and the retirement triggers fire on it), so it follows the state
-- until the contract drops both.
UPDATE tasks SET planning_state='completed'
WHERE planning_completed=1 AND planning_state IS NULL;
CREATE TRIGGER tasks_completed_follows_state_insert AFTER INSERT ON tasks
WHEN NEW.planning_completed IS NOT (NEW.planning_state IS 'completed')
BEGIN UPDATE tasks SET planning_completed=(NEW.planning_state IS 'completed') WHERE id=NEW.id; END;
CREATE TRIGGER tasks_completed_follows_state_update AFTER UPDATE OF planning_state,planning_completed ON tasks
WHEN NEW.planning_completed IS NOT (NEW.planning_state IS 'completed')
BEGIN UPDATE tasks SET planning_completed=(NEW.planning_state IS 'completed') WHERE id=NEW.id; END;

-- The provider's revision for the last accepted observation is the
-- observation's own: `tracker_items.revision`, `tracker_projects.revision`.
-- leaving: tasks.planning_completed LOO-877
-- leaving: tasks.planning_provider_revision LOO-877
-- leaving: projects.planning_provider_revision LOO-877

-- One order: a Task's rank. The Working board follows Project rank, then Task
-- rank, and a tile Move edits rank, so its own saved order goes. The tracker's
-- order of a Project is its observed items by rank.
-- leaving: task_orders LOO-877
-- leaving: tracker_projects.task_order_json LOO-877
-- leaving: pm_projects.task_order_json LOO-877
