-- A completed mechanical land operation records handoff, while its Flow
-- remains at that occurrence until this exact delivery is settled.
ALTER TABLE flow_events ADD COLUMN landing_id TEXT REFERENCES pr_landings(id);
-- depends_on: session_ownership
