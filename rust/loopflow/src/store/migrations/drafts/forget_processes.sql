-- A finished Process is deleted at exit unless a row references it. Each
-- reference probe needs an index, or every exit and every prune candidate
-- scans the referencing table.
CREATE INDEX agent_sessions_agent_process ON agent_sessions(agent_process_id)
    WHERE agent_process_id IS NOT NULL;
CREATE INDEX waves_project_activation_process ON waves(project_activation_lf_process_id)
    WHERE project_activation_lf_process_id IS NOT NULL;
CREATE INDEX ci_incidents_repair_process ON ci_incidents(repair_lf_process_id)
    WHERE repair_lf_process_id IS NOT NULL;
CREATE INDEX tasks_ship_process ON tasks(ship_lf_process_id)
    WHERE ship_lf_process_id IS NOT NULL;
