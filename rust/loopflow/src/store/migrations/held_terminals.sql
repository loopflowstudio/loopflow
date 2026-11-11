-- `processes.terminal_id` already names the Desktop pane a process is placed in;
-- these columns say `held_terminal`.
-- A held terminal: one PTY a holder keeps for a managed Session's agent. It is
-- not a Process: the holder is one process however many terminals it holds.
-- A terminal carries what an owner's Process row carries for an owned
-- execution: who is viewing it and the input its provider has not acknowledged.
CREATE TABLE held_terminals (
    id TEXT PRIMARY KEY,
    -- SHA-256 of the holder's executable; names the holder that has the PTY.
    holder_build TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    ended_at INTEGER,
    -- NULL after ending when the holder died before reporting a status.
    exit_code INTEGER,
    native_input TEXT,
    viewer_hold TEXT CHECK(viewer_hold IN ('viewed','absent')),
    viewer_hold_at INTEGER,
    viewed INTEGER CHECK(viewed IN (0,1))
);
CREATE TRIGGER store_revision_held_terminals_insert AFTER INSERT ON held_terminals BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_held_terminals_update AFTER UPDATE ON held_terminals BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_held_terminals_delete AFTER DELETE ON held_terminals BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
-- A Session runs under an owner Process or in a terminal, never both.
ALTER TABLE agent_sessions ADD COLUMN held_terminal_id TEXT REFERENCES held_terminals(id);
CREATE INDEX agent_sessions_held_terminal ON agent_sessions(held_terminal_id)
    WHERE held_terminal_id IS NOT NULL;
-- The terminal an lf process runs in: a Graph started there holds it open.
ALTER TABLE processes ADD COLUMN held_terminal_id TEXT REFERENCES held_terminals(id);
