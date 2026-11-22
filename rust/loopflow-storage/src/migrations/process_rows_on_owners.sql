-- A Graph run and each of its steps record their own lifecycle: when they
-- started and ended, the OS process they run as while live, and how they ended.
-- A run also names the held terminal it runs in, which it holds open, and the
-- Session whose agent started it.
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
    cwd TEXT,
    held_terminal_id TEXT,
    caller_session_id TEXT
) STRICT;
INSERT INTO graph_processes_owned(lf_process_id,graph,compiled,task_id,started_at,ended_at,pid,os_started_at,exit_code,error,repo,cwd,held_terminal_id,caller_session_id)
SELECT g.lf_process_id,g.graph,g.compiled,g.task_id,e.started_at,e.completed_at,
    CASE WHEN e.completed_at IS NULL THEN e.pid END,
    CASE WHEN e.completed_at IS NULL THEN e.os_started_at END,
    e.exit_code,e.error,e.repo,e.cwd,e.held_terminal_id,
    coalesce(e.caller_session_id,(SELECT parent.caller_session_id FROM processes parent WHERE parent.id=e.parent_lf_process_id))
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
-- The Session whose agent approved, carried or stopped it, when one did.
ALTER TABLE task_ship_events ADD COLUMN caller_session_id TEXT;
DROP TRIGGER task_ship_events_append_only;
UPDATE task_ship_events SET caller_session_id=(
    SELECT coalesce(e.caller_session_id,parent.caller_session_id) FROM processes e
    LEFT JOIN processes parent ON parent.id=e.parent_lf_process_id
    WHERE e.id=task_ship_events.lf_process_id)
WHERE lf_process_id IS NOT NULL;
UPDATE task_ship_events SET (pid,os_started_at,ended_at,exit_code,error)=(
    SELECT CASE WHEN completed_at IS NULL THEN pid END,
        CASE WHEN completed_at IS NULL THEN os_started_at END,completed_at,exit_code,error
    FROM processes WHERE id=task_ship_events.lf_process_id)
WHERE kind IN ('approved','carried') AND lf_process_id IS NOT NULL;
CREATE TRIGGER task_ship_events_launch_is_immutable BEFORE UPDATE ON task_ship_events
WHEN NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.kind IS NOT OLD.kind
    OR NEW.at IS NOT OLD.at OR NEW.lf_process_id IS NOT OLD.lf_process_id
    OR NEW.person IS NOT OLD.person OR NEW.caller_session_id IS NOT OLD.caller_session_id
BEGIN
    SELECT RAISE(ABORT,'Task ship events are append-only');
