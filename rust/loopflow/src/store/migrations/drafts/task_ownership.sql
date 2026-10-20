-- A Task is owned by (person, Machine) through an append-only log. Ownership
-- replaces the write-once Started time, inherited Work placement, and the
-- creating Machine's first-start admission.
CREATE TABLE task_ownership (
    id TEXT PRIMARY KEY CHECK (id GLOB 'own_*'),
    -- Entries outlive their Task, so the log carries no foreign key.
    task_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('claim', 'handoff', 'unclaim', 'recorded')),
    person TEXT,
    machine_id TEXT,
    at INTEGER NOT NULL,
    -- Provider order in milliseconds, stamped once when the provider reports it.
    ordered_at INTEGER,
    -- Machine-local: whether this Machine appended the entry.
    written_here INTEGER NOT NULL CHECK (written_here IN (0, 1)),
    CHECK (kind = 'recorded' OR machine_id IS NOT NULL)
);
CREATE INDEX idx_task_ownership_task ON task_ownership(task_id);
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;
CREATE TRIGGER task_ownership_retained BEFORE DELETE ON task_ownership
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are retained'); END;
CREATE TRIGGER store_revision_task_ownership_insert AFTER INSERT ON task_ownership
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_ownership_update AFTER UPDATE ON task_ownership
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- This person's Machine for each Wave's unowned Tasks. Personal, never planning
-- content: each of the person's Machines holds its own copy.
CREATE TABLE wave_machines (
    wave_id TEXT PRIMARY KEY,
    machine_id TEXT NOT NULL CHECK (machine_id GLOB 'home_*'),
    set_at INTEGER NOT NULL
);
CREATE TRIGGER store_revision_wave_machines_insert AFTER INSERT ON wave_machines
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_wave_machines_update AFTER UPDATE ON wave_machines
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_wave_machines_delete AFTER DELETE ON wave_machines
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- One entry records what each Machine knew. It invents no person, and a Machine
-- it cannot name stays unknown. These entries are local evidence, never delivered.
INSERT INTO task_ownership(id, task_id, kind, person, machine_id, at, ordered_at, written_here)
SELECT 'own_' || lower(hex(randomblob(16))), t.id, 'recorded', NULL,
    coalesce(a.machine_id, t.checkout_machine_id),
    coalesce(t.started_at, t.created_at),
    coalesce(t.started_at, t.created_at) * 1000,
    1
FROM tasks t LEFT JOIN task_admissions a ON a.task_id = t.id
WHERE t.started_at IS NOT NULL
   OR coalesce(t.worktree, '') != ''
   OR a.task_id IS NOT NULL;

DROP TRIGGER task_first_input;
DROP TRIGGER task_first_conversation_bind;
DROP TRIGGER task_first_agent_turn;
DROP TRIGGER validate_task_started_insert;
DROP TRIGGER validate_task_started_update;
DROP TRIGGER task_admissions_immutable;
DROP TRIGGER task_admissions_retained;
DROP TRIGGER store_revision_task_admissions_insert;
DROP TABLE task_admissions;
DROP TRIGGER store_revision_work_placements_insert;
DROP TRIGGER store_revision_work_placements_update;
DROP TRIGGER store_revision_work_placements_delete;
DROP INDEX idx_work_placements_machine;
DROP TABLE work_placements;
-- Canonical identity triggers name the admission column; rebuild them without it.
DROP TRIGGER canonical_tasks_insert;
DROP TRIGGER canonical_tasks_update;
ALTER TABLE tasks DROP COLUMN started_at;
ALTER TABLE tasks DROP COLUMN admission_authority;
CREATE TRIGGER canonical_tasks_insert BEFORE INSERT ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
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
CREATE TRIGGER canonical_tasks_update BEFORE UPDATE OF external_issue_id,planning_assignee,planning_team_id,id,project_id,checkout_machine_id,parent_pr_id,primary_session_id,issue_identifier,repository_id ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
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

-- Retired planning values have no decoder. Remove their journal history and
-- any export baseline that still names them.
DELETE FROM planning_peer_routes WHERE mutation_id IN (
    SELECT id FROM planning_peer_changes WHERE field IN
        ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority'));
DELETE FROM planning_peer_observed WHERE id IN (
    SELECT id FROM planning_peer_changes WHERE field IN
        ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority'));
DELETE FROM planning_peer_changes WHERE field IN
    ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority');
UPDATE project_transitions SET export_json = json_set(export_json, '$.baseline', (
        SELECT json_group_array(json(b.value)) FROM json_each(project_transitions.export_json, '$.baseline') b
        WHERE json_extract(b.value, '$.field') NOT IN
            ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority')))
WHERE json_type(export_json, '$.baseline') = 'array' AND EXISTS (
    SELECT 1 FROM json_each(project_transitions.export_json, '$.baseline') b
    WHERE json_extract(b.value, '$.field') IN
        ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority'));
UPDATE task_creation_intents SET export_json = json_set(export_json, '$.baseline', (
        SELECT json_group_array(json(b.value)) FROM json_each(task_creation_intents.export_json, '$.baseline') b
        WHERE json_extract(b.value, '$.field') NOT IN
            ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority')))
WHERE json_type(export_json, '$.baseline') = 'array' AND EXISTS (
    SELECT 1 FROM json_each(task_creation_intents.export_json, '$.baseline') b
    WHERE json_extract(b.value, '$.field') IN
        ('wave_delegation', 'project_delegation', 'task_delegation', 'task_admission_authority'));

-- A claim moves Linear to started; state delivery accepts that target.
CREATE TABLE task_state_deliveries_ownership (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    move_seq INTEGER REFERENCES task_workflow_moves(seq),
    target TEXT NOT NULL CHECK(target IN ('completed','unstarted','started','canceled')),
    base_revision TEXT,
    base_state TEXT,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    settled INTEGER NOT NULL DEFAULT 0 CHECK(settled IN (0,1)),
    error TEXT,
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json))
);
INSERT INTO task_state_deliveries_ownership(seq,id,task_id,move_seq,target,base_revision,base_state,attempted,settled,error,conflict_json) SELECT seq,id,task_id,move_seq,target,base_revision,base_state,attempted,settled,error,conflict_json FROM task_state_deliveries;
DROP TABLE task_state_deliveries;
ALTER TABLE task_state_deliveries_ownership RENAME TO task_state_deliveries;
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
