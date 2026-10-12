-- A failed check consumes freshness but never advances the successful watermark.
CREATE TABLE linear_team_checks (
    team_id TEXT PRIMARY KEY,
    attempted_at INTEGER NOT NULL,
    synced_through TEXT
);