END;
CREATE TRIGGER store_revision_task_ship_events_update AFTER UPDATE ON task_ship_events
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE INDEX graph_processes_task ON graph_processes(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX graph_processes_unended ON graph_processes(held_terminal_id) WHERE ended_at IS NULL;
CREATE INDEX graph_process_steps_graph ON graph_process_steps(graph_lf_process_id, seq);
-- The liveness pass reads every unended step.
CREATE INDEX graph_process_steps_unended ON graph_process_steps(lf_process_id) WHERE ended_at IS NULL;

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
    OR NEW.caller_session_id IS NOT OLD.caller_session_id
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
    UPDATE graph_processes SET (started_at,ended_at,pid,os_started_at,exit_code,error,repo,cwd,held_terminal_id)=(
        SELECT started_at,completed_at,CASE WHEN completed_at IS NULL THEN pid END,
            CASE WHEN completed_at IS NULL THEN os_started_at END,exit_code,error,repo,cwd,held_terminal_id
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
CREATE INDEX agent_sessions_agent_ended ON agent_sessions(agent_ended_at)
    WHERE agent_ended_at IS NOT NULL;

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

-- No lf writes a Process row: every run and Session records its own OS
-- process above, so nothing may still require one. These tables keep every
-- column and lose only their foreign keys to `processes`. A released lf still
-- writes Process rows, which the bridges above copy; the contract drops them
-- with the table.
-- leaving: processes LOO-965
-- leaving: relocated_task_processes LOO-965
-- leaving: agent_sessions.transport_lf_process_id LOO-965
-- generated: rebuild_without_process_keys
PRAGMA legacy_alter_table = ON;
CREATE TABLE agent_sessions_owned (
    id TEXT PRIMARY KEY NOT NULL,
    current_capture INTEGER UNIQUE REFERENCES session_events(seq),
    title TEXT NOT NULL,
    title_source TEXT NOT NULL CHECK (title_source IN ('generated', 'human')),
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    request TEXT,
    interactive INTEGER NOT NULL DEFAULT 1 CHECK (interactive IN (0, 1)),
    repo TEXT,
    provider_thread TEXT,
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    work_source TEXT CHECK (work_source IN ('declared', 'checkout', 'inherited', 'bound')),
    bound_at INTEGER,
    input_published INTEGER NOT NULL CHECK(input_published IN (0,1)),
    cwd TEXT NOT NULL,
    skill TEXT,
    provider TEXT,
    model TEXT,
    primary_scope TEXT CHECK (primary_scope IN ('repository', 'wave')), agent_process_id TEXT, transport_lf_process_id TEXT, fence_locator TEXT, moved_to_machine_id TEXT, held_terminal_id TEXT REFERENCES held_terminals(id), skill_version TEXT, opened_at INTEGER, transition TEXT, departed_attachment TEXT, departure TEXT, agent_started_at INTEGER, agent_pid INTEGER, agent_os_started_at INTEGER, agent_ended_at INTEGER, agent_exit_code INTEGER, agent_launcher_lf_process_id TEXT, agent_spawn_state TEXT
    CHECK(agent_spawn_state IN ('reserved','spawn_requested','spawn_failed','exited')), agent_command TEXT, agent_interactive INTEGER CHECK(agent_interactive IN (0,1)), agent_endpoint TEXT, agent_inbox TEXT, attachment_token TEXT, attached_lf_process_id TEXT, attached_pid INTEGER, attached_os_started_at INTEGER, attached_terminal_id TEXT);
INSERT INTO agent_sessions_owned (id,current_capture,title,title_source,completed_at,created_at,request,interactive,repo,provider_thread,task_id,wave_id,work_source,bound_at,input_published,cwd,skill,provider,model,primary_scope,agent_process_id,transport_lf_process_id,fence_locator,moved_to_machine_id,held_terminal_id,skill_version,opened_at,transition,departed_attachment,departure,agent_started_at,agent_pid,agent_os_started_at,agent_ended_at,agent_exit_code,agent_launcher_lf_process_id,agent_spawn_state,agent_command,agent_interactive,agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id,attached_pid,attached_os_started_at,attached_terminal_id) SELECT id,current_capture,title,title_source,completed_at,created_at,request,interactive,repo,provider_thread,task_id,wave_id,work_source,bound_at,input_published,cwd,skill,provider,model,primary_scope,agent_process_id,transport_lf_process_id,fence_locator,moved_to_machine_id,held_terminal_id,skill_version,opened_at,transition,departed_attachment,departure,agent_started_at,agent_pid,agent_os_started_at,agent_ended_at,agent_exit_code,agent_launcher_lf_process_id,agent_spawn_state,agent_command,agent_interactive,agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id,attached_pid,attached_os_started_at,attached_terminal_id FROM agent_sessions;
DROP TABLE agent_sessions;
ALTER TABLE agent_sessions_owned RENAME TO agent_sessions;
CREATE TABLE ci_incidents_owned (
    identity TEXT PRIMARY KEY,
    landing_id TEXT REFERENCES pr_landings(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    pr_id TEXT REFERENCES task_prs(id) ON DELETE SET NULL,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha TEXT NOT NULL,
    failure_set_json TEXT NOT NULL,
    provider_completed_at INTEGER,
    poll_observed_at INTEGER,
    webhook_received_at INTEGER,
    claimed_landing_generation INTEGER,
    responded_at INTEGER,
    green_at INTEGER,
    merged_at INTEGER,
    blocked_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    repaired_head_sha TEXT, "repair_lf_process_id" TEXT, repair_session_id TEXT REFERENCES agent_sessions(id), repair_retries INTEGER NOT NULL DEFAULT 0, repair_finished_at INTEGER, repair_error TEXT, repair_conclusion TEXT CHECK (repair_conclusion IN ('published','blocked')), repair_started_at INTEGER, repair_pid INTEGER, repair_os_started_at INTEGER,
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL)),
    CHECK (landing_id IS NULL OR claimed_landing_generation IS NULL OR claimed_landing_generation > 0)
);
INSERT INTO ci_incidents_owned (identity,landing_id,task_id,pr_id,repo,pr_number,failed_head_sha,failure_set_json,provider_completed_at,poll_observed_at,webhook_received_at,claimed_landing_generation,responded_at,green_at,merged_at,blocked_at,blocked_reason,created_at,updated_at,repaired_head_sha,repair_lf_process_id,repair_session_id,repair_retries,repair_finished_at,repair_error,repair_conclusion,repair_started_at,repair_pid,repair_os_started_at) SELECT identity,landing_id,task_id,pr_id,repo,pr_number,failed_head_sha,failure_set_json,provider_completed_at,poll_observed_at,webhook_received_at,claimed_landing_generation,responded_at,green_at,merged_at,blocked_at,blocked_reason,created_at,updated_at,repaired_head_sha,repair_lf_process_id,repair_session_id,repair_retries,repair_finished_at,repair_error,repair_conclusion,repair_started_at,repair_pid,repair_os_started_at FROM ci_incidents;
DROP TABLE ci_incidents;
ALTER TABLE ci_incidents_owned RENAME TO ci_incidents;
CREATE TABLE desktop_shells_owned (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    checkout TEXT NOT NULL,
    owner TEXT,
    created_at INTEGER NOT NULL,
    jobs_settled INTEGER NOT NULL DEFAULT 0,
    close_requested INTEGER NOT NULL DEFAULT 0
, terminal_id TEXT REFERENCES held_terminals(id), launcher_lf_process_id TEXT);
INSERT INTO desktop_shells_owned (id,task_id,checkout,owner,created_at,jobs_settled,close_requested,terminal_id,launcher_lf_process_id) SELECT id,task_id,checkout,owner,created_at,jobs_settled,close_requested,terminal_id,launcher_lf_process_id FROM desktop_shells;
DROP TABLE desktop_shells;
ALTER TABLE desktop_shells_owned RENAME TO desktop_shells;
CREATE TABLE session_events_owned (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed','output','captured')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    "lf_process_id" TEXT,
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    captured_event INTEGER REFERENCES session_events(seq)
, agent_process_id TEXT, provider_account TEXT, independent INTEGER, provider_lineage TEXT, turn_thread TEXT, turn_id TEXT, turn_ended_at INTEGER, turn_status TEXT, turn_error TEXT);
INSERT INTO session_events_owned (seq,session_id,provider_thread,provider_turn,kind,receipt_key,provider_generation,lf_process_id,task_id,wave_id,observed_at,payload,captured_event,agent_process_id,provider_account,independent,provider_lineage,turn_thread,turn_id,turn_ended_at,turn_status,turn_error) SELECT seq,session_id,provider_thread,provider_turn,kind,receipt_key,provider_generation,lf_process_id,task_id,wave_id,observed_at,payload,captured_event,agent_process_id,provider_account,independent,provider_lineage,turn_thread,turn_id,turn_ended_at,turn_status,turn_error FROM session_events;
DROP TABLE session_events;
ALTER TABLE session_events_owned RENAME TO session_events;
CREATE TABLE steer_adapters_owned (
    agent_process_id TEXT PRIMARY KEY,
    capability TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    activated_at INTEGER
);
INSERT INTO steer_adapters_owned (agent_process_id,capability,created_at,activated_at) SELECT agent_process_id,capability,created_at,activated_at FROM steer_adapters;
DROP TABLE steer_adapters;
ALTER TABLE steer_adapters_owned RENAME TO steer_adapters;
CREATE TABLE task_graphs_owned (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    graph TEXT NOT NULL CHECK (json_valid(graph)),
    step TEXT NOT NULL,
    run_start TEXT CHECK (run_start IS NULL OR json_valid(run_start)),
    lf_process_id TEXT,
    updated_at INTEGER NOT NULL, run_pid INTEGER, run_os_started_at INTEGER,
    CHECK ((run_start IS NULL) = (lf_process_id IS NULL))
) STRICT;
INSERT INTO task_graphs_owned (task_id,graph,step,run_start,lf_process_id,updated_at,run_pid,run_os_started_at) SELECT task_id,graph,step,run_start,lf_process_id,updated_at,run_pid,run_os_started_at FROM task_graphs;
DROP TABLE task_graphs;
ALTER TABLE task_graphs_owned RENAME TO task_graphs;
CREATE TABLE tasks_owned (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    issue_title TEXT,
    issue_description TEXT,
    pm_snapshot_synced_at INTEGER,
    worktree TEXT,
    workspace_slug TEXT,
    abandon_reason TEXT,
    updated_at INTEGER,
    abandoned_at INTEGER,
    primary_session_id TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
    planning_revision INTEGER NOT NULL DEFAULT 0 CHECK (planning_revision >= 0),
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK (planning_rank >= 0),
    planning_assignee TEXT,
    planning_state TEXT,
    planning_completed_at TEXT,
    planning_provider_revision TEXT,
    planning_url TEXT,
    planning_branch_name TEXT,
    planning_team_id TEXT,
    planning_completed INTEGER NOT NULL DEFAULT 0 CHECK (planning_completed IN (0,1)),
    planning_deleted_at INTEGER
, branch TEXT NOT NULL DEFAULT '', base_commit TEXT NOT NULL DEFAULT '', parent_pr_id TEXT REFERENCES task_prs(id), completion_request INTEGER, completion_error TEXT, planning_due_date TEXT, checkout_machine_id TEXT REFERENCES machines(id), paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0,1)), shipped_at INTEGER, ship_lf_process_id TEXT, ship_restarts INTEGER NOT NULL DEFAULT 0, ship_approved_lf_process_id TEXT, ship_stopped_at INTEGER, ship_stopped_lf_process_id TEXT, regressed_in TEXT, ship_stopped_person TEXT);
INSERT INTO tasks_owned (id,repository_id,project_id,external_issue_id,issue_identifier,created_at,issue_title,issue_description,pm_snapshot_synced_at,worktree,workspace_slug,abandon_reason,updated_at,abandoned_at,primary_session_id,planning_revision,planning_rank,planning_assignee,planning_state,planning_completed_at,planning_provider_revision,planning_url,planning_branch_name,planning_team_id,planning_completed,planning_deleted_at,branch,base_commit,parent_pr_id,completion_request,completion_error,planning_due_date,checkout_machine_id,paused,shipped_at,ship_lf_process_id,ship_restarts,ship_approved_lf_process_id,ship_stopped_at,ship_stopped_lf_process_id,regressed_in,ship_stopped_person) SELECT id,repository_id,project_id,external_issue_id,issue_identifier,created_at,issue_title,issue_description,pm_snapshot_synced_at,worktree,workspace_slug,abandon_reason,updated_at,abandoned_at,primary_session_id,planning_revision,planning_rank,planning_assignee,planning_state,planning_completed_at,planning_provider_revision,planning_url,planning_branch_name,planning_team_id,planning_completed,planning_deleted_at,branch,base_commit,parent_pr_id,completion_request,completion_error,planning_due_date,checkout_machine_id,paused,shipped_at,ship_lf_process_id,ship_restarts,ship_approved_lf_process_id,ship_stopped_at,ship_stopped_lf_process_id,regressed_in,ship_stopped_person FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_owned RENAME TO tasks;
CREATE TABLE waves_owned (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES "waves"(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    retired_at INTEGER,
    superseded_by_wave_id TEXT REFERENCES "waves"(id) ON DELETE RESTRICT,
    retirement_reason TEXT,
    work_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (work_state IN ('ready', 'done', 'abandoned')),
    work_terminal_at INTEGER,
    current_project_id TEXT REFERENCES projects(id),
    project_activation_lf_process_id TEXT, project_activation_started_at INTEGER, project_activation_pid INTEGER, project_activation_os_started_at INTEGER, project_activation_ended_at INTEGER, project_activation_exit_code INTEGER, project_activation_error TEXT,
    CHECK (
        retired_at IS NOT NULL
        OR (superseded_by_wave_id IS NULL AND retirement_reason IS NULL)
    ),
    CHECK (retirement_reason IS NULL OR length(trim(retirement_reason)) > 0)
);
INSERT INTO waves_owned (id,name,repo,created_at,parent_wave_id,promoted_at,retired_at,superseded_by_wave_id,retirement_reason,work_state,work_terminal_at,current_project_id,project_activation_lf_process_id,project_activation_started_at,project_activation_pid,project_activation_os_started_at,project_activation_ended_at,project_activation_exit_code,project_activation_error) SELECT id,name,repo,created_at,parent_wave_id,promoted_at,retired_at,superseded_by_wave_id,retirement_reason,work_state,work_terminal_at,current_project_id,project_activation_lf_process_id,project_activation_started_at,project_activation_pid,project_activation_os_started_at,project_activation_ended_at,project_activation_exit_code,project_activation_error FROM waves;
DROP TABLE waves;
ALTER TABLE waves_owned RENAME TO waves;
CREATE INDEX agent_sessions_agent_ended ON agent_sessions(agent_ended_at)
    WHERE agent_ended_at IS NOT NULL;
CREATE INDEX agent_sessions_agent_process ON agent_sessions(agent_process_id)
    WHERE agent_process_id IS NOT NULL;
CREATE INDEX agent_sessions_attached ON agent_sessions(attached_lf_process_id)
    WHERE attached_lf_process_id IS NOT NULL;
CREATE INDEX agent_sessions_held_terminal ON agent_sessions(held_terminal_id)
    WHERE held_terminal_id IS NOT NULL;
CREATE UNIQUE INDEX agent_sessions_primary_repository ON agent_sessions(repo)
    WHERE primary_scope = 'repository' AND completed_at IS NULL;
CREATE UNIQUE INDEX agent_sessions_primary_wave ON agent_sessions(wave_id)
    WHERE primary_scope = 'wave' AND completed_at IS NULL;
CREATE INDEX agent_sessions_provider_thread ON agent_sessions(provider_thread)
    WHERE provider_thread IS NOT NULL;
CREATE INDEX agent_sessions_transport ON agent_sessions(transport_lf_process_id)
    WHERE transport_lf_process_id IS NOT NULL;
CREATE INDEX agent_sessions_unended_agent ON agent_sessions(agent_process_id)
    WHERE agent_process_id IS NOT NULL AND agent_ended_at IS NULL;
CREATE INDEX agent_sessions_wave ON agent_sessions(wave_id)
    WHERE wave_id IS NOT NULL;
CREATE INDEX conversation_task_inventory ON agent_sessions(task_id,title,id);
CREATE INDEX open_sessions ON "agent_sessions"(created_at, id) WHERE completed_at IS NULL;
CREATE INDEX session_inventory ON "agent_sessions"(interactive, completed_at, title, id);
CREATE INDEX session_repo_inventory ON "agent_sessions"(repo, interactive, completed_at, title, id);
CREATE INDEX ci_incidents_repair_process ON ci_incidents(repair_lf_process_id)
    WHERE repair_lf_process_id IS NOT NULL;
CREATE INDEX idx_ci_incidents_landing ON ci_incidents(landing_id, created_at);
CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_open ON ci_incidents(green_at, merged_at, updated_at);
CREATE INDEX idx_ci_incidents_pr ON ci_incidents(pr_id, created_at);
CREATE UNIQUE INDEX desktop_shells_owner ON desktop_shells(owner);
CREATE INDEX desktop_shells_task ON desktop_shells(task_id);
CREATE UNIQUE INDEX desktop_shells_terminal ON desktop_shells(terminal_id) WHERE terminal_id IS NOT NULL;
CREATE INDEX session_capture_caller ON session_events(
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.caller_key') END)
    WHERE kind='captured';
CREATE UNIQUE INDEX session_capture_key ON session_events(receipt_key) WHERE kind='captured';
CREATE INDEX session_capture_provider_thread ON session_events(provider_thread)
    WHERE kind='captured' AND provider_thread IS NOT NULL;
CREATE INDEX session_capture_turn ON session_events(session_id,turn_thread,turn_id)
    WHERE kind='captured' AND turn_id IS NOT NULL;
CREATE INDEX session_capture_window ON session_events(observed_at)
    WHERE kind='captured';
CREATE INDEX session_event_input ON session_events(session_id,captured_event) WHERE kind='started';
CREATE UNIQUE INDEX session_event_receipt ON session_events(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);
CREATE INDEX session_events_captured_event ON session_events(captured_event)
    WHERE captured_event IS NOT NULL;
CREATE INDEX session_events_history ON session_events(session_id,seq);
CREATE INDEX session_events_process ON session_events(lf_process_id, session_id) WHERE lf_process_id IS NOT NULL;
CREATE INDEX session_events_provider_session ON session_events(
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.evidence.provider_session_id') END)
    WHERE kind='observed' AND instr(receipt_key,':provider-session:')>0;
CREATE INDEX session_events_task ON session_events(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX session_events_turn ON session_events(session_id,provider_thread,provider_turn,seq);
CREATE INDEX session_events_wave ON session_events(wave_id) WHERE wave_id IS NOT NULL;
CREATE INDEX session_input_membership ON session_events(
    session_id, captured_event, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';
CREATE INDEX session_input_outcome_window ON session_events(observed_at)
    WHERE kind='observed' AND substr(receipt_key,-14) IN (':manifest.json',':terminal.json');
CREATE INDEX session_observation_receipt ON session_events(session_id,receipt_key) WHERE kind='observed';
CREATE INDEX session_process_membership ON session_events(session_id,lf_process_id) WHERE lf_process_id IS NOT NULL;
CREATE INDEX session_provider_transitions ON session_events(session_id, seq)
WHERE kind = 'observed'
    AND (receipt_key GLOB 'provider-transition:*' OR receipt_key GLOB 'agent-session:*');
CREATE INDEX session_turn_events ON session_events(
    session_id, provider_thread, provider_turn, kind, observed_at)
    WHERE kind IN ('started','usage','completed','output');
CREATE INDEX session_turn_window ON session_events(observed_at, kind, session_id, provider_thread, provider_turn)
    WHERE kind IN ('started','usage','completed','output');
CREATE INDEX task_graphs_process ON task_graphs(lf_process_id)
    WHERE lf_process_id IS NOT NULL;
CREATE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);
CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);
CREATE INDEX idx_tasks_repository ON tasks(repository_id, project_id, planning_rank);
CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);
CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);
CREATE INDEX tasks_abandoned ON tasks(id) WHERE abandoned_at IS NOT NULL;
CREATE INDEX tasks_abandoned_checkout ON tasks(checkout_machine_id)
    WHERE abandoned_at IS NOT NULL AND worktree IS NOT NULL;
CREATE INDEX tasks_branch ON tasks(branch);
CREATE INDEX tasks_checkout_machine ON tasks(checkout_machine_id);
CREATE INDEX tasks_checkout_root ON tasks(checkout_machine_id, rtrim(worktree,'/'))
    WHERE worktree!='';
CREATE INDEX tasks_parent_pr ON tasks(parent_pr_id)
    WHERE parent_pr_id IS NOT NULL;
CREATE INDEX tasks_primary_session ON tasks(primary_session_id)
    WHERE primary_session_id IS NOT NULL;
CREATE INDEX tasks_ship_process ON tasks(ship_lf_process_id)
    WHERE ship_lf_process_id IS NOT NULL;
CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(parent_wave_id,''),name) WHERE retired_at IS NULL;
CREATE INDEX idx_waves_parent
    ON waves(parent_wave_id) WHERE retired_at IS NULL;
