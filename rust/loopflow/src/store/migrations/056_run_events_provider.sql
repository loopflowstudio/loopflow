-- Token usage gets one home. The journal already writes tokens, cost, and
-- duration onto run_events; run_token_usage was a second store that no
-- production code ever wrote to (its only callers were tests), so lf usage
-- and GET /v0/usage aggregated an always-empty table.
--
-- It is also the wrong shape: run_id is its PRIMARY KEY and its upsert
-- overwrites, but a run_id is shared by a run and every nested `lf` it
-- spawns (see 047_run_events.sql), so the last writer would win and a wave's
-- tokens would be attributed to whichever child finished last.
--
-- provider is the one dimension run_events lacked. Historical rows keep NULL.
ALTER TABLE run_events ADD COLUMN provider TEXT;
DROP TABLE IF EXISTS run_token_usage;
