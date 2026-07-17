-- Durable input spine: stable Work, one Epoch revision, Steer/Send, typed
-- decisions, and immutable root-Turn Basis. Direction is migrated once; the
-- old directive store is removed rather than dual-written.

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    external_project_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_projects_wave ON projects(wave_id, created_at);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT NOT NULL UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);

INSERT INTO projects (id, wave_id, external_project_id, created_at)
SELECT
    'proj_' || lower(hex(randomblob(16))),
    project_sessions.wave_id,
    project_sessions.project_id,
    MIN(project_sessions.created_at)
FROM project_sessions
GROUP BY project_sessions.project_id;

INSERT INTO tasks (id, project_id, external_issue_id, issue_identifier, created_at)
SELECT
    'task_' || lower(hex(randomblob(16))),
    projects.id,
    task_sessions.issue_id,
    MAX(task_sessions.issue_identifier),
    MIN(task_sessions.created_at)
FROM task_sessions
JOIN projects ON projects.external_project_id = task_sessions.project_id
GROUP BY task_sessions.issue_id;

CREATE TABLE epochs (
    id TEXT PRIMARY KEY,
    number INTEGER NOT NULL CHECK (number > 0),
    wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK (state IN ('open', 'done', 'abandoned')),
    current_rev INTEGER NOT NULL CHECK (current_rev >= 0),
    created_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (wave_id IS NOT NULL) +
        (project_id IS NOT NULL) +
        (task_id IS NOT NULL) = 1
    ),
    CHECK ((state = 'open') = (terminal_at IS NULL)),
    UNIQUE (wave_id, number),
    UNIQUE (project_id, number),
    UNIQUE (task_id, number)
);
CREATE UNIQUE INDEX idx_epochs_one_open_wave
    ON epochs(wave_id) WHERE state = 'open' AND wave_id IS NOT NULL;
CREATE UNIQUE INDEX idx_epochs_one_open_project
    ON epochs(project_id) WHERE state = 'open' AND project_id IS NOT NULL;
CREATE UNIQUE INDEX idx_epochs_one_open_task
    ON epochs(task_id) WHERE state = 'open' AND task_id IS NOT NULL;

ALTER TABLE project_sessions ADD COLUMN epoch_id TEXT;
ALTER TABLE task_sessions ADD COLUMN epoch_id TEXT;

INSERT INTO epochs (
    id, number, wave_id, project_id, task_id, state, current_rev,
    created_at, terminal_at
)
SELECT
    'epoch_' || lower(hex(randomblob(16))),
    1,
    waves.id,
    NULL,
    NULL,
    'open',
    0,
    waves.created_at,
    NULL
FROM waves;

CREATE TEMPORARY TABLE project_epoch_map AS
SELECT
    project_sessions.id AS session_id,
    projects.id AS project_id,
    'epoch_' || lower(hex(randomblob(16))) AS epoch_id,
    row_number() OVER (
        PARTITION BY project_sessions.project_id
        ORDER BY project_sessions.created_at, project_sessions.id
    ) AS number,
    CASE project_sessions.status
        WHEN 'completed' THEN 'done'
        WHEN 'abandoned' THEN 'abandoned'
        ELSE 'open'
    END AS state,
    project_sessions.created_at,
    CASE
        WHEN project_sessions.status IN ('completed', 'abandoned')
            THEN project_sessions.updated_at
        ELSE NULL
    END AS terminal_at
FROM project_sessions
JOIN projects ON projects.external_project_id = project_sessions.project_id;

INSERT INTO epochs (
    id, number, wave_id, project_id, task_id, state, current_rev,
    created_at, terminal_at
)
SELECT
    epoch_id, number, NULL, project_id, NULL, state, 0, created_at, terminal_at
FROM project_epoch_map;

UPDATE project_sessions
SET epoch_id = (
    SELECT project_epoch_map.epoch_id
    FROM project_epoch_map
    WHERE project_epoch_map.session_id = project_sessions.id
);

CREATE TEMPORARY TABLE task_epoch_map AS
SELECT
    task_sessions.id AS session_id,
    tasks.id AS task_id,
    'epoch_' || lower(hex(randomblob(16))) AS epoch_id,
    row_number() OVER (
        PARTITION BY task_sessions.issue_id
        ORDER BY task_sessions.created_at, task_sessions.id
    ) AS number,
    CASE task_sessions.status
        WHEN 'completed' THEN 'done'
        WHEN 'abandoned' THEN 'abandoned'
        ELSE 'open'
    END AS state,
    task_sessions.created_at,
    CASE
        WHEN task_sessions.status IN ('completed', 'abandoned')
            THEN task_sessions.updated_at
        ELSE NULL
    END AS terminal_at
FROM task_sessions
JOIN tasks ON tasks.external_issue_id = task_sessions.issue_id;