CREATE INDEX idx_waves_superseded_by
    ON waves(superseded_by_wave_id) WHERE superseded_by_wave_id IS NOT NULL;
CREATE INDEX waves_project_activation_process ON waves(project_activation_lf_process_id)
    WHERE project_activation_lf_process_id IS NOT NULL;
CREATE TRIGGER canonical_agent_sessions_insert BEFORE INSERT ON agent_sessions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: agent_sessions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.transport_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
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
CREATE TRIGGER session_fence_locator_immutable BEFORE UPDATE OF fence_locator ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL AND NEW.fence_locator IS NOT OLD.fence_locator
BEGIN
    SELECT RAISE(ABORT, 'Session fence locator is immutable');
END;
CREATE TRIGGER session_fence_locator_insert AFTER INSERT ON agent_sessions
WHEN NEW.fence_locator IS NULL
BEGIN
    UPDATE agent_sessions SET fence_locator=NEW.id WHERE id=NEW.id;
END;
CREATE TRIGGER session_preview_agent_session_update
AFTER UPDATE OF provider_thread,agent_process_id ON agent_sessions
WHEN NEW.provider_thread IS NOT OLD.provider_thread OR NEW.agent_process_id IS NOT OLD.agent_process_id
BEGIN
    UPDATE session_activity SET preview=NULL WHERE session_id=NEW.id;
