-- A Graph run and each of its steps record their own lifecycle: when they
-- started and ended, the OS process they run as while live, and how they ended.
-- The spawner mints a step's id and writes its row at spawn; the run writes its
-- end, and the spawner writes it if the run died first. Neither table names a
-- `processes` row any more, so both are rebuilt without those foreign keys or
-- the step-parent check, and what a run launched with stays immutable while
-- its lifecycle columns change. Every column is kept.
PRAGMA legacy_alter_table = ON;

CREATE TABLE graph_processes_owned (
    lf_process_id TEXT PRIMARY KEY,
    graph TEXT NOT NULL,
    compiled TEXT NOT NULL CHECK (json_valid(compiled)),
    task_id TEXT REFERENCES tasks(id),
    started_at INTEGER,
    ended_at INTEGER,
    pid INTEGER,
    os_started_at INTEGER,
    exit_code INTEGER,
    error TEXT,
    repo TEXT,
    cwd TEXT
) STRICT;
INSERT INTO graph_processes_owned(lf_process_id,graph,compiled,task_id,started_at,ended_at,pid,os_started_at,exit_code,error,repo,cwd)
SELECT g.lf_process_id,g.graph,g.compiled,g.task_id,e.started_at,e.completed_at,
    CASE WHEN e.completed_at IS NULL THEN e.pid END,
    CASE WHEN e.completed_at IS NULL THEN e.os_started_at END,
    e.exit_code,e.error,e.repo,e.cwd
FROM graph_processes g LEFT JOIN processes e ON e.id=g.lf_process_id;

CREATE TABLE graph_process_steps_owned (
    seq INTEGER PRIMARY KEY,
    graph_lf_process_id TEXT NOT NULL REFERENCES "graph_processes"(lf_process_id),
    lf_process_id TEXT NOT NULL UNIQUE,
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL CHECK (json_valid(iterations)),
    started_at INTEGER,
    ended_at INTEGER,
    pid INTEGER,
    os_started_at INTEGER,
    exit_code INTEGER,
    error TEXT
) STRICT;
INSERT INTO graph_process_steps_owned(seq,graph_lf_process_id,lf_process_id,node,iterations,started_at,ended_at,pid,os_started_at,exit_code,error)
SELECT s.seq,s.graph_lf_process_id,s.lf_process_id,s.node,s.iterations,e.started_at,e.completed_at,
    CASE WHEN e.completed_at IS NULL THEN e.pid END,
    CASE WHEN e.completed_at IS NULL THEN e.os_started_at END,
    e.exit_code,e.error
FROM graph_process_steps s LEFT JOIN processes e ON e.id=s.lf_process_id;

