-- name: project_metric_observations
-- id: 8a7dd94340bc4fb9a7836c5dc39e0c21
-- depends_on:

CREATE TABLE metric_instruments (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    metric_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    registered_at INTEGER NOT NULL,
    PRIMARY KEY (wave_id, metric_id)
);

CREATE TABLE metric_observations (
    observation_id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    metric_id TEXT NOT NULL,
    contract_revision TEXT NOT NULL,
    instrument TEXT NOT NULL,
    source_time_seconds INTEGER NOT NULL,
    source_time_nanoseconds INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    graduation_qualifying INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX metric_observations_identity_source
ON metric_observations (
    wave_id,
    metric_id,
    source_time_seconds,
    source_time_nanoseconds,
    observation_id
);

CREATE INDEX metric_observations_contract_evidence
ON metric_observations (
    wave_id,
    metric_id,
    contract_revision,
    graduation_qualifying
);
