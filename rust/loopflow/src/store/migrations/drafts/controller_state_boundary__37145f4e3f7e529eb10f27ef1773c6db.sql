-- name: controller_state_boundary
-- id: 37145f4e3f7e529eb10f27ef1773c6db
-- depends_on: stable_work_state

CREATE TABLE project_controller_state (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    observation_cursor INTEGER NOT NULL CHECK (observation_cursor >= 0),
    last_state_fingerprint TEXT,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    updated_at INTEGER NOT NULL
) STRICT;

INSERT INTO project_controller_state (
    project_id, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, updated_at
)
SELECT
    id, iteration, observation_cursor, last_state_fingerprint,
    agent, provider, provider_session_id, updated_at
FROM projects;

CREATE TABLE task_controller_state (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    kickoff_flow TEXT NOT NULL,
    iterate_flow TEXT NOT NULL,
    gate_flow TEXT NOT NULL,
    lifecycle_phase TEXT NOT NULL
        CHECK (lifecycle_phase IN ('kickoff', 'iterate', 'gate')),
    phase_cursor INTEGER NOT NULL CHECK (phase_cursor >= 0),
    phase_iteration INTEGER NOT NULL CHECK (phase_iteration >= 0),
    gate_cycle INTEGER NOT NULL CHECK (gate_cycle >= 0),
    gate_proposal_json TEXT,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    updated_at INTEGER NOT NULL
) STRICT;

INSERT INTO task_controller_state (
    task_id, kickoff_flow, iterate_flow, gate_flow, lifecycle_phase,
    phase_cursor, phase_iteration, gate_cycle, gate_proposal_json,
    agent, provider, provider_session_id, updated_at
)
SELECT
    id, kickoff_flow, iterate_flow, gate_flow, lifecycle_phase,
    phase_cursor, phase_iteration, gate_cycle, gate_proposal_json,
    agent, provider, provider_session_id, updated_at
FROM tasks;

ALTER TABLE projects DROP COLUMN iteration;
ALTER TABLE projects DROP COLUMN observation_cursor;
ALTER TABLE projects DROP COLUMN last_state_fingerprint;
ALTER TABLE projects DROP COLUMN agent;
ALTER TABLE projects DROP COLUMN provider;
ALTER TABLE projects DROP COLUMN provider_session_id;

ALTER TABLE tasks DROP COLUMN kickoff_flow;
ALTER TABLE tasks DROP COLUMN iterate_flow;
ALTER TABLE tasks DROP COLUMN gate_flow;
ALTER TABLE tasks DROP COLUMN lifecycle_phase;
ALTER TABLE tasks DROP COLUMN phase_epoch;
ALTER TABLE tasks DROP COLUMN phase_cursor;
ALTER TABLE tasks DROP COLUMN phase_iteration;
ALTER TABLE tasks DROP COLUMN gate_cycle;
ALTER TABLE tasks DROP COLUMN gate_proposal_json;
ALTER TABLE tasks DROP COLUMN agent;
ALTER TABLE tasks DROP COLUMN provider;
ALTER TABLE tasks DROP COLUMN provider_session_id;
