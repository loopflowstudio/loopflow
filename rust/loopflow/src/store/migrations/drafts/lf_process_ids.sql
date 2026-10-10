-- depends_on: agent_process, optional_task_pr
-- An LfProcess's identity is its id; every reference names its role. SQLite
-- rewrites surviving indexes, triggers and foreign keys during each rename.
-- Rows, IDs, outcomes and history payloads remain unchanged.
ALTER TABLE processes RENAME COLUMN lfid TO id;
ALTER TABLE processes RENAME COLUMN parent_process_lfid TO parent_lf_process_id;
ALTER TABLE waves RENAME COLUMN project_activation_process_lfid TO project_activation_lf_process_id;
ALTER TABLE ci_incidents RENAME COLUMN repair_process_lfid TO repair_lf_process_id;
ALTER TABLE session_events RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE flow_processes RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE flow_process_steps RENAME COLUMN flow_process_lfid TO flow_lf_process_id;
ALTER TABLE flow_process_steps RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE task_workflows RENAME COLUMN process_lfid TO lf_process_id;
ALTER TABLE task_workflow_moves RENAME COLUMN process_lfid TO lf_process_id;
DROP TRIGGER validate_flow_process_step;
CREATE TRIGGER validate_flow_process_step BEFORE INSERT ON flow_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.id=NEW.lf_process_id AND step.parent_lf_process_id=NEW.flow_lf_process_id
    ) THEN RAISE(ABORT,'A Flow step is a process its Flow process started') END;
END;
-- Attachment exits recorded under the released receipt prefix keep their
-- generation and payload; only the key's prefix changes.
UPDATE session_events SET receipt_key='attachment:'||substr(receipt_key,8)
WHERE kind='observed' AND receipt_key>='driver:' AND receipt_key<'driver;';
