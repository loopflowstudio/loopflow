-- name: answer_invocations
-- id: f73609b8cdb78ee7bfe025377cc0b556
-- depends_on: ask_linear_comment_outbox

-- Correlate a detached answer attempt to the exact durable Ask it serves.
-- The relation is purpose, not authority: only the supervising Run lease can
-- commit the Answer.
ALTER TABLE agent_invocations ADD COLUMN answer_ask_id TEXT
    REFERENCES ask_exchanges(id);

CREATE UNIQUE INDEX idx_agent_invocations_one_live_answer
    ON agent_invocations(answer_ask_id)
    WHERE answer_ask_id IS NOT NULL AND ended_at IS NULL;
