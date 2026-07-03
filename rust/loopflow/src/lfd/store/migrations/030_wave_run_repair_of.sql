-- Track repair lineage: when a run is a repair attempt for a failed run,
-- repair_of points to the original failed run's id.
ALTER TABLE runs ADD COLUMN repair_of TEXT REFERENCES runs(id);
