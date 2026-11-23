-- A Task's work is its Sessions and its Graph runs. Each records its Task
-- once, when it starts: the Task it names, else the deepest checkout holding
-- its directory on this Machine, else the Task of the Session that started
-- it. Membership reads that column and never matches a path, so a moved
-- repository or a reused checkout path changes no Task's work.
--
-- Sessions written before this rule take it here. A Task's primary
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

-- Graph runs that named no Task take theirs by the same rule, once. A run's
-- Task is part of its launch, so the trigger that fixes it steps aside here.
DROP TRIGGER graph_processes_launch_is_immutable;
CREATE TEMP TABLE graph_run_tasks AS
SELECT g.lf_process_id AS run, coalesce(
    (WITH RECURSIVE up(path) AS (
        SELECT rtrim(g.cwd,'/') WHERE g.cwd IS NOT NULL
        UNION ALL
        SELECT rtrim(rtrim(path,replace(path,'/','')),'/') FROM up WHERE path!='')
     SELECT c.task_id FROM up CROSS JOIN task_checkouts c INDEXED BY task_checkouts_root
        ON rtrim(c.worktree,'/')=up.path
     WHERE up.path!='' OR substr(g.cwd,1,1)='/' OR g.cwd=''
     ORDER BY length(up.path) DESC, c.task_id LIMIT 1),
    (SELECT s.task_id FROM agent_sessions s
     WHERE s.id=g.caller_session_id AND s.primary_scope IS NULL)) AS task_id
FROM graph_processes g WHERE g.task_id IS NULL;
UPDATE graph_processes SET task_id=found.task_id
FROM graph_run_tasks found
WHERE found.run=graph_processes.lf_process_id AND found.task_id IS NOT NULL;
DROP TABLE graph_run_tasks;
CREATE TRIGGER graph_processes_launch_is_immutable BEFORE UPDATE ON graph_processes
WHEN NEW.lf_process_id IS NOT OLD.lf_process_id OR NEW.graph IS NOT OLD.graph
    OR NEW.compiled IS NOT OLD.compiled OR NEW.task_id IS NOT OLD.task_id
    OR NEW.caller_session_id IS NOT OLD.caller_session_id
BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;

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

-- `work_source='inherited'` goes, unwritten since 2026-10-07: those rows
-- declared their Task through the Session that started them, which is
-- `declared`. SQLite cannot alter a CHECK, so the table is rebuilt with its
-- column order, indexes and triggers.
PRAGMA legacy_alter_table = ON;
CREATE TABLE agent_sessions_task_primary (
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
    work_source TEXT CHECK (work_source IN ('declared', 'checkout', 'bound')),
    bound_at INTEGER,
    input_published INTEGER NOT NULL CHECK(input_published IN (0,1)),
    cwd TEXT NOT NULL,
    skill TEXT,
    provider TEXT,
    model TEXT,
    primary_scope TEXT CHECK (primary_scope IN ('repository', 'wave')), agent_process_id TEXT, transport_lf_process_id TEXT, fence_locator TEXT, moved_to_machine_id TEXT, held_terminal_id TEXT REFERENCES held_terminals(id), skill_version TEXT, opened_at INTEGER, transition TEXT, departed_attachment TEXT, departure TEXT, agent_started_at INTEGER, agent_pid INTEGER, agent_os_started_at INTEGER, agent_ended_at INTEGER, agent_exit_code INTEGER, agent_launcher_lf_process_id TEXT, agent_spawn_state TEXT
    CHECK(agent_spawn_state IN ('reserved','spawn_requested','spawn_failed','exited')), agent_command TEXT, agent_interactive INTEGER CHECK(agent_interactive IN (0,1)), agent_endpoint TEXT, agent_inbox TEXT, attachment_token TEXT, attached_lf_process_id TEXT, attached_pid INTEGER, attached_os_started_at INTEGER, attached_terminal_id TEXT,
    starts_since_turn INTEGER NOT NULL DEFAULT 0,
    last_started_at INTEGER,
    start_failure TEXT,
    agent_socket TEXT
);
INSERT INTO agent_sessions_task_primary(id,current_capture,title,title_source,completed_at,created_at,request,interactive,repo,provider_thread,task_id,wave_id,work_source,bound_at,input_published,cwd,skill,provider,model,primary_scope,agent_process_id,transport_lf_process_id,fence_locator,moved_to_machine_id,held_terminal_id,skill_version,opened_at,transition,departed_attachment,departure,agent_started_at,agent_pid,agent_os_started_at,agent_ended_at,agent_exit_code,agent_launcher_lf_process_id,agent_spawn_state,agent_command,agent_interactive,agent_endpoint,agent_inbox,attachment_token,attached_lf_process_id,attached_pid,attached_os_started_at,attached_terminal_id,starts_since_turn,last_started_at,start_failure,agent_socket)
SELECT s.id,s.current_capture,s.title,s.title_source,s.completed_at,s.created_at,s.request,s.interactive,s.repo,s.provider_thread,s.task_id,s.wave_id,CASE s.work_source WHEN 'inherited' THEN 'declared' ELSE s.work_source END,s.bound_at,s.input_published,s.cwd,s.skill,s.provider,s.model,s.primary_scope,s.agent_process_id,s.transport_lf_process_id,s.fence_locator,s.moved_to_machine_id,s.held_terminal_id,s.skill_version,s.opened_at,s.transition,s.departed_attachment,s.departure,s.agent_started_at,s.agent_pid,s.agent_os_started_at,s.agent_ended_at,s.agent_exit_code,s.agent_launcher_lf_process_id,s.agent_spawn_state,s.agent_command,s.agent_interactive,s.agent_endpoint,s.agent_inbox,s.attachment_token,s.attached_lf_process_id,s.attached_pid,s.attached_os_started_at,s.attached_terminal_id,s.starts_since_turn,s.last_started_at,s.start_failure,s.agent_socket
FROM agent_sessions s;
DROP TABLE agent_sessions;
ALTER TABLE agent_sessions_task_primary RENAME TO agent_sessions;
PRAGMA legacy_alter_table = OFF;
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
