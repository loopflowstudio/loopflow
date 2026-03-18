-- Track repair lineage: when a run is a repair attempt for a failed run,
-- repair_of points to the original failed run's id.
ALTER TABLE wave_runs ADD COLUMN repair_of TEXT REFERENCES wave_runs(id);
