-- name: durable_ask_invocations
-- id: ef354215067cff23930e3c3adc5426c4
-- depends_on: 

-- Replace Turn-local Ask/Answer exchanges with durable requested sessions.
-- Keeping the table name avoids rewriting the Linear outbox and historical
-- AgentInvocation foreign keys; every row is nevertheless rebuilt into the
-- new Ask shape.
CREATE TABLE ask_exchanges_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    origin_work_kind TEXT NOT NULL CHECK (
        origin_work_kind IN ('wave', 'project', 'task')
    ),
    origin_work_id TEXT NOT NULL CHECK (length(trim(origin_work_id)) > 0),
    origin_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    origin_turn_id TEXT REFERENCES agent_turns(id) ON DELETE RESTRICT,
    origin_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
    origin_home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    origin_cwd TEXT NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('user', 'parent')),
    target_work_kind TEXT CHECK (
        target_work_kind IN ('wave', 'project', 'task')
    ),
    target_work_id TEXT,
    request_kind TEXT NOT NULL CHECK (request_kind IN ('intervention', 'flow_step')),
    request_prompt TEXT,
    request_flow TEXT,
    request_node_id TEXT,
    request_skill TEXT,
    request_iteration INTEGER CHECK (request_iteration >= 0),
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'claimed', 'resolved', 'declined', 'cancelled')
    ),
    active_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
    result_kind TEXT CHECK (result_kind IN ('resolved', 'declined', 'cancelled')),
    result_text TEXT,
    terminal_author_kind TEXT CHECK (terminal_author_kind IN ('user', 'run')),
    terminal_author_id TEXT,
    asked_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (origin_turn_id IS NULL AND origin_invocation_id IS NULL)
        OR
        (origin_turn_id IS NOT NULL AND origin_invocation_id IS NOT NULL)
    ),
    CHECK (
        (target_kind = 'user'
         AND target_work_kind IS NULL AND target_work_id IS NULL)
        OR
        (target_kind = 'parent'
         AND target_work_kind IS NOT NULL AND target_work_id IS NOT NULL
         AND length(trim(target_work_id)) > 0)
    ),
    CHECK (
        (request_kind = 'intervention'
         AND request_prompt IS NOT NULL AND length(trim(request_prompt)) > 0
         AND request_flow IS NULL AND request_node_id IS NULL
         AND request_skill IS NULL AND request_iteration IS NULL)
        OR
        (request_kind = 'flow_step' AND request_prompt IS NULL
         AND request_flow IS NOT NULL AND length(trim(request_flow)) > 0
         AND request_node_id IS NOT NULL AND length(trim(request_node_id)) > 0
         AND request_skill IS NOT NULL AND length(trim(request_skill)) > 0
         AND request_iteration IS NOT NULL)
    ),
    CHECK (
        (state = 'queued' AND active_invocation_id IS NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state = 'claimed' AND active_invocation_id IS NOT NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state IN ('resolved', 'declined', 'cancelled')
         AND active_invocation_id IS NULL
         AND result_kind = state AND result_text IS NOT NULL
         AND length(trim(result_text)) > 0 AND terminal_at IS NOT NULL)
    ),
    CHECK (
        (terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'user' AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'run' AND terminal_author_id IS NOT NULL
         AND length(trim(terminal_author_id)) > 0)
    ),
    CHECK (
        (state IN ('queued', 'claimed')
         AND terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (state IN ('resolved', 'declined')
         AND terminal_author_kind IS NOT NULL)
        OR state = 'cancelled'
    )
);

INSERT INTO ask_exchanges_next (
    id, epoch_id, origin_work_kind, origin_work_id, origin_run_id,
    origin_turn_id, origin_invocation_id, origin_home_id, origin_cwd,
    target_kind, target_work_kind, target_work_id,
    request_kind, request_prompt, request_flow, request_node_id,
    request_skill, request_iteration, state, active_invocation_id,
    result_kind, result_text, terminal_author_kind, terminal_author_id,
    asked_at, terminal_at
)
SELECT
    a.id,
    e.id,
    CASE
        WHEN e.wave_id IS NOT NULL THEN 'wave'
        WHEN e.project_id IS NOT NULL THEN 'project'
        ELSE 'task'
    END,
    COALESCE(e.wave_id, e.project_id, e.task_id),
    r.id,
    a.turn_id,
    i.id,
    r.home_id,
    COALESCE(r.cwd, ''),
    a.route_kind,
    a.route_work_kind,
    a.route_work_id,
    'intervention',
    a.question,
    NULL,
    NULL,
    NULL,
    NULL,
    CASE WHEN a.answered_at IS NULL THEN 'queued' ELSE 'resolved' END,
    NULL,
    CASE WHEN a.answered_at IS NULL THEN NULL ELSE 'resolved' END,
    a.answer_text,
    a.answer_author_kind,
    a.answer_author_id,
    a.asked_at,
    a.answered_at
FROM ask_exchanges a
JOIN agent_turns t ON t.id = a.turn_id
JOIN agent_invocations i ON i.id = t.invocation_id
JOIN runs r ON r.id = i.supervising_run_id
JOIN epochs e ON e.id = t.epoch_id;

DROP TABLE ask_exchanges;
ALTER TABLE ask_exchanges_next RENAME TO ask_exchanges;

CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(target_work_kind, target_work_id, asked_at)
    WHERE target_kind = 'parent' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE target_kind = 'user' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_epoch_pending
    ON ask_exchanges(epoch_id, asked_at)
    WHERE state IN ('queued', 'claimed');

-- An Ask session is its ordinary AgentInvocation. These columns hold the two
-- presentation facts that do not already exist on that row. Legacy Answer
-- Invocations remain ordinary historical Invocations, not Ask sessions.
UPDATE agent_invocations SET answer_ask_id=NULL WHERE answer_ask_id IS NOT NULL;

ALTER TABLE agent_invocations ADD COLUMN ask_ready_at INTEGER;
ALTER TABLE agent_invocations ADD COLUMN ask_presented_at INTEGER;
