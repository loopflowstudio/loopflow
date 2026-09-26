-- name: drop_task_flow_step_projection
-- id: 3ddf67770302639aa5d3833354d71645
-- depends_on: 

-- The captured definition and cursor select the current step and its policy.
-- Keep the capture, review feedback, exact worker claim and historical cursor
-- bytes unchanged; only their redundant display projection leaves the table.
ALTER TABLE task_flow_positions DROP COLUMN flow;
ALTER TABLE task_flow_positions DROP COLUMN step;
ALTER TABLE task_flow_positions DROP COLUMN node_id;
ALTER TABLE task_flow_positions DROP COLUMN human;
