-- draft: generic_ask_run_claim
CREATE TABLE ask_exchanges_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    origin_work_kind TEXT NOT NULL CHECK (
        origin_work_kind IN ('wave', 'project', 'task')
    ),
    origin_work_id TEXT NOT NULL CHECK (length(trim(origin_work_id)) > 0),
    source_run_id TEXT,
    origin_home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    origin_cwd TEXT NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('user', 'parent')),
    target_work_kind TEXT CHECK (
        target_work_kind IN ('wave', 'project', 'task')
    ),
    target_work_id TEXT,
    request_kind TEXT NOT NULL CHECK (request_kind IN ('intervention', 'flow_step')),
    request_prompt TEXT,
    request_flow TEXT,
    request_node_id TEXT,
    request_skill TEXT,
    request_iteration INTEGER CHECK (request_iteration >= 0),
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'claimed', 'resolved', 'declined', 'cancelled')
    ),
    active_run_id TEXT,
    ready_at INTEGER,
    presented_at INTEGER,
    result_kind TEXT CHECK (result_kind IN ('resolved', 'declined', 'cancelled')),
    result_text TEXT,
    terminal_author_kind TEXT CHECK (terminal_author_kind IN ('user', 'run')),
    terminal_author_id TEXT,
    asked_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (target_kind = 'user'
         AND target_work_kind IS NULL AND target_work_id IS NULL)
        OR
        (target_kind = 'parent'
         AND target_work_kind IS NOT NULL AND target_work_id IS NOT NULL
         AND length(trim(target_work_id)) > 0)
    ),
    CHECK (
        (request_kind = 'intervention'
         AND request_prompt IS NOT NULL AND length(trim(request_prompt)) > 0
         AND request_flow IS NULL AND request_node_id IS NULL
         AND request_skill IS NULL AND request_iteration IS NULL)
        OR
        (request_kind = 'flow_step' AND request_prompt IS NULL
         AND request_flow IS NOT NULL AND length(trim(request_flow)) > 0
         AND request_node_id IS NOT NULL AND length(trim(request_node_id)) > 0
         AND request_skill IS NOT NULL AND length(trim(request_skill)) > 0
         AND request_iteration IS NOT NULL)
    ),
    CHECK (
        (state = 'queued' AND active_run_id IS NULL
         AND ready_at IS NULL AND presented_at IS NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state = 'claimed' AND active_run_id IS NOT NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state IN ('resolved', 'declined', 'cancelled')
         AND active_run_id IS NULL
         AND result_kind = state AND result_text IS NOT NULL
         AND length(trim(result_text)) > 0 AND terminal_at IS NOT NULL)
    ),
    CHECK (presented_at IS NULL OR ready_at IS NOT NULL),
    CHECK (
        (terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'user' AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'run' AND terminal_author_id IS NOT NULL
         AND length(trim(terminal_author_id)) > 0)
    ),
    CHECK (
        (state IN ('queued', 'claimed')
         AND terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (state IN ('resolved', 'declined')
         AND terminal_author_kind IS NOT NULL)
        OR state = 'cancelled'
    )
);

INSERT INTO ask_exchanges_next (
    id, epoch_id, origin_work_kind, origin_work_id, source_run_id,
    origin_home_id, origin_cwd, target_kind, target_work_kind, target_work_id,
    request_kind, request_prompt, request_flow, request_node_id,
    request_skill, request_iteration, state, active_run_id, ready_at,
    presented_at, result_kind, result_text, terminal_author_kind,
    terminal_author_id, asked_at, terminal_at
)
SELECT
    id, epoch_id, origin_work_kind, origin_work_id, origin_run_id,
    origin_home_id, origin_cwd, target_kind, target_work_kind, target_work_id,
    request_kind, request_prompt, request_flow, request_node_id,
    request_skill, request_iteration,
    CASE WHEN state = 'claimed' THEN 'queued' ELSE state END,
    NULL, NULL, NULL, result_kind, result_text, terminal_author_kind,
    terminal_author_id, asked_at, terminal_at
FROM ask_exchanges;

-- The generic Run record is now the answering harness record. Retaining this
-- back-reference would keep the parallel SQL Invocation lifecycle alive.
UPDATE agent_invocations
SET answer_ask_id = NULL, ask_ready_at = NULL, ask_presented_at = NULL
WHERE answer_ask_id IS NOT NULL;

DROP TABLE ask_exchanges;
ALTER TABLE ask_exchanges_next RENAME TO ask_exchanges;

CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(target_work_kind, target_work_id, asked_at)
    WHERE target_kind = 'parent' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE target_kind = 'user' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_epoch_pending
    ON ask_exchanges(epoch_id, asked_at)
    WHERE state IN ('queued', 'claimed');

-- draft: opaque_steer_run_provenance
CREATE TABLE steers_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'run')),
    author_run_id TEXT,
    text TEXT NOT NULL CHECK (length(trim(text)) > 0),
    issued_at INTEGER NOT NULL,
    CHECK ((author_kind = 'user') = (author_run_id IS NULL)),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);

INSERT INTO steers_next (
    id, epoch_id, rev, author_kind, author_run_id, text, issued_at
)
SELECT id, epoch_id, rev, author_kind, author_run_id, text, issued_at
FROM steers;

