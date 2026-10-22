-- draft: link_landing_operation
-- A completed mechanical land operation records handoff, while its Flow
-- remains at that occurrence until this exact delivery is settled.
ALTER TABLE flow_events ADD COLUMN landing_id TEXT REFERENCES pr_landings(id);

-- draft: normalize_pm_planning
-- Planning facts have repository/provider identity, independent of execution.
CREATE TABLE pm_projects (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY(repo, provider, id)
);
CREATE TABLE pm_items (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    identifier TEXT NOT NULL COLLATE NOCASE,
    project_id TEXT,
    observed_at INTEGER NOT NULL,
    needs_refresh INTEGER NOT NULL DEFAULT 0 CHECK(needs_refresh IN (0,1)),
    body TEXT NOT NULL,
    PRIMARY KEY(repo, provider, id)
);
CREATE INDEX pm_items_identifier ON pm_items(repo, provider, identifier);
CREATE INDEX pm_items_project ON pm_items(repo, provider, project_id);
CREATE TABLE pm_wave_sync (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL
);
CREATE TABLE pm_wave_projects (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(wave_id, project_id)
);

-- Overlapping historical snapshots converge on their latest observation.
-- An unreadable payload is not an empty plan to discard during migration.
CREATE TEMP TABLE validate_pm_snapshots(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO validate_pm_snapshots
SELECT COALESCE(json_type(payload,'$.projects')='array' AND json_type(payload,'$.items')='array',0)
FROM pm_snapshots;
DROP TABLE validate_pm_snapshots;

INSERT INTO pm_projects(repo, provider, id, observed_at, body)
SELECT w.repo, s.provider, json_extract(p.value, '$.id'), s.synced_at, p.value
FROM pm_snapshots s JOIN waves w ON w.id=s.wave_id,
     json_each(s.payload, '$.projects') p
WHERE true ORDER BY s.synced_at, s.wave_id
ON CONFLICT(repo, provider, id) DO UPDATE SET
    observed_at=excluded.observed_at, body=excluded.body;
INSERT INTO pm_items(repo, provider, id, identifier, project_id, observed_at, body)
SELECT w.repo, s.provider, json_extract(i.value, '$.id'),
       json_extract(i.value, '$.identifier'), json_extract(i.value, '$.project_id'),
       s.synced_at, i.value
FROM pm_snapshots s JOIN waves w ON w.id=s.wave_id,
     json_each(s.payload, '$.items') i
WHERE true ORDER BY s.synced_at, s.wave_id
ON CONFLICT(repo, provider, id) DO UPDATE SET
    identifier=excluded.identifier, project_id=excluded.project_id,
    observed_at=excluded.observed_at, body=excluded.body;
INSERT INTO pm_wave_sync SELECT wave_id, provider, initiative, synced_at FROM pm_snapshots;
INSERT INTO pm_wave_projects
SELECT s.wave_id, json_extract(p.value, '$.id'), CAST(p.key AS INTEGER)
FROM pm_snapshots s, json_each(s.payload, '$.projects') p;
DROP TABLE pm_snapshots;

-- draft: pm_issue_revisions
-- Signed Linear issue events fence delayed reads, even before an issue is cached.
-- Linear UUIDs identify the same issue across repository views in this store.
CREATE TABLE pm_issue_changes (
    issue_id TEXT PRIMARY KEY,
    revision_ns INTEGER,
    removed INTEGER NOT NULL CHECK(removed IN (0,1))
);

-- draft: pm_project_evidence
-- These are planning evidence, independent of execution and acquisition age.
ALTER TABLE pm_projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1));
ALTER TABLE pm_projects ADD COLUMN membership_unresolved INTEGER NOT NULL DEFAULT 0 CHECK(membership_unresolved IN (0,1));

-- draft: primary_session_scope
-- A primary Session is an ordinary conversation that is the one ongoing
-- conversation of its scope. The scope's identity stays in the row's own
-- columns; completed predecessors remain history.
ALTER TABLE agent_sessions ADD COLUMN primary_scope TEXT CHECK (primary_scope IN ('repository', 'wave'));
CREATE UNIQUE INDEX agent_sessions_primary_repository ON agent_sessions(repo)
    WHERE primary_scope = 'repository' AND completed_at IS NULL;
CREATE UNIQUE INDEX agent_sessions_primary_wave ON agent_sessions(wave_id)
    WHERE primary_scope = 'wave' AND completed_at IS NULL;

