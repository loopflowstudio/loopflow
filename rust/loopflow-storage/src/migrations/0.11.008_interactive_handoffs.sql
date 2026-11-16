-- One durable interactive child beneath a Wave, Project Session, or Task
-- Session. The row is the parent's waiting marker and presentation contract.
-- It references the existing body generation but owns no process or lease.

CREATE TABLE interactive_handoffs (
    id TEXT PRIMARY KEY,
    parent_kind TEXT NOT NULL CHECK (parent_kind IN ('wave', 'project', 'task')),
    parent_id TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    home TEXT NOT NULL,
    cwd TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    body_generation INTEGER NOT NULL CHECK (body_generation > 0),
    reason TEXT NOT NULL,
    environment_json TEXT NOT NULL,
    attach_argv_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'waiting', 'attached', 'completed', 'handed_back', 'failed'
    )),
    outcome_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    attached_at INTEGER,
    terminal_at INTEGER,
    wake_claimed_at INTEGER,
    wake_claimed_by_generation INTEGER CHECK (wake_claimed_by_generation > 0),
    CHECK (
        (status IN ('completed', 'handed_back', 'failed')) =
        (terminal_at IS NOT NULL AND outcome_json IS NOT NULL)
    ),
    CHECK ((wake_claimed_at IS NULL) = (wake_claimed_by_generation IS NULL)),
    CHECK (wake_claimed_at IS NULL OR terminal_at IS NOT NULL)
);

CREATE UNIQUE INDEX idx_interactive_handoffs_active_parent
    ON interactive_handoffs(parent_kind, parent_id)
    WHERE terminal_at IS NULL;

CREATE INDEX idx_interactive_handoffs_wave_created
    ON interactive_handoffs(wave_id, created_at DESC);
