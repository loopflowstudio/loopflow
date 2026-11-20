-- Name the tracker tables for what they hold (LOO-955). Each `pm_*` and
-- `planning_peer_*` table gets its tracker name; `provider` columns become
-- `tracker` and a destination's `acquisition_error` its `fetch_error`.
--
-- Expand step: a released lf still reads and writes the old names, so each old
-- table stays beside its new one and mirror triggers copy every write both
-- ways. A mirrored write that changes nothing writes nothing, which ends the
-- round trip (the store leaves `recursive_triggers` off). Identity and
-- revision triggers move to the new tables, so every change is checked and
-- counted once. A contract drops the old tables and their mirrors once
-- released lf no longer writes them.

CREATE TABLE git_tracker_destinations (
    repo TEXT NOT NULL,
    id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    reference TEXT NOT NULL,
    fetched_revision TEXT,
    fetch_error TEXT,
    publication_revision TEXT,
    publication_state TEXT CHECK(publication_state IN ('pending','unconfirmed','confirmed')),
    publication_digest TEXT,
    publication_error TEXT,
    PRIMARY KEY(repo,id)
);
CREATE TABLE git_tracker_changes (
    id TEXT PRIMARY KEY,
    record_table TEXT NOT NULL CHECK(record_table IN ('waves','projects','tasks','task_comments')),
    mutation TEXT NOT NULL CHECK(json_valid(mutation)),
    object_id TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.target')) VIRTUAL NOT NULL,
    field TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.change.field')) VIRTUAL NOT NULL,
    clock INTEGER GENERATED ALWAYS AS (json_extract(mutation,'$.clock')) VIRTUAL NOT NULL CHECK(clock>=0)
);
CREATE INDEX git_tracker_changes_clock ON git_tracker_changes(clock);
CREATE INDEX git_tracker_changes_object ON git_tracker_changes(object_id,field);
CREATE TABLE git_tracker_seen (
    id TEXT PRIMARY KEY REFERENCES git_tracker_changes(id)
);
CREATE TABLE git_tracker_routes (
    mutation_id TEXT NOT NULL REFERENCES git_tracker_changes(id),
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    PRIMARY KEY(repo,destination,mutation_id),
    FOREIGN KEY(repo,destination) REFERENCES git_tracker_destinations(repo,id)
);
CREATE INDEX git_tracker_routes_mutation ON git_tracker_routes(mutation_id);
CREATE TABLE git_tracker_imports (
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    revision TEXT NOT NULL,
    PRIMARY KEY(repo,destination)
);
CREATE TABLE git_tracker_conflicts (
    repo TEXT NOT NULL,
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    PRIMARY KEY(kind,object_id,reason)
);
CREATE TABLE tracker_projects (
    repo TEXT NOT NULL,
    tracker TEXT NOT NULL,
    id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    membership_unresolved INTEGER NOT NULL DEFAULT 0 CHECK(membership_unresolved IN (0,1)),
    task_order_json TEXT CHECK(task_order_json IS NULL OR json_valid(task_order_json)),
    revision TEXT,
    slug TEXT,
    name TEXT,
    summary TEXT,
    status TEXT,
    krs TEXT,
    metric_targets TEXT,
    initiative_ids TEXT,
    team_ids TEXT,
    description TEXT,
    description_revision TEXT,
    description_observed_at INTEGER,
    PRIMARY KEY(repo, tracker, id)
);
CREATE TABLE tracker_items (
    repo TEXT NOT NULL,
    tracker TEXT NOT NULL,
    id TEXT NOT NULL,
    identifier TEXT NOT NULL COLLATE NOCASE,
    project_id TEXT,
    observed_at INTEGER NOT NULL,
    needs_refresh INTEGER NOT NULL DEFAULT 0 CHECK(needs_refresh IN (0,1)),
    revision TEXT,
    branch_name TEXT,
    url TEXT,
    name TEXT,
    description TEXT,
    rank INTEGER,
    completed INTEGER,
    completed_at TEXT,
    due_date TEXT,
    state TEXT,
    team_id TEXT,
    assignee TEXT,
    completed_at_observed INTEGER NOT NULL DEFAULT 0,
    due_date_observed INTEGER NOT NULL DEFAULT 0,
    assignee_observed INTEGER NOT NULL DEFAULT 0,
    claimed_project_id TEXT,
    project_id_observed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(repo, tracker, id)
);
CREATE INDEX tracker_items_identifier ON tracker_items(repo, tracker, identifier);
CREATE INDEX tracker_items_project ON tracker_items(repo, tracker, project_id);
CREATE TABLE tracker_events (
    issue_id TEXT PRIMARY KEY,
    revision_ns INTEGER,
    removed INTEGER NOT NULL CHECK(removed IN (0,1))
);
CREATE TABLE tracker_wave_projects (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(wave_id, project_id)
);
CREATE TABLE tracker_wave_sync (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    tracker TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL
);

INSERT INTO git_tracker_destinations(repo,id,endpoint,reference,fetched_revision,fetch_error,publication_revision,publication_state,publication_digest,publication_error) SELECT repo,id,endpoint,reference,fetched_revision,acquisition_error,publication_revision,publication_state,publication_digest,publication_error FROM planning_destinations;
INSERT INTO git_tracker_changes(id,record_table,mutation) SELECT id,record_table,mutation FROM planning_peer_changes;
INSERT INTO git_tracker_seen(id) SELECT id FROM planning_peer_observed;
INSERT INTO git_tracker_routes(mutation_id,repo,destination) SELECT mutation_id,repo,destination FROM planning_peer_routes;
INSERT INTO git_tracker_imports(repo,destination,revision) SELECT repo,destination,revision FROM planning_peer_imports;
INSERT INTO git_tracker_conflicts(repo,kind,object_id,reason,active) SELECT repo,kind,object_id,reason,active FROM planning_peer_conflicts;
INSERT INTO tracker_projects(repo,tracker,id,observed_at,archived,membership_unresolved,task_order_json,revision,slug,name,summary,status,krs,metric_targets,initiative_ids,team_ids,description,description_revision,description_observed_at) SELECT repo,provider,id,observed_at,archived,membership_unresolved,task_order_json,revision,slug,name,summary,status,krs,metric_targets,initiative_ids,team_ids,description,description_revision,description_observed_at FROM pm_projects;
INSERT INTO tracker_items(repo,tracker,id,identifier,project_id,observed_at,needs_refresh,revision,branch_name,url,name,description,rank,completed,completed_at,due_date,state,team_id,assignee,completed_at_observed,due_date_observed,assignee_observed,claimed_project_id,project_id_observed) SELECT repo,provider,id,identifier,project_id,observed_at,needs_refresh,revision,branch_name,url,name,description,rank,completed,completed_at,due_date,state,team_id,assignee,completed_at_observed,due_date_observed,assignee_observed,claimed_project_id,project_id_observed FROM pm_items;
INSERT INTO tracker_events(issue_id,revision_ns,removed) SELECT issue_id,revision_ns,removed FROM pm_issue_changes;
INSERT INTO tracker_wave_projects(wave_id,project_id,position) SELECT wave_id,project_id,position FROM pm_wave_projects;
INSERT INTO tracker_wave_sync(wave_id,tracker,initiative,synced_at) SELECT wave_id,provider,initiative,synced_at FROM pm_wave_sync;

