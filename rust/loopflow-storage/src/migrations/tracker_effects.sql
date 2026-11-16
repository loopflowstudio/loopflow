-- One outbound queue for every tracker bridge, in the store's words. Each row
-- addresses one owner (a Wave, Project or Task) on the bridge named by
-- `provider`; `seq` is the send order. `value_json` is a WorkValue (a `create`
-- holds its creation baseline, an `order` the desired Task ids), never
-- provider input. Settled rows (`acknowledged`, `retired`) stay as history.
CREATE TABLE tracker_effects (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    subject TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('create','field','order','state','comment','delete','link')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    phase TEXT NOT NULL CHECK(phase IN ('pending','attempted','acknowledged','retired')),
    provider TEXT NOT NULL CHECK(provider IN ('linear','asana')),
    remote_id TEXT,
    acknowledged_revision TEXT,
    error TEXT,
    attempted_at INTEGER,
    looked_at INTEGER,
    CHECK((wave_id IS NOT NULL)+(project_id IS NOT NULL)+(task_id IS NOT NULL)=1)
);
CREATE INDEX tracker_effects_unsettled ON tracker_effects(phase,seq)
    WHERE phase IN ('pending','attempted');
CREATE INDEX tracker_effects_task ON tracker_effects(task_id,subject,seq) WHERE task_id IS NOT NULL;
CREATE INDEX tracker_effects_project ON tracker_effects(project_id,subject,seq) WHERE project_id IS NOT NULL;
CREATE INDEX tracker_effects_wave ON tracker_effects(wave_id,subject,seq) WHERE wave_id IS NOT NULL;

CREATE TRIGGER store_revision_tracker_effects_insert AFTER INSERT ON tracker_effects
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
-- A look is a reading time, not a change: only NULL <-> value counts.
CREATE TRIGGER store_revision_tracker_effects_update AFTER UPDATE ON tracker_effects
WHEN NEW.seq IS NOT OLD.seq OR NEW.id IS NOT OLD.id OR
    NEW.wave_id IS NOT OLD.wave_id OR NEW.project_id IS NOT OLD.project_id OR
    NEW.task_id IS NOT OLD.task_id OR NEW.subject IS NOT OLD.subject OR
    NEW.kind IS NOT OLD.kind OR NEW.value_json IS NOT OLD.value_json OR
    NEW.base_json IS NOT OLD.base_json OR NEW.conflict_json IS NOT OLD.conflict_json OR
    NEW.phase IS NOT OLD.phase OR NEW.provider IS NOT OLD.provider OR
    NEW.remote_id IS NOT OLD.remote_id OR
    NEW.acknowledged_revision IS NOT OLD.acknowledged_revision OR
    NEW.error IS NOT OLD.error OR NEW.attempted_at IS NOT OLD.attempted_at OR
    (NEW.looked_at IS NULL) IS NOT (OLD.looked_at IS NULL)
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_tracker_effects_delete AFTER DELETE ON tracker_effects
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER canonical_tracker_effects_insert BEFORE INSERT ON tracker_effects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_effects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
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

CREATE TRIGGER canonical_tracker_effects_update BEFORE UPDATE OF id,wave_id,project_id,task_id,value_json,base_json,conflict_json ON tracker_effects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_effects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
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

-- Carry every unsettled receipt across, owners' creations first so they head
-- their owner. Released rows predate `provider`; every released bridge row was
-- written as Linear. A creation never prepared (`export_json IS NULL`) is left
-- to the drain's connect-time enqueue.
INSERT INTO tracker_effects(id,project_id,kind,value_json,phase,provider,error,looked_at)
SELECT lower(hex(randomblob(16))),c.successor_id,'create',json_extract(c.export_json,'$.baseline'),
    CASE WHEN c.export_attempted=1 THEN 'attempted' ELSE 'pending' END,'linear',c.export_error,
    c.export_observed_at
FROM project_transitions c JOIN projects p ON p.id=c.successor_id AND p.wave_id=c.wave_id
WHERE p.external_project_id IS NULL AND c.local_plan_json IS NOT NULL
    AND json_type(c.export_json,'$.baseline')='array';
INSERT INTO tracker_effects(id,project_id,subject,kind,value_json,phase,provider)
SELECT lower(hex(randomblob(16))),c.successor_id,'initiative','link',
    json_object('field','project_initiatives','value',json_array(json_extract(c.export_json,'$.initiative'))),
    CASE WHEN c.export_link_attempted=1 THEN 'attempted' ELSE 'pending' END,'linear'
FROM project_transitions c JOIN projects p ON p.id=c.successor_id AND p.wave_id=c.wave_id
WHERE p.external_project_id IS NULL AND c.local_plan_json IS NOT NULL
    AND json_type(c.export_json,'$.baseline')='array'
    AND json_extract(c.export_json,'$.initiative') IS NOT NULL;
INSERT INTO tracker_effects(id,task_id,kind,value_json,phase,provider,error,looked_at)
SELECT lower(hex(randomblob(16))),c.task_id,'create',json_extract(c.export_json,'$.baseline'),
    CASE WHEN c.export_attempted=1 THEN 'attempted' ELSE 'pending' END,'linear',c.export_error,
    c.export_observed_at
FROM task_creation_intents c JOIN tasks t ON t.id=c.task_id
WHERE t.external_issue_id IS NULL AND (t.planning_deleted_at IS NULL OR c.export_attempted=1)
    AND json_type(c.export_json,'$.baseline')='array';

