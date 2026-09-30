-- name: fold_flow_passes
-- id: 7ab1cd6819ce77acd75bda26b3b2de0c
-- depends_on: record_flow_repository

-- Preserve original rows before moving the live position and history. No event
-- sequence, payload, native completion, or capture is rewritten.
CREATE TEMP TABLE folded_flows AS
WITH RECURSIVE tree(id,root,depth) AS (
    SELECT id,id,0 FROM flow_sessions WHERE parent_id IS NULL
    UNION ALL SELECT f.id,t.root,t.depth+1 FROM flow_sessions f JOIN tree t ON f.parent_id=t.id
) SELECT * FROM tree;

DROP TRIGGER validate_flow_parent;
DROP TRIGGER retain_flow_parent;
DROP VIEW managed_flows;
DROP INDEX flow_current_child;
DROP INDEX flow_parent;

-- The archive retains original selectors; its FK resolves to the surviving
-- owner without altering the immutable source payload.
DROP TRIGGER preserve_import_evidence_update;
DROP TRIGGER validate_flow_input_parents;
DROP INDEX import_evidence_flow;
ALTER TABLE import_evidence DROP COLUMN historical_flow_id;
ALTER TABLE import_evidence ADD COLUMN resolved_flow_id TEXT;
ALTER TABLE import_evidence ADD COLUMN historical_flow_id TEXT
    GENERATED ALWAYS AS (COALESCE(resolved_flow_id,json_extract(payload,'$.invocation_id')))
    VIRTUAL REFERENCES flow_sessions(id);
UPDATE import_evidence SET resolved_flow_id=(SELECT root FROM folded_flows WHERE id=json_extract(payload,'$.invocation_id'))
WHERE json_extract(payload,'$.invocation_id') IN (SELECT id FROM folded_flows WHERE id!=root);
CREATE INDEX import_evidence_flow ON import_evidence(historical_flow_id);
INSERT INTO import_evidence(source,selector,payload,resolved_flow_id)
SELECT 'flow_pass',f.id,json_object(
    'id',f.id,
    'task_id',f.task_id,
    'invocation_json',f.invocation_json,
    'historical_session_run_id',f.historical_session_run_id,
    'historical_ready_summary',f.historical_ready_summary,
    'step_index',f.step_index,
    'iteration',f.iteration,
    'position_version',f.position_version,
    'worker_generation',f.worker_generation,
    'claim_json',f.claim_json,
    'failure_json',f.failure_json,
    'updated_at',f.updated_at,
    'review_json',f.review_json,
    'state',f.state,
    'ended_at',f.ended_at,
    'pending_session_id',f.pending_session_id,
    'cwd',f.cwd,
    'message',f.message,
    'model',f.model,
    'wave_id',f.wave_id,
    'selected_start',f.selected_start,
    'operation_start',f.operation_start,
    'parent_id',f.parent_id,
    'current_capture',f.current_capture,
    'unbound_repo',f.unbound_repo),m.root
FROM flow_sessions f JOIN folded_flows m ON m.id=f.id
WHERE m.root IN (SELECT root FROM folded_flows WHERE id!=root);
CREATE TRIGGER preserve_import_evidence_update BEFORE UPDATE ON import_evidence
BEGIN SELECT RAISE(ABORT,'Historical SQL evidence is immutable'); END;
CREATE TRIGGER validate_flow_input_parents BEFORE UPDATE OF task_id ON flow_sessions
WHEN EXISTS(SELECT 1 FROM import_evidence r WHERE r.historical_flow_id=NEW.id AND r.historical_task_id IS NOT NEW.task_id)
BEGIN SELECT RAISE(ABORT,'Flow would change historical input ancestry'); END;

DROP TRIGGER validate_conversation_ancestry_update;
UPDATE agent_sessions SET flow_session_id=(SELECT root FROM folded_flows WHERE id=flow_session_id)
WHERE flow_session_id IN (SELECT id FROM folded_flows WHERE id!=root);
CREATE TRIGGER validate_conversation_ancestry_update
BEFORE UPDATE OF task_id,wave_id,flow_session_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT,'AgentSession Task and Wave disagree') END;
    SELECT CASE WHEN NEW.flow_session_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM flow_sessions f WHERE f.id=NEW.flow_session_id
        AND f.task_id IS NEW.task_id AND (f.wave_id IS NULL OR f.wave_id IS NEW.wave_id)
    ) THEN RAISE(ABORT,'AgentSession and FlowSession ancestry disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'AgentSession binding is permanent') END;
    SELECT CASE WHEN OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        THEN RAISE(ABORT,'AgentSession Wave is permanent') END;
    SELECT CASE WHEN OLD.flow_session_id IS NOT NULL AND NEW.flow_session_id IS NOT OLD.flow_session_id
        THEN RAISE(ABORT,'AgentSession Flow membership is permanent') END;
END;

UPDATE flow_events SET flow_id=(SELECT root FROM folded_flows WHERE id=flow_id)
WHERE flow_id IN (SELECT id FROM folded_flows WHERE id!=root);

-- Current children contain the complete cursor, including every return counter.
-- Completed siblings contribute history only and must never replace live state.
CREATE TEMP TABLE active_passes AS
SELECT m.root,f.* FROM folded_flows m JOIN flow_sessions f ON f.id=m.id
WHERE f.state='current' AND m.id!=m.root
AND NOT EXISTS(SELECT 1 FROM flow_sessions c WHERE c.parent_id=f.id AND c.state='current');
UPDATE flow_sessions SET (
    review_json,step_index,iteration,position_version,worker_generation,
    claim_json,failure_json,updated_at,pending_session_id,current_capture,
    selected_start,operation_start,historical_session_run_id,historical_ready_summary
)=(SELECT review_json,step_index,iteration,position_version,worker_generation,
    CASE WHEN claim_json IS NOT NULL THEN json_set(claim_json,'$.invocation_id',root) END,
    failure_json,updated_at,pending_session_id,current_capture,selected_start,operation_start,
    historical_session_run_id,historical_ready_summary
    FROM active_passes WHERE root=flow_sessions.id)
WHERE id IN (SELECT root FROM active_passes);
UPDATE tasks SET current_invocation_id=(SELECT root FROM folded_flows WHERE id=current_invocation_id)
WHERE current_invocation_id IN (SELECT id FROM folded_flows WHERE id!=root);
DELETE FROM flow_sessions WHERE id IN (SELECT id FROM folded_flows WHERE id!=root);
ALTER TABLE flow_sessions DROP COLUMN parent_id;
DROP TABLE active_passes;
DROP TABLE folded_flows;
