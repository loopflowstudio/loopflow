-- A completed mechanical land operation records handoff, while its Flow
-- remains at that occurrence until this exact delivery is settled.
ALTER TABLE runs ADD COLUMN landing_id TEXT REFERENCES pr_landings(id);
-- name: link_landing_operation
-- id: a8c61a1c4783413b949e43db0785aa31
-- depends_on: record_run_end
