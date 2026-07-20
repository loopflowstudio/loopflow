-- A question is Turn-local tool I/O. Its Work and Basis are derived through
-- Turn -> AgentInvocation -> Run -> Epoch; only the answering route is stored.

CREATE TABLE ask_exchanges (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE RESTRICT,
    route_kind TEXT NOT NULL CHECK (route_kind IN ('user', 'parent')),
    route_work_kind TEXT CHECK (
        route_work_kind IN ('wave', 'project', 'task')
    ),
    route_work_id TEXT,
    question TEXT NOT NULL CHECK (length(trim(question)) > 0),
    asked_at INTEGER NOT NULL,
    answer_author_kind TEXT CHECK (answer_author_kind IN ('user', 'run')),
    answer_author_id TEXT,
    answer_text TEXT,
    answered_at INTEGER,
    CHECK (
        (route_kind = 'user'
         AND route_work_kind IS NULL AND route_work_id IS NULL)
        OR
        (route_kind = 'parent'
         AND route_work_kind IS NOT NULL AND route_work_id IS NOT NULL
         AND length(trim(route_work_id)) > 0)
    ),
    CHECK (
        (answer_author_kind IS NULL AND answer_author_id IS NULL
         AND answer_text IS NULL AND answered_at IS NULL)
        OR
        (answer_author_kind = 'user' AND answer_author_id IS NULL
         AND answer_text IS NOT NULL AND length(trim(answer_text)) > 0
         AND answered_at IS NOT NULL)
        OR
        (answer_author_kind = 'run' AND answer_author_id IS NOT NULL
         AND length(trim(answer_author_id)) > 0
         AND answer_text IS NOT NULL AND length(trim(answer_text)) > 0
         AND answered_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_ask_exchanges_one_pending_turn
    ON ask_exchanges(turn_id) WHERE answered_at IS NULL;
CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(route_work_kind, route_work_id, asked_at)
    WHERE route_kind = 'parent' AND answered_at IS NULL;
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE route_kind = 'user' AND answered_at IS NULL;

DROP INDEX idx_agent_invocations_attention;
ALTER TABLE agent_invocations DROP COLUMN attention_kind;
ALTER TABLE agent_invocations DROP COLUMN attention_work_kind;
ALTER TABLE agent_invocations DROP COLUMN attention_work_id;
ALTER TABLE agent_invocations DROP COLUMN attention_at;

ALTER TABLE work_flow_positions DROP COLUMN interactive;

ALTER TABLE tasks DROP COLUMN iterate_reviewer;
ALTER TABLE tasks DROP COLUMN kickoff_reviewer;
ALTER TABLE tasks DROP COLUMN gate_reviewer;
