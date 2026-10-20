-- Starting work tells the planning provider: state deliveries may target started.

PRAGMA legacy_alter_table = ON;

CREATE TABLE task_state_deliveries_migration (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    move_seq INTEGER REFERENCES task_workflow_moves(seq),
    target TEXT NOT NULL CHECK(target IN ('completed','started','unstarted','canceled')),
    base_revision TEXT,
    base_state TEXT,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    settled INTEGER NOT NULL DEFAULT 0 CHECK(settled IN (0,1)),
    error TEXT,
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json))
);

INSERT INTO task_state_deliveries_migration (seq, id, task_id, move_seq, target, base_revision, base_state, attempted, settled, error, conflict_json)
SELECT seq, id, task_id, move_seq, target, base_revision, base_state, attempted, settled, error, conflict_json FROM task_state_deliveries;

DROP TABLE task_state_deliveries;

ALTER TABLE task_state_deliveries_migration RENAME TO task_state_deliveries;

PRAGMA legacy_alter_table = OFF;

CREATE INDEX task_state_deliveries_task ON task_state_deliveries(task_id,seq);

CREATE TRIGGER store_revision_task_state_deliveries_insert AFTER INSERT ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_state_deliveries_update AFTER UPDATE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_task_state_deliveries_delete AFTER DELETE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER canonical_task_state_deliveries_insert BEFORE INSERT ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
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

CREATE TRIGGER canonical_task_state_deliveries_update BEFORE UPDATE OF id,task_id ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
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
