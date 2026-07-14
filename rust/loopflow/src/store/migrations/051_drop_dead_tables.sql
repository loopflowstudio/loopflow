-- Two tables that never earned their keep:
--
-- secrets_provider_config (from 031) has zero references in all of src/ --
-- nothing ever read or wrote it.
--
-- wave_pr_merge_events (from 009, recreated in 011) was insert-only via
-- record_merge_event with no SELECT anywhere; the writer and its only callers
-- lived in test code. That writer and the QueueMergeEvent type go with it.
--
-- IF EXISTS keeps the drops convergent on every history.
DROP TABLE IF EXISTS secrets_provider_config;
DROP TABLE IF EXISTS wave_pr_merge_events;
