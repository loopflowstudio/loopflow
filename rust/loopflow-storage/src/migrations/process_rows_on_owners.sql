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
-- OS process it runs as while live, and when and how it exited.
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

-- A Session holds its current agent's attachment and reach as well: the lf
-- attached to it and that lf's OS process and terminal, the token fencing its
-- writes, the lf that launched the agent, how far its spawn got and with what
-- command, and the Codex endpoint or Claude inbox that reaches it. A dead
-- holder reads dead from its own pid, without an lf row.
ALTER TABLE agent_sessions ADD COLUMN agent_launcher_lf_process_id TEXT;
ALTER TABLE agent_sessions ADD COLUMN agent_spawn_state TEXT
    CHECK(agent_spawn_state IN ('reserved','spawn_requested','spawn_failed','exited'));
ALTER TABLE agent_sessions ADD COLUMN agent_command TEXT;
ALTER TABLE agent_sessions ADD COLUMN agent_interactive INTEGER CHECK(agent_interactive IN (0,1));
ALTER TABLE agent_sessions ADD COLUMN agent_endpoint TEXT;
ALTER TABLE agent_sessions ADD COLUMN agent_inbox TEXT;
ALTER TABLE agent_sessions ADD COLUMN attachment_token TEXT;
ALTER TABLE agent_sessions ADD COLUMN attached_lf_process_id TEXT;
ALTER TABLE agent_sessions ADD COLUMN attached_pid INTEGER;
ALTER TABLE agent_sessions ADD COLUMN attached_os_started_at INTEGER;
ALTER TABLE agent_sessions ADD COLUMN attached_terminal_id TEXT;
UPDATE agent_sessions SET (agent_launcher_lf_process_id,agent_spawn_state,agent_command,
    agent_interactive,agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id)=(
    SELECT parent_lf_process_id,spawn_state,command,agent_interactive,endpoint,inbox,
        attachment_token,attached_lf_process_id
    FROM processes WHERE id=agent_sessions.agent_process_id)
WHERE agent_process_id IS NOT NULL;
UPDATE agent_sessions SET (attached_pid,attached_os_started_at,attached_terminal_id)=(
    SELECT pid,os_started_at,terminal_id FROM processes WHERE id=agent_sessions.attached_lf_process_id)
WHERE attached_lf_process_id IS NOT NULL;
CREATE INDEX agent_sessions_attached ON agent_sessions(attached_lf_process_id)
    WHERE attached_lf_process_id IS NOT NULL;
CREATE INDEX agent_sessions_unended_agent ON agent_sessions(agent_process_id)
    WHERE agent_process_id IS NOT NULL AND agent_ended_at IS NULL;

-- Bridge, old → new: a released lf writes the agent row and its attachment
-- on `processes`; these copy them onto the Session. It reserves an agent's
-- row first, then points the Session at it alone, so a Session whose agent
-- changes while its token stays copies that row. This lf changes both in one
-- write. Each copy writes only when a value differs. The contract migration
-- drops both halves with the table.
CREATE TRIGGER session_agent_from_process_insert AFTER INSERT ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE agent_sessions SET agent_launcher_lf_process_id=NEW.parent_lf_process_id,
        agent_started_at=NEW.started_at,
        agent_pid=CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END,
        agent_os_started_at=CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END,
        agent_ended_at=NEW.completed_at,agent_exit_code=NEW.exit_code,
        agent_spawn_state=NEW.spawn_state,agent_command=NEW.command,
        agent_interactive=NEW.agent_interactive,agent_endpoint=NEW.endpoint,agent_inbox=NEW.inbox,
        attachment_token=NEW.attachment_token,attached_lf_process_id=NEW.attached_lf_process_id,
        attached_pid=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id THEN attached_pid
            ELSE (SELECT pid FROM processes WHERE id=NEW.attached_lf_process_id) END,
        attached_os_started_at=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id
            THEN attached_os_started_at
            ELSE (SELECT os_started_at FROM processes WHERE id=NEW.attached_lf_process_id) END,
        attached_terminal_id=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id
            THEN attached_terminal_id
            ELSE (SELECT terminal_id FROM processes WHERE id=NEW.attached_lf_process_id) END
    WHERE agent_process_id=NEW.id AND (agent_launcher_lf_process_id IS NOT NEW.parent_lf_process_id
        OR agent_started_at IS NOT NEW.started_at
        OR agent_pid IS NOT CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END
        OR agent_os_started_at IS NOT CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END
        OR agent_ended_at IS NOT NEW.completed_at OR agent_exit_code IS NOT NEW.exit_code
        OR agent_spawn_state IS NOT NEW.spawn_state OR agent_command IS NOT NEW.command
        OR agent_interactive IS NOT NEW.agent_interactive OR agent_endpoint IS NOT NEW.endpoint OR agent_inbox IS NOT NEW.inbox
        OR attachment_token IS NOT NEW.attachment_token
        OR attached_lf_process_id IS NOT NEW.attached_lf_process_id);