DROP TABLE steers;
ALTER TABLE steers_next RENAME TO steers;
CREATE INDEX idx_steers_epoch_revision ON steers(epoch_id, rev);

-- draft: stable_work_state
ALTER TABLE waves ADD COLUMN work_state TEXT NOT NULL DEFAULT 'ready'
    CHECK (work_state IN ('ready', 'done', 'abandoned'));
ALTER TABLE waves ADD COLUMN work_terminal_at INTEGER;
ALTER TABLE projects ADD COLUMN work_state TEXT NOT NULL DEFAULT 'ready'
    CHECK (work_state IN ('ready', 'done', 'abandoned'));
ALTER TABLE projects ADD COLUMN work_terminal_at INTEGER;
ALTER TABLE tasks ADD COLUMN work_state TEXT NOT NULL DEFAULT 'ready'
    CHECK (work_state IN ('ready', 'done', 'abandoned'));
ALTER TABLE tasks ADD COLUMN work_terminal_at INTEGER;

DROP TABLE ask_linear_comment_outbox;
DROP TABLE ask_exchanges;
DROP TABLE work_flow_positions;
DROP TABLE steers;
DROP TABLE tool_responses;
DROP TABLE waits;
DROP TABLE work_truth;
DROP TABLE epoch_revisions;

CREATE TABLE steers (
    id TEXT PRIMARY KEY,
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'run')),
    author_run_id TEXT,
    text TEXT NOT NULL CHECK (length(trim(text)) > 0),
    issued_at INTEGER NOT NULL,
    CHECK ((author_kind = 'user') = (author_run_id IS NULL)),
    UNIQUE (work_kind, work_id, sequence)
);
CREATE INDEX idx_steers_work_time
    ON steers(work_kind, work_id, issued_at, sequence);

CREATE TABLE tool_responses (
    id TEXT PRIMARY KEY,
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    request_id TEXT NOT NULL,
    choice TEXT NOT NULL CHECK (length(trim(choice)) > 0),
    responded_at INTEGER NOT NULL,
    UNIQUE (work_kind, work_id, request_id)
);

CREATE TABLE ask_exchanges (
    id TEXT PRIMARY KEY,
    origin_work_kind TEXT NOT NULL CHECK (origin_work_kind IN ('wave', 'project', 'task')),
    origin_work_id TEXT NOT NULL CHECK (length(trim(origin_work_id)) > 0),
    source_run_id TEXT,
    origin_home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    origin_cwd TEXT NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('user', 'parent')),
    target_work_kind TEXT CHECK (target_work_kind IN ('wave', 'project', 'task')),
    target_work_id TEXT,
    request_kind TEXT NOT NULL CHECK (request_kind IN ('intervention', 'flow_step')),
    request_prompt TEXT,
    request_flow TEXT,
    request_node_id TEXT,
    request_skill TEXT,
    request_iteration INTEGER CHECK (request_iteration >= 0),
    state TEXT NOT NULL CHECK (state IN ('queued', 'claimed', 'resolved', 'declined', 'cancelled')),
    active_run_id TEXT,
    ready_at INTEGER,
    presented_at INTEGER,
    result_kind TEXT CHECK (result_kind IN ('resolved', 'declined', 'cancelled')),
    result_text TEXT,
    terminal_author_kind TEXT CHECK (terminal_author_kind IN ('user', 'run')),
    terminal_author_id TEXT,
    asked_at INTEGER NOT NULL,
    terminal_at INTEGER
);
CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(target_work_kind, target_work_id, asked_at)
    WHERE target_kind='parent' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE target_kind='user' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_origin
    ON ask_exchanges(origin_work_kind, origin_work_id, asked_at);

CREATE TABLE ask_linear_comment_outbox (
    ask_id TEXT NOT NULL REFERENCES ask_exchanges(id) ON DELETE RESTRICT,
    transition TEXT NOT NULL CHECK (transition IN ('ask', 'answer')),
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    issue_id TEXT NOT NULL CHECK (length(trim(issue_id)) > 0),
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    created_at INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    attempt_started_at INTEGER,
    last_error TEXT,
    linear_comment_id TEXT,
    delivered_at INTEGER,
    PRIMARY KEY (ask_id, transition)
);
CREATE INDEX idx_ask_linear_comment_outbox_pending
    ON ask_linear_comment_outbox(delivered_at, created_at, ask_id, transition);

-- draft: controller_state_boundary
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

-- draft: obsolete_sql_lifecycle
DROP TABLE sends;
DROP TABLE done_proposals;
DROP TABLE context_decisions;
DROP TABLE context_assets;
DROP TABLE turn_usage_samples;
DROP TABLE agent_turns;
DROP TABLE agent_invocations;
DROP TABLE run_liveness;
DROP TABLE home_upgrade_work;
DROP TABLE home_upgrades;

DROP INDEX idx_project_events_run;
ALTER TABLE project_events DROP COLUMN run_id;

DROP TABLE performance_evidence_authority;
DROP TABLE runs;
DROP TABLE home_runtime_generations;
DROP TABLE epochs;
