-- Rename recorded commands in place; IDs, outcomes and provenance remain unchanged.

DROP INDEX "execs_parent";

DROP INDEX "execs_trace";

DROP INDEX "execs_recent";

DROP INDEX "session_driver_exec";

DROP INDEX "session_unknown_engine_origin";

DROP INDEX "execs_unfinished";

DROP INDEX "session_events_exec";

DROP TRIGGER "store_revision_execs_insert";

DROP TRIGGER "store_revision_execs_update";

DROP TRIGGER "store_revision_execs_delete";

DROP INDEX "flow_exec_steps_flow";

DROP TRIGGER "flow_execs_are_append_only";

DROP TRIGGER "flow_exec_steps_are_append_only";

DROP TRIGGER "validate_flow_exec_step";

DROP TRIGGER "validate_task_started_update";

DROP TRIGGER "store_revision_flow_execs_insert";

DROP TRIGGER "store_revision_flow_execs_update";

DROP TRIGGER "store_revision_flow_execs_delete";

DROP TRIGGER "store_revision_flow_exec_steps_insert";

DROP TRIGGER "store_revision_flow_exec_steps_update";

DROP TRIGGER "store_revision_flow_exec_steps_delete";

DROP INDEX "session_exec_membership";

ALTER TABLE "waves" RENAME COLUMN "project_activation_exec_id" TO "project_activation_process_id";

ALTER TABLE "ci_incidents" RENAME COLUMN "repair_exec_id" TO "repair_process_id";

ALTER TABLE "execs" RENAME COLUMN "parent_exec_id" TO "parent_process_id";

ALTER TABLE "execs" RENAME TO "processes";

ALTER TABLE "session_events" RENAME COLUMN "exec_id" TO "process_id";

ALTER TABLE "agent_sessions" RENAME COLUMN "driver_exec_id" TO "driver_process_id";

ALTER TABLE "agent_sessions" RENAME COLUMN "provider_exec_id" TO "provider_process_id";

ALTER TABLE "flow_execs" RENAME COLUMN "exec_id" TO "process_id";

ALTER TABLE "flow_execs" RENAME TO "flow_processes";

ALTER TABLE "flow_exec_steps" RENAME COLUMN "flow_exec_id" TO "flow_process_id";

ALTER TABLE "flow_exec_steps" RENAME COLUMN "exec_id" TO "process_id";

ALTER TABLE "flow_exec_steps" RENAME TO "flow_process_steps";

ALTER TABLE "task_workflows" RENAME COLUMN "exec_id" TO "process_id";

ALTER TABLE "task_workflow_moves" RENAME COLUMN "exec_id" TO "process_id";

CREATE TEMP TABLE saved_process_revisions AS SELECT * FROM store_revisions;

DROP TABLE store_revisions;

CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning', 'sessions', 'flows', 'processes', 'usage')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;

INSERT INTO store_revisions SELECT CASE domain WHEN 'execs' THEN 'processes' ELSE domain END, revision FROM saved_process_revisions;

DROP TABLE saved_process_revisions;

CREATE INDEX processes_parent ON processes(parent_process_id, started_at, id);

CREATE INDEX processes_trace ON processes(trace_id, started_at, id);

CREATE INDEX processes_recent ON processes(started_at DESC, id);

CREATE INDEX session_driver_process ON agent_sessions(driver_process_id) WHERE driver_process_id IS NOT NULL;

CREATE INDEX session_unknown_engine_origin ON agent_sessions(provider_process_id) WHERE provider_pid IS NULL;

CREATE INDEX processes_unfinished ON processes(started_at, id) WHERE completed_at IS NULL;

CREATE INDEX session_events_process ON session_events(process_id, session_id) WHERE process_id IS NOT NULL;

CREATE TRIGGER store_revision_processes_insert AFTER INSERT ON processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;

CREATE TRIGGER store_revision_processes_update AFTER UPDATE ON processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;

CREATE TRIGGER store_revision_processes_delete AFTER DELETE ON processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;

CREATE INDEX flow_process_steps_flow ON flow_process_steps(flow_process_id, seq);

CREATE TRIGGER flow_processes_are_append_only BEFORE UPDATE ON flow_processes BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER flow_process_steps_are_append_only BEFORE UPDATE ON flow_process_steps BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER validate_flow_process_step BEFORE INSERT ON flow_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.id=NEW.process_id AND step.parent_process_id=NEW.flow_process_id
    ) THEN RAISE(ABORT,'A Flow step is a process its driver started') END;
END;

CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN NEW.started_at IS NOT NULL AND NOT (
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_processes f JOIN processes driver ON driver.id=f.process_id WHERE NEW.worktree!=''
            AND (driver.cwd=rtrim(NEW.worktree,'/') OR instr(driver.cwd,rtrim(NEW.worktree,'/')||'/')=1)) OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

CREATE TRIGGER store_revision_flow_processes_insert AFTER INSERT ON flow_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE TRIGGER store_revision_flow_processes_update AFTER UPDATE ON flow_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE TRIGGER store_revision_flow_processes_delete AFTER DELETE ON flow_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE TRIGGER store_revision_flow_process_steps_insert AFTER INSERT ON flow_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE TRIGGER store_revision_flow_process_steps_update AFTER UPDATE ON flow_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE TRIGGER store_revision_flow_process_steps_delete AFTER DELETE ON flow_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'flows';
END;

CREATE INDEX session_process_membership ON session_events(session_id,process_id) WHERE process_id IS NOT NULL;
