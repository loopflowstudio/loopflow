-- Durable delivery inbox for provider webhook ingress (`lfd`).
--
-- The inbox is the ingress gate: it deduplicates *deliveries* (by provider
-- delivery id), while the domain tables (`task_linear_observations`,
-- `task_linear_ingested_comments`) deduplicate *events*. Both gates are needed
-- — a redelivered webhook is dropped at the inbox; an out-of-order or
-- crash-mid-flight delivery re-processes at the inbox but is a no-op at the
-- domain. Append-mostly; pruning is a follow-on Task.
CREATE TABLE provider_deliveries (
    delivery_id   TEXT    NOT NULL,
    provider      TEXT    NOT NULL CHECK (provider IN ('linear', 'github')),
    -- "issue_edit" | "comment" | "ignored" | null (null only for unknown providers)
    event_kind    TEXT,
    -- "task_session" | null (null when no target Session resolved)
    target_kind   TEXT,
    target_id     TEXT,
    status        TEXT    NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','processed','ignored','no_target','error')),
    -- JSON summary of the processing outcome, for ops inspection.
    outcome       TEXT,
    -- Unix milliseconds.
    received_at   INTEGER NOT NULL,
    processed_at  INTEGER,
    PRIMARY KEY (delivery_id, provider)
);

CREATE INDEX idx_provider_deliveries_status ON provider_deliveries(status);
CREATE INDEX idx_provider_deliveries_received ON provider_deliveries(received_at);