DROP TRIGGER store_revision_planning_destinations_delete;
DROP TRIGGER store_revision_planning_destinations_insert;
DROP TRIGGER store_revision_planning_destinations_update;
DROP TRIGGER canonical_planning_peer_changes_insert;
DROP TRIGGER store_revision_planning_peer_routes_delete;
DROP TRIGGER store_revision_planning_peer_routes_insert;
DROP TRIGGER store_revision_planning_peer_routes_update;
DROP TRIGGER store_revision_planning_peer_imports_delete;
DROP TRIGGER store_revision_planning_peer_imports_insert;
DROP TRIGGER store_revision_planning_peer_imports_update;
DROP TRIGGER canonical_planning_peer_conflicts_insert;
DROP TRIGGER store_revision_planning_peer_conflicts_delete;
DROP TRIGGER store_revision_planning_peer_conflicts_insert;
DROP TRIGGER store_revision_planning_peer_conflicts_update;
DROP TRIGGER canonical_pm_projects_insert;
DROP TRIGGER canonical_pm_projects_update;
DROP TRIGGER store_revision_pm_projects_delete;
DROP TRIGGER store_revision_pm_projects_insert;
DROP TRIGGER store_revision_pm_projects_update;
DROP TRIGGER canonical_pm_items_insert;
DROP TRIGGER canonical_pm_items_update;
DROP TRIGGER store_revision_pm_items_delete;
DROP TRIGGER store_revision_pm_items_insert;
DROP TRIGGER store_revision_pm_items_update;
DROP TRIGGER canonical_pm_issue_changes_insert;
DROP TRIGGER store_revision_pm_issue_changes_delete;
DROP TRIGGER store_revision_pm_issue_changes_insert;
DROP TRIGGER store_revision_pm_issue_changes_update;
DROP TRIGGER canonical_pm_wave_projects_insert;
DROP TRIGGER store_revision_pm_wave_projects_delete;
DROP TRIGGER store_revision_pm_wave_projects_insert;
DROP TRIGGER store_revision_pm_wave_projects_update;
DROP TRIGGER canonical_pm_wave_sync_insert;
DROP TRIGGER canonical_pm_wave_sync_update;
DROP TRIGGER store_revision_pm_wave_sync_delete;
DROP TRIGGER store_revision_pm_wave_sync_insert;
DROP TRIGGER store_revision_pm_wave_sync_update;

