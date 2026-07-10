CREATE TABLE trace_capture_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    required_after BIGINT NOT NULL
);

INSERT INTO trace_capture_meta (id, required_after) VALUES (1, unixepoch());

CREATE TABLE agent_launches (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave TEXT,
    flow TEXT,
    skill TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    surface TEXT NOT NULL,
    capture_status TEXT NOT NULL CHECK (
        capture_status IN ('capturing', 'complete', 'partial', 'prompt_only')
    ),
    incomplete_reason TEXT,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'completed', 'failed', 'interrupted')
    ),
    artifact_dir TEXT NOT NULL,
    conversation_path TEXT NOT NULL,
    provider_events_path TEXT,
    provider_session_id TEXT,
    provider_session_path TEXT,
    context_gather_ms BIGINT NOT NULL,
    context_render_ms BIGINT NOT NULL,
    context_persist_ms BIGINT NOT NULL,
    conversation_event_count BIGINT NOT NULL,
    conversation_bytes BIGINT NOT NULL
);

CREATE INDEX idx_agent_launches_run ON agent_launches(run_id, started_at);
CREATE INDEX idx_agent_launches_process ON agent_launches(process_id, started_at);
CREATE INDEX idx_agent_launches_wave ON agent_launches(wave, started_at);

CREATE TABLE agent_turns (
    id TEXT PRIMARY KEY,
    launch_id TEXT NOT NULL REFERENCES agent_launches(id),
    ordinal BIGINT NOT NULL,
    provider_turn_id TEXT,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'failed', 'interrupted', 'partial')
    ),
    input_op TEXT NOT NULL CHECK (
        input_op IN ('initial', 'message', 'steer', 'queued')
    ),
    context_coverage TEXT NOT NULL CHECK (
        context_coverage IN ('assembled', 'provider_total_only', 'unknown')
    ),
    tokenizer TEXT NOT NULL,
    system_prompt_path TEXT,
    task_prompt_path TEXT NOT NULL,
    system_tokens BIGINT NOT NULL,
    task_tokens BIGINT NOT NULL,
    supplied_context_tokens BIGINT NOT NULL,
    provider_input_tokens BIGINT,
    provider_output_tokens BIGINT,
    reasoning_tokens BIGINT,
    cache_read_tokens BIGINT,
    cache_write_tokens BIGINT,
    cost_usd REAL,
    first_event_seq BIGINT,
    last_event_seq BIGINT,
    UNIQUE (launch_id, ordinal)
);

CREATE INDEX idx_agent_turns_launch ON agent_turns(launch_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);

CREATE TABLE context_assets (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id),
    position BIGINT NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('system', 'task')),
    kind TEXT NOT NULL CHECK (kind IN (
        'loopflow', 'surface', 'structured_reply', 'provider_wrapper',
        'repo_instructions', 'skill', 'direction', 'wave_goal', 'project',
        'wave_memory', 'wave_chat', 'parent_summary', 'docs', 'scratch',
        'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'system', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    included_by TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    byte_start BIGINT NOT NULL,
    byte_end BIGINT NOT NULL,
    bytes BIGINT NOT NULL,
    isolated_tokens BIGINT NOT NULL,
    attributed_tokens BIGINT NOT NULL,
    PRIMARY KEY (turn_id, position)
);

CREATE INDEX idx_context_assets_kind ON context_assets(kind);
CREATE INDEX idx_context_assets_hash ON context_assets(content_sha256);

CREATE TABLE context_decisions (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id),
    position BIGINT NOT NULL,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    source_path TEXT,
    decision TEXT NOT NULL CHECK (decision IN (
        'included', 'excluded', 'summarized', 'stat_only', 'truncated',
        'deduplicated'
    )),
    reason TEXT NOT NULL,
    original_bytes BIGINT,
    original_tokens BIGINT,
    asset_position BIGINT,
    PRIMARY KEY (turn_id, position)
);

CREATE INDEX idx_context_decisions_decision ON context_decisions(decision);

ALTER TABLE run_events DROP COLUMN context;
