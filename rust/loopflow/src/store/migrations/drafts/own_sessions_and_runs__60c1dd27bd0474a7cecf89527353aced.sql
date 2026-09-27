-- name: own_sessions_and_runs
-- id: 60c1dd27bd0474a7cecf89527353aced
-- depends_on: retain_flow_invocations

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    current_run_id TEXT NOT NULL,
    title TEXT NOT NULL,
    title_source TEXT NOT NULL CHECK (title_source IN ('generated', 'human')),
    ready_summary TEXT,
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (current_run_id, id) REFERENCES runs(id, session_id)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE runs (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT REFERENCES sessions(id) ON DELETE RESTRICT
        DEFERRABLE INITIALLY DEFERRED,
    invocation_id TEXT REFERENCES flow_invocations(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT,
    created_at INTEGER NOT NULL,
    cwd TEXT NOT NULL,
    skill TEXT,
    published INTEGER NOT NULL CHECK (published IN (0, 1)),
    UNIQUE (id, session_id),
    CHECK (task_id IS NULL OR wave_id IS NOT NULL)
);
CREATE INDEX run_session_history ON runs(session_id, created_at, id);
CREATE INDEX run_task_history ON runs(task_id, created_at, id);
CREATE INDEX run_wave_history ON runs(wave_id, created_at, id);
CREATE INDEX run_invocation_history ON runs(invocation_id, created_at, id);
CREATE INDEX open_sessions ON sessions(created_at, id) WHERE completed_at IS NULL;
ALTER TABLE flow_invocations ADD COLUMN pending_session_id TEXT REFERENCES sessions(id);

-- Walk the captured cursor, including selected XOR children. Do not reload a
-- template or infer a human boundary from the presence of a Run or feedback.
CREATE TEMP TABLE imported_reviews AS
WITH RECURSIVE selected(id, steps, cursor) AS (
    SELECT id, json_extract(invocation_json, '$.steps'),
           CASE WHEN json_type(review_json, '$.index') IS 'integer'
                THEN review_json ELSE json_object('index', step_index) END
    FROM flow_invocations
    UNION ALL
    SELECT id,
           json_extract(steps, '$[' || json_extract(cursor, '$.index') || '].Xor.paths.'
               || json_quote(json_extract(cursor, '$.child.selected')) || '.steps'),
           json_extract(cursor, '$.child.cursor')
    FROM selected WHERE json_type(cursor, '$.child') IS 'object'
), leaves AS (
    SELECT id, json_extract(steps, '$[' || json_extract(cursor, '$.index') || '].Skill') AS skill
    FROM selected WHERE json_type(cursor, '$.child') IS NOT 'object'
)
SELECT f.id AS invocation_id,
       f.task_id || ':' || f.id || ':' || json_extract(f.invocation_json, '$.flow')
           || ':' || json_extract(l.skill, '$.policy.id') || ':' || f.iteration AS session_id,
       coalesce(f.session_run_id, 'run_' || lower(hex(randomblob(16)))) AS run_id,
       f.session_run_id IS NOT NULL AS published,
       f.task_id, p.wave_id, t.issue_title AS title, t.worktree AS cwd,
       json_extract(l.skill, '$.skill.name') AS skill,
       f.ready_summary, f.updated_at AS created_at, f.ended_at AS completed_at
FROM leaves l JOIN flow_invocations f ON f.id=l.id
JOIN tasks t ON t.id=f.task_id JOIN projects p ON p.id=t.project_id
WHERE json_extract(l.skill, '$.policy.human')=1;

INSERT INTO sessions(id, current_run_id, title, title_source, ready_summary, completed_at, created_at)
SELECT session_id, run_id, title, 'generated', ready_summary, completed_at, created_at
FROM imported_reviews;
INSERT INTO runs(id, session_id, invocation_id, task_id, wave_id, created_at, published, cwd, skill)
SELECT run_id, session_id, invocation_id, task_id, wave_id, created_at, published, cwd, skill
FROM imported_reviews;
UPDATE flow_invocations SET pending_session_id=(
    SELECT session_id FROM imported_reviews r WHERE r.invocation_id=flow_invocations.id
);
DROP TABLE imported_reviews;

-- Preserve first-start evidence even when an old capture cannot be mapped.
INSERT INTO task_events(task_id, kind_json, created_at)
SELECT task_id, '{"kind":"started"}', min(updated_at) FROM flow_invocations f
WHERE task_id IS NOT NULL AND session_run_id IS NOT NULL
AND NOT EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=f.task_id
    AND json_extract(e.kind_json, '$.kind')='started') GROUP BY task_id;

-- Preserve raw historical inputs for offline Home conversion, including any
-- records whose capture cannot establish a human boundary. Runtime never reads
-- or updates these import-only columns.
ALTER TABLE flow_invocations RENAME COLUMN session_run_id TO historical_session_run_id;
ALTER TABLE flow_invocations RENAME COLUMN ready_summary TO historical_ready_summary;

CREATE TRIGGER validate_run_parents_insert BEFORE INSERT ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id AND f.task_id IS NEW.task_id
    ) THEN RAISE(ABORT, 'Run and Invocation nullable Tasks disagree') END;
END;
CREATE TRIGGER validate_run_parents_update BEFORE UPDATE OF task_id,wave_id,invocation_id ON runs BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM tasks t JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id=NEW.wave_id
    ) THEN RAISE(ABORT, 'Run Task and Wave disagree') END;
    SELECT CASE WHEN NEW.invocation_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM flow_invocations f WHERE f.id=NEW.invocation_id AND f.task_id IS NEW.task_id
    ) THEN RAISE(ABORT, 'Run and Invocation nullable Tasks disagree') END;
END;
CREATE TRIGGER validate_invocation_run_parents BEFORE UPDATE OF task_id ON flow_invocations
WHEN EXISTS (SELECT 1 FROM runs WHERE invocation_id=NEW.id AND task_id IS NOT NEW.task_id)
BEGIN SELECT RAISE(ABORT, 'Invocation would change Run ancestry'); END;
CREATE TRIGGER validate_project_run_parents BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM runs r JOIN tasks t ON t.id=r.task_id
    WHERE t.project_id=NEW.id AND r.wave_id IS NOT NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'Project would change Run ancestry'); END;
CREATE TRIGGER validate_task_run_parents BEFORE UPDATE OF project_id ON tasks
WHEN EXISTS (SELECT 1 FROM runs r JOIN projects p ON p.id=NEW.project_id
    WHERE r.task_id=NEW.id AND r.wave_id IS NOT p.wave_id)
BEGIN SELECT RAISE(ABORT, 'Task would change Run ancestry'); END;
