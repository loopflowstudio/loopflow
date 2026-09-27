-- name: record_ask_sessions
-- id: ec6d1b7d0212a1de5be053d42b6d7721
-- depends_on: record_session_kind

-- An Ask Session stores the question its caller asked. Its selected Skill is
-- its Run's skill, its keyed retry identity is its id, and its answer is the
-- ready summary of a completed Session.
ALTER TABLE sessions ADD COLUMN request TEXT;

-- Causality only: the Run that asked. It confers no membership or authority,
-- and the caller need not have a row.
ALTER TABLE runs ADD COLUMN caller_run_id TEXT;
