-- One spend grain: the Turn.
--
-- `run_events` carried token/cost columns alongside `agent_turns`, so two
-- ledgers answered "what did this cost" and disagreed. They were never
-- complementary: every usage-bearing process in the exec ledger also has a
-- captured turn, and the exec ledger saw strictly less of the spend -- it only
-- observed the stream events that reached a journaled boundary, while the turn
-- capture records what the provider itself measured. Reading the exec ledger
-- therefore under-reported spend, and `lf usage` and `lf top` both read it.
--
-- The provider measures per turn, so that is the grain that gets stored. Spend
-- now lives only on `agent_turns`; readers reach it by joining
-- run_events -> agent_launches -> agent_turns. This table keeps what it alone
-- knows: process lineage, and which flow/skill boundary ran.
--
-- No spend is copied forward. The turns already hold it -- these columns were
-- the lossy duplicate, not the original.
ALTER TABLE run_events DROP COLUMN input_tokens;
ALTER TABLE run_events DROP COLUMN output_tokens;
ALTER TABLE run_events DROP COLUMN cache_read_tokens;
ALTER TABLE run_events DROP COLUMN cost_usd;
ALTER TABLE run_events DROP COLUMN duration_secs;
ALTER TABLE run_events DROP COLUMN provider;
ALTER TABLE run_events DROP COLUMN model;

-- `trace_capture_meta` held one timestamp: when trace capture became required,
-- used to scope a doctor check that reconciled the two ledgers -- "this process
-- reported spend but has no launch". With one ledger that state is
-- unrepresentable: spend hangs off a turn, and a turn hangs off a launch. The
-- check went, and this marker has no other reader.
DROP TABLE trace_capture_meta;
