-- Every Session summary reads its latest provider transition. Those rows are
-- observations whose receipt keys name them (`provider-transition:` now,
-- `agent-session:` in released history), so a partial index over just them
-- replaces a JSON scan of the Session's whole history.
CREATE INDEX session_provider_transitions ON session_events(session_id, seq)
WHERE kind = 'observed'
    AND (receipt_key GLOB 'provider-transition:*' OR receipt_key GLOB 'agent-session:*');
