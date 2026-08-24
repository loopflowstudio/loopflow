-- name: replay_contracts
-- id: 3cf7cc7e3d8c4d7ea92cb65e8848aaf6
-- depends_on: none

CREATE TABLE replay_contracts (
    invocation_id TEXT PRIMARY KEY
        REFERENCES agent_invocations(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    contract_path TEXT NOT NULL CHECK (length(trim(contract_path)) > 0),
    contract_sha256 TEXT NOT NULL CHECK (
        length(contract_sha256) = 64
        AND contract_sha256 NOT GLOB '*[^0-9a-f]*'
    ),
    captured_at INTEGER NOT NULL CHECK (captured_at >= 0)
);
