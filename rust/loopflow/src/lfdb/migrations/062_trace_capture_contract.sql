ALTER TABLE context_decisions RENAME TO context_decisions_061;
ALTER TABLE context_assets RENAME TO context_assets_061;
ALTER TABLE agent_turns RENAME TO agent_turns_061;
ALTER TABLE agent_launches RENAME TO agent_launches_061;

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
    context_gather_ms BIGINT NOT NULL,
    context_render_ms BIGINT NOT NULL,
    context_persist_ms BIGINT NOT NULL,
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
        'included', 'excluded', 'summarized', 'stat_only', 'truncated',
        'deduplicated'
    )),
    reason TEXT NOT NULL,
    original_bytes BIGINT,
    original_tokens BIGINT,
    asset_position BIGINT,
    PRIMARY KEY (turn_id, position),
    FOREIGN KEY (turn_id, asset_position)
        REFERENCES context_assets(turn_id, position)
);

CREATE INDEX idx_context_decisions_decision ON context_decisions(decision);

INSERT INTO agent_launches (
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
    artifact_dir, conversation_path, provider_events_path, provider_session_id,
    provider_session_path, conversation_event_count, conversation_bytes
)
SELECT
    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
    CASE
        WHEN instr(artifact_dir, '/traces/') > 0
            THEN substr(artifact_dir, instr(artifact_dir, '/traces/') + 8)
        ELSE artifact_dir
    END,
    CASE
        WHEN instr(conversation_path, '/traces/') > 0
            THEN substr(conversation_path, instr(conversation_path, '/traces/') + 8)
        ELSE conversation_path
    END,
    CASE
        WHEN provider_events_path IS NULL THEN NULL
        WHEN instr(provider_events_path, '/traces/') > 0
            THEN substr(provider_events_path, instr(provider_events_path, '/traces/') + 8)
        ELSE provider_events_path
    END,
    provider_session_id, provider_session_path, conversation_event_count,
    conversation_bytes
FROM agent_launches_061;

INSERT INTO agent_turns (
    id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status,
    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
    system_tokens, task_tokens, supplied_context_tokens, provider_input_tokens,
    provider_output_tokens, reasoning_tokens, cache_read_tokens,
    cache_write_tokens, cost_usd, context_gather_ms, context_render_ms,
    context_persist_ms, first_event_seq, last_event_seq
)
SELECT
    turn.id, turn.launch_id, turn.ordinal, turn.provider_turn_id,
    turn.started_at, turn.ended_at, turn.status, turn.input_op,
    turn.context_coverage, turn.tokenizer,
    CASE
        WHEN turn.system_prompt_path IS NULL THEN NULL
        WHEN instr(turn.system_prompt_path, '/traces/') > 0
            THEN substr(turn.system_prompt_path, instr(turn.system_prompt_path, '/traces/') + 8)
        ELSE turn.system_prompt_path
    END,
    CASE
        WHEN instr(turn.task_prompt_path, '/traces/') > 0
            THEN substr(turn.task_prompt_path, instr(turn.task_prompt_path, '/traces/') + 8)
        ELSE turn.task_prompt_path
    END,
    turn.system_tokens, turn.task_tokens, turn.supplied_context_tokens,
    turn.provider_input_tokens, turn.provider_output_tokens,
    turn.reasoning_tokens, turn.cache_read_tokens, turn.cache_write_tokens,
    turn.cost_usd, launch.context_gather_ms, launch.context_render_ms,
    launch.context_persist_ms, turn.first_event_seq, turn.last_event_seq
FROM agent_turns_061 AS turn
JOIN agent_launches_061 AS launch ON launch.id = turn.launch_id;

INSERT INTO context_assets (
    turn_id, position, channel, kind, scope, label, source_path, included_by,
    content_sha256, byte_start, byte_end, bytes, isolated_tokens,
    attributed_tokens
)
SELECT
    turn_id, position, channel,
    CASE kind
        WHEN 'loopflow' THEN 'operating_instructions'
        WHEN 'surface' THEN 'surface_instructions'
        WHEN 'structured_reply' THEN 'provider_instructions'
        WHEN 'provider_wrapper' THEN 'provider_instructions'
        WHEN 'repo_instructions' THEN 'repo_instructions'
        WHEN 'skill' THEN 'skill_instructions'
        WHEN 'direction' THEN 'direction'
        WHEN 'wave_goal' THEN 'goal'
        WHEN 'project' THEN 'goal'
        WHEN 'wave_memory' THEN 'memory'
        WHEN 'wave_chat' THEN 'chat'
        WHEN 'parent_summary' THEN 'summary'
        WHEN 'docs' THEN 'document'
        ELSE kind
    END,
    CASE
        WHEN kind IN ('structured_reply', 'provider_wrapper') THEN 'provider'
        WHEN scope = 'system' THEN 'global'
        ELSE scope
    END,
    label, source_path, included_by, content_sha256, byte_start, byte_end,
    bytes, isolated_tokens, attributed_tokens
FROM context_assets_061;

INSERT INTO context_decisions (
    turn_id, position, kind, scope, label, source_path, decision, reason,
    original_bytes, original_tokens, asset_position
)
SELECT
    decision.turn_id, decision.position,
    CASE decision.kind
        WHEN 'loopflow' THEN 'operating_instructions'
        WHEN 'surface' THEN 'surface_instructions'
        WHEN 'structured_reply' THEN 'provider_instructions'
        WHEN 'provider_wrapper' THEN 'provider_instructions'
        WHEN 'repo_instructions' THEN 'repo_instructions'
        WHEN 'skill' THEN 'skill_instructions'
        WHEN 'wave_goal' THEN 'goal'
        WHEN 'project' THEN 'goal'
        WHEN 'wave_memory' THEN 'memory'
        WHEN 'wave_chat' THEN 'chat'
        WHEN 'parent_summary' THEN 'summary'
        WHEN 'docs' THEN 'document'
        ELSE decision.kind
    END,
    COALESCE(asset.scope,
        CASE
            WHEN decision.kind IN ('structured_reply', 'provider_wrapper') THEN 'provider'
            WHEN decision.kind IN ('loopflow', 'surface') THEN 'global'
            WHEN decision.kind IN ('repo_instructions', 'docs', 'scratch', 'diff') THEN 'repo'
            WHEN decision.kind IN ('wave_goal', 'wave_memory', 'wave_chat') THEN 'wave'
            WHEN decision.kind = 'project' THEN 'project'
            WHEN decision.kind IN ('skill', 'parent_summary') THEN 'step'
            ELSE 'user'
        END
    ),
    decision.label, decision.source_path, decision.decision, decision.reason,
    decision.original_bytes, decision.original_tokens, decision.asset_position
FROM context_decisions_061 AS decision
LEFT JOIN context_assets AS asset
    ON asset.turn_id = decision.turn_id
   AND asset.position = decision.asset_position;

DROP TABLE context_decisions_061;
DROP TABLE context_assets_061;
DROP TABLE agent_turns_061;
DROP TABLE agent_launches_061;
