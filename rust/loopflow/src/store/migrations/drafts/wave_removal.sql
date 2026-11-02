-- Removing a Wave retires it without a successor: `lf wave remove` and Git
-- planning exchange record only `retired_at`, with a reason when one is known.
-- A superseded Wave still names its successor. SQLite cannot alter a CHECK, so
-- the table is rebuilt with its column order (the address view selects w.*),
-- indexes and triggers.
PRAGMA legacy_alter_table = ON;
DROP VIEW wave_addresses;
CREATE TABLE waves_removal (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES "waves"(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    retired_at INTEGER,
    superseded_by_wave_id TEXT REFERENCES "waves"(id) ON DELETE RESTRICT,
    retirement_reason TEXT,
    work_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (work_state IN ('ready', 'done', 'abandoned')),
    work_terminal_at INTEGER,
    current_project_id TEXT REFERENCES projects(id),
    project_activation_lf_process_id TEXT REFERENCES processes(id),
    CHECK (
        retired_at IS NOT NULL
        OR (superseded_by_wave_id IS NULL AND retirement_reason IS NULL)
    ),
    CHECK (retirement_reason IS NULL OR length(trim(retirement_reason)) > 0)
);
INSERT INTO waves_removal(id,name,repo,created_at,parent_wave_id,promoted_at,retired_at,superseded_by_wave_id,retirement_reason,work_state,work_terminal_at,current_project_id,project_activation_lf_process_id)
SELECT id,name,repo,created_at,parent_wave_id,promoted_at,retired_at,superseded_by_wave_id,retirement_reason,work_state,work_terminal_at,current_project_id,project_activation_lf_process_id FROM waves;
DROP TABLE waves;
ALTER TABLE waves_removal RENAME TO waves;
PRAGMA legacy_alter_table = OFF;
CREATE INDEX idx_waves_parent
    ON waves(parent_wave_id) WHERE retired_at IS NULL;
CREATE INDEX idx_waves_superseded_by
    ON waves(superseded_by_wave_id) WHERE superseded_by_wave_id IS NOT NULL;
CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(parent_wave_id,''),name) WHERE retired_at IS NULL;
CREATE INDEX waves_project_activation_process ON waves(project_activation_lf_process_id)
    WHERE project_activation_lf_process_id IS NOT NULL;
CREATE TRIGGER store_revision_waves_insert AFTER INSERT ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_update AFTER UPDATE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_delete AFTER DELETE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER selected_project_wave_update BEFORE UPDATE OF current_project_id ON waves
WHEN NEW.current_project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects WHERE id=NEW.current_project_id AND wave_id=NEW.id
) BEGIN SELECT RAISE(ABORT, 'selected Project belongs to another Wave'); END;
CREATE TRIGGER canonical_waves_insert BEFORE INSERT ON waves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: waves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.superseded_by_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_activation_lf_process_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.current_project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE VIEW wave_addresses AS
WITH RECURSIVE addresses(id,slug) AS (
    SELECT id,name FROM waves WHERE parent_wave_id IS NULL
    UNION ALL
    SELECT w.id,a.slug || '/' || w.name FROM waves w
    JOIN addresses a ON w.parent_wave_id=a.id
)
SELECT w.*,a.slug FROM waves w JOIN addresses a ON a.id=w.id;
