-- One spend grain: every usage-bearing row carries the spend attributable to
-- that boundary alone, and readers sum. Historical rows were cumulative
-- readings (each skill row repeated everything before it; the terminal row
-- repeated the whole run), so convert them in place by diffing each
-- usage-bearing row against the previous one in the same process.

-- Cumulative readings are monotone, so "the previous reading" for the
-- sometimes-NULL cost/duration columns is the running MAX over earlier rows —
-- LAG alone would reset the baseline whenever one boundary skipped a report.
CREATE TEMPORARY TABLE usage_deltas AS
SELECT
    process_id,
    seq,
    MAX(input_tokens - COALESCE(LAG(input_tokens) OVER w, 0), 0) AS input_tokens,
    MAX(output_tokens - COALESCE(LAG(output_tokens) OVER w, 0), 0) AS output_tokens,
    MAX(cache_read_tokens - COALESCE(LAG(cache_read_tokens) OVER w, 0), 0) AS cache_read_tokens,
    MAX(cost_usd - COALESCE(MAX(cost_usd) OVER w_prior, 0.0), 0.0) AS cost_usd,
    MAX(duration_secs - COALESCE(MAX(duration_secs) OVER w_prior, 0.0), 0.0) AS duration_secs
FROM run_events
WHERE input_tokens IS NOT NULL
WINDOW w AS (PARTITION BY process_id ORDER BY seq),
       w_prior AS (PARTITION BY process_id ORDER BY seq
                   ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING);

UPDATE run_events
SET input_tokens = (SELECT d.input_tokens FROM usage_deltas d
                    WHERE d.process_id = run_events.process_id AND d.seq = run_events.seq),
    output_tokens = (SELECT d.output_tokens FROM usage_deltas d
                     WHERE d.process_id = run_events.process_id AND d.seq = run_events.seq),
    cache_read_tokens = (SELECT d.cache_read_tokens FROM usage_deltas d
                         WHERE d.process_id = run_events.process_id AND d.seq = run_events.seq),
    cost_usd = CASE WHEN cost_usd IS NULL THEN NULL ELSE
                   (SELECT d.cost_usd FROM usage_deltas d
                    WHERE d.process_id = run_events.process_id AND d.seq = run_events.seq) END,
    duration_secs = CASE WHEN duration_secs IS NULL THEN NULL ELSE
                        (SELECT d.duration_secs FROM usage_deltas d
                         WHERE d.process_id = run_events.process_id AND d.seq = run_events.seq) END
WHERE input_tokens IS NOT NULL;

DROP TABLE usage_deltas;

-- Codex turn receipts recorded the thread's lifetime totals on every turn.
-- The thread survives launch boundaries (resumed sessions), so the diff
-- partitions by provider session, in time order, and reports input net of
-- cache reads (the shape Claude already reports).
CREATE TEMPORARY TABLE codex_turn_deltas AS
SELECT
    t.id,
    MAX(t.provider_input_tokens - COALESCE(LAG(t.provider_input_tokens) OVER w, 0), 0) AS gross_input,
    MAX(t.provider_output_tokens - COALESCE(LAG(t.provider_output_tokens) OVER w, 0), 0) AS output,
    MAX(t.reasoning_tokens - COALESCE(LAG(t.reasoning_tokens) OVER w, 0), 0) AS reasoning,
    MAX(t.cache_read_tokens - COALESCE(LAG(t.cache_read_tokens) OVER w, 0), 0) AS cached
FROM agent_turns t
JOIN agent_launches l ON l.id = t.launch_id
WHERE l.provider = 'codex' AND t.provider_input_tokens IS NOT NULL
WINDOW w AS (PARTITION BY COALESCE(l.provider_session_id, t.launch_id)
             ORDER BY t.started_at, t.ordinal);

UPDATE agent_turns
SET provider_input_tokens = (SELECT MAX(d.gross_input - d.cached, 0) FROM codex_turn_deltas d WHERE d.id = agent_turns.id),
    provider_total_input_tokens = (SELECT d.gross_input FROM codex_turn_deltas d WHERE d.id = agent_turns.id),
    provider_output_tokens = (SELECT d.output FROM codex_turn_deltas d WHERE d.id = agent_turns.id),
    reasoning_tokens = (SELECT d.reasoning FROM codex_turn_deltas d WHERE d.id = agent_turns.id),
    cache_read_tokens = (SELECT d.cached FROM codex_turn_deltas d WHERE d.id = agent_turns.id)
WHERE id IN (SELECT id FROM codex_turn_deltas);

DROP TABLE codex_turn_deltas;

-- Residue from integration tests that ran before test stores were isolated
-- (#908): runs attributed to throwaway tempfile checkouts. Migrations run
-- with foreign keys off, so dependents go explicitly, deepest first.
DELETE FROM run_events
WHERE repo GLOB '*/.tmp??????' OR worktree GLOB '*/.tmp??????*';
CREATE TEMPORARY TABLE junk_turns AS
SELECT t.id FROM agent_turns t
JOIN agent_launches l ON l.id = t.launch_id
WHERE l.repo GLOB '*/.tmp??????*';
DELETE FROM context_assets WHERE turn_id IN (SELECT id FROM junk_turns);
DELETE FROM context_decisions WHERE turn_id IN (SELECT id FROM junk_turns);
DELETE FROM agent_turns WHERE id IN (SELECT id FROM junk_turns);
DROP TABLE junk_turns;
DELETE FROM agent_launches WHERE repo GLOB '*/.tmp??????*';

-- Subscription window state per managed provider account: how much of the
-- plan each rate-limit window has consumed. Fed by harness stream snapshots
-- during runs and by on-demand polls; read by `lf usage` and `lf auth`.
CREATE TABLE provider_account_limits (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    window TEXT NOT NULL,
    used_percent INTEGER NOT NULL,
    resets_at INTEGER,
    plan TEXT,
    observed_at INTEGER NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('stream', 'poll')),
    PRIMARY KEY (provider, account_id, window)
);