END;
CREATE TRIGGER session_agent_from_process_update
AFTER UPDATE OF parent_lf_process_id,started_at,pid,os_started_at,completed_at,exit_code,
    spawn_state,command,agent_interactive,endpoint,inbox,attachment_token,attached_lf_process_id
    ON processes
WHEN NEW.kind='agent'
BEGIN
    UPDATE agent_sessions SET agent_launcher_lf_process_id=NEW.parent_lf_process_id,
        agent_started_at=NEW.started_at,
        agent_pid=CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END,
        agent_os_started_at=CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END,
        agent_ended_at=NEW.completed_at,agent_exit_code=NEW.exit_code,
        agent_spawn_state=NEW.spawn_state,agent_command=NEW.command,
        agent_interactive=NEW.agent_interactive,agent_endpoint=NEW.endpoint,agent_inbox=NEW.inbox,
        attachment_token=NEW.attachment_token,attached_lf_process_id=NEW.attached_lf_process_id,
        attached_pid=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id THEN attached_pid
            ELSE (SELECT pid FROM processes WHERE id=NEW.attached_lf_process_id) END,
        attached_os_started_at=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id
            THEN attached_os_started_at
            ELSE (SELECT os_started_at FROM processes WHERE id=NEW.attached_lf_process_id) END,
        attached_terminal_id=CASE WHEN attached_lf_process_id IS NEW.attached_lf_process_id
            THEN attached_terminal_id
            ELSE (SELECT terminal_id FROM processes WHERE id=NEW.attached_lf_process_id) END
    WHERE agent_process_id=NEW.id AND (agent_launcher_lf_process_id IS NOT NEW.parent_lf_process_id
        OR agent_started_at IS NOT NEW.started_at
        OR agent_pid IS NOT CASE WHEN NEW.completed_at IS NULL THEN NEW.pid END
        OR agent_os_started_at IS NOT CASE WHEN NEW.completed_at IS NULL THEN NEW.os_started_at END
        OR agent_ended_at IS NOT NEW.completed_at OR agent_exit_code IS NOT NEW.exit_code
        OR agent_spawn_state IS NOT NEW.spawn_state OR agent_command IS NOT NEW.command
        OR agent_interactive IS NOT NEW.agent_interactive OR agent_endpoint IS NOT NEW.endpoint OR agent_inbox IS NOT NEW.inbox
        OR attachment_token IS NOT NEW.attachment_token
        OR attached_lf_process_id IS NOT NEW.attached_lf_process_id);
END;
CREATE TRIGGER session_agent_from_attachment AFTER UPDATE OF agent_process_id ON agent_sessions
WHEN NEW.agent_process_id IS NOT OLD.agent_process_id AND NEW.attachment_token IS OLD.attachment_token
BEGIN
    UPDATE agent_sessions SET (agent_launcher_lf_process_id,agent_started_at,agent_pid,
        agent_os_started_at,agent_ended_at,agent_exit_code,agent_spawn_state,agent_command,
        agent_interactive,agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id)=(
        SELECT parent_lf_process_id,started_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,
            spawn_state,command,agent_interactive,endpoint,inbox,attachment_token,
            attached_lf_process_id
        FROM processes WHERE id=NEW.agent_process_id)
    WHERE id=NEW.id;
    UPDATE agent_sessions SET (attached_pid,attached_os_started_at,attached_terminal_id)=(
        SELECT pid,os_started_at,terminal_id FROM processes WHERE id=agent_sessions.attached_lf_process_id)
    WHERE id=NEW.id;
END;
CREATE TRIGGER session_attached_terminal_from_process AFTER UPDATE OF terminal_id ON processes
WHEN NEW.kind='lf' AND NEW.terminal_id IS NOT OLD.terminal_id
BEGIN
    UPDATE agent_sessions SET attached_terminal_id=NEW.terminal_id
    WHERE attached_lf_process_id=NEW.id AND attached_terminal_id IS NOT NEW.terminal_id;
