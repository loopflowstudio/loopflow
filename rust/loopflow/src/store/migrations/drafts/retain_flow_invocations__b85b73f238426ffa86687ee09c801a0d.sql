-- name: retain_flow_invocations
-- id: b85b73f238426ffa86687ee09c801a0d
-- depends_on: drop_task_flow_step_projection

-- An invocation outlives the Task's current pointer. Preserve capture, cursor,
-- review and claim bytes; terminal transitions retain the last exact attempt.
CREATE TABLE flow_invocations (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    invocation_json TEXT NOT NULL CHECK (json_valid(invocation_json)),
    session_run_id TEXT,
    ready_summary TEXT,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    position_version INTEGER NOT NULL CHECK (position_version > 0),
    worker_generation INTEGER NOT NULL CHECK (worker_generation >= 0),
    claim_json TEXT,
    failure_json TEXT,
    updated_at INTEGER NOT NULL,
    review_json TEXT,
    state TEXT NOT NULL CHECK (state IN ('current', 'completed', 'replaced')),
    ended_at INTEGER,
    CHECK (json_type(invocation_json, '$.id') IS 'text'
        AND id = json_extract(invocation_json, '$.id')),
    CHECK ((state = 'current') = (ended_at IS NULL))
);
CREATE UNIQUE INDEX task_current_invocation ON flow_invocations(task_id)
    WHERE task_id IS NOT NULL AND state = 'current';
CREATE INDEX task_invocation_history ON flow_invocations(task_id, updated_at, id);

INSERT INTO flow_invocations (
    id, task_id, invocation_json, session_run_id, ready_summary, step_index,
    iteration, position_version, worker_generation, claim_json, failure_json,
    updated_at, review_json, state, ended_at
)
SELECT json_extract(invocation_json, '$.id'), task_id, invocation_json,
       session_run_id, ready_summary, step_index, iteration, position_version,
       worker_generation, claim_json, failure_json, updated_at, review_json,
       'current', NULL
FROM task_flow_positions;

DROP TRIGGER task_chapter_started;
DROP TABLE task_flow_positions;

-- Keep first-start evidence until Runs become the indexed execution owner.
CREATE TRIGGER task_chapter_started AFTER UPDATE OF worker_generation ON flow_invocations
WHEN NEW.state = 'current' AND NEW.task_id IS NOT NULL AND NEW.worker_generation > 0
BEGIN
    INSERT INTO task_events(task_id, kind_json, created_at)
    SELECT NEW.task_id, '{"kind":"started"}', NEW.updated_at
    WHERE NOT EXISTS(SELECT 1 FROM task_events WHERE task_id=NEW.task_id
      AND json_extract(kind_json, '$.kind')='started');
END;
