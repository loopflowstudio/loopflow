-- Session selections are execution history, never future Task policy.
ALTER TABLE tasks DROP COLUMN agent;

-- Preserve existing personal skill preferences under their canonical target key.
INSERT OR IGNORE INTO scope_settings(scope_kind, scope_id, key, value, set_at)
SELECT scope_kind, scope_id, 'skills.' || substr(key, 14) || '.agent', value, set_at
FROM scope_settings WHERE key LIKE 'skill_agents.%';
DELETE FROM scope_settings WHERE key LIKE 'skill_agents.%';
