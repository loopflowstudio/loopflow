CREATE TABLE waves (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE
);
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);

CREATE TABLE provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    oauth_client_id TEXT,
    expires_at INTEGER,
    login TEXT,
    updated_at INTEGER NOT NULL,
    credential_type TEXT NOT NULL,
    encrypted INTEGER NOT NULL
);

CREATE TABLE run_events (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    parent_process_id TEXT,
    seq INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    repo TEXT,
    worktree TEXT,
    wave TEXT,
    node TEXT NOT NULL CHECK (node IN ('run', 'flow', 'skill')),
    event TEXT NOT NULL CHECK (event IN ('started', 'completed', 'errored', 'escalated')),
    command TEXT,
    flow TEXT,
    skill TEXT,
    step_index INTEGER,
    error TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cache_read_tokens INTEGER,
    cost_usd REAL,
    duration_secs REAL,
    provider TEXT,
    model TEXT
);
CREATE INDEX idx_run_events_ts ON run_events(ts);
CREATE INDEX idx_run_events_run ON run_events(run_id);
CREATE INDEX idx_run_events_process ON run_events(process_id);

CREATE TABLE blob_tokens (
    sha TEXT PRIMARY KEY,
    lines INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    tokens INTEGER NOT NULL
);

CREATE TABLE bus_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL,
    byline TEXT NOT NULL,
    text TEXT NOT NULL,
    at INTEGER NOT NULL
);
CREATE INDEX idx_bus_messages_at ON bus_messages(at);

CREATE TABLE bus_cursors (
    subscriber TEXT PRIMARY KEY,
    cursor INTEGER NOT NULL
);

CREATE TABLE pm_snapshots (
    repo TEXT NOT NULL,
    wave TEXT NOT NULL,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (repo, wave)
);

CREATE TABLE trace_capture_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    required_after INTEGER NOT NULL
);
INSERT INTO trace_capture_meta (id, required_after) VALUES (1, unixepoch());

CREATE TABLE agent_launches (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
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
    conversation_event_count INTEGER NOT NULL,
    conversation_bytes INTEGER NOT NULL
);
CREATE INDEX idx_agent_launches_run ON agent_launches(run_id, started_at);
CREATE INDEX idx_agent_launches_process ON agent_launches(process_id, started_at);
CREATE INDEX idx_agent_launches_wave ON agent_launches(wave, started_at);

CREATE TABLE agent_turns (
    id TEXT PRIMARY KEY,
    launch_id TEXT NOT NULL REFERENCES agent_launches(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    provider_turn_id TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
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
    system_tokens INTEGER NOT NULL,
    task_tokens INTEGER NOT NULL,
    supplied_context_tokens INTEGER NOT NULL,
    provider_input_tokens INTEGER,
    provider_output_tokens INTEGER,
    reasoning_tokens INTEGER,
    cache_read_tokens INTEGER,
    cache_write_tokens INTEGER,
    cost_usd REAL,
    context_gather_ms INTEGER NOT NULL,
    context_render_ms INTEGER NOT NULL,
    context_persist_ms INTEGER NOT NULL,
    first_event_seq INTEGER,
    last_event_seq INTEGER,
    UNIQUE (launch_id, ordinal)
);
CREATE INDEX idx_agent_turns_launch ON agent_turns(launch_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);

CREATE TABLE context_assets (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('system', 'task')),
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    included_by TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    isolated_tokens INTEGER NOT NULL,
    attributed_tokens INTEGER NOT NULL,
    PRIMARY KEY (turn_id, position)
);
CREATE INDEX idx_context_assets_kind ON context_assets(kind);
CREATE INDEX idx_context_assets_hash ON context_assets(content_sha256);