INSERT INTO epochs (
    id, number, wave_id, project_id, task_id, state, current_rev,
    created_at, terminal_at
)
SELECT epoch_id, number, NULL, NULL, task_id, state, 0, created_at, terminal_at
FROM task_epoch_map;

UPDATE task_sessions
SET epoch_id = (
    SELECT task_epoch_map.epoch_id
    FROM task_epoch_map
    WHERE task_epoch_map.session_id = task_sessions.id
);

CREATE TABLE epoch_revisions (
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE CASCADE,
    rev INTEGER NOT NULL CHECK (rev >= 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'truth', 'steer', 'decision', 'approval', 'evidence'
    )),
    source_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (epoch_id, rev),
    UNIQUE (kind, source_id)
);

CREATE TABLE work_truth (
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);

INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
SELECT id, 0, 'truth', 'truth:' || id || ':0', created_at FROM epochs;

INSERT INTO work_truth (epoch_id, rev, payload_json, created_at)
SELECT
    epochs.id,
    0,
    json_object('name', waves.name, 'repo', waves.repo),
    epochs.created_at
FROM epochs
JOIN waves ON waves.id = epochs.wave_id;

INSERT INTO work_truth (epoch_id, rev, payload_json, created_at)
SELECT
    project_sessions.epoch_id,
    0,
    json_object(
        'external_project_id', project_sessions.project_id,
        'slug', project_sessions.project_slug,
        'name', project_sessions.project_name,
        'prompt_context', project_sessions.project_prompt_context,
        'pm_snapshot_synced_at', project_sessions.pm_snapshot_synced_at
    ),
    project_sessions.created_at
FROM project_sessions;

INSERT INTO work_truth (epoch_id, rev, payload_json, created_at)
SELECT
    task_sessions.epoch_id,
    0,
    json_object(
        'external_issue_id', task_sessions.issue_id,
        'identifier', task_sessions.issue_identifier,
        'title', task_sessions.issue_title,
        'description', task_sessions.issue_description,
        'pm_snapshot_synced_at', task_sessions.pm_snapshot_synced_at
    ),
    task_sessions.created_at
FROM task_sessions;

CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK (state IN ('reserved', 'active', 'stopping', 'ended')),
    lease_generation INTEGER,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('wave', 'project', 'task', 'migration')),
    source_id TEXT,
    created_at INTEGER NOT NULL,
    ended_at INTEGER
);
CREATE UNIQUE INDEX idx_runs_one_active_epoch
    ON runs(epoch_id) WHERE state != 'ended';
CREATE UNIQUE INDEX idx_runs_source_generation
    ON runs(source_kind, source_id, lease_generation)
    WHERE source_id IS NOT NULL AND lease_generation IS NOT NULL;

-- Historical author provenance needs a Run, but migration does not resurrect
-- execution authority. Every imported Run is terminal.
INSERT INTO runs (
    id, epoch_id, state, lease_generation, source_kind, source_id,
    created_at, ended_at
)
SELECT
    'run_' || lower(hex(randomblob(16))),
    epochs.id,
    'ended',
    NULL,
    'migration',
    NULL,
    epochs.created_at,
    COALESCE(epochs.terminal_at, unixepoch())
FROM epochs;

CREATE TABLE steers (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'run')),
    author_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    text TEXT NOT NULL CHECK (length(trim(text)) > 0),
    issued_at INTEGER NOT NULL,
    CHECK ((author_kind = 'user') = (author_run_id IS NULL)),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);
CREATE INDEX idx_steers_epoch_revision ON steers(epoch_id, rev);

CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    request_id TEXT NOT NULL,
    choice TEXT NOT NULL CHECK (length(trim(choice)) > 0),
    decided_at INTEGER NOT NULL,
    UNIQUE (epoch_id, request_id),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);

CREATE TABLE approvals (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    request_id TEXT NOT NULL,
    approved INTEGER NOT NULL CHECK (approved IN (0, 1)),
    decided_at INTEGER NOT NULL,
    UNIQUE (epoch_id, request_id),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);

-- Normalize every legacy authored/typed input into one ordered Epoch stream.
CREATE TEMPORARY TABLE legacy_inputs (
    id TEXT NOT NULL,
    epoch_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    source_json TEXT,
    text TEXT,
    request_id TEXT,
    choice TEXT,
    created_at INTEGER NOT NULL,
    sort_key TEXT NOT NULL
);

INSERT INTO legacy_inputs (
    id, epoch_id, kind, source_json, text, request_id, choice, created_at, sort_key
)
SELECT
    'steer_' || lower(hex(randomblob(16))),
    CASE child_directives.target_kind
        WHEN 'project' THEN (SELECT epoch_id FROM project_sessions WHERE id = child_directives.target_id)
        WHEN 'task' THEN (SELECT epoch_id FROM task_sessions WHERE id = child_directives.target_id)
    END,
    'steer',
    child_directives.source_json,
    child_directives.text,
    NULL,
    NULL,
    child_directives.issued_at,
    'directive:' || child_directives.id
