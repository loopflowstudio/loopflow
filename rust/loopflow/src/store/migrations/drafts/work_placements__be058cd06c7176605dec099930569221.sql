-- name: work_placements
-- id: be058cd06c7176605dec099930569221
-- depends_on: 
CREATE TABLE work_placements (
    wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    placed_at INTEGER NOT NULL,
    CHECK (
        (wave_id IS NOT NULL) +
        (project_id IS NOT NULL) +
        (task_id IS NOT NULL) = 1
    ),
    UNIQUE (wave_id),
    UNIQUE (project_id),
    UNIQUE (task_id)
);
CREATE INDEX idx_work_placements_home
    ON work_placements(home_id, placed_at);

INSERT INTO work_placements (
    wave_id, project_id, task_id, home_id, placed_at
)
SELECT
    waves.id,
    NULL,
    NULL,
    (SELECT id FROM homes WHERE route = 'local' ORDER BY created_at LIMIT 1),
    waves.created_at
FROM waves;

INSERT INTO work_placements (
    wave_id, project_id, task_id, home_id, placed_at
)
SELECT
    NULL,
    projects.id,
    NULL,
    work_placements.home_id,
    projects.created_at
FROM projects
JOIN work_placements ON work_placements.wave_id = projects.wave_id;

INSERT INTO work_placements (
    wave_id, project_id, task_id, home_id, placed_at
)
SELECT
    NULL,
    NULL,
    tasks.id,
    work_placements.home_id,
    tasks.created_at
FROM tasks
JOIN work_placements ON work_placements.project_id = tasks.project_id;
