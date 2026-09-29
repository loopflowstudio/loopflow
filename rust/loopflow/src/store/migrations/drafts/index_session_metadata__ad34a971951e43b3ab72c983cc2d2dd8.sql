-- name: index_session_metadata
-- id: ad34a971951e43b3ab72c983cc2d2dd8
-- depends_on: record_native_input, retain_input_sql_evidence

-- SQLite owns these projections on every insert/update and backfills existing
-- rows when building the indexes. No second writer or mutable cache exists.
-- Queries deliberately use these covering indexes: captures/history remain
-- detail evidence, and unrelated invalid historical envelopes remain readable.
CREATE INDEX flow_metadata ON flow_sessions(
    id, CASE WHEN json_valid(invocation_json) THEN CASE WHEN json_type(invocation_json,'$.flow')='text' THEN json_extract(invocation_json,'$.flow') END END, state, current_run_id, pending_session_id,
    task_id, wave_id, updated_at);
-- Pending review filtering precedes paging, including empty/deep pages.
CREATE INDEX flow_pending_review ON flow_sessions(pending_session_id, state);
CREATE INDEX session_input_membership ON session_events(
    session_id, receipt_key, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.run_id')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';