END;
CREATE TRIGGER store_revision_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_sessions_update AFTER UPDATE ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER task_update_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_agent_sessions_update AFTER UPDATE OF task_id,primary_scope,cwd,interactive,completed_at ON agent_sessions
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.primary_scope IS NOT OLD.primary_scope OR NEW.cwd IS NOT OLD.cwd OR NEW.interactive IS NOT OLD.interactive OR NEW.completed_at IS NOT OLD.completed_at
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER validate_conversation_ancestry_insert BEFORE INSERT ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
END;
CREATE TRIGGER validate_conversation_ancestry_update BEFORE UPDATE OF task_id,wave_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'Session Task binding is permanent') END;
    SELECT CASE WHEN OLD.task_id IS NULL AND OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        AND NOT (OLD.primary_scope IS 'wave' AND NEW.primary_scope IS NULL AND NEW.wave_id IS NULL)
        THEN RAISE(ABORT,'Wave Session binding is permanent') END;
END;
CREATE TRIGGER validate_session_capture BEFORE UPDATE OF current_capture ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
CREATE TRIGGER validate_session_capture_insert BEFORE INSERT ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
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
CREATE TRIGGER store_revision_ci_incidents_delete AFTER DELETE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_insert AFTER INSERT ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_update AFTER UPDATE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_desktop_shells_delete AFTER DELETE ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_desktop_shells_insert AFTER INSERT ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_desktop_shells_update AFTER UPDATE ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER canonical_session_events_insert BEFORE INSERT ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE TRIGGER canonical_session_events_update BEFORE UPDATE OF lf_process_id,wave_id,agent_process_id,task_id,session_id,payload ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE TRIGGER preserve_capture_delete BEFORE DELETE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence must be retained'); END;
CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
    AND (NEW.seq IS NOT OLD.seq OR NEW.session_id IS NOT OLD.session_id OR NEW.kind IS NOT OLD.kind
    OR NEW.provider_turn IS NOT OLD.provider_turn OR NEW.receipt_key IS NOT OLD.receipt_key
    OR NEW.provider_generation IS NOT OLD.provider_generation OR NEW.lf_process_id IS NOT OLD.lf_process_id
    OR NEW.task_id IS NOT OLD.task_id OR NEW.wave_id IS NOT OLD.wave_id
    OR NEW.observed_at IS NOT OLD.observed_at OR NEW.payload IS NOT OLD.payload
    OR NEW.captured_event IS NOT OLD.captured_event OR NEW.agent_process_id IS NOT OLD.agent_process_id)
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;
CREATE TRIGGER store_revision_session_events_delete AFTER DELETE ON session_events
WHEN OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_insert AFTER INSERT ON session_events
WHEN NEW.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_update AFTER UPDATE ON session_events
WHEN NEW.kind != 'usage' OR OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER task_update_session_events_delete AFTER DELETE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_session_events_insert AFTER INSERT ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_session_events_update AFTER UPDATE ON session_events
WHEN EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER store_revision_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER task_carrier_start_from_process AFTER UPDATE OF lf_process_id ON task_graphs
WHEN NEW.lf_process_id IS NOT NULL AND NEW.lf_process_id IS NOT OLD.lf_process_id
    AND NEW.run_pid IS NULL
