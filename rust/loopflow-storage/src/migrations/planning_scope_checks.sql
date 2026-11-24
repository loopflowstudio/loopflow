-- leaving: linear_team_checks LOO-1084
CREATE TABLE planning_scope_checks (
    scope TEXT PRIMARY KEY,
    attempted_at INTEGER NOT NULL,
    synced_through TEXT
);
INSERT INTO planning_scope_checks(scope,attempted_at,synced_through)
SELECT team_id,attempted_at,synced_through FROM linear_team_checks;