-- Field edits and deletions: each attempted one (for its lost-reply look) and
-- the newest unsent one per field. An owner neither bound nor creating has
-- nothing to send; its creation will carry its current values.
INSERT INTO tracker_effects(id,task_id,subject,kind,value_json,base_json,phase,provider,error,acknowledged_revision)
SELECT c.id,c.task_id,CASE WHEN c.field='deleted' THEN NULL ELSE c.field END,
    CASE WHEN c.field='deleted' THEN 'delete' ELSE 'field' END,c.value_json,c.base_json,
    CASE WHEN c.attempted=1 THEN 'attempted' ELSE 'pending' END,'linear',c.error,c.acknowledged_revision
FROM task_changes c JOIN tasks t ON t.id=c.task_id
WHERE c.acknowledged=0 AND c.conflict_json IS NULL
    AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM task_changes WHERE task_id=c.task_id AND field=c.field))
    AND (t.external_issue_id IS NOT NULL OR EXISTS(SELECT 1 FROM tracker_effects e WHERE e.task_id=c.task_id AND e.kind='create'))
ORDER BY c.seq;
INSERT INTO tracker_effects(id,project_id,subject,kind,value_json,base_json,phase,provider,error,acknowledged_revision)
SELECT c.id,c.project_id,CASE WHEN c.field='task_order' THEN NULL ELSE c.field END,
    CASE WHEN c.field='task_order' THEN 'order' ELSE 'field' END,c.value_json,c.base_json,
    CASE WHEN c.attempted=1 THEN 'attempted' ELSE 'pending' END,'linear',c.error,c.acknowledged_revision
FROM project_changes c JOIN projects p ON p.id=c.project_id
WHERE c.acknowledged=0 AND c.conflict_json IS NULL
    AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM project_changes WHERE project_id=c.project_id AND field=c.field))
    AND (p.external_project_id IS NOT NULL OR EXISTS(SELECT 1 FROM tracker_effects e WHERE e.project_id=c.project_id AND e.kind='create'))
ORDER BY c.seq;

-- State: the target as a Disposition, against the state last observed.
INSERT INTO tracker_effects(id,task_id,kind,value_json,base_json,phase,provider,error)
SELECT d.id,d.task_id,'state',
    json_object('field','disposition','value',json_object('planning_state',d.target,
        'planning_completed',json(CASE WHEN d.target='completed' THEN 'true' ELSE 'false' END),
        'planning_completed_at',CASE WHEN d.target='completed' THEN t.planning_completed_at END)),
    CASE WHEN d.base_state IS NULL AND d.base_revision IS NULL THEN NULL ELSE
    json_object('revision',d.base_revision,'value',CASE WHEN d.base_state IS NULL THEN NULL ELSE
        json_object('field','disposition','value',json_object('planning_state',d.base_state,
            'planning_completed',json(CASE WHEN d.base_state='completed' THEN 'true' ELSE 'false' END),
            'planning_completed_at',NULL)) END) END,
    CASE WHEN d.attempted=1 THEN 'attempted' ELSE 'pending' END,'linear',d.error
FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
WHERE d.settled=0 AND d.conflict_json IS NULL
    AND (d.attempted=1 OR d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=d.task_id))
    AND (t.external_issue_id IS NOT NULL OR EXISTS(SELECT 1 FROM tracker_effects e WHERE e.task_id=d.task_id AND e.kind='create'))
ORDER BY d.seq;

-- Comments carry no attempt flag; each is treated as sent with its reply
-- unknown, which a Linear resend (same id) or an Asana text match settles.
INSERT INTO tracker_effects(id,task_id,subject,kind,value_json,phase,provider,error)
SELECT lower(hex(randomblob(16))),c.task_id,d.comment_id,'comment',
    json_object('field','comment_content','value',json_object('body',json_extract(d.comment_json,'$.body'),
        'author',json(json_extract(d.comment_json,'$.author')),'created_at',json_extract(d.comment_json,'$.created_at'))),
    'attempted','linear',d.error
FROM task_comment_deliveries d JOIN task_comments c ON c.id=d.comment_id JOIN tasks t ON t.id=c.task_id
WHERE d.acknowledged=0 AND d.conflicting_comment_json IS NULL
    AND (t.external_issue_id IS NOT NULL OR EXISTS(SELECT 1 FROM tracker_effects e WHERE e.task_id=c.task_id AND e.kind='create'))
ORDER BY d.rowid;

-- An older lf still running on this store reads the old receipts. Mark every
-- unsettled one settled so it sends nothing this queue now owns. A creation has
-- no settled flag: it becomes attempted and looked at far in the future, so the
-- older lf neither sends it nor looks for it. The tables drop in a contract
-- migration.
UPDATE task_changes SET acknowledged=1 WHERE acknowledged=0 AND conflict_json IS NULL;
UPDATE project_changes SET acknowledged=1 WHERE acknowledged=0 AND conflict_json IS NULL;
UPDATE task_state_deliveries SET settled=1 WHERE settled=0;
UPDATE task_comment_deliveries SET acknowledged=1 WHERE acknowledged=0;
UPDATE task_creation_intents SET export_attempted=1,export_observed_at=9007199254740991
WHERE export_json IS NOT NULL
    AND task_id IN (SELECT id FROM tasks WHERE external_issue_id IS NULL);
UPDATE project_transitions SET export_attempted=1,export_link_attempted=1,export_observed_at=9007199254740991
WHERE export_json IS NOT NULL
    AND successor_id IN (SELECT id FROM projects WHERE external_project_id IS NULL);