-- draft: scheduled_task_operation
ALTER TABLE tasks ADD COLUMN automation_enabled INTEGER CHECK (automation_enabled IN (0,1));
ALTER TABLE tasks ADD COLUMN automation_exec_id TEXT REFERENCES execs(id);
ALTER TABLE tasks ADD COLUMN automation_retry_key TEXT;
ALTER TABLE tasks ADD COLUMN automation_retries INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN automation_checked_at INTEGER;
ALTER TABLE tasks ADD COLUMN automation_detail TEXT;
ALTER TABLE ci_incidents ADD COLUMN repair_exec_id TEXT REFERENCES execs(id);
ALTER TABLE ci_incidents ADD COLUMN repair_session_id TEXT REFERENCES agent_sessions(id);
ALTER TABLE ci_incidents ADD COLUMN repair_retries INTEGER NOT NULL DEFAULT 0;
ALTER TABLE ci_incidents ADD COLUMN repair_finished_at INTEGER;
ALTER TABLE ci_incidents ADD COLUMN repair_error TEXT;
ALTER TABLE pr_landings ADD COLUMN ci_pending_head TEXT;
ALTER TABLE pr_landings ADD COLUMN ci_pending_since INTEGER;
ALTER TABLE pr_landings ADD COLUMN ci_timeout_reruns INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pr_landings ADD COLUMN ci_pending_attempt TEXT;
ALTER TABLE ci_incidents ADD COLUMN repair_conclusion TEXT CHECK (repair_conclusion IN ('published','blocked'));

-- draft: session_attention
-- Keep the passive Session inventory bounded to indexed lifecycle scalars.
CREATE INDEX session_driver_exit ON session_events(
    session_id, receipt_key,
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.outcome') END
) WHERE kind='observed';
CREATE INDEX session_turn_attention ON session_events(
    session_id, provider_thread, provider_turn,
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.status') END
) WHERE kind='completed';

-- draft: wave_directory_parents
DROP INDEX idx_waves_active_locator;

-- Expand old full-path names before replacing them with leaf names. Existing
-- IDs, including Projects' Wave references, survive the conversion.
CREATE TEMP TABLE wave_directory_paths AS
WITH RECURSIVE paths(repo, path, rest, name, parent_path, active, source_id) AS (
    SELECT repo, '', name || '/', '', NULL, retired_at IS NULL, id FROM waves
    UNION
    SELECT repo,
           CASE WHEN path='' THEN substr(rest,1,instr(rest,'/')-1)
                ELSE path || '/' || substr(rest,1,instr(rest,'/')-1) END,
           substr(rest,instr(rest,'/')+1), substr(rest,1,instr(rest,'/')-1),
           NULLIF(path,''), active, source_id
    FROM paths WHERE rest != ''
)
SELECT repo,path,name,parent_path,max(active) AS active,min(source_id) AS source_id
FROM paths WHERE path != '' GROUP BY repo,path,name,parent_path;

-- An active descendant needs an active directory ancestor even when an older
-- registration at that address is retired. Historical-only paths stay retired.
INSERT INTO waves(id,name,repo,created_at,retired_at,superseded_by_wave_id,retirement_reason)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' ||
       lower(hex(randomblob(6))), p.path,p.repo,unixepoch(),
       CASE WHEN p.active=0 THEN source.retired_at END,
       CASE WHEN p.active=0 THEN source.superseded_by_wave_id END,
       CASE WHEN p.active=0 THEN 'Directory ancestor of retired Wave ' || source.id END
FROM wave_directory_paths p JOIN waves source ON source.id=p.source_id
WHERE NOT EXISTS(SELECT 1 FROM waves w WHERE w.repo=p.repo AND w.name=p.path
    AND (p.active=0 OR w.retired_at IS NULL));

-- Old parent links described promotion ancestry. Directory paths now determine
-- parentage; promoted_at remains untouched as historical promotion evidence.
UPDATE waves AS child SET parent_wave_id=(
    SELECT parent.id FROM wave_directory_paths p JOIN waves parent
    ON parent.repo=p.repo AND parent.name=p.parent_path
    WHERE p.repo=child.repo AND p.path=child.name
    ORDER BY parent.retired_at IS NOT NULL, parent.created_at DESC, parent.id LIMIT 1
);

UPDATE waves SET name=(SELECT p.name FROM wave_directory_paths p
    WHERE p.repo=waves.repo AND p.path=waves.name);
DROP TABLE wave_directory_paths;

CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(parent_wave_id,''),name) WHERE retired_at IS NULL;

CREATE VIEW wave_addresses AS
WITH RECURSIVE addresses(id,slug) AS (
    SELECT id,name FROM waves WHERE parent_wave_id IS NULL
    UNION ALL
    SELECT w.id,a.slug || '/' || w.name FROM waves w
    JOIN addresses a ON w.parent_wave_id=a.id
)
SELECT w.*,a.slug FROM waves w JOIN addresses a ON a.id=w.id;
