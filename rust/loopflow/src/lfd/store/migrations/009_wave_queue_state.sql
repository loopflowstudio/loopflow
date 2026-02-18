CREATE TABLE IF NOT EXISTS wave_queue_blocks (
    wave_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    attempted_at BIGINT NOT NULL,
    conflict_files TEXT NOT NULL DEFAULT '[]',
    error TEXT,
    PRIMARY KEY (wave_id, run_id),
    FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES wave_runs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wave_queue_blocks_wave_id
ON wave_queue_blocks(wave_id);

CREATE TABLE IF NOT EXISTS wave_pr_merge_events (
    wave_id TEXT NOT NULL,
    pr_number BIGINT NOT NULL,
    merged_at BIGINT NOT NULL,
    processed_at BIGINT NOT NULL,
    PRIMARY KEY (wave_id, pr_number, merged_at),
    FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wave_pr_merge_events_wave_id
ON wave_pr_merge_events(wave_id, processed_at);
