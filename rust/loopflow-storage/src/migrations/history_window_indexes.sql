-- Session history reads (`lf session history`, Desktop activity, Wave detail)
-- start from their window rather than from every input ever recorded.
-- Captures recorded in the window: the start of an input without a manifest.
CREATE INDEX session_capture_window ON session_events(observed_at)
    WHERE kind='captured';
-- Manifest and terminal observations in the window: an input's start when its
-- manifest names one, and its end from a terminal record.
CREATE INDEX session_input_outcome_window ON session_events(observed_at)
    WHERE kind='observed' AND substr(receipt_key,-14) IN (':manifest.json',':terminal.json');
-- Provider turn events in the window: native turns, and inputs a turn
-- completion ends.
CREATE INDEX session_turn_window ON session_events(observed_at, kind, session_id, provider_thread, provider_turn)
    WHERE kind IN ('started','usage','completed','output');
-- Processes that completed in the window end the inputs they ran.
CREATE INDEX processes_completed ON processes(completed_at, id)
    WHERE completed_at IS NOT NULL;
