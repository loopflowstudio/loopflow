-- A process generation is an explicit write lease, not an observation inferred
-- from Session status. Existing live bodies cannot know the new capability and
-- therefore enter the one-way legacy cutover state; reconciliation must stop
-- them before a token-aware successor starts.

ALTER TABLE task_sessions ADD COLUMN process_lease_token TEXT;
ALTER TABLE task_sessions ADD COLUMN process_group_id INTEGER;
ALTER TABLE task_sessions ADD COLUMN process_agent TEXT;
ALTER TABLE task_sessions ADD COLUMN process_provider TEXT;
ALTER TABLE task_sessions ADD COLUMN process_provider_session_id TEXT;
ALTER TABLE task_sessions ADD COLUMN process_lease_state TEXT
    CHECK (process_lease_state IN ('legacy', 'reserved', 'active', 'revoked', 'finished'));
ALTER TABLE task_sessions ADD COLUMN process_outcome_json TEXT;

UPDATE task_sessions
SET process_lease_token = 'cl_' || lower(hex(randomblob(16))),
    process_agent = agent,
    process_provider = provider,
    process_provider_session_id = provider_session_id,
    process_lease_state = CASE
        WHEN status IN ('starting', 'running') THEN 'legacy'
        ELSE 'finished'
    END,
    process_outcome_json = CASE
        WHEN status = 'completed' THEN '{"kind":"completed"}'
        WHEN status = 'failed' THEN '{"kind":"failed","reason":"body predates explicit leases"}'
        WHEN status = 'abandoned' THEN '{"kind":"interrupted","reason":"Session was abandoned before explicit leases"}'
        WHEN status NOT IN ('starting', 'running') THEN '{"kind":"legacy_stopped","reason":"body predates explicit leases"}'
        ELSE NULL
    END
WHERE process_generation IS NOT NULL;

ALTER TABLE project_sessions ADD COLUMN process_lease_token TEXT;
ALTER TABLE project_sessions ADD COLUMN process_group_id INTEGER;
ALTER TABLE project_sessions ADD COLUMN process_agent TEXT;
ALTER TABLE project_sessions ADD COLUMN process_provider TEXT;
ALTER TABLE project_sessions ADD COLUMN process_provider_session_id TEXT;
ALTER TABLE project_sessions ADD COLUMN process_lease_state TEXT
    CHECK (process_lease_state IN ('legacy', 'reserved', 'active', 'revoked', 'finished'));
ALTER TABLE project_sessions ADD COLUMN process_outcome_json TEXT;

UPDATE project_sessions
SET process_lease_token = 'cl_' || lower(hex(randomblob(16))),
    process_agent = agent,
    process_provider = provider,
    process_provider_session_id = provider_session_id,
    process_lease_state = CASE
        WHEN status IN ('starting', 'running') THEN 'legacy'
        ELSE 'finished'
    END,
    process_outcome_json = CASE
        WHEN status = 'completed' THEN '{"kind":"completed"}'
        WHEN status = 'failed' THEN '{"kind":"failed","reason":"body predates explicit leases"}'
        WHEN status = 'abandoned' THEN '{"kind":"interrupted","reason":"Session was abandoned before explicit leases"}'
        WHEN status NOT IN ('starting', 'running') THEN '{"kind":"legacy_stopped","reason":"body predates explicit leases"}'
        ELSE NULL
    END
WHERE process_generation IS NOT NULL;
