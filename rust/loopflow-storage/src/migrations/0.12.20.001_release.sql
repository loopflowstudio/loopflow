-- draft: task_worker_claim
-- Existing Task positions did not persist the Flow definition they claimed to
-- run, so they cannot resume automatically. Preserve their exact visible
-- boundary and Session evidence behind a restart-only blocker. Project
-- positions are discarded because Project operations are finite Runs with no
-- cursor; their domain iteration remains on the Project row.
ALTER TABLE work_flow_positions RENAME TO prior_work_flow_positions;

CREATE TABLE task_flow_positions (
    task_id TEXT PRIMARY KEY CHECK (length(trim(task_id)) > 0),
    invocation_json TEXT NOT NULL,
    flow TEXT NOT NULL,
    step TEXT NOT NULL,
    node_id TEXT,
    human INTEGER NOT NULL CHECK (human IN (0, 1)),
    session_run_id TEXT,
    ready_summary TEXT,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    position_version INTEGER NOT NULL CHECK (position_version > 0),
    worker_generation INTEGER NOT NULL CHECK (worker_generation >= 0),
    claim_json TEXT,
    failure_json TEXT,
    updated_at INTEGER NOT NULL
);

INSERT INTO task_flow_positions (
    task_id, invocation_json, flow, step, node_id, human,
    session_run_id, ready_summary, step_index, iteration, position_version,
    worker_generation, claim_json, failure_json, updated_at
)
SELECT
    work_id,
    json_object(
        'id', 'migrated-' || work_kind || '-' || work_id || '-' || updated_at,
        'flow', flow,
        'steps', json_array(json_object(
            'Skill', json_object(
                'skill', json_object('name', step),
                'policy', json_object(
                    'id', node_id,
                    'human', json(CASE WHEN human = 1 THEN 'true' ELSE 'false' END)
                ),
                'flow_parents', json_array()
            )
        ))
    ),
    flow,
    step,
    node_id,
    human,
    session_run_id,
    ready_summary,
    0,
    iteration,
    1,
    0,
    NULL,
    json_object(
        'reason', printf(
            'Flow position predates executable invocation storage (previous step %d); explicitly restart this Task to continue',
            step_index + 1
        ),
        'restart_required', json('true'),
        'observed_at', strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
    ),
    updated_at
FROM prior_work_flow_positions
WHERE work_kind = 'task';

DROP TABLE prior_work_flow_positions;

-- draft: work_domain_state
ALTER TABLE projects ADD COLUMN iteration INTEGER NOT NULL DEFAULT 0
    CHECK (iteration >= 0);
ALTER TABLE projects ADD COLUMN last_state_fingerprint TEXT;

UPDATE projects
SET iteration = (
        SELECT iteration FROM project_controller_state
        WHERE project_controller_state.project_id = projects.id
    ),
    last_state_fingerprint = (
        SELECT last_state_fingerprint FROM project_controller_state
        WHERE project_controller_state.project_id = projects.id
    )
WHERE EXISTS (
    SELECT 1 FROM project_controller_state
    WHERE project_controller_state.project_id = projects.id
);

DROP TABLE task_controller_state;
DROP TABLE project_controller_state;

-- draft: drop_project_fingerprint
ALTER TABLE projects DROP COLUMN last_state_fingerprint;
