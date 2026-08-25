-- name: obsolete_sql_lifecycle
-- id: 3a901dfdabc607b3d062fb1466831150
-- depends_on: stable_work_state

DROP TABLE sends;
DROP TABLE done_proposals;
DROP TABLE context_decisions;
DROP TABLE context_assets;
DROP TABLE turn_usage_samples;
DROP TABLE agent_turns;
DROP TABLE agent_invocations;
DROP TABLE run_liveness;
DROP TABLE home_upgrade_work;
DROP TABLE home_upgrades;

DROP INDEX idx_project_events_run;
ALTER TABLE project_events DROP COLUMN run_id;

DROP TABLE performance_evidence_authority;
DROP TABLE runs;
DROP TABLE home_runtime_generations;
DROP TABLE epochs;
