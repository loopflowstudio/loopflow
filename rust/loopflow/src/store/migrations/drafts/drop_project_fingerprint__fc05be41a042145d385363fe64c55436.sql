-- name: drop_project_fingerprint
-- id: fc05be41a042145d385363fe64c55436
-- depends_on: work_domain_state

ALTER TABLE projects DROP COLUMN last_state_fingerprint;