CREATE TRIGGER store_revision_git_tracker_destinations_delete AFTER DELETE ON git_tracker_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_destinations_insert AFTER INSERT ON git_tracker_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_destinations_update AFTER UPDATE ON git_tracker_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER canonical_git_tracker_changes_insert BEFORE INSERT ON git_tracker_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: git_tracker_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.edit.target' AND NEW.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND NEW.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND NEW.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND NEW.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.mutation AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.mutation) THEN NEW.mutation END) WHERE type='text')
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
CREATE TRIGGER store_revision_git_tracker_routes_delete AFTER DELETE ON git_tracker_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_routes_insert AFTER INSERT ON git_tracker_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_routes_update AFTER UPDATE ON git_tracker_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_imports_delete AFTER DELETE ON git_tracker_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_imports_insert AFTER INSERT ON git_tracker_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_imports_update AFTER UPDATE ON git_tracker_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER canonical_git_tracker_conflicts_insert BEFORE INSERT ON git_tracker_conflicts
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: git_tracker_conflicts') FROM (
 WITH incoming AS (
SELECT CASE NEW.kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AS domain,NEW.object_id AS original
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
CREATE TRIGGER store_revision_git_tracker_conflicts_delete AFTER DELETE ON git_tracker_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_conflicts_insert AFTER INSERT ON git_tracker_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_git_tracker_conflicts_update AFTER UPDATE ON git_tracker_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER canonical_tracker_projects_insert BEFORE INSERT ON tracker_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
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
CREATE TRIGGER canonical_tracker_projects_update BEFORE UPDATE OF id,initiative_ids,team_ids,task_order_json ON tracker_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
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
CREATE TRIGGER store_revision_tracker_projects_delete AFTER DELETE ON tracker_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_projects_insert AFTER INSERT ON tracker_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_projects_update AFTER UPDATE ON tracker_projects
WHEN NEW.repo IS NOT OLD.repo OR NEW.tracker IS NOT OLD.tracker OR
    NEW.id IS NOT OLD.id OR NEW.archived IS NOT OLD.archived OR
    NEW.membership_unresolved IS NOT OLD.membership_unresolved OR
    NEW.task_order_json IS NOT OLD.task_order_json OR
    NEW.revision IS NOT OLD.revision OR NEW.slug IS NOT OLD.slug OR
    NEW.name IS NOT OLD.name OR NEW.summary IS NOT OLD.summary OR
    NEW.status IS NOT OLD.status OR NEW.krs IS NOT OLD.krs OR
    NEW.metric_targets IS NOT OLD.metric_targets OR
    NEW.initiative_ids IS NOT OLD.initiative_ids OR
    NEW.team_ids IS NOT OLD.team_ids OR NEW.description IS NOT OLD.description OR
    NEW.description_revision IS NOT OLD.description_revision OR
    (NEW.observed_at IS NULL) IS NOT (OLD.observed_at IS NULL) OR
    (NEW.description_observed_at IS NULL) IS NOT (OLD.description_observed_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER canonical_tracker_items_insert BEFORE INSERT ON tracker_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
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
CREATE TRIGGER canonical_tracker_items_update BEFORE UPDATE OF id,project_id,team_id,assignee,claimed_project_id,identifier ON tracker_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
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
CREATE TRIGGER store_revision_tracker_items_delete AFTER DELETE ON tracker_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_items_insert AFTER INSERT ON tracker_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_items_update AFTER UPDATE ON tracker_items
WHEN NEW.repo IS NOT OLD.repo OR NEW.tracker IS NOT OLD.tracker OR
    NEW.id IS NOT OLD.id OR NEW.identifier IS NOT OLD.identifier COLLATE BINARY OR
    NEW.project_id IS NOT OLD.project_id OR
    NEW.needs_refresh IS NOT OLD.needs_refresh OR
    NEW.revision IS NOT OLD.revision OR NEW.branch_name IS NOT OLD.branch_name OR
    NEW.url IS NOT OLD.url OR NEW.name IS NOT OLD.name OR
    NEW.description IS NOT OLD.description OR NEW.rank IS NOT OLD.rank OR
    NEW.completed IS NOT OLD.completed OR
    NEW.completed_at IS NOT OLD.completed_at OR NEW.due_date IS NOT OLD.due_date OR
    NEW.state IS NOT OLD.state OR NEW.team_id IS NOT OLD.team_id OR
    NEW.assignee IS NOT OLD.assignee OR
    NEW.completed_at_observed IS NOT OLD.completed_at_observed OR
    NEW.due_date_observed IS NOT OLD.due_date_observed OR
    NEW.assignee_observed IS NOT OLD.assignee_observed OR
    NEW.claimed_project_id IS NOT OLD.claimed_project_id OR
    NEW.project_id_observed IS NOT OLD.project_id_observed OR
    (NEW.observed_at IS NULL) IS NOT (OLD.observed_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER canonical_tracker_events_insert BEFORE INSERT ON tracker_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.issue_id AS original
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
CREATE TRIGGER store_revision_tracker_events_delete AFTER DELETE ON tracker_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_events_insert AFTER INSERT ON tracker_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_events_update AFTER UPDATE ON tracker_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER canonical_tracker_wave_projects_insert BEFORE INSERT ON tracker_wave_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_wave_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
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
CREATE TRIGGER store_revision_tracker_wave_projects_delete AFTER DELETE ON tracker_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_wave_projects_insert AFTER INSERT ON tracker_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_wave_projects_update AFTER UPDATE ON tracker_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER canonical_tracker_wave_sync_insert BEFORE INSERT ON tracker_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
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
CREATE TRIGGER canonical_tracker_wave_sync_update BEFORE UPDATE OF wave_id,initiative ON tracker_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tracker_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
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
CREATE TRIGGER store_revision_tracker_wave_sync_delete AFTER DELETE ON tracker_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_wave_sync_insert AFTER INSERT ON tracker_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tracker_wave_sync_update AFTER UPDATE ON tracker_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;

CREATE TRIGGER mirror_planning_destinations_insert AFTER INSERT ON planning_destinations
BEGIN
    INSERT INTO git_tracker_destinations(repo, id, endpoint, reference, fetched_revision,
        fetch_error, publication_revision, publication_state, publication_digest, publication_error)
    VALUES(NEW.repo, NEW.id, NEW.endpoint, NEW.reference, NEW.fetched_revision,
        NEW.acquisition_error, NEW.publication_revision, NEW.publication_state,
        NEW.publication_digest, NEW.publication_error)
    ON CONFLICT(repo, id) DO UPDATE
    SET endpoint=excluded.endpoint, reference=excluded.reference,
        fetched_revision=excluded.fetched_revision, fetch_error=excluded.fetch_error,
        publication_revision=excluded.publication_revision,
        publication_state=excluded.publication_state,
        publication_digest=excluded.publication_digest, publication_error=excluded.publication_error
    WHERE endpoint IS NOT excluded.endpoint OR reference IS NOT excluded.reference OR
        fetched_revision IS NOT excluded.fetched_revision OR fetch_error IS NOT excluded.fetch_error
        OR publication_revision IS NOT excluded.publication_revision OR publication_state IS NOT
        excluded.publication_state OR publication_digest IS NOT excluded.publication_digest OR
        publication_error IS NOT excluded.publication_error;
END;

CREATE TRIGGER mirror_planning_destinations_update AFTER UPDATE ON planning_destinations
BEGIN
    UPDATE git_tracker_destinations
    SET repo=NEW.repo, id=NEW.id, endpoint=NEW.endpoint, reference=NEW.reference,
        fetched_revision=NEW.fetched_revision, fetch_error=NEW.acquisition_error,
        publication_revision=NEW.publication_revision, publication_state=NEW.publication_state,
        publication_digest=NEW.publication_digest, publication_error=NEW.publication_error
    WHERE repo=OLD.repo AND id=OLD.id AND (repo IS NOT NEW.repo OR id IS NOT NEW.id OR endpoint IS
        NOT NEW.endpoint OR reference IS NOT NEW.reference OR fetched_revision IS NOT
        NEW.fetched_revision OR fetch_error IS NOT NEW.acquisition_error OR publication_revision IS
        NOT NEW.publication_revision OR publication_state IS NOT NEW.publication_state OR
        publication_digest IS NOT NEW.publication_digest OR publication_error IS NOT
        NEW.publication_error);
END;

CREATE TRIGGER mirror_planning_destinations_delete AFTER DELETE ON planning_destinations
BEGIN
    DELETE FROM git_tracker_destinations
    WHERE repo=OLD.repo AND id=OLD.id;
END;

CREATE TRIGGER mirror_git_tracker_destinations_insert AFTER INSERT ON git_tracker_destinations
BEGIN
    INSERT INTO planning_destinations(repo, id, endpoint, reference, fetched_revision,
        acquisition_error, publication_revision, publication_state, publication_digest,
        publication_error)
    VALUES(NEW.repo, NEW.id, NEW.endpoint, NEW.reference, NEW.fetched_revision, NEW.fetch_error,
        NEW.publication_revision, NEW.publication_state, NEW.publication_digest,
        NEW.publication_error)
    ON CONFLICT(repo, id) DO UPDATE
    SET endpoint=excluded.endpoint, reference=excluded.reference,
        fetched_revision=excluded.fetched_revision, acquisition_error=excluded.acquisition_error,
        publication_revision=excluded.publication_revision,
        publication_state=excluded.publication_state,
        publication_digest=excluded.publication_digest, publication_error=excluded.publication_error
    WHERE endpoint IS NOT excluded.endpoint OR reference IS NOT excluded.reference OR
        fetched_revision IS NOT excluded.fetched_revision OR acquisition_error IS NOT
        excluded.acquisition_error OR publication_revision IS NOT excluded.publication_revision OR
        publication_state IS NOT excluded.publication_state OR publication_digest IS NOT
        excluded.publication_digest OR publication_error IS NOT excluded.publication_error;
END;

CREATE TRIGGER mirror_git_tracker_destinations_update AFTER UPDATE ON git_tracker_destinations
BEGIN
    UPDATE planning_destinations
    SET repo=NEW.repo, id=NEW.id, endpoint=NEW.endpoint, reference=NEW.reference,
        fetched_revision=NEW.fetched_revision, acquisition_error=NEW.fetch_error,
        publication_revision=NEW.publication_revision, publication_state=NEW.publication_state,
        publication_digest=NEW.publication_digest, publication_error=NEW.publication_error
    WHERE repo=OLD.repo AND id=OLD.id AND (repo IS NOT NEW.repo OR id IS NOT NEW.id OR endpoint IS
        NOT NEW.endpoint OR reference IS NOT NEW.reference OR fetched_revision IS NOT
        NEW.fetched_revision OR acquisition_error IS NOT NEW.fetch_error OR publication_revision IS
        NOT NEW.publication_revision OR publication_state IS NOT NEW.publication_state OR
        publication_digest IS NOT NEW.publication_digest OR publication_error IS NOT
        NEW.publication_error);
END;

CREATE TRIGGER mirror_git_tracker_destinations_delete AFTER DELETE ON git_tracker_destinations
BEGIN
    DELETE FROM planning_destinations
    WHERE repo=OLD.repo AND id=OLD.id;
END;

CREATE TRIGGER mirror_planning_peer_changes_insert AFTER INSERT ON planning_peer_changes
BEGIN
    INSERT INTO git_tracker_changes(id, record_table, mutation)
    VALUES(NEW.id, NEW.record_table, NEW.mutation)
    ON CONFLICT(id) DO UPDATE
    SET record_table=excluded.record_table, mutation=excluded.mutation
    WHERE record_table IS NOT excluded.record_table OR mutation IS NOT excluded.mutation;
END;

CREATE TRIGGER mirror_planning_peer_changes_update AFTER UPDATE ON planning_peer_changes
BEGIN
    UPDATE git_tracker_changes
    SET id=NEW.id, record_table=NEW.record_table, mutation=NEW.mutation
    WHERE id=OLD.id AND (id IS NOT NEW.id OR record_table IS NOT NEW.record_table OR mutation IS NOT
        NEW.mutation);
END;

CREATE TRIGGER mirror_planning_peer_changes_delete AFTER DELETE ON planning_peer_changes
BEGIN
    DELETE FROM git_tracker_changes
    WHERE id=OLD.id;
END;

CREATE TRIGGER mirror_git_tracker_changes_insert AFTER INSERT ON git_tracker_changes
BEGIN
    INSERT INTO planning_peer_changes(id, record_table, mutation)
    VALUES(NEW.id, NEW.record_table, NEW.mutation)
    ON CONFLICT(id) DO UPDATE
    SET record_table=excluded.record_table, mutation=excluded.mutation
    WHERE record_table IS NOT excluded.record_table OR mutation IS NOT excluded.mutation;
END;

CREATE TRIGGER mirror_git_tracker_changes_update AFTER UPDATE ON git_tracker_changes
BEGIN
    UPDATE planning_peer_changes
    SET id=NEW.id, record_table=NEW.record_table, mutation=NEW.mutation
    WHERE id=OLD.id AND (id IS NOT NEW.id OR record_table IS NOT NEW.record_table OR mutation IS NOT
        NEW.mutation);
END;

CREATE TRIGGER mirror_git_tracker_changes_delete AFTER DELETE ON git_tracker_changes
BEGIN
    DELETE FROM planning_peer_changes
    WHERE id=OLD.id;
END;

CREATE TRIGGER mirror_planning_peer_observed_insert AFTER INSERT ON planning_peer_observed
BEGIN
    INSERT INTO git_tracker_seen(id)
    VALUES(NEW.id)
    ON CONFLICT(id) DO NOTHING;
END;

CREATE TRIGGER mirror_planning_peer_observed_update AFTER UPDATE ON planning_peer_observed
BEGIN
    UPDATE git_tracker_seen
    SET id=NEW.id
    WHERE id=OLD.id AND (id IS NOT NEW.id);
END;

CREATE TRIGGER mirror_planning_peer_observed_delete AFTER DELETE ON planning_peer_observed
BEGIN
    DELETE FROM git_tracker_seen
    WHERE id=OLD.id;
END;

CREATE TRIGGER mirror_git_tracker_seen_insert AFTER INSERT ON git_tracker_seen
BEGIN
    INSERT INTO planning_peer_observed(id)
    VALUES(NEW.id)
    ON CONFLICT(id) DO NOTHING;
END;

CREATE TRIGGER mirror_git_tracker_seen_update AFTER UPDATE ON git_tracker_seen
BEGIN
    UPDATE planning_peer_observed
    SET id=NEW.id
    WHERE id=OLD.id AND (id IS NOT NEW.id);
END;

CREATE TRIGGER mirror_git_tracker_seen_delete AFTER DELETE ON git_tracker_seen
BEGIN
    DELETE FROM planning_peer_observed
    WHERE id=OLD.id;
END;

CREATE TRIGGER mirror_planning_peer_routes_insert AFTER INSERT ON planning_peer_routes
BEGIN
    INSERT INTO git_tracker_routes(mutation_id, repo, destination)
    VALUES(NEW.mutation_id, NEW.repo, NEW.destination)
    ON CONFLICT(repo, destination, mutation_id) DO NOTHING;
END;

CREATE TRIGGER mirror_planning_peer_routes_update AFTER UPDATE ON planning_peer_routes
BEGIN
    UPDATE git_tracker_routes
    SET mutation_id=NEW.mutation_id, repo=NEW.repo, destination=NEW.destination
    WHERE repo=OLD.repo AND destination=OLD.destination AND mutation_id=OLD.mutation_id AND
        (mutation_id IS NOT NEW.mutation_id OR repo IS NOT NEW.repo OR destination IS NOT
        NEW.destination);
END;

CREATE TRIGGER mirror_planning_peer_routes_delete AFTER DELETE ON planning_peer_routes
BEGIN
    DELETE FROM git_tracker_routes
    WHERE repo=OLD.repo AND destination=OLD.destination AND mutation_id=OLD.mutation_id;
END;

CREATE TRIGGER mirror_git_tracker_routes_insert AFTER INSERT ON git_tracker_routes
BEGIN
    INSERT INTO planning_peer_routes(mutation_id, repo, destination)
    VALUES(NEW.mutation_id, NEW.repo, NEW.destination)
    ON CONFLICT(repo, destination, mutation_id) DO NOTHING;
END;

CREATE TRIGGER mirror_git_tracker_routes_update AFTER UPDATE ON git_tracker_routes
BEGIN
    UPDATE planning_peer_routes
    SET mutation_id=NEW.mutation_id, repo=NEW.repo, destination=NEW.destination
    WHERE repo=OLD.repo AND destination=OLD.destination AND mutation_id=OLD.mutation_id AND
        (mutation_id IS NOT NEW.mutation_id OR repo IS NOT NEW.repo OR destination IS NOT
        NEW.destination);
END;

CREATE TRIGGER mirror_git_tracker_routes_delete AFTER DELETE ON git_tracker_routes
BEGIN
    DELETE FROM planning_peer_routes
    WHERE repo=OLD.repo AND destination=OLD.destination AND mutation_id=OLD.mutation_id;
END;

CREATE TRIGGER mirror_planning_peer_imports_insert AFTER INSERT ON planning_peer_imports
BEGIN
    INSERT INTO git_tracker_imports(repo, destination, revision)
    VALUES(NEW.repo, NEW.destination, NEW.revision)
    ON CONFLICT(repo, destination) DO UPDATE
    SET revision=excluded.revision
    WHERE revision IS NOT excluded.revision;
END;

CREATE TRIGGER mirror_planning_peer_imports_update AFTER UPDATE ON planning_peer_imports
BEGIN
    UPDATE git_tracker_imports
    SET repo=NEW.repo, destination=NEW.destination, revision=NEW.revision
    WHERE repo=OLD.repo AND destination=OLD.destination AND (repo IS NOT NEW.repo OR destination IS
        NOT NEW.destination OR revision IS NOT NEW.revision);
END;

CREATE TRIGGER mirror_planning_peer_imports_delete AFTER DELETE ON planning_peer_imports
BEGIN
    DELETE FROM git_tracker_imports
    WHERE repo=OLD.repo AND destination=OLD.destination;
END;

CREATE TRIGGER mirror_git_tracker_imports_insert AFTER INSERT ON git_tracker_imports
BEGIN
    INSERT INTO planning_peer_imports(repo, destination, revision)
    VALUES(NEW.repo, NEW.destination, NEW.revision)
    ON CONFLICT(repo, destination) DO UPDATE
    SET revision=excluded.revision
    WHERE revision IS NOT excluded.revision;
END;

CREATE TRIGGER mirror_git_tracker_imports_update AFTER UPDATE ON git_tracker_imports
BEGIN
    UPDATE planning_peer_imports
    SET repo=NEW.repo, destination=NEW.destination, revision=NEW.revision
    WHERE repo=OLD.repo AND destination=OLD.destination AND (repo IS NOT NEW.repo OR destination IS
        NOT NEW.destination OR revision IS NOT NEW.revision);
END;

CREATE TRIGGER mirror_git_tracker_imports_delete AFTER DELETE ON git_tracker_imports
BEGIN
    DELETE FROM planning_peer_imports
    WHERE repo=OLD.repo AND destination=OLD.destination;
END;

CREATE TRIGGER mirror_planning_peer_conflicts_insert AFTER INSERT ON planning_peer_conflicts
BEGIN
    INSERT INTO git_tracker_conflicts(repo, kind, object_id, reason, active)
    VALUES(NEW.repo, NEW.kind, NEW.object_id, NEW.reason, NEW.active)
    ON CONFLICT(kind, object_id, reason) DO UPDATE
    SET repo=excluded.repo, active=excluded.active
    WHERE repo IS NOT excluded.repo OR active IS NOT excluded.active;
END;

CREATE TRIGGER mirror_planning_peer_conflicts_update AFTER UPDATE ON planning_peer_conflicts
BEGIN
    UPDATE git_tracker_conflicts
    SET repo=NEW.repo, kind=NEW.kind, object_id=NEW.object_id, reason=NEW.reason, active=NEW.active
    WHERE kind=OLD.kind AND object_id=OLD.object_id AND reason=OLD.reason AND (repo IS NOT NEW.repo
        OR kind IS NOT NEW.kind OR object_id IS NOT NEW.object_id OR reason IS NOT NEW.reason OR
        active IS NOT NEW.active);
END;

CREATE TRIGGER mirror_planning_peer_conflicts_delete AFTER DELETE ON planning_peer_conflicts
BEGIN
    DELETE FROM git_tracker_conflicts
    WHERE kind=OLD.kind AND object_id=OLD.object_id AND reason=OLD.reason;
END;

CREATE TRIGGER mirror_git_tracker_conflicts_insert AFTER INSERT ON git_tracker_conflicts
BEGIN
    INSERT INTO planning_peer_conflicts(repo, kind, object_id, reason, active)
    VALUES(NEW.repo, NEW.kind, NEW.object_id, NEW.reason, NEW.active)
    ON CONFLICT(kind, object_id, reason) DO UPDATE
    SET repo=excluded.repo, active=excluded.active
    WHERE repo IS NOT excluded.repo OR active IS NOT excluded.active;
END;

CREATE TRIGGER mirror_git_tracker_conflicts_update AFTER UPDATE ON git_tracker_conflicts
BEGIN
    UPDATE planning_peer_conflicts
    SET repo=NEW.repo, kind=NEW.kind, object_id=NEW.object_id, reason=NEW.reason, active=NEW.active
    WHERE kind=OLD.kind AND object_id=OLD.object_id AND reason=OLD.reason AND (repo IS NOT NEW.repo
        OR kind IS NOT NEW.kind OR object_id IS NOT NEW.object_id OR reason IS NOT NEW.reason OR
        active IS NOT NEW.active);
END;

CREATE TRIGGER mirror_git_tracker_conflicts_delete AFTER DELETE ON git_tracker_conflicts
BEGIN
    DELETE FROM planning_peer_conflicts
    WHERE kind=OLD.kind AND object_id=OLD.object_id AND reason=OLD.reason;
END;

CREATE TRIGGER mirror_pm_projects_insert AFTER INSERT ON pm_projects
BEGIN
    INSERT INTO tracker_projects(repo, tracker, id, observed_at, archived, membership_unresolved,
        task_order_json, revision, slug, name, summary, status, krs, metric_targets, initiative_ids,
        team_ids, description, description_revision, description_observed_at)
    VALUES(NEW.repo, NEW.provider, NEW.id, NEW.observed_at, NEW.archived, NEW.membership_unresolved,
        NEW.task_order_json, NEW.revision, NEW.slug, NEW.name, NEW.summary, NEW.status, NEW.krs,
        NEW.metric_targets, NEW.initiative_ids, NEW.team_ids, NEW.description,
        NEW.description_revision, NEW.description_observed_at)
    ON CONFLICT(repo, tracker, id) DO UPDATE
    SET observed_at=excluded.observed_at, archived=excluded.archived,
        membership_unresolved=excluded.membership_unresolved,
        task_order_json=excluded.task_order_json, revision=excluded.revision, slug=excluded.slug,
        name=excluded.name, summary=excluded.summary, status=excluded.status, krs=excluded.krs,
        metric_targets=excluded.metric_targets, initiative_ids=excluded.initiative_ids,
        team_ids=excluded.team_ids, description=excluded.description,
        description_revision=excluded.description_revision,
        description_observed_at=excluded.description_observed_at
    WHERE observed_at IS NOT excluded.observed_at OR archived IS NOT excluded.archived OR
        membership_unresolved IS NOT excluded.membership_unresolved OR task_order_json IS NOT
        excluded.task_order_json OR revision IS NOT excluded.revision OR slug IS NOT excluded.slug
        OR name IS NOT excluded.name OR summary IS NOT excluded.summary OR status IS NOT
        excluded.status OR krs IS NOT excluded.krs OR metric_targets IS NOT excluded.metric_targets
        OR initiative_ids IS NOT excluded.initiative_ids OR team_ids IS NOT excluded.team_ids OR
        description IS NOT excluded.description OR description_revision IS NOT
        excluded.description_revision OR description_observed_at IS NOT
        excluded.description_observed_at;
END;

CREATE TRIGGER mirror_pm_projects_update AFTER UPDATE ON pm_projects
BEGIN
    UPDATE tracker_projects
    SET repo=NEW.repo, tracker=NEW.provider, id=NEW.id, observed_at=NEW.observed_at,
        archived=NEW.archived, membership_unresolved=NEW.membership_unresolved,
        task_order_json=NEW.task_order_json, revision=NEW.revision, slug=NEW.slug, name=NEW.name,
        summary=NEW.summary, status=NEW.status, krs=NEW.krs, metric_targets=NEW.metric_targets,
        initiative_ids=NEW.initiative_ids, team_ids=NEW.team_ids, description=NEW.description,
        description_revision=NEW.description_revision,
        description_observed_at=NEW.description_observed_at
    WHERE repo=OLD.repo AND tracker=OLD.provider AND id=OLD.id AND (repo IS NOT NEW.repo OR tracker
        IS NOT NEW.provider OR id IS NOT NEW.id OR observed_at IS NOT NEW.observed_at OR archived IS
        NOT NEW.archived OR membership_unresolved IS NOT NEW.membership_unresolved OR
        task_order_json IS NOT NEW.task_order_json OR revision IS NOT NEW.revision OR slug IS NOT
        NEW.slug OR name IS NOT NEW.name OR summary IS NOT NEW.summary OR status IS NOT NEW.status
        OR krs IS NOT NEW.krs OR metric_targets IS NOT NEW.metric_targets OR initiative_ids IS NOT
        NEW.initiative_ids OR team_ids IS NOT NEW.team_ids OR description IS NOT NEW.description OR
        description_revision IS NOT NEW.description_revision OR description_observed_at IS NOT
        NEW.description_observed_at);
END;

CREATE TRIGGER mirror_pm_projects_delete AFTER DELETE ON pm_projects
BEGIN
    DELETE FROM tracker_projects
    WHERE repo=OLD.repo AND tracker=OLD.provider AND id=OLD.id;
END;

CREATE TRIGGER mirror_tracker_projects_insert AFTER INSERT ON tracker_projects
BEGIN
    INSERT INTO pm_projects(repo, provider, id, observed_at, archived, membership_unresolved,
        task_order_json, revision, slug, name, summary, status, krs, metric_targets, initiative_ids,
        team_ids, description, description_revision, description_observed_at)
    VALUES(NEW.repo, NEW.tracker, NEW.id, NEW.observed_at, NEW.archived, NEW.membership_unresolved,
        NEW.task_order_json, NEW.revision, NEW.slug, NEW.name, NEW.summary, NEW.status, NEW.krs,
        NEW.metric_targets, NEW.initiative_ids, NEW.team_ids, NEW.description,
        NEW.description_revision, NEW.description_observed_at)
    ON CONFLICT(repo, provider, id) DO UPDATE
    SET observed_at=excluded.observed_at, archived=excluded.archived,
        membership_unresolved=excluded.membership_unresolved,
        task_order_json=excluded.task_order_json, revision=excluded.revision, slug=excluded.slug,
        name=excluded.name, summary=excluded.summary, status=excluded.status, krs=excluded.krs,
        metric_targets=excluded.metric_targets, initiative_ids=excluded.initiative_ids,
        team_ids=excluded.team_ids, description=excluded.description,
        description_revision=excluded.description_revision,
        description_observed_at=excluded.description_observed_at
    WHERE observed_at IS NOT excluded.observed_at OR archived IS NOT excluded.archived OR
        membership_unresolved IS NOT excluded.membership_unresolved OR task_order_json IS NOT
        excluded.task_order_json OR revision IS NOT excluded.revision OR slug IS NOT excluded.slug
        OR name IS NOT excluded.name OR summary IS NOT excluded.summary OR status IS NOT
        excluded.status OR krs IS NOT excluded.krs OR metric_targets IS NOT excluded.metric_targets
        OR initiative_ids IS NOT excluded.initiative_ids OR team_ids IS NOT excluded.team_ids OR
        description IS NOT excluded.description OR description_revision IS NOT
        excluded.description_revision OR description_observed_at IS NOT
        excluded.description_observed_at;
END;

CREATE TRIGGER mirror_tracker_projects_update AFTER UPDATE ON tracker_projects
BEGIN
    UPDATE pm_projects
    SET repo=NEW.repo, provider=NEW.tracker, id=NEW.id, observed_at=NEW.observed_at,
        archived=NEW.archived, membership_unresolved=NEW.membership_unresolved,
        task_order_json=NEW.task_order_json, revision=NEW.revision, slug=NEW.slug, name=NEW.name,
        summary=NEW.summary, status=NEW.status, krs=NEW.krs, metric_targets=NEW.metric_targets,
        initiative_ids=NEW.initiative_ids, team_ids=NEW.team_ids, description=NEW.description,
        description_revision=NEW.description_revision,
        description_observed_at=NEW.description_observed_at
    WHERE repo=OLD.repo AND provider=OLD.tracker AND id=OLD.id AND (repo IS NOT NEW.repo OR provider
        IS NOT NEW.tracker OR id IS NOT NEW.id OR observed_at IS NOT NEW.observed_at OR archived IS
        NOT NEW.archived OR membership_unresolved IS NOT NEW.membership_unresolved OR
        task_order_json IS NOT NEW.task_order_json OR revision IS NOT NEW.revision OR slug IS NOT
        NEW.slug OR name IS NOT NEW.name OR summary IS NOT NEW.summary OR status IS NOT NEW.status
        OR krs IS NOT NEW.krs OR metric_targets IS NOT NEW.metric_targets OR initiative_ids IS NOT
        NEW.initiative_ids OR team_ids IS NOT NEW.team_ids OR description IS NOT NEW.description OR
        description_revision IS NOT NEW.description_revision OR description_observed_at IS NOT
        NEW.description_observed_at);
END;

CREATE TRIGGER mirror_tracker_projects_delete AFTER DELETE ON tracker_projects
BEGIN
    DELETE FROM pm_projects
    WHERE repo=OLD.repo AND provider=OLD.tracker AND id=OLD.id;
END;

CREATE TRIGGER mirror_pm_items_insert AFTER INSERT ON pm_items
BEGIN
    INSERT INTO tracker_items(repo, tracker, id, identifier, project_id, observed_at, needs_refresh,
        revision, branch_name, url, name, description, rank, completed, completed_at, due_date,
        state, team_id, assignee, completed_at_observed, due_date_observed, assignee_observed,
        claimed_project_id, project_id_observed)
    VALUES(NEW.repo, NEW.provider, NEW.id, NEW.identifier, NEW.project_id, NEW.observed_at,
        NEW.needs_refresh, NEW.revision, NEW.branch_name, NEW.url, NEW.name, NEW.description,
        NEW.rank, NEW.completed, NEW.completed_at, NEW.due_date, NEW.state, NEW.team_id,
        NEW.assignee, NEW.completed_at_observed, NEW.due_date_observed, NEW.assignee_observed,
        NEW.claimed_project_id, NEW.project_id_observed)
    ON CONFLICT(repo, tracker, id) DO UPDATE
    SET identifier=excluded.identifier, project_id=excluded.project_id,
        observed_at=excluded.observed_at, needs_refresh=excluded.needs_refresh,
        revision=excluded.revision, branch_name=excluded.branch_name, url=excluded.url,
        name=excluded.name, description=excluded.description, rank=excluded.rank,
        completed=excluded.completed, completed_at=excluded.completed_at,
        due_date=excluded.due_date, state=excluded.state, team_id=excluded.team_id,
        assignee=excluded.assignee, completed_at_observed=excluded.completed_at_observed,
        due_date_observed=excluded.due_date_observed, assignee_observed=excluded.assignee_observed,
        claimed_project_id=excluded.claimed_project_id,
        project_id_observed=excluded.project_id_observed
    WHERE identifier IS NOT excluded.identifier COLLATE BINARY OR project_id IS NOT
        excluded.project_id OR observed_at IS NOT excluded.observed_at OR needs_refresh IS NOT
        excluded.needs_refresh OR revision IS NOT excluded.revision OR branch_name IS NOT
        excluded.branch_name OR url IS NOT excluded.url OR name IS NOT excluded.name OR description
        IS NOT excluded.description OR rank IS NOT excluded.rank OR completed IS NOT
        excluded.completed OR completed_at IS NOT excluded.completed_at OR due_date IS NOT
        excluded.due_date OR state IS NOT excluded.state OR team_id IS NOT excluded.team_id OR
        assignee IS NOT excluded.assignee OR completed_at_observed IS NOT
        excluded.completed_at_observed OR due_date_observed IS NOT excluded.due_date_observed OR
        assignee_observed IS NOT excluded.assignee_observed OR claimed_project_id IS NOT
        excluded.claimed_project_id OR project_id_observed IS NOT excluded.project_id_observed;
END;

CREATE TRIGGER mirror_pm_items_update AFTER UPDATE ON pm_items
BEGIN
    UPDATE tracker_items
    SET repo=NEW.repo, tracker=NEW.provider, id=NEW.id, identifier=NEW.identifier,
        project_id=NEW.project_id, observed_at=NEW.observed_at, needs_refresh=NEW.needs_refresh,
        revision=NEW.revision, branch_name=NEW.branch_name, url=NEW.url, name=NEW.name,
        description=NEW.description, rank=NEW.rank, completed=NEW.completed,
        completed_at=NEW.completed_at, due_date=NEW.due_date, state=NEW.state, team_id=NEW.team_id,
        assignee=NEW.assignee, completed_at_observed=NEW.completed_at_observed,
        due_date_observed=NEW.due_date_observed, assignee_observed=NEW.assignee_observed,
        claimed_project_id=NEW.claimed_project_id, project_id_observed=NEW.project_id_observed
    WHERE repo=OLD.repo AND tracker=OLD.provider AND id=OLD.id AND (repo IS NOT NEW.repo OR tracker
        IS NOT NEW.provider OR id IS NOT NEW.id OR identifier IS NOT NEW.identifier COLLATE BINARY
        OR project_id IS NOT NEW.project_id OR observed_at IS NOT NEW.observed_at OR needs_refresh
        IS NOT NEW.needs_refresh OR revision IS NOT NEW.revision OR branch_name IS NOT
        NEW.branch_name OR url IS NOT NEW.url OR name IS NOT NEW.name OR description IS NOT
        NEW.description OR rank IS NOT NEW.rank OR completed IS NOT NEW.completed OR completed_at IS
        NOT NEW.completed_at OR due_date IS NOT NEW.due_date OR state IS NOT NEW.state OR team_id IS
        NOT NEW.team_id OR assignee IS NOT NEW.assignee OR completed_at_observed IS NOT
        NEW.completed_at_observed OR due_date_observed IS NOT NEW.due_date_observed OR
        assignee_observed IS NOT NEW.assignee_observed OR claimed_project_id IS NOT
        NEW.claimed_project_id OR project_id_observed IS NOT NEW.project_id_observed);
END;

CREATE TRIGGER mirror_pm_items_delete AFTER DELETE ON pm_items
BEGIN
    DELETE FROM tracker_items
    WHERE repo=OLD.repo AND tracker=OLD.provider AND id=OLD.id;
END;

CREATE TRIGGER mirror_tracker_items_insert AFTER INSERT ON tracker_items
BEGIN
    INSERT INTO pm_items(repo, provider, id, identifier, project_id, observed_at, needs_refresh,
        revision, branch_name, url, name, description, rank, completed, completed_at, due_date,
        state, team_id, assignee, completed_at_observed, due_date_observed, assignee_observed,
        claimed_project_id, project_id_observed)
    VALUES(NEW.repo, NEW.tracker, NEW.id, NEW.identifier, NEW.project_id, NEW.observed_at,
        NEW.needs_refresh, NEW.revision, NEW.branch_name, NEW.url, NEW.name, NEW.description,
        NEW.rank, NEW.completed, NEW.completed_at, NEW.due_date, NEW.state, NEW.team_id,
        NEW.assignee, NEW.completed_at_observed, NEW.due_date_observed, NEW.assignee_observed,
        NEW.claimed_project_id, NEW.project_id_observed)
    ON CONFLICT(repo, provider, id) DO UPDATE
    SET identifier=excluded.identifier, project_id=excluded.project_id,
        observed_at=excluded.observed_at, needs_refresh=excluded.needs_refresh,
        revision=excluded.revision, branch_name=excluded.branch_name, url=excluded.url,
        name=excluded.name, description=excluded.description, rank=excluded.rank,
        completed=excluded.completed, completed_at=excluded.completed_at,
        due_date=excluded.due_date, state=excluded.state, team_id=excluded.team_id,
        assignee=excluded.assignee, completed_at_observed=excluded.completed_at_observed,
        due_date_observed=excluded.due_date_observed, assignee_observed=excluded.assignee_observed,
        claimed_project_id=excluded.claimed_project_id,
        project_id_observed=excluded.project_id_observed
    WHERE identifier IS NOT excluded.identifier COLLATE BINARY OR project_id IS NOT
        excluded.project_id OR observed_at IS NOT excluded.observed_at OR needs_refresh IS NOT
        excluded.needs_refresh OR revision IS NOT excluded.revision OR branch_name IS NOT
        excluded.branch_name OR url IS NOT excluded.url OR name IS NOT excluded.name OR description
        IS NOT excluded.description OR rank IS NOT excluded.rank OR completed IS NOT
        excluded.completed OR completed_at IS NOT excluded.completed_at OR due_date IS NOT
        excluded.due_date OR state IS NOT excluded.state OR team_id IS NOT excluded.team_id OR
        assignee IS NOT excluded.assignee OR completed_at_observed IS NOT
        excluded.completed_at_observed OR due_date_observed IS NOT excluded.due_date_observed OR
        assignee_observed IS NOT excluded.assignee_observed OR claimed_project_id IS NOT
        excluded.claimed_project_id OR project_id_observed IS NOT excluded.project_id_observed;
END;

CREATE TRIGGER mirror_tracker_items_update AFTER UPDATE ON tracker_items
BEGIN
    UPDATE pm_items
    SET repo=NEW.repo, provider=NEW.tracker, id=NEW.id, identifier=NEW.identifier,
        project_id=NEW.project_id, observed_at=NEW.observed_at, needs_refresh=NEW.needs_refresh,
        revision=NEW.revision, branch_name=NEW.branch_name, url=NEW.url, name=NEW.name,
        description=NEW.description, rank=NEW.rank, completed=NEW.completed,
        completed_at=NEW.completed_at, due_date=NEW.due_date, state=NEW.state, team_id=NEW.team_id,
        assignee=NEW.assignee, completed_at_observed=NEW.completed_at_observed,
        due_date_observed=NEW.due_date_observed, assignee_observed=NEW.assignee_observed,
        claimed_project_id=NEW.claimed_project_id, project_id_observed=NEW.project_id_observed
    WHERE repo=OLD.repo AND provider=OLD.tracker AND id=OLD.id AND (repo IS NOT NEW.repo OR provider
        IS NOT NEW.tracker OR id IS NOT NEW.id OR identifier IS NOT NEW.identifier COLLATE BINARY OR
        project_id IS NOT NEW.project_id OR observed_at IS NOT NEW.observed_at OR needs_refresh IS
        NOT NEW.needs_refresh OR revision IS NOT NEW.revision OR branch_name IS NOT NEW.branch_name
        OR url IS NOT NEW.url OR name IS NOT NEW.name OR description IS NOT NEW.description OR rank
        IS NOT NEW.rank OR completed IS NOT NEW.completed OR completed_at IS NOT NEW.completed_at OR
        due_date IS NOT NEW.due_date OR state IS NOT NEW.state OR team_id IS NOT NEW.team_id OR
        assignee IS NOT NEW.assignee OR completed_at_observed IS NOT NEW.completed_at_observed OR
        due_date_observed IS NOT NEW.due_date_observed OR assignee_observed IS NOT
        NEW.assignee_observed OR claimed_project_id IS NOT NEW.claimed_project_id OR
        project_id_observed IS NOT NEW.project_id_observed);
END;

CREATE TRIGGER mirror_tracker_items_delete AFTER DELETE ON tracker_items
BEGIN
    DELETE FROM pm_items
    WHERE repo=OLD.repo AND provider=OLD.tracker AND id=OLD.id;
END;

CREATE TRIGGER mirror_pm_issue_changes_insert AFTER INSERT ON pm_issue_changes
BEGIN
    INSERT INTO tracker_events(issue_id, revision_ns, removed)
    VALUES(NEW.issue_id, NEW.revision_ns, NEW.removed)
    ON CONFLICT(issue_id) DO UPDATE
    SET revision_ns=excluded.revision_ns, removed=excluded.removed
    WHERE revision_ns IS NOT excluded.revision_ns OR removed IS NOT excluded.removed;
END;

CREATE TRIGGER mirror_pm_issue_changes_update AFTER UPDATE ON pm_issue_changes
BEGIN
    UPDATE tracker_events
    SET issue_id=NEW.issue_id, revision_ns=NEW.revision_ns, removed=NEW.removed
    WHERE issue_id=OLD.issue_id AND (issue_id IS NOT NEW.issue_id OR revision_ns IS NOT
        NEW.revision_ns OR removed IS NOT NEW.removed);
END;

CREATE TRIGGER mirror_pm_issue_changes_delete AFTER DELETE ON pm_issue_changes
BEGIN
    DELETE FROM tracker_events
    WHERE issue_id=OLD.issue_id;
END;

CREATE TRIGGER mirror_tracker_events_insert AFTER INSERT ON tracker_events
BEGIN
    INSERT INTO pm_issue_changes(issue_id, revision_ns, removed)
    VALUES(NEW.issue_id, NEW.revision_ns, NEW.removed)
    ON CONFLICT(issue_id) DO UPDATE
    SET revision_ns=excluded.revision_ns, removed=excluded.removed
    WHERE revision_ns IS NOT excluded.revision_ns OR removed IS NOT excluded.removed;
END;

CREATE TRIGGER mirror_tracker_events_update AFTER UPDATE ON tracker_events
BEGIN
    UPDATE pm_issue_changes
    SET issue_id=NEW.issue_id, revision_ns=NEW.revision_ns, removed=NEW.removed
    WHERE issue_id=OLD.issue_id AND (issue_id IS NOT NEW.issue_id OR revision_ns IS NOT
        NEW.revision_ns OR removed IS NOT NEW.removed);
END;

CREATE TRIGGER mirror_tracker_events_delete AFTER DELETE ON tracker_events
BEGIN
    DELETE FROM pm_issue_changes
    WHERE issue_id=OLD.issue_id;
END;

CREATE TRIGGER mirror_pm_wave_projects_insert AFTER INSERT ON pm_wave_projects
BEGIN
    INSERT INTO tracker_wave_projects(wave_id, project_id, position)
    VALUES(NEW.wave_id, NEW.project_id, NEW.position)
    ON CONFLICT(wave_id, project_id) DO UPDATE
    SET position=excluded.position
    WHERE position IS NOT excluded.position;
END;

CREATE TRIGGER mirror_pm_wave_projects_update AFTER UPDATE ON pm_wave_projects
BEGIN
    UPDATE tracker_wave_projects
    SET wave_id=NEW.wave_id, project_id=NEW.project_id, position=NEW.position
    WHERE wave_id=OLD.wave_id AND project_id=OLD.project_id AND (wave_id IS NOT NEW.wave_id OR
        project_id IS NOT NEW.project_id OR position IS NOT NEW.position);
END;

CREATE TRIGGER mirror_pm_wave_projects_delete AFTER DELETE ON pm_wave_projects
BEGIN
    DELETE FROM tracker_wave_projects
    WHERE wave_id=OLD.wave_id AND project_id=OLD.project_id;
END;

CREATE TRIGGER mirror_tracker_wave_projects_insert AFTER INSERT ON tracker_wave_projects
BEGIN
    INSERT INTO pm_wave_projects(wave_id, project_id, position)
    VALUES(NEW.wave_id, NEW.project_id, NEW.position)
    ON CONFLICT(wave_id, project_id) DO UPDATE
    SET position=excluded.position
    WHERE position IS NOT excluded.position;
END;

CREATE TRIGGER mirror_tracker_wave_projects_update AFTER UPDATE ON tracker_wave_projects
BEGIN
    UPDATE pm_wave_projects
    SET wave_id=NEW.wave_id, project_id=NEW.project_id, position=NEW.position
    WHERE wave_id=OLD.wave_id AND project_id=OLD.project_id AND (wave_id IS NOT NEW.wave_id OR
        project_id IS NOT NEW.project_id OR position IS NOT NEW.position);
END;

CREATE TRIGGER mirror_tracker_wave_projects_delete AFTER DELETE ON tracker_wave_projects
BEGIN
    DELETE FROM pm_wave_projects
    WHERE wave_id=OLD.wave_id AND project_id=OLD.project_id;
END;

CREATE TRIGGER mirror_pm_wave_sync_insert AFTER INSERT ON pm_wave_sync
BEGIN
    INSERT INTO tracker_wave_sync(wave_id, tracker, initiative, synced_at)
    VALUES(NEW.wave_id, NEW.provider, NEW.initiative, NEW.synced_at)
    ON CONFLICT(wave_id) DO UPDATE
    SET tracker=excluded.tracker, initiative=excluded.initiative, synced_at=excluded.synced_at
    WHERE tracker IS NOT excluded.tracker OR initiative IS NOT excluded.initiative OR synced_at IS
        NOT excluded.synced_at;
END;

CREATE TRIGGER mirror_pm_wave_sync_update AFTER UPDATE ON pm_wave_sync
BEGIN
    UPDATE tracker_wave_sync
    SET wave_id=NEW.wave_id, tracker=NEW.provider, initiative=NEW.initiative,
        synced_at=NEW.synced_at
    WHERE wave_id=OLD.wave_id AND (wave_id IS NOT NEW.wave_id OR tracker IS NOT NEW.provider OR
        initiative IS NOT NEW.initiative OR synced_at IS NOT NEW.synced_at);
END;

CREATE TRIGGER mirror_pm_wave_sync_delete AFTER DELETE ON pm_wave_sync
BEGIN
    DELETE FROM tracker_wave_sync
    WHERE wave_id=OLD.wave_id;
END;

CREATE TRIGGER mirror_tracker_wave_sync_insert AFTER INSERT ON tracker_wave_sync
BEGIN
    INSERT INTO pm_wave_sync(wave_id, provider, initiative, synced_at)
    VALUES(NEW.wave_id, NEW.tracker, NEW.initiative, NEW.synced_at)
    ON CONFLICT(wave_id) DO UPDATE
    SET provider=excluded.provider, initiative=excluded.initiative, synced_at=excluded.synced_at
    WHERE provider IS NOT excluded.provider OR initiative IS NOT excluded.initiative OR synced_at IS
        NOT excluded.synced_at;
END;

CREATE TRIGGER mirror_tracker_wave_sync_update AFTER UPDATE ON tracker_wave_sync
BEGIN
    UPDATE pm_wave_sync
    SET wave_id=NEW.wave_id, provider=NEW.tracker, initiative=NEW.initiative,
        synced_at=NEW.synced_at
    WHERE wave_id=OLD.wave_id AND (wave_id IS NOT NEW.wave_id OR provider IS NOT NEW.tracker OR
        initiative IS NOT NEW.initiative OR synced_at IS NOT NEW.synced_at);
END;

CREATE TRIGGER mirror_tracker_wave_sync_delete AFTER DELETE ON tracker_wave_sync
BEGIN
    DELETE FROM pm_wave_sync
    WHERE wave_id=OLD.wave_id;
END;