END;

-- Bridge, new → old: this lf writes the agent and its attachment on the
-- Session alone; these write the agent row a released reader joins. A new
-- agent and its token arrive in one write, which inserts its row. Each write
-- happens only when a value differs, so the two halves cannot ping-pong.
CREATE TRIGGER process_agent_from_session_insert AFTER UPDATE OF agent_process_id ON agent_sessions
WHEN NEW.agent_process_id IS NOT NULL AND NEW.agent_process_id IS NOT OLD.agent_process_id
    AND NEW.attachment_token IS NOT OLD.attachment_token
BEGIN
    INSERT INTO processes(id,kind,trace_id,parent_lf_process_id,command,repo,cwd,started_at,
        completed_at,exit_code,pid,os_started_at,agent_session_id,agent_provider,agent_interactive,
        spawn_state,attached_lf_process_id,attachment_token,endpoint,inbox)
    SELECT NEW.agent_process_id,'agent',
        COALESCE((SELECT trace_id FROM processes WHERE id=NEW.agent_launcher_lf_process_id),
            NEW.agent_process_id),
        NEW.agent_launcher_lf_process_id,NEW.agent_command,NEW.repo,NEW.cwd,
        COALESCE(NEW.agent_started_at,CAST(strftime('%s','now') AS INTEGER)),NEW.agent_ended_at,
        NEW.agent_exit_code,NEW.agent_pid,NEW.agent_os_started_at,NEW.id,NEW.provider,
        NEW.agent_interactive,NEW.agent_spawn_state,NEW.attached_lf_process_id,NEW.attachment_token,
        NEW.agent_endpoint,NEW.agent_inbox
    WHERE NOT EXISTS(SELECT 1 FROM processes WHERE id=NEW.agent_process_id);
END;
CREATE TRIGGER process_agent_from_session_update
AFTER UPDATE OF agent_launcher_lf_process_id,agent_started_at,agent_pid,agent_os_started_at,
    agent_ended_at,agent_exit_code,agent_spawn_state,agent_command,agent_interactive,
    agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id ON agent_sessions
WHEN NEW.agent_process_id IS NOT NULL AND NEW.agent_process_id IS OLD.agent_process_id
BEGIN
    UPDATE processes SET parent_lf_process_id=NEW.agent_launcher_lf_process_id,
        started_at=COALESCE(NEW.agent_started_at,started_at),
        pid=CASE WHEN NEW.agent_ended_at IS NULL THEN NEW.agent_pid ELSE pid END,
        os_started_at=CASE WHEN NEW.agent_ended_at IS NULL THEN NEW.agent_os_started_at
            ELSE os_started_at END,
        completed_at=NEW.agent_ended_at,exit_code=NEW.agent_exit_code,
        spawn_state=NEW.agent_spawn_state,command=NEW.agent_command,
        agent_interactive=NEW.agent_interactive,endpoint=NEW.agent_endpoint,
        inbox=NEW.agent_inbox,attachment_token=NEW.attachment_token,
        attached_lf_process_id=NEW.attached_lf_process_id
    WHERE id=NEW.agent_process_id AND (parent_lf_process_id IS NOT NEW.agent_launcher_lf_process_id
        OR started_at IS NOT COALESCE(NEW.agent_started_at,started_at)
        OR (NEW.agent_ended_at IS NULL AND (pid IS NOT NEW.agent_pid
            OR os_started_at IS NOT NEW.agent_os_started_at))
        OR completed_at IS NOT NEW.agent_ended_at OR exit_code IS NOT NEW.agent_exit_code
        OR spawn_state IS NOT NEW.agent_spawn_state OR command IS NOT NEW.agent_command
        OR agent_interactive IS NOT NEW.agent_interactive OR endpoint IS NOT NEW.agent_endpoint OR inbox IS NOT NEW.agent_inbox
        OR attachment_token IS NOT NEW.attachment_token
        OR attached_lf_process_id IS NOT NEW.attached_lf_process_id);
END;
CREATE TRIGGER process_terminal_from_session AFTER UPDATE OF attached_terminal_id ON agent_sessions
WHEN NEW.attached_lf_process_id IS NOT NULL AND NEW.attached_terminal_id IS NOT OLD.attached_terminal_id
BEGIN
    UPDATE processes SET terminal_id=NEW.attached_terminal_id
    WHERE id=NEW.attached_lf_process_id AND kind='lf' AND terminal_id IS NOT NEW.attached_terminal_id;
END;
