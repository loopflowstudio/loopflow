-- Preserve the durable Project and Task identity surrounding each agent launch.
-- Historical rows remain explicitly unattributed.

ALTER TABLE agent_launches ADD COLUMN project TEXT;
ALTER TABLE agent_launches ADD COLUMN task TEXT;

CREATE INDEX idx_agent_launches_project ON agent_launches(project, started_at);
CREATE INDEX idx_agent_launches_task ON agent_launches(task, started_at);
