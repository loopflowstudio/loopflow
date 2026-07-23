-- name: add_turn_usage_samples
-- id: c6b6b00eb57a8e1c651937be4ccbf2a1
-- depends_on: 

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
