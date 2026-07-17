-- Gate proposals now carry the exact PR settlement intent and reviewed head.
-- Preserve every lifecycle policy as-is; require/require/require is valid and
-- needs no policy rewrite. Existing in-flight gates have no settlement intent,
-- so they remain reviewable but cannot mechanically arm a merge until they
-- return through Iterate and publish fresh PR evidence.

UPDATE task_sessions
SET gate_proposal_json = json_set(gate_proposal_json, '$.settlement', json('null'))
WHERE gate_proposal_json IS NOT NULL
  AND json_type(gate_proposal_json, '$.settlement') IS NULL;
