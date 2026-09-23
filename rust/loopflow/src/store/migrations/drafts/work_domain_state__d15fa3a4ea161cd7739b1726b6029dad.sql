-- name: work_domain_state
-- id: d15fa3a4ea161cd7739b1726b6029dad
-- depends_on: 

ALTER TABLE projects ADD COLUMN iteration INTEGER NOT NULL DEFAULT 0
    CHECK (iteration >= 0);
ALTER TABLE projects ADD COLUMN last_state_fingerprint TEXT;

UPDATE projects
SET iteration = (
        SELECT iteration FROM project_controller_state
        WHERE project_controller_state.project_id = projects.id
    ),
    last_state_fingerprint = (
        SELECT last_state_fingerprint FROM project_controller_state
        WHERE project_controller_state.project_id = projects.id
    )
WHERE EXISTS (
    SELECT 1 FROM project_controller_state
    WHERE project_controller_state.project_id = projects.id
);

DROP TABLE task_controller_state;
DROP TABLE project_controller_state;
