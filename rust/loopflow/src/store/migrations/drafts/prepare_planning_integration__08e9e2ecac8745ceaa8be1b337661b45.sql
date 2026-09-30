-- name: prepare_planning_integration
-- id: 08e9e2ecac8745ceaa8be1b337661b45
-- depends_on:

-- The execution branch already consumed these receipts. Supply an empty input
-- there, without inventing archive evidence. Other frontiers keep their rows.
-- finish_planning_integration removes this migration-only input.
CREATE TABLE IF NOT EXISTS wave_chapters (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    chapter_id TEXT NOT NULL,
    project_id TEXT NOT NULL UNIQUE,
    current INTEGER NOT NULL CHECK (current IN (0, 1)),
    receipt TEXT NOT NULL CHECK (json_valid(receipt)),
    PRIMARY KEY (wave_id, chapter_id)
);
