-- name: retain_named_operation_history
-- id: 3704bb0dbb1b462fa165973072c5309d
-- depends_on: retain_task_start_evidence

-- Old managed mechanical boundaries carried the step name in skill. Provider
-- loopflow establishes their kind; skill presence never establishes an agent.
-- Preserve the complete SQL observation, with no invented Exec or start time.
CREATE TEMP TABLE imported_operations AS
SELECT r.*, json_object('id',r.id,'session_id',r.session_id,'invocation_id',r.invocation_id,
    'task_id',r.task_id,'wave_id',r.wave_id,'work_source',r.work_source,
    'created_at',r.created_at,'published',r.published,'cwd',r.cwd,'skill',r.skill,
    'node',r.node,'iterations',r.iterations,'attempt',r.attempt,'provider',r.provider,
    'model',r.model,'caller_run_id',r.caller_run_id,'outcome',r.outcome,'ended_at',r.ended_at) AS evidence
FROM runs r JOIN flow_sessions f ON f.id=r.invocation_id
WHERE r.provider='loopflow' AND r.published=1
    AND r.node IS NOT NULL AND r.iterations IS NOT NULL;

INSERT INTO flow_events(flow_id,version,node,iterations,kind,payload)
SELECT r.invocation_id,CASE WHEN f.current_run_id=r.id THEN f.position_version END,
    r.node,r.iterations,'operation_started',json_object('legacy_run_id',r.id,'reserved_at',r.created_at)
FROM imported_operations r JOIN flow_sessions f ON f.id=r.invocation_id
WHERE NOT EXISTS(SELECT 1 FROM flow_events e WHERE e.kind='operation_started'
    AND e.flow_id=r.invocation_id AND json_extract(e.payload,'$.legacy_run_id')=r.id);

INSERT INTO flow_events(flow_id,version,node,iterations,kind,operation_start,outcome,observed_at,payload)
SELECT e.flow_id,e.version,e.node,e.iterations,'operation_completed',e.seq,r.outcome,r.ended_at,e.payload
FROM flow_events e JOIN imported_operations r ON r.id=json_extract(e.payload,'$.legacy_run_id')
WHERE e.kind='operation_started' AND r.outcome IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM flow_events completed WHERE completed.operation_start=e.seq);

UPDATE flow_events SET payload=json_set(payload,'$.sql',json((SELECT evidence FROM imported_operations
    WHERE id=json_extract(flow_events.payload,'$.legacy_run_id'))))
WHERE kind IN ('operation_started','operation_completed')
    AND json_extract(payload,'$.legacy_run_id') IN (SELECT id FROM imported_operations);

UPDATE flow_sessions SET operation_start=(SELECT e.seq FROM flow_events e
    WHERE e.kind='operation_started' AND e.flow_id=flow_sessions.id
        AND json_extract(e.payload,'$.legacy_run_id')=flow_sessions.current_run_id), current_run_id=NULL
WHERE current_run_id IN (SELECT id FROM imported_operations);
DROP TABLE imported_operations;
