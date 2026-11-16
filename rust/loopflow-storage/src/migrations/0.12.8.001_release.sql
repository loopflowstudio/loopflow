-- draft: add_turn_usage_samples
-- A Turn owns lifecycle and context. Provider usage changes during the Turn,
-- so persist it at its own observation grain instead of rewriting the Turn.
CREATE TEMP TABLE turn_usage_backfill AS
SELECT
    t.id AS turn_id,
    COALESCE(t.ended_at, t.started_at) AS observed_at,
    t.status != 'running' AS final_receipt,
    t.provider_input_tokens AS input_tokens,
    t.provider_total_input_tokens AS total_input_tokens,
    t.peak_input_tokens,
    t.context_window_tokens,
    t.provider_output_tokens AS output_tokens,
    t.reasoning_tokens,
    t.cache_read_tokens,
    t.cache_write_tokens,
    l.model,
    t.cost_usd
FROM agent_turns t
JOIN agent_invocations l ON l.id = t.invocation_id
WHERE t.provider_input_tokens IS NOT NULL
   OR t.provider_total_input_tokens IS NOT NULL
   OR t.peak_input_tokens IS NOT NULL
   OR t.context_window_tokens IS NOT NULL
   OR t.provider_output_tokens IS NOT NULL
   OR t.reasoning_tokens IS NOT NULL
   OR t.cache_read_tokens IS NOT NULL
   OR t.cache_write_tokens IS NOT NULL
   OR t.cost_usd IS NOT NULL;

CREATE TABLE agent_turns_next (
    id TEXT PRIMARY KEY,
    invocation_id TEXT NOT NULL REFERENCES agent_invocations(id) ON DELETE CASCADE,
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
    context_gather_ms INTEGER NOT NULL,
    context_render_ms INTEGER NOT NULL,
    context_persist_ms INTEGER NOT NULL,
    first_event_seq INTEGER,
    last_event_seq INTEGER,
    root_output TEXT,
    epoch_id TEXT,
    basis_rev INTEGER,
    CHECK ((epoch_id IS NULL) = (basis_rev IS NULL)),
    UNIQUE (invocation_id, ordinal),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);

INSERT INTO agent_turns_next (
    id, invocation_id, ordinal, provider_turn_id, started_at, ended_at, status,
    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
    system_tokens, task_tokens, supplied_context_tokens, context_gather_ms,
    context_render_ms, context_persist_ms, first_event_seq, last_event_seq,
    root_output, epoch_id, basis_rev
)
SELECT
    id, invocation_id, ordinal, provider_turn_id, started_at, ended_at, status,
    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
    system_tokens, task_tokens, supplied_context_tokens, context_gather_ms,
    context_render_ms, context_persist_ms, first_event_seq, last_event_seq,
    root_output, epoch_id, basis_rev
FROM agent_turns;

DROP TABLE agent_turns;
ALTER TABLE agent_turns_next RENAME TO agent_turns;
CREATE INDEX idx_agent_turns_invocation
    ON agent_turns(invocation_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);
CREATE INDEX idx_agent_turns_epoch_basis
    ON agent_turns(epoch_id, basis_rev, status);

-- Cumulative checkpoints are provider-relative to one Turn. A same-second
-- update replaces the prior observation, bounding write volume without losing
-- a measurable interval. Missing counters remain NULL; missing is never zero.
CREATE TABLE turn_usage_samples (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    observed_at INTEGER NOT NULL,
    final_receipt INTEGER NOT NULL CHECK (final_receipt IN (0, 1)),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    total_input_tokens INTEGER CHECK (
        total_input_tokens IS NULL OR total_input_tokens >= 0
    ),
    peak_input_tokens INTEGER CHECK (
        peak_input_tokens IS NULL OR peak_input_tokens >= 0
    ),
    context_window_tokens INTEGER CHECK (
        context_window_tokens IS NULL OR context_window_tokens > 0
    ),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    reasoning_tokens INTEGER CHECK (
        reasoning_tokens IS NULL OR reasoning_tokens >= 0
    ),
    cache_read_tokens INTEGER CHECK (
        cache_read_tokens IS NULL OR cache_read_tokens >= 0
    ),
    cache_write_tokens INTEGER CHECK (
        cache_write_tokens IS NULL OR cache_write_tokens >= 0
    ),
    model TEXT,
    cost_usd REAL CHECK (cost_usd IS NULL OR cost_usd >= 0),
    PRIMARY KEY (turn_id, observed_at),
    CHECK (
        reasoning_tokens IS NULL OR output_tokens IS NULL
        OR reasoning_tokens <= output_tokens
    )
);
CREATE INDEX idx_turn_usage_samples_observed
    ON turn_usage_samples(observed_at, turn_id);

