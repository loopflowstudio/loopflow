ALTER TABLE task_sessions ADD COLUMN supervisor_kind TEXT NOT NULL DEFAULT 'wave';
ALTER TABLE task_sessions ADD COLUMN supervisor_id TEXT NOT NULL DEFAULT '';

UPDATE task_sessions
SET supervisor_id = wave_id
WHERE supervisor_id = '';

CREATE TABLE project_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL UNIQUE,
    project_slug TEXT NOT NULL,
    project_name TEXT NOT NULL,
    project_context TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    wave_name TEXT NOT NULL,
    repo TEXT NOT NULL,
    pm_snapshot_synced_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    status_reason TEXT NOT NULL,
    status_at INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    task_event_cursor INTEGER NOT NULL,
    state_fingerprint TEXT,
    agent TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_session_id TEXT,
    process_generation INTEGER,
    process_pid INTEGER,
    process_tmux_name TEXT,
    process_started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_project_sessions_wave_status
ON project_sessions(wave_id, status, updated_at DESC);

CREATE TABLE child_commands (
    id TEXT PRIMARY KEY,
    target_kind TEXT NOT NULL,
    session_id TEXT NOT NULL,
    source_json TEXT NOT NULL,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    claimed_by_generation INTEGER,
    accepted_at INTEGER,
    state TEXT NOT NULL,
    effect TEXT,
    error TEXT
);

INSERT INTO child_commands (
    id, target_kind, session_id, source_json, kind_json, created_at,
    claimed_by_generation, accepted_at, state, effect, error
)
SELECT id, 'task', session_id, source_json, kind_json, created_at,
       claimed_by_generation, accepted_at, state, effect, error
FROM task_commands;

UPDATE child_commands
SET id = 'cc_' || substr(id, 4)
WHERE target_kind = 'task' AND id LIKE 'tc_%';

UPDATE task_events
SET kind_json = replace(kind_json, '"command_id":"tc_', '"command_id":"cc_')
WHERE kind_json LIKE '%"command_id":"tc_%';

UPDATE child_commands
SET kind_json = replace(kind_json, '"decision_id":"td_', '"decision_id":"cd_')
WHERE target_kind = 'task' AND kind_json LIKE '%"decision_id":"td_%';

UPDATE child_commands
SET source_json = replace(source_json, '"wave_id":', '"id":')
WHERE source_json LIKE '%"wave_id":%';

UPDATE task_events
SET kind_json = replace(kind_json, '"decision_id":"td_', '"decision_id":"cd_')
WHERE kind_json LIKE '%"decision_id":"td_%';

DROP TABLE task_commands;

CREATE INDEX idx_child_commands_pending
ON child_commands(target_kind, session_id, state, created_at, id);

CREATE TABLE project_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES project_sessions(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_project_events_session
ON project_events(session_id, id);

CREATE TABLE observation_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supervisor_kind TEXT NOT NULL,
    supervisor_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_id TEXT NOT NULL,
    event_id INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    delivered_at INTEGER,
    UNIQUE(supervisor_kind, supervisor_id, source_kind, source_id, event_id)
);

CREATE INDEX idx_observation_outbox_pending
ON observation_outbox(supervisor_kind, supervisor_id, delivered_at, id);

INSERT OR IGNORE INTO observation_outbox (
    supervisor_kind, supervisor_id, source_kind, source_id,
    event_id, payload_json, created_at, delivered_at
)
SELECT sessions.supervisor_kind,
       sessions.supervisor_id,
       'task',
       events.session_id,
       events.id,
       json_object('kind', 'task', 'event', json(events.kind_json)),
       events.created_at,
       NULL
FROM task_events AS events
JOIN task_sessions AS sessions ON sessions.id = events.session_id
WHERE json_extract(events.kind_json, '$.kind') NOT IN ('started', 'progress');
