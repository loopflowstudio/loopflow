ALTER TABLE wave_runs
ADD COLUMN parent_run_id TEXT;

ALTER TABLE wave_runs
ADD COLUMN parent_pr_number BIGINT;

ALTER TABLE wave_runs
ADD COLUMN stack_position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE wave_runs
ADD COLUMN stack_group_id TEXT NOT NULL DEFAULT '';

ALTER TABLE wave_runs
ADD COLUMN stack_status INTEGER NOT NULL DEFAULT 1;

ALTER TABLE wave_runs
ADD COLUMN lineage_inferred INTEGER NOT NULL DEFAULT 0;

UPDATE wave_runs
SET stack_group_id = wave_id
WHERE stack_group_id = '';

WITH ordered AS (
    SELECT
        id,
        wave_id,
        LAG(id) OVER (PARTITION BY wave_id ORDER BY iteration, started_at, id) AS parent_id,
        ROW_NUMBER() OVER (PARTITION BY wave_id ORDER BY iteration, started_at, id) - 1 AS position
    FROM wave_runs
    WHERE run_kind = 1
)
UPDATE wave_runs
SET
    parent_run_id = CASE
        WHEN wave_runs.parent_run_id IS NULL THEN (
            SELECT ordered.parent_id
            FROM ordered
            WHERE ordered.id = wave_runs.id
        )
        ELSE wave_runs.parent_run_id
    END,
    stack_position = CASE
        WHEN wave_runs.parent_run_id IS NULL THEN COALESCE((
            SELECT ordered.position
            FROM ordered
            WHERE ordered.id = wave_runs.id
        ), stack_position)
        ELSE stack_position
    END,
    lineage_inferred = CASE
        WHEN wave_runs.parent_run_id IS NULL AND (
            SELECT ordered.parent_id
            FROM ordered
            WHERE ordered.id = wave_runs.id
        ) IS NOT NULL THEN 1
        ELSE lineage_inferred
    END
WHERE run_kind = 1;

CREATE INDEX IF NOT EXISTS idx_wave_runs_wave_id_stack_position
ON wave_runs(wave_id, stack_position);

CREATE INDEX IF NOT EXISTS idx_wave_runs_parent_run_id
ON wave_runs(parent_run_id);

CREATE TABLE IF NOT EXISTS live_pr_states (
    repo_id TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    state INTEGER NOT NULL,
    is_draft INTEGER NOT NULL DEFAULT 0,
    head_ref TEXT NOT NULL DEFAULT '',
    head_sha TEXT NOT NULL DEFAULT '',
    base_ref TEXT NOT NULL DEFAULT '',
    updated_at BIGINT NOT NULL,
    merged_at BIGINT,
    synced_at BIGINT NOT NULL,
    PRIMARY KEY (repo_id, pr_number)
);

CREATE INDEX IF NOT EXISTS idx_live_pr_states_state
ON live_pr_states(state);