INSERT INTO turn_usage_samples (
    turn_id, observed_at, final_receipt, input_tokens, total_input_tokens,
    peak_input_tokens, context_window_tokens, output_tokens, reasoning_tokens,
    cache_read_tokens, cache_write_tokens, model, cost_usd
)
SELECT
    turn_id, observed_at, final_receipt, input_tokens, total_input_tokens,
    peak_input_tokens, context_window_tokens, output_tokens, reasoning_tokens,
    cache_read_tokens, cache_write_tokens, model, cost_usd
FROM turn_usage_backfill;

DROP TABLE turn_usage_backfill;

-- draft: durable_ask_invocations
-- Replace Turn-local Ask/Answer exchanges with durable requested sessions.
-- Keeping the table name avoids rewriting the Linear outbox and historical
-- AgentInvocation foreign keys; every row is nevertheless rebuilt into the
-- new Ask shape.
CREATE TABLE ask_exchanges_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    origin_work_kind TEXT NOT NULL CHECK (
        origin_work_kind IN ('wave', 'project', 'task')
    ),
    origin_work_id TEXT NOT NULL CHECK (length(trim(origin_work_id)) > 0),
    origin_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    origin_turn_id TEXT REFERENCES agent_turns(id) ON DELETE RESTRICT,
    origin_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
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
    active_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
    result_kind TEXT CHECK (result_kind IN ('resolved', 'declined', 'cancelled')),
    result_text TEXT,
    terminal_author_kind TEXT CHECK (terminal_author_kind IN ('user', 'run')),
    terminal_author_id TEXT,
    asked_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (origin_turn_id IS NULL AND origin_invocation_id IS NULL)
        OR
        (origin_turn_id IS NOT NULL AND origin_invocation_id IS NOT NULL)
    ),
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
        (state = 'queued' AND active_invocation_id IS NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state = 'claimed' AND active_invocation_id IS NOT NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state IN ('resolved', 'declined', 'cancelled')
         AND active_invocation_id IS NULL
         AND result_kind = state AND result_text IS NOT NULL
         AND length(trim(result_text)) > 0 AND terminal_at IS NOT NULL)
    ),
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
    id, epoch_id, origin_work_kind, origin_work_id, origin_run_id,
    origin_turn_id, origin_invocation_id, origin_home_id, origin_cwd,
    target_kind, target_work_kind, target_work_id,
    request_kind, request_prompt, request_flow, request_node_id,
    request_skill, request_iteration, state, active_invocation_id,
    result_kind, result_text, terminal_author_kind, terminal_author_id,
    asked_at, terminal_at
)
SELECT
    a.id,
    e.id,
    CASE
        WHEN e.wave_id IS NOT NULL THEN 'wave'
        WHEN e.project_id IS NOT NULL THEN 'project'
        ELSE 'task'
    END,
    COALESCE(e.wave_id, e.project_id, e.task_id),
    r.id,
    a.turn_id,
    i.id,
    r.home_id,
    COALESCE(r.cwd, ''),
    a.route_kind,
    a.route_work_kind,
    a.route_work_id,
    'intervention',
    a.question,
    NULL,
    NULL,
    NULL,
    NULL,
    CASE WHEN a.answered_at IS NULL THEN 'queued' ELSE 'resolved' END,
    NULL,
    CASE WHEN a.answered_at IS NULL THEN NULL ELSE 'resolved' END,
    a.answer_text,
    a.answer_author_kind,
    a.answer_author_id,
    a.asked_at,
    a.answered_at
FROM ask_exchanges a
JOIN agent_turns t ON t.id = a.turn_id
JOIN agent_invocations i ON i.id = t.invocation_id
JOIN runs r ON r.id = i.supervising_run_id
JOIN epochs e ON e.id = t.epoch_id;

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

-- An Ask session is its ordinary AgentInvocation. These columns hold the two
-- presentation facts that do not already exist on that row. Legacy Answer
-- Invocations remain ordinary historical Invocations, not Ask sessions.
UPDATE agent_invocations SET answer_ask_id=NULL WHERE answer_ask_id IS NOT NULL;

ALTER TABLE agent_invocations ADD COLUMN ask_ready_at INTEGER;
ALTER TABLE agent_invocations ADD COLUMN ask_presented_at INTEGER;

-- draft: human_flow_positions
ALTER TABLE work_flow_positions ADD COLUMN node_id TEXT;
ALTER TABLE work_flow_positions ADD COLUMN human INTEGER NOT NULL DEFAULT 0
    CHECK (
        human IN (0, 1)
        AND (human = 0 OR (node_id IS NOT NULL AND length(trim(node_id)) > 0))
    );
