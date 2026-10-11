-- The provider capability survives attachment handoff, never replacement.
CREATE TABLE steer_adapters (
    agent_process_id TEXT PRIMARY KEY REFERENCES processes(id),
    capability TEXT NOT NULL UNIQUE,
    provider_thread TEXT,
    created_at INTEGER NOT NULL,
    activated_at INTEGER
);