BEGIN
    UPDATE task_graphs SET (run_pid,run_os_started_at)=(
        SELECT pid,os_started_at FROM processes WHERE id=NEW.lf_process_id AND completed_at IS NULL)
    WHERE task_id=NEW.task_id;
END;
CREATE TRIGGER task_graph_is_fixed BEFORE UPDATE OF graph ON task_graphs BEGIN
    SELECT RAISE(ABORT,'A Task Graph is fixed when taken up; take up another to replace it');
END;
CREATE TRIGGER task_update_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER cancel_task_retirement
AFTER UPDATE OF planning_completed,planning_completed_at,abandoned_at,worktree,branch ON tasks
WHEN (NEW.planning_completed=0 AND NEW.abandoned_at IS NULL)
  OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.abandoned_at IS NOT OLD.abandoned_at
  OR NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN
    DELETE FROM task_retirements WHERE task_id=NEW.id;
END;
CREATE TRIGGER canonical_tasks_insert BEFORE INSERT ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE TRIGGER canonical_tasks_update BEFORE UPDATE OF external_issue_id,planning_assignee,planning_team_id,id,project_id,checkout_machine_id,parent_pr_id,primary_session_id,issue_identifier,repository_id ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE TRIGGER store_revision_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
WHEN NEW.id IS NOT OLD.id OR NEW.repository_id IS NOT OLD.repository_id OR
    NEW.project_id IS NOT OLD.project_id OR
    NEW.external_issue_id IS NOT OLD.external_issue_id OR
    NEW.issue_identifier IS NOT OLD.issue_identifier OR
    NEW.created_at IS NOT OLD.created_at OR
    NEW.issue_title IS NOT OLD.issue_title OR
    NEW.issue_description IS NOT OLD.issue_description OR
    NEW.worktree IS NOT OLD.worktree OR
    NEW.workspace_slug IS NOT OLD.workspace_slug OR
    NEW.abandon_reason IS NOT OLD.abandon_reason OR
    NEW.updated_at IS NOT OLD.updated_at OR
    NEW.abandoned_at IS NOT OLD.abandoned_at OR
    NEW.primary_session_id IS NOT OLD.primary_session_id OR
    NEW.planning_revision IS NOT OLD.planning_revision OR
    NEW.planning_rank IS NOT OLD.planning_rank OR
    NEW.planning_assignee IS NOT OLD.planning_assignee OR
    NEW.planning_state IS NOT OLD.planning_state OR
    NEW.planning_completed_at IS NOT OLD.planning_completed_at OR
    NEW.planning_provider_revision IS NOT OLD.planning_provider_revision OR
    NEW.planning_url IS NOT OLD.planning_url OR
    NEW.planning_branch_name IS NOT OLD.planning_branch_name OR
    NEW.planning_team_id IS NOT OLD.planning_team_id OR
    NEW.planning_completed IS NOT OLD.planning_completed OR
    NEW.planning_deleted_at IS NOT OLD.planning_deleted_at OR
    NEW.branch IS NOT OLD.branch OR NEW.base_commit IS NOT OLD.base_commit OR
    NEW.parent_pr_id IS NOT OLD.parent_pr_id OR
    NEW.completion_request IS NOT OLD.completion_request OR
    NEW.completion_error IS NOT OLD.completion_error OR
    NEW.planning_due_date IS NOT OLD.planning_due_date OR
    NEW.checkout_machine_id IS NOT OLD.checkout_machine_id OR
    NEW.paused IS NOT OLD.paused OR NEW.shipped_at IS NOT OLD.shipped_at OR
    NEW.ship_lf_process_id IS NOT OLD.ship_lf_process_id OR
    NEW.ship_restarts IS NOT OLD.ship_restarts OR
    NEW.ship_approved_lf_process_id IS NOT OLD.ship_approved_lf_process_id OR
    NEW.ship_stopped_at IS NOT OLD.ship_stopped_at OR
    NEW.ship_stopped_lf_process_id IS NOT OLD.ship_stopped_lf_process_id OR
    NEW.regressed_in IS NOT OLD.regressed_in OR
    NEW.ship_stopped_person IS NOT OLD.ship_stopped_person OR
    (NEW.pm_snapshot_synced_at IS NULL) IS NOT (OLD.pm_snapshot_synced_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;
CREATE TRIGGER task_repository_is_fixed BEFORE UPDATE OF repository_id ON tasks
WHEN NEW.repository_id IS NOT OLD.repository_id
BEGIN SELECT RAISE(ABORT, 'Task repository identity cannot change'); END;
CREATE TRIGGER task_session_membership AFTER UPDATE OF project_id ON tasks BEGIN
    UPDATE agent_sessions SET wave_id=(SELECT wave_id FROM projects WHERE id=NEW.project_id) WHERE task_id=NEW.id;
END;
CREATE TRIGGER task_update_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_tasks_update AFTER UPDATE OF issue_identifier,worktree,checkout_machine_id ON tasks
WHEN NEW.issue_identifier IS NOT OLD.issue_identifier OR NEW.worktree IS NOT OLD.worktree OR NEW.checkout_machine_id IS NOT OLD.checkout_machine_id
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER validate_task_repository_insert BEFORE INSERT ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;
CREATE TRIGGER validate_task_repository_update BEFORE UPDATE OF project_id,repository_id ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;
CREATE TRIGGER canonical_waves_insert BEFORE INSERT ON waves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: waves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.superseded_by_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_activation_lf_process_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.current_project_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
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
CREATE TRIGGER selected_project_wave_update BEFORE UPDATE OF current_project_id ON waves
WHEN NEW.current_project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects WHERE id=NEW.current_project_id AND wave_id=NEW.id
) BEGIN SELECT RAISE(ABORT, 'selected Project belongs to another Wave'); END;
CREATE TRIGGER store_revision_waves_delete AFTER DELETE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_insert AFTER INSERT ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_update AFTER UPDATE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
PRAGMA legacy_alter_table = OFF;
