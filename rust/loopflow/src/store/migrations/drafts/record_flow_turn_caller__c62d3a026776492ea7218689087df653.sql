-- name: record_flow_turn_caller
-- id: c62d3a026776492ea7218689087df653
-- depends_on: flow_operation_history

-- Old commands retain unknown turn origin. Never assign the current turn to
-- a descendant that was launched before this evidence was recorded.
ALTER TABLE execs ADD COLUMN caller_flow_turn TEXT;
CREATE UNIQUE INDEX flow_turn_caller ON flow_events(json_extract(payload,'$.caller_token'))
WHERE kind='selected' AND json_extract(payload,'$.caller_token') IS NOT NULL;
