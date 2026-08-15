-- name: human_flow_positions
-- id: 6f5c5c4049b353f3a1d7ad79c7b1c40f
-- depends_on:

ALTER TABLE work_flow_positions ADD COLUMN node_id TEXT;
ALTER TABLE work_flow_positions ADD COLUMN human INTEGER NOT NULL DEFAULT 0
    CHECK (
        human IN (0, 1)
        AND (human = 0 OR (node_id IS NOT NULL AND length(trim(node_id)) > 0))
    );
