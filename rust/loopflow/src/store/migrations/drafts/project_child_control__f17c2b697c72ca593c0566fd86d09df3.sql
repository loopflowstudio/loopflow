-- name: project_child_control
-- id: f17c2b697c72ca593c0566fd86d09df3
-- depends_on: obsolete_sql_lifecycle

CREATE TABLE project_child_controls (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL CHECK (length(trim(run_id)) > 0),
    token_hash TEXT NOT NULL CHECK (length(token_hash) = 64),
    steer_sequence INTEGER NOT NULL CHECK (steer_sequence >= 0)
);
