ALTER TABLE task_commands
RENAME COLUMN acknowledged_at TO accepted_at;

ALTER TABLE task_commands
ADD COLUMN state TEXT NOT NULL DEFAULT 'persisted';

ALTER TABLE task_commands
ADD COLUMN effect TEXT;

ALTER TABLE task_commands
ADD COLUMN error TEXT;

UPDATE task_commands
SET created_at = created_at * 1000000000,
    accepted_at = accepted_at * 1000000000,
    state = CASE
        WHEN accepted_at IS NOT NULL THEN 'accepted'
        WHEN claimed_by_generation IS NOT NULL THEN 'claimed'
        ELSE 'persisted'
    END,
    kind_json = REPLACE(
        REPLACE(kind_json, '"kind":"message"', '"kind":"follow_up"'),
        '"next_message":',
        '"replacement":'
    );

UPDATE task_events
SET kind_json = json_object(
    'kind', 'command_changed',
    'command_id', json_extract(kind_json, '$.command_id'),
    'state', 'accepted',
    'effect', NULL,
    'error', NULL
)
WHERE json_extract(kind_json, '$.kind') = 'command_accepted';

DROP INDEX idx_task_commands_pending;

CREATE INDEX idx_task_commands_pending
ON task_commands(session_id, state, created_at, id);