FROM child_directives
WHERE child_directives.kind = 'replacement';

INSERT INTO legacy_inputs (
    id, epoch_id, kind, source_json, text, request_id, choice, created_at, sort_key
)
SELECT
    'steer_' || lower(hex(randomblob(16))),
    CASE child_commands.target_kind
        WHEN 'project' THEN (SELECT epoch_id FROM project_sessions WHERE id = child_commands.session_id)
        WHEN 'task' THEN (SELECT epoch_id FROM task_sessions WHERE id = child_commands.session_id)
    END,
    'steer',
    child_commands.source_json,
    CASE json_extract(child_commands.kind_json, '$.kind')
        WHEN 'follow_up' THEN json_extract(child_commands.kind_json, '$.text')
        WHEN 'resume' THEN json_extract(child_commands.kind_json, '$.message')
    END,
    NULL,
    NULL,
    child_commands.created_at,
    'command:' || child_commands.id
FROM child_commands
WHERE json_extract(child_commands.kind_json, '$.kind') = 'follow_up'
   OR (
       json_extract(child_commands.kind_json, '$.kind') = 'resume'
       AND json_extract(child_commands.kind_json, '$.message') IS NOT NULL
   );

INSERT INTO legacy_inputs (
    id, epoch_id, kind, source_json, text, request_id, choice, created_at, sort_key
)
SELECT
    'decision_' || lower(hex(randomblob(16))),
    CASE child_commands.target_kind
        WHEN 'project' THEN (SELECT epoch_id FROM project_sessions WHERE id = child_commands.session_id)
        WHEN 'task' THEN (SELECT epoch_id FROM task_sessions WHERE id = child_commands.session_id)
    END,
    'decision',
    child_commands.source_json,
    NULL,
    json_extract(child_commands.kind_json, '$.decision_id'),
    json_extract(child_commands.kind_json, '$.choice'),
    child_commands.created_at,
    'decision:' || child_commands.id
FROM child_commands
WHERE json_extract(child_commands.kind_json, '$.kind') = 'decide';

-- Extra prose on a typed decision is an ordinary Steer, never part of the
-- machine-semantic resolution.
INSERT INTO legacy_inputs (
    id, epoch_id, kind, source_json, text, request_id, choice, created_at, sort_key
)
SELECT
    'steer_' || lower(hex(randomblob(16))),
    CASE child_commands.target_kind
        WHEN 'project' THEN (SELECT epoch_id FROM project_sessions WHERE id = child_commands.session_id)
        WHEN 'task' THEN (SELECT epoch_id FROM task_sessions WHERE id = child_commands.session_id)
    END,
    'steer',
    child_commands.source_json,
    json_extract(child_commands.kind_json, '$.message'),
    NULL,
    NULL,
    child_commands.created_at,
    'decision-message:' || child_commands.id
FROM child_commands
WHERE json_extract(child_commands.kind_json, '$.kind') = 'decide'
  AND json_extract(child_commands.kind_json, '$.message') IS NOT NULL;

CREATE TEMPORARY TABLE numbered_legacy_inputs AS
SELECT
    legacy_inputs.*,
    row_number() OVER (
        PARTITION BY legacy_inputs.epoch_id
        ORDER BY legacy_inputs.created_at, legacy_inputs.sort_key
    ) AS rev
FROM legacy_inputs
WHERE legacy_inputs.epoch_id IS NOT NULL;

INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
SELECT epoch_id, rev, kind, id, created_at
FROM numbered_legacy_inputs;

INSERT INTO steers (
    id, epoch_id, rev, author_kind, author_run_id, text, issued_at
)
SELECT
    numbered.id,
    numbered.epoch_id,
    numbered.rev,
    CASE json_extract(numbered.source_json, '$.kind')
        WHEN 'wave' THEN 'run'
        WHEN 'project' THEN 'run'
        ELSE 'user'
    END,
    CASE json_extract(numbered.source_json, '$.kind')
        WHEN 'wave' THEN (
            SELECT runs.id
            FROM runs
            JOIN epochs ON epochs.id = runs.epoch_id
            WHERE epochs.wave_id = json_extract(numbered.source_json, '$.id')
            LIMIT 1
        )
        WHEN 'project' THEN (
            SELECT runs.id
            FROM runs
            JOIN project_sessions ON project_sessions.epoch_id = runs.epoch_id
            WHERE project_sessions.id = json_extract(numbered.source_json, '$.id')
            LIMIT 1
        )
        ELSE NULL
    END,
    numbered.text,
    numbered.created_at