-- A usage-limit wait belongs to the step it holds, not to a Process row.
CREATE TABLE agent_limit_recoveries_owned (
    lf_process_id TEXT PRIMARY KEY REFERENCES "graph_process_steps"(lf_process_id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    reason TEXT NOT NULL,
    retry_at INTEGER,
    agent TEXT,
    updated_at INTEGER NOT NULL
);
INSERT INTO agent_limit_recoveries_owned(lf_process_id,provider,reason,retry_at,agent,updated_at)
SELECT r.lf_process_id,r.provider,r.reason,r.retry_at,r.agent,r.updated_at FROM agent_limit_recoveries r
WHERE EXISTS(SELECT 1 FROM graph_process_steps s WHERE s.lf_process_id=r.lf_process_id);

DROP TABLE agent_limit_recoveries;
DROP TABLE graph_process_steps;
DROP TABLE graph_processes;
ALTER TABLE graph_processes_owned RENAME TO graph_processes;
ALTER TABLE graph_process_steps_owned RENAME TO graph_process_steps;
ALTER TABLE agent_limit_recoveries_owned RENAME TO agent_limit_recoveries;
PRAGMA legacy_alter_table = OFF;

CREATE TRIGGER store_revision_agent_limit_recoveries_insert AFTER INSERT ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_update AFTER UPDATE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_delete AFTER DELETE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;

-- A Wave's Project activation is on the Wave: the activating lf records when
-- it started and the OS process it runs as, then writes its own end, exit
-- code and error there. An activating lf that died unended reads dead from
-- its pid.
ALTER TABLE waves ADD COLUMN project_activation_started_at INTEGER;
ALTER TABLE waves ADD COLUMN project_activation_pid INTEGER;
ALTER TABLE waves ADD COLUMN project_activation_os_started_at INTEGER;
ALTER TABLE waves ADD COLUMN project_activation_ended_at INTEGER;
ALTER TABLE waves ADD COLUMN project_activation_exit_code INTEGER;
ALTER TABLE waves ADD COLUMN project_activation_error TEXT;
UPDATE waves SET (project_activation_started_at,project_activation_pid,project_activation_os_started_at,
        project_activation_ended_at,project_activation_exit_code,project_activation_error)=(
    SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
        CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,error
    FROM processes WHERE id=waves.project_activation_lf_process_id)
WHERE project_activation_lf_process_id IS NOT NULL;

-- A CI repair's run is on its incident: when it started and the OS process
-- it runs as until the attempt is settled. A ci-fix run that died unsettled
-- reads dead from its pid.
ALTER TABLE ci_incidents ADD COLUMN repair_started_at INTEGER;
ALTER TABLE ci_incidents ADD COLUMN repair_pid INTEGER;
ALTER TABLE ci_incidents ADD COLUMN repair_os_started_at INTEGER;
UPDATE ci_incidents SET (repair_started_at,repair_pid,repair_os_started_at)=(
    SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
        CASE WHEN completed_at IS NULL THEN os_started_at END
    FROM processes WHERE id=ci_incidents.repair_lf_process_id)
WHERE repair_lf_process_id IS NOT NULL AND repair_finished_at IS NULL;
UPDATE ci_incidents SET repair_started_at=(
    SELECT started_at FROM processes WHERE id=ci_incidents.repair_lf_process_id)
WHERE repair_lf_process_id IS NOT NULL AND repair_finished_at IS NOT NULL;

-- A Task's run names the lf carrying it and, while that lf runs, its OS
-- process. The carrier's end clears the OS process and keeps its id: the next
-- `lf task run` sets out from where that run stopped.
ALTER TABLE task_graphs ADD COLUMN run_pid INTEGER;
ALTER TABLE task_graphs ADD COLUMN run_os_started_at INTEGER;
UPDATE task_graphs SET (run_pid,run_os_started_at)=(
    SELECT pid,os_started_at FROM processes WHERE id=task_graphs.lf_process_id AND completed_at IS NULL)
WHERE lf_process_id IS NOT NULL;

-- A ship carrier's run is on the Auto-Ship event naming it: the OS process
-- it runs as, then its end, exit code and error. An event's kind, time, lf
-- and person stay as written; only the carrier's lifecycle changes.
ALTER TABLE task_ship_events ADD COLUMN pid INTEGER;
ALTER TABLE task_ship_events ADD COLUMN os_started_at INTEGER;
ALTER TABLE task_ship_events ADD COLUMN ended_at INTEGER;
ALTER TABLE task_ship_events ADD COLUMN exit_code INTEGER;
ALTER TABLE task_ship_events ADD COLUMN error TEXT;
DROP TRIGGER task_ship_events_append_only;
UPDATE task_ship_events SET (pid,os_started_at,ended_at,exit_code,error)=(
    SELECT CASE WHEN completed_at IS NULL THEN pid END,
        CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,error
    FROM processes WHERE id=task_ship_events.lf_process_id)
WHERE kind IN ('approved','carried') AND lf_process_id IS NOT NULL;
CREATE TRIGGER task_ship_events_launch_is_immutable BEFORE UPDATE ON task_ship_events
WHEN NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.kind IS NOT OLD.kind
    OR NEW.at IS NOT OLD.at OR NEW.lf_process_id IS NOT OLD.lf_process_id
    OR NEW.person IS NOT OLD.person
BEGIN
    SELECT RAISE(ABORT,'Task ship events are append-only');
END;
CREATE TRIGGER store_revision_task_ship_events_update AFTER UPDATE ON task_ship_events
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE INDEX graph_processes_task ON graph_processes(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX graph_process_steps_graph ON graph_process_steps(graph_lf_process_id, seq);

CREATE TRIGGER store_revision_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER task_update_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER store_revision_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER task_update_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;

CREATE TRIGGER graph_processes_launch_is_immutable BEFORE UPDATE ON graph_processes
WHEN NEW.lf_process_id IS NOT OLD.lf_process_id OR NEW.graph IS NOT OLD.graph
    OR NEW.compiled IS NOT OLD.compiled OR NEW.task_id IS NOT OLD.task_id
BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
CREATE TRIGGER graph_process_steps_launch_is_immutable BEFORE UPDATE ON graph_process_steps
WHEN NEW.seq IS NOT OLD.seq OR NEW.graph_lf_process_id IS NOT OLD.graph_lf_process_id
    OR NEW.lf_process_id IS NOT OLD.lf_process_id OR NEW.node IS NOT OLD.node
    OR NEW.iterations IS NOT OLD.iterations
BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;

-- Bridge for a released lf still running after install: it records a Graph
-- run or step without these columns, after its Process row, and its end (and
-- a Project activation's) only on `processes`. These copy both onto the owner row; the contract migration drops them with the
-- table.
CREATE TRIGGER graph_run_start_from_process AFTER INSERT ON graph_processes
WHEN NEW.started_at IS NULL
BEGIN
    UPDATE graph_processes SET (started_at,ended_at,pid,os_started_at,exit_code,error,repo,cwd)=(
        SELECT started_at,completed_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,exit_code,error,repo,cwd
        FROM processes WHERE id=NEW.lf_process_id)
    WHERE lf_process_id=NEW.lf_process_id;
END;
CREATE TRIGGER graph_step_start_from_process AFTER INSERT ON graph_process_steps
WHEN NEW.started_at IS NULL
BEGIN
    UPDATE graph_process_steps SET (started_at,ended_at,pid,os_started_at,exit_code,error)=(
        SELECT started_at,completed_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,exit_code,error
        FROM processes WHERE id=NEW.lf_process_id)
    WHERE lf_process_id=NEW.lf_process_id;
END;
CREATE TRIGGER graph_run_end_from_process_insert AFTER INSERT ON processes
WHEN NEW.completed_at IS NOT NULL
BEGIN
    UPDATE graph_processes SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND ended_at IS NULL;
    UPDATE graph_process_steps SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND ended_at IS NULL;
    UPDATE waves SET project_activation_ended_at=NEW.completed_at,
        project_activation_exit_code=NEW.exit_code,project_activation_error=NEW.error,
        project_activation_pid=NULL,project_activation_os_started_at=NULL
    WHERE project_activation_lf_process_id=NEW.id AND project_activation_ended_at IS NULL;
    UPDATE task_graphs SET run_pid=NULL,run_os_started_at=NULL
    WHERE lf_process_id=NEW.id AND run_pid IS NOT NULL;
    UPDATE task_ship_events SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND kind IN ('approved','carried') AND ended_at IS NULL;
END;
CREATE TRIGGER graph_run_end_from_process_update AFTER UPDATE OF completed_at ON processes
WHEN OLD.completed_at IS NULL AND NEW.completed_at IS NOT NULL
BEGIN
    UPDATE graph_processes SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND ended_at IS NULL;
    UPDATE graph_process_steps SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND ended_at IS NULL;
    UPDATE waves SET project_activation_ended_at=NEW.completed_at,
        project_activation_exit_code=NEW.exit_code,project_activation_error=NEW.error,
        project_activation_pid=NULL,project_activation_os_started_at=NULL
    WHERE project_activation_lf_process_id=NEW.id AND project_activation_ended_at IS NULL;
    UPDATE task_graphs SET run_pid=NULL,run_os_started_at=NULL
    WHERE lf_process_id=NEW.id AND run_pid IS NOT NULL;
    UPDATE task_ship_events SET ended_at=NEW.completed_at,exit_code=NEW.exit_code,error=NEW.error,
        pid=NULL,os_started_at=NULL
    WHERE lf_process_id=NEW.id AND kind IN ('approved','carried') AND ended_at IS NULL;
END;
-- A released lf records an activation by id alone, after that id's Process
-- row; its run's start, OS process and any end come from that row.
CREATE TRIGGER project_activation_start_from_process AFTER UPDATE OF project_activation_lf_process_id ON waves
WHEN NEW.project_activation_lf_process_id IS NOT NULL
    AND NEW.project_activation_lf_process_id IS NOT OLD.project_activation_lf_process_id
    AND NEW.project_activation_started_at IS OLD.project_activation_started_at
    AND NEW.project_activation_pid IS OLD.project_activation_pid
BEGIN
    UPDATE waves SET (project_activation_started_at,project_activation_pid,project_activation_os_started_at,
            project_activation_ended_at,project_activation_exit_code,project_activation_error)=(
        SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,error
        FROM processes WHERE id=NEW.project_activation_lf_process_id)
    WHERE id=NEW.id;
END;
-- A released lf carries a Task's run by id alone, after that id's Process
-- row; its OS process comes from that row.
CREATE TRIGGER task_carrier_start_from_process AFTER UPDATE OF lf_process_id ON task_graphs
WHEN NEW.lf_process_id IS NOT NULL AND NEW.lf_process_id IS NOT OLD.lf_process_id
    AND NEW.run_pid IS NULL
BEGIN
    UPDATE task_graphs SET (run_pid,run_os_started_at)=(
        SELECT pid,os_started_at FROM processes WHERE id=NEW.lf_process_id AND completed_at IS NULL)
    WHERE task_id=NEW.task_id;
END;
-- A released lf approves or carries shipping by id alone, after that id's
-- Process row; its OS process and any end come from that row.
CREATE TRIGGER ship_carrier_start_from_process AFTER INSERT ON task_ship_events
WHEN NEW.kind IN ('approved','carried') AND NEW.lf_process_id IS NOT NULL
    AND NEW.pid IS NULL AND NEW.ended_at IS NULL
BEGIN
    UPDATE task_ship_events SET (pid,os_started_at,ended_at,exit_code,error)=(
        SELECT CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,error
        FROM processes WHERE id=NEW.lf_process_id)
    WHERE id=NEW.id AND EXISTS(SELECT 1 FROM processes WHERE id=NEW.lf_process_id);
END;
-- A released lf reserves or hands off a repair by id alone, after that id's
-- Process row; its run's start and OS process come from that row.
CREATE TRIGGER ci_repair_start_from_process AFTER UPDATE OF repair_lf_process_id ON ci_incidents
WHEN NEW.repair_lf_process_id IS NOT NULL AND NEW.repair_lf_process_id IS NOT OLD.repair_lf_process_id
    AND NEW.repair_started_at IS OLD.repair_started_at AND NEW.repair_pid IS OLD.repair_pid
BEGIN
    UPDATE ci_incidents SET (repair_started_at,repair_pid,repair_os_started_at)=(
        SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END
        FROM processes WHERE id=NEW.repair_lf_process_id)
    WHERE identity=NEW.identity;
END;

-- A Session records its current agent's lifetime: when it was reserved, the
-- OS process it runs as while live, and when and how it exited. lf writes the
-- agent row with its attachment until the attachment moves onto the Session;
-- these copy the agent row's lifetime there, for this lf and a released one.
-- The contract migration drops them with the table.
ALTER TABLE agent_sessions ADD COLUMN agent_started_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN agent_pid INTEGER;
ALTER TABLE agent_sessions ADD COLUMN agent_os_started_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN agent_ended_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN agent_exit_code INTEGER;
UPDATE agent_sessions SET (agent_started_at,agent_pid,agent_os_started_at,agent_ended_at,agent_exit_code)=(
    SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
        CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code
    FROM processes WHERE id=agent_sessions.agent_process_id)
WHERE agent_process_id IS NOT NULL;

CREATE TRIGGER session_agent_from_process_insert AFTER INSERT ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE agent_sessions SET agent_started_at=NEW.started_at,
        agent_pid=CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END,
        agent_os_started_at=CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END,
        agent_ended_at=NEW.completed_at,agent_exit_code=NEW.exit_code
    WHERE agent_process_id=NEW.id;
END;
CREATE TRIGGER session_agent_from_process_update
AFTER UPDATE OF started_at,pid,os_started_at,completed_at,exit_code ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE agent_sessions SET agent_started_at=NEW.started_at,
        agent_pid=CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END,
        agent_os_started_at=CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END,
        agent_ended_at=NEW.completed_at,agent_exit_code=NEW.exit_code
    WHERE agent_process_id=NEW.id;
END;
CREATE TRIGGER session_agent_from_attachment AFTER UPDATE OF agent_process_id ON agent_sessions
WHEN NEW.agent_process_id IS NOT OLD.agent_process_id
BEGIN
    UPDATE agent_sessions SET (agent_started_at,agent_pid,agent_os_started_at,agent_ended_at,agent_exit_code)=(
        SELECT started_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code
        FROM processes WHERE id=NEW.agent_process_id)
    WHERE id=NEW.id;
END;
