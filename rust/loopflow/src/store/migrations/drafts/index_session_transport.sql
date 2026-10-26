-- Every execution owner judges which owners have gone unviewed every few
-- minutes; without this, each judgement scanned every Session per owner.
CREATE INDEX agent_sessions_transport ON agent_sessions(transport_lf_process_id)
    WHERE transport_lf_process_id IS NOT NULL;