CREATE TABLE context_decisions (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    decision TEXT NOT NULL CHECK (decision IN (
        'included', 'excluded', 'summarized', 'stat_only', 'truncated', 'deduplicated'
    )),
    reason TEXT NOT NULL,
    original_bytes INTEGER,
    original_tokens INTEGER,
    asset_position INTEGER,
    PRIMARY KEY (turn_id, position),
    FOREIGN KEY (turn_id, asset_position)
        REFERENCES context_assets(turn_id, position)
);
CREATE INDEX idx_context_decisions_decision ON context_decisions(decision);

CREATE TABLE project_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL UNIQUE,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    project_prompt_context TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    pm_snapshot_synced_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    observation_cursor INTEGER NOT NULL,
    last_state_fingerprint TEXT,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    current_directive_version INTEGER NOT NULL,
    incorporated_directive_version INTEGER NOT NULL,
    lf_bin TEXT NOT NULL,
    db_path TEXT NOT NULL,
    lf_home TEXT NOT NULL,
    abandon_requested_at INTEGER,
    abandon_reason TEXT
);
CREATE INDEX idx_project_sessions_wave_status
    ON project_sessions(wave_id, status, updated_at DESC);

CREATE TABLE task_sessions (
    id TEXT PRIMARY KEY,
    issue_id TEXT NOT NULL UNIQUE,
    issue_identifier TEXT NOT NULL UNIQUE,
    issue_title TEXT NOT NULL,
    issue_description TEXT NOT NULL,
    project_id TEXT NOT NULL,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    project_prompt_context TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    worktree TEXT NOT NULL UNIQUE,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    pr_number INTEGER,
    pr_url TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    pm_snapshot_synced_at INTEGER NOT NULL,
    pm_writeback_json TEXT NOT NULL,
    project_session_id TEXT NOT NULL REFERENCES project_sessions(id) ON DELETE RESTRICT,
    current_directive_version INTEGER NOT NULL,
    incorporated_directive_version INTEGER NOT NULL,
    lf_bin TEXT NOT NULL,
    db_path TEXT NOT NULL,
    lf_home TEXT NOT NULL,
    abandon_requested_at INTEGER,
    abandon_reason TEXT
);
CREATE INDEX idx_task_sessions_wave_status
    ON task_sessions(wave_id, status, updated_at DESC);

CREATE TABLE task_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES task_sessions(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_task_events_session ON task_events(session_id, id);

CREATE TABLE project_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES project_sessions(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_project_events_session ON project_events(session_id, id);

CREATE TABLE child_commands (
    id TEXT PRIMARY KEY,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('project', 'task')),
    session_id TEXT NOT NULL,
    source_json TEXT NOT NULL,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    claimed_by_generation INTEGER,
    accepted_at INTEGER,
    state TEXT NOT NULL,
    effect TEXT,
    error TEXT
);
CREATE INDEX idx_child_commands_pending
    ON child_commands(target_kind, session_id, state, created_at, id);

CREATE TABLE observation_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recipient_kind TEXT NOT NULL CHECK (recipient_kind IN ('wave', 'project')),
    recipient_id TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('project', 'task')),
    source_id TEXT NOT NULL,
    event_id INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    delivered_at INTEGER,
    UNIQUE(recipient_kind, recipient_id, source_kind, source_id, event_id)
);
CREATE INDEX idx_observation_outbox_pending
    ON observation_outbox(recipient_kind, recipient_id, delivered_at, id);

CREATE TABLE child_directives (
    id TEXT PRIMARY KEY,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('project', 'task')),
    target_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    source_json TEXT NOT NULL,
    command_id TEXT,
    issued_at INTEGER NOT NULL,
    applied_at INTEGER,
    incorporated_at INTEGER,
    incorporated_summary TEXT,
    UNIQUE(target_kind, target_id, version)
);
CREATE INDEX idx_child_directives_target
    ON child_directives(target_kind, target_id, version);
CREATE UNIQUE INDEX idx_child_directives_command
    ON child_directives(command_id) WHERE command_id IS NOT NULL;
