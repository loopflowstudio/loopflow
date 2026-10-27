-- draft: designated_panes
-- Desktop pane layouts become three designated panes: session, shell and browser.
-- Released free trees (empty, session, shell, files, log) are not converted (Jack
-- Heart, 2026-10-25: existing layouts may reset); Desktop rebuilds the default.
-- breaks: released Desktop reads Empty, Files and Log panes, which this shape no longer writes
DELETE FROM work_pane_layouts;

-- draft: index_session_transport
-- Every execution owner judges which owners have gone unviewed every few
-- minutes; without this, each judgement scanned every Session per owner.
CREATE INDEX agent_sessions_transport ON agent_sessions(transport_lf_process_id)
    WHERE transport_lf_process_id IS NOT NULL;

-- draft: session_turn_events
-- Provider turns are a sliver of session_events, which observations dominate.
-- History groups them by turn on every Desktop refresh; covering them keeps
-- that read off the observation payload pages.
CREATE INDEX session_turn_events ON session_events(
    session_id, provider_thread, provider_turn, kind, observed_at)
    WHERE kind IN ('started','usage','completed','output');

-- Each execution owner times its own viewers; no one reads when the last left.
ALTER TABLE processes DROP COLUMN detached_at;
