-- name: retain_input_sql_evidence
-- id: 08eb814a9a324ef38c09ed1c7958f65e
-- depends_on: retain_named_operation_history

-- The input catalog retains immutable historical source bytes, including inputs
-- whose conversation or operation kind is unknown. This is import evidence, not
-- an execution owner: new launches never write an old SQL lifecycle here.
CREATE TABLE retained_inputs (
    input_id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT REFERENCES agent_sessions(id),
    caller_input_id TEXT,
    imported_sql TEXT CHECK(imported_sql IS NULL OR
        (json_valid(imported_sql) AND json_extract(imported_sql,'$.id') IS input_id)),
    historical_task_id TEXT GENERATED ALWAYS AS (json_extract(imported_sql,'$.task_id')) VIRTUAL REFERENCES tasks(id),
    historical_wave_id TEXT GENERATED ALWAYS AS (json_extract(imported_sql,'$.wave_id')) VIRTUAL REFERENCES waves(id),
    historical_flow_id TEXT GENERATED ALWAYS AS (json_extract(imported_sql,'$.invocation_id')) VIRTUAL REFERENCES flow_sessions(id),
    historical_session_id TEXT GENERATED ALWAYS AS (json_extract(imported_sql,'$.session_id')) VIRTUAL REFERENCES agent_sessions(id)
);
INSERT INTO retained_inputs(input_id,session_id,caller_input_id)
SELECT input_id,session_id,caller_input_id FROM agent_session_inputs;
INSERT INTO retained_inputs(input_id,session_id,caller_input_id,imported_sql)
SELECT r.id,r.session_id,r.caller_run_id,
    json_object('id',r.id,'session_id',r.session_id,'invocation_id',r.invocation_id,
        'task_id',r.task_id,'wave_id',r.wave_id,'work_source',r.work_source,
        'created_at',r.created_at,'published',r.published,'cwd',r.cwd,'skill',r.skill,
        'node',r.node,'iterations',r.iterations,'attempt',r.attempt,'provider',r.provider,
        'model',r.model,'caller_run_id',r.caller_run_id,'outcome',r.outcome,'ended_at',r.ended_at)
FROM runs r WHERE true
ON CONFLICT(input_id) DO UPDATE SET imported_sql=excluded.imported_sql;
DROP TABLE agent_session_inputs;
ALTER TABLE retained_inputs RENAME TO agent_session_inputs;
CREATE INDEX agent_session_input_history ON agent_session_inputs(session_id,input_id);
CREATE INDEX historical_input_task ON agent_session_inputs(historical_task_id) WHERE imported_sql IS NOT NULL;
CREATE INDEX historical_input_flow ON agent_session_inputs(historical_flow_id) WHERE imported_sql IS NOT NULL;
CREATE TRIGGER retain_imported_input BEFORE UPDATE OF imported_sql ON agent_session_inputs
WHEN OLD.imported_sql IS NOT NULL AND NEW.imported_sql IS NOT OLD.imported_sql
BEGIN SELECT RAISE(ABORT,'Historical input SQL evidence is immutable'); END;
CREATE TRIGGER retain_historical_input BEFORE DELETE ON agent_session_inputs
WHEN OLD.imported_sql IS NOT NULL
BEGIN SELECT RAISE(ABORT,'Historical input SQL evidence must be retained'); END;

DROP TRIGGER validate_invocation_run_parents;
CREATE TRIGGER validate_flow_input_parents BEFORE UPDATE OF task_id ON "flow_sessions"
WHEN EXISTS (SELECT 1 FROM agent_session_inputs WHERE historical_flow_id=NEW.id AND historical_task_id IS NOT NEW.task_id)
BEGIN SELECT RAISE(ABORT, 'FlowSession would change historical input ancestry'); END;

DROP TRIGGER validate_project_run_parents;
CREATE TRIGGER validate_project_input_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM agent_session_inputs r JOIN tasks t ON t.id=r.historical_task_id
    WHERE t.project_id=NEW.id AND r.historical_wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'Project would change historical input ancestry'); END;

DROP TRIGGER validate_task_run_parents;
CREATE TRIGGER validate_task_input_parents BEFORE UPDATE OF project_id ON tasks
WHEN EXISTS (SELECT 1 FROM agent_session_inputs r JOIN projects p ON p.id=NEW.project_id
    WHERE r.historical_task_id=NEW.id AND r.historical_wave_id IS NOT p.wave_id)
BEGIN SELECT RAISE(ABORT, 'Task would change historical input ancestry'); END;

DROP TRIGGER retain_task_agent_turn;
CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM agent_session_inputs WHERE historical_task_id=OLD.task_id)
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;

DROP TRIGGER validate_task_started_update;
CREATE TRIGGER validate_task_started_update BEFORE UPDATE OF started_at ON tasks BEGIN
    SELECT CASE WHEN OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at
        THEN RAISE(ABORT,'Task first assignment time cannot change') END;
    SELECT CASE WHEN (NEW.started_at IS NOT NULL) != (
        EXISTS(SELECT 1 FROM agent_session_inputs WHERE historical_task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=NEW.id) OR
        EXISTS(SELECT 1 FROM flow_sessions f JOIN flow_events e ON e.flow_id=f.id
            WHERE f.task_id=NEW.id AND e.kind='operation_started') OR
        EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=NEW.id
            AND json_extract(e.kind_json,'$.kind')='started')
    ) THEN RAISE(ABORT,'Started requires recorded Task work') END;
END;

DROP TABLE runs;
