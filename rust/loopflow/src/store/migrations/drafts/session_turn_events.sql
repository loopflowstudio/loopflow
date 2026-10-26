-- Provider turns are a sliver of session_events, which observations dominate.
-- History groups them by turn on every Desktop refresh; covering them keeps
-- that read off the observation payload pages.
CREATE INDEX session_turn_events ON session_events(
    session_id, provider_thread, provider_turn, kind, observed_at)
    WHERE kind IN ('started','usage','completed','output');

-- Each execution owner times its own viewers; no one reads when the last left.
ALTER TABLE processes DROP COLUMN detached_at;