FROM numbered_legacy_inputs AS numbered
WHERE numbered.kind = 'steer';

INSERT INTO decisions (
    id, epoch_id, rev, request_id, choice, decided_at
)
SELECT id, epoch_id, rev, request_id, choice, created_at
FROM numbered_legacy_inputs
WHERE kind = 'decision';

UPDATE epochs
SET current_rev = COALESCE((
    SELECT MAX(epoch_revisions.rev)
    FROM epoch_revisions
    WHERE epoch_revisions.epoch_id = epochs.id
), 0);

DROP TABLE numbered_legacy_inputs;
DROP TABLE legacy_inputs;
DROP TABLE project_epoch_map;
DROP TABLE task_epoch_map;

-- Add fixed Basis to the sole observed Turn/spend row. Generic launches remain
-- explicitly unattributed; Project/Task root Turns require a Basis at runtime.
CREATE TABLE agent_turns_next (
    id TEXT PRIMARY KEY,
    launch_id TEXT NOT NULL REFERENCES agent_launches(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    provider_turn_id TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'failed', 'interrupted', 'partial')
    ),
    input_op TEXT NOT NULL CHECK (
        input_op IN ('initial', 'message', 'steer', 'queued')
    ),
    context_coverage TEXT NOT NULL CHECK (
        context_coverage IN ('assembled', 'provider_total_only', 'unknown')
    ),
    tokenizer TEXT NOT NULL,
    system_prompt_path TEXT,
    task_prompt_path TEXT NOT NULL,
    system_tokens INTEGER NOT NULL,
    task_tokens INTEGER NOT NULL,
    supplied_context_tokens INTEGER NOT NULL,
    provider_input_tokens INTEGER,
    provider_output_tokens INTEGER,
    reasoning_tokens INTEGER,
    cache_read_tokens INTEGER,
    cache_write_tokens INTEGER,
    cost_usd REAL,
    context_gather_ms INTEGER NOT NULL,
    context_render_ms INTEGER NOT NULL,
    context_persist_ms INTEGER NOT NULL,
    first_event_seq INTEGER,
    last_event_seq INTEGER,
    provider_total_input_tokens INTEGER,
    peak_input_tokens INTEGER,
    context_window_tokens INTEGER,
    epoch_id TEXT,
    basis_rev INTEGER,
    CHECK ((epoch_id IS NULL) = (basis_rev IS NULL)),
    UNIQUE (launch_id, ordinal),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);

INSERT INTO agent_turns_next (
    id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status,
    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
    system_tokens, task_tokens, supplied_context_tokens,
    provider_input_tokens, provider_output_tokens, reasoning_tokens,
    cache_read_tokens, cache_write_tokens, cost_usd, context_gather_ms,
    context_render_ms, context_persist_ms, first_event_seq, last_event_seq,
    provider_total_input_tokens, peak_input_tokens, context_window_tokens,
    epoch_id, basis_rev
)
SELECT
    id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status,
    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
    system_tokens, task_tokens, supplied_context_tokens,
    provider_input_tokens, provider_output_tokens, reasoning_tokens,
    cache_read_tokens, cache_write_tokens, cost_usd, context_gather_ms,
    context_render_ms, context_persist_ms, first_event_seq, last_event_seq,
    provider_total_input_tokens, peak_input_tokens, context_window_tokens,
    NULL, NULL
FROM agent_turns;

DROP TABLE agent_turns;
ALTER TABLE agent_turns_next RENAME TO agent_turns;
CREATE INDEX idx_agent_turns_launch ON agent_turns(launch_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);
CREATE INDEX idx_agent_turns_epoch_basis ON agent_turns(epoch_id, basis_rev, status);

CREATE TABLE sends (
    id TEXT PRIMARY KEY,
    steer_id TEXT NOT NULL REFERENCES steers(id) ON DELETE CASCADE,
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    via TEXT NOT NULL CHECK (via IN ('live', 'seed')),
    state TEXT NOT NULL CHECK (state IN ('sending', 'sent', 'failed', 'unknown')),
    provider_turn_id TEXT,
    reason TEXT,
    attempted_at INTEGER NOT NULL,
    finished_at INTEGER,
    CHECK ((state = 'sending') = (finished_at IS NULL)),
    UNIQUE (steer_id, turn_id, via)
);
CREATE INDEX idx_sends_turn ON sends(turn_id, attempted_at);

-- The command ledger now carries lifecycle and CI wake only. Authored prose
-- and typed decisions have moved, so keeping their rows live would be a second
-- reader waiting to happen.
DELETE FROM child_commands
WHERE json_extract(kind_json, '$.kind') IN ('follow_up', 'steer', 'decide')
   OR (
       json_extract(kind_json, '$.kind') = 'resume'
       AND json_extract(kind_json, '$.message') IS NOT NULL
   );
DROP TABLE child_directives;
