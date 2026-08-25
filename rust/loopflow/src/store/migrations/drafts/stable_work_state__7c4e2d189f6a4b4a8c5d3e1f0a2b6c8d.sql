-- name: stable_work_state
-- id: 7c4e2d189f6a4b4a8c5d3e1f0a2b6c8d
-- depends_on: generic_ask_run_claim, opaque_steer_run_provenance

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

CREATE TABLE work_flow_positions (
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    flow TEXT NOT NULL,
    step TEXT NOT NULL,
    node_id TEXT,
    human INTEGER NOT NULL CHECK (human IN (0, 1)),
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (work_kind, work_id)
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
