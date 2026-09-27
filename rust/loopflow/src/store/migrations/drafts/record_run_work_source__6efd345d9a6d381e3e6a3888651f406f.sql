-- name: record_run_work_source
-- id: 6efd345d9a6d381e3e6a3888651f406f
-- depends_on: own_sessions_and_runs

-- Historical attribution has no provenance unless the import can establish it.
-- Do not relabel old inferred or declared launch evidence as inherited.
ALTER TABLE runs ADD COLUMN work_source TEXT
    CHECK (work_source IN ('declared', 'checkout', 'inherited', 'bound'));
