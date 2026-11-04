-- Device grants are local Machine authority, never copied as Work records.
CREATE TABLE machine_pairings (
    ticket_hash BLOB PRIMARY KEY CHECK(length(ticket_hash) = 32),
    name TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE machine_devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash BLOB NOT NULL UNIQUE CHECK(length(token_hash) = 32),
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);
