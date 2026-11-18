-- draft: process_names
-- Rename recorded commands in place; IDs, outcomes and provenance remain unchanged.
-- SQLite updates references in surviving indexes and triggers during each rename.

DROP INDEX "execs_parent";

DROP INDEX "execs_trace";

DROP INDEX "execs_recent";

DROP INDEX "session_driver_exec";

DROP INDEX "execs_unfinished";

DROP INDEX "session_events_exec";

DROP TRIGGER "store_revision_execs_insert";

DROP TRIGGER "store_revision_execs_update";

DROP TRIGGER "store_revision_execs_delete";

DROP INDEX "flow_exec_steps_flow";

DROP TRIGGER "flow_execs_are_append_only";

DROP TRIGGER "flow_exec_steps_are_append_only";

DROP TRIGGER "validate_flow_exec_step";

DROP TRIGGER "store_revision_flow_execs_insert";

DROP TRIGGER "store_revision_flow_execs_update";

DROP TRIGGER "store_revision_flow_execs_delete";

DROP TRIGGER "store_revision_flow_exec_steps_insert";

DROP TRIGGER "store_revision_flow_exec_steps_update";

DROP TRIGGER "store_revision_flow_exec_steps_delete";

DROP INDEX "session_exec_membership";

ALTER TABLE "waves" RENAME COLUMN "project_activation_exec_id" TO "project_activation_process_lfid";

ALTER TABLE "ci_incidents" RENAME COLUMN "repair_exec_id" TO "repair_process_lfid";

ALTER TABLE "execs" RENAME COLUMN "parent_exec_id" TO "parent_process_lfid";

ALTER TABLE "execs" RENAME TO "processes";

ALTER TABLE "processes" RENAME COLUMN "id" TO "lfid";

ALTER TABLE "processes" ADD COLUMN "pid" INTEGER;

ALTER TABLE "session_events" RENAME COLUMN "exec_id" TO "process_lfid";

ALTER TABLE "agent_sessions" RENAME COLUMN "driver_exec_id" TO "driver_process_lfid";

ALTER TABLE "agent_sessions" RENAME COLUMN "provider_exec_id" TO "provider_process_lfid";

ALTER TABLE "flow_execs" RENAME COLUMN "exec_id" TO "process_lfid";

ALTER TABLE "flow_execs" RENAME TO "flow_processes";

ALTER TABLE "flow_exec_steps" RENAME COLUMN "flow_exec_id" TO "flow_process_lfid";

ALTER TABLE "flow_exec_steps" RENAME COLUMN "exec_id" TO "process_lfid";

ALTER TABLE "flow_exec_steps" RENAME TO "flow_process_steps";

ALTER TABLE "task_workflows" RENAME COLUMN "exec_id" TO "process_lfid";

ALTER TABLE "task_workflow_moves" RENAME COLUMN "exec_id" TO "process_lfid";

CREATE TEMP TABLE saved_process_revisions AS SELECT * FROM store_revisions;

DROP TABLE store_revisions;

CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning', 'sessions', 'flows', 'processes', 'usage')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;

INSERT INTO store_revisions SELECT CASE domain WHEN 'execs' THEN 'processes' ELSE domain END, revision FROM saved_process_revisions;

DROP TABLE saved_process_revisions;

CREATE INDEX processes_parent ON processes(parent_process_lfid, started_at, lfid);

CREATE INDEX processes_trace ON processes(trace_id, started_at, lfid);

CREATE INDEX processes_recent ON processes(started_at DESC, lfid);

CREATE INDEX session_driver_process ON agent_sessions(driver_process_lfid) WHERE driver_process_lfid IS NOT NULL;

CREATE INDEX processes_unfinished ON processes(started_at, lfid) WHERE completed_at IS NULL;

CREATE INDEX session_events_process ON session_events(process_lfid, session_id) WHERE process_lfid IS NOT NULL;

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

CREATE INDEX flow_process_steps_flow ON flow_process_steps(flow_process_lfid, seq);

CREATE TRIGGER flow_processes_are_append_only BEFORE UPDATE ON flow_processes BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER flow_process_steps_are_append_only BEFORE UPDATE ON flow_process_steps BEGIN
    SELECT RAISE(ABORT,'A Flow record is append-only');
END;

CREATE TRIGGER validate_flow_process_step BEFORE INSERT ON flow_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.lfid=NEW.process_lfid AND step.parent_process_lfid=NEW.flow_process_lfid
    ) THEN RAISE(ABORT,'A Flow step is a process its driver started') END;
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

CREATE INDEX session_process_membership ON session_events(session_id,process_lfid) WHERE process_lfid IS NOT NULL;

-- draft: program_status
ALTER TABLE session_activity ADD COLUMN provider_generation INTEGER;
ALTER TABLE session_activity ADD COLUMN program_status TEXT CHECK (program_status IS NULL OR json_valid(program_status));
ALTER TABLE session_activity ADD COLUMN status_stream TEXT;
ALTER TABLE session_activity ADD COLUMN status_sequence INTEGER;
-- Prior inference has no provider-generation witness; wait for a fresh reading.
DROP TRIGGER store_revision_session_activity_update;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.driver_generation IS NOT OLD.driver_generation
    OR NEW.provider_generation IS NOT OLD.provider_generation
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;

-- draft: rename_home_to_machine
-- Rename the authority, retaining opaque IDs and all placement/history bytes.
ALTER TABLE homes RENAME TO machines;
ALTER TABLE work_placements RENAME COLUMN home_id TO machine_id;
ALTER TABLE pr_landings RENAME COLUMN supervisor_home_id TO supervisor_machine_id;

DROP INDEX idx_homes_route;
CREATE UNIQUE INDEX idx_machines_route ON machines(route);
DROP INDEX idx_work_placements_home;
CREATE INDEX idx_work_placements_machine ON work_placements(machine_id, placed_at);

-- The retired supervisor discriminator and JSON remain historical evidence.
-- Neither spelling may acquire remote supervision authority.
DROP TRIGGER pr_landings_no_home_insert;
DROP TRIGGER pr_landings_no_home_update;
CREATE TRIGGER pr_landings_no_machine_insert
BEFORE INSERT ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;
CREATE TRIGGER pr_landings_no_machine_update
BEFORE UPDATE OF supervisor_placement ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;

-- draft: machine_connections
-- Connection removal preserves the machine and every historical reference.
ALTER TABLE machines ADD COLUMN label TEXT;
ALTER TABLE machines ADD COLUMN repo TEXT CHECK ((label IS NULL) = (repo IS NULL));
CREATE UNIQUE INDEX idx_machines_label ON machines(label) WHERE label IS NOT NULL;
DROP INDEX idx_machines_route;
CREATE UNIQUE INDEX idx_machines_route ON machines(route)
WHERE route='local' OR label IS NOT NULL;
