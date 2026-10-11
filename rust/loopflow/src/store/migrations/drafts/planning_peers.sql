-- depends_on: optional_task_pr
-- Structured Project content replaces the internal Markdown round trip.
-- The migration runner converts the retained description before seeding the journal.
ALTER TABLE projects RENAME COLUMN project_prompt_context TO description;
ALTER TABLE projects ADD COLUMN krs TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(krs));
ALTER TABLE projects ADD COLUMN metric_targets TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(metric_targets));

-- Reject lossy conversion before dropping the original evidence. Missing and
-- explicitly null fields remain distinct where they affect acquisition/receipts.
-- SQLite extraction collapses JSON arrays into strings and booleans into numbers;
-- unsupported scalar types must leave the released rows and schema intact.
CREATE TEMP TABLE linear_observation_conversion(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO linear_observation_conversion
SELECT json_type(body)='object' AND NOT EXISTS(
    SELECT 1 FROM json_each(body) WHERE
    (key IN ('id','slug','name','summary','workflow','status') AND type!='text')
    OR (key IN ('revision','description') AND type NOT IN ('text','null'))
    OR (key IN ('krs','metric_targets','initiative_ids','team_ids') AND type!='array')
    OR (key='id' AND type='text' AND value IS NOT p.id)
) FROM pm_projects p;
INSERT INTO linear_observation_conversion
SELECT json_type(body)='object' AND NOT EXISTS(
    SELECT 1 FROM json_each(body) WHERE
    (key IN ('id','identifier','name','description') AND type!='text')
    OR (key IN ('project_id','revision','branch_name','url','completed_at','due_date','state','team_id','assignee') AND type NOT IN ('text','null'))
    OR (key='rank' AND type!='integer')
    OR (key='completed' AND type NOT IN ('true','false'))
    OR (key='id' AND type='text' AND value IS NOT i.id)
    OR (key='identifier' AND type='text' AND value IS NOT i.identifier)
) FROM pm_items i;
DROP TABLE linear_observation_conversion;

-- Concrete provider facts retain their own absence and acquisition evidence.
ALTER TABLE pm_projects ADD COLUMN revision TEXT;
ALTER TABLE pm_projects ADD COLUMN slug TEXT;
ALTER TABLE pm_projects ADD COLUMN name TEXT;
ALTER TABLE pm_projects ADD COLUMN summary TEXT;
ALTER TABLE pm_projects ADD COLUMN workflow TEXT;
ALTER TABLE pm_projects ADD COLUMN status TEXT;
ALTER TABLE pm_projects ADD COLUMN krs TEXT;
ALTER TABLE pm_projects ADD COLUMN metric_targets TEXT;
ALTER TABLE pm_projects ADD COLUMN initiative_ids TEXT;
ALTER TABLE pm_projects ADD COLUMN team_ids TEXT;
ALTER TABLE pm_projects ADD COLUMN description TEXT;
ALTER TABLE pm_projects ADD COLUMN description_revision TEXT;
ALTER TABLE pm_projects ADD COLUMN description_observed_at INTEGER;
UPDATE pm_projects SET revision=json_extract(body,'$.revision'),
    slug=json_extract(body,'$.slug'),
    name=json_extract(body,'$.name'),
    summary=json_extract(body,'$.summary'),
    workflow=json_extract(body,'$.workflow'),
    status=json_extract(body,'$.status'),
    krs=body -> '$.krs',
    metric_targets=body -> '$.metric_targets',
    initiative_ids=body -> '$.initiative_ids',
    team_ids=body -> '$.team_ids',
    description=json_extract(body,'$.description'),
    description_revision=CASE WHEN json_type(body,'$.description')='text' THEN json_extract(body,'$.revision') END,
    description_observed_at=CASE WHEN json_type(body,'$.description')='text' THEN observed_at END;
ALTER TABLE pm_projects DROP COLUMN body;
ALTER TABLE pm_items ADD COLUMN revision TEXT;
ALTER TABLE pm_items ADD COLUMN branch_name TEXT;
ALTER TABLE pm_items ADD COLUMN url TEXT;
ALTER TABLE pm_items ADD COLUMN name TEXT;
ALTER TABLE pm_items ADD COLUMN description TEXT;
ALTER TABLE pm_items ADD COLUMN rank INTEGER;
ALTER TABLE pm_items ADD COLUMN completed INTEGER;
ALTER TABLE pm_items ADD COLUMN completed_at TEXT;
ALTER TABLE pm_items ADD COLUMN due_date TEXT;
ALTER TABLE pm_items ADD COLUMN state TEXT;
ALTER TABLE pm_items ADD COLUMN team_id TEXT;
ALTER TABLE pm_items ADD COLUMN assignee TEXT;
ALTER TABLE pm_items ADD COLUMN completed_at_observed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pm_items ADD COLUMN due_date_observed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pm_items ADD COLUMN assignee_observed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pm_items ADD COLUMN claimed_project_id TEXT;
ALTER TABLE pm_items ADD COLUMN project_id_observed INTEGER NOT NULL DEFAULT 0;
UPDATE pm_items SET revision=json_extract(body,'$.revision'),
    branch_name=json_extract(body,'$.branch_name'),
    url=json_extract(body,'$.url'),
    name=json_extract(body,'$.name'),
    description=json_extract(body,'$.description'),
    rank=json_extract(body,'$.rank'),
    completed=json_extract(body,'$.completed'),
    completed_at=json_extract(body,'$.completed_at'),
    due_date=json_extract(body,'$.due_date'),
    state=json_extract(body,'$.state'),
    team_id=json_extract(body,'$.team_id'),
    assignee=json_extract(body,'$.assignee'),
    completed_at_observed=json_type(body,'$.completed_at') IS NOT NULL,
    due_date_observed=json_type(body,'$.due_date') IS NOT NULL,
    assignee_observed=json_type(body,'$.assignee') IS NOT NULL,
    claimed_project_id=json_extract(body,'$.project_id'),
    project_id_observed=json_type(body,'$.project_id') IS NOT NULL;
ALTER TABLE pm_items DROP COLUMN body;

-- Typed content receipts retain sequence, attempts, baselines and conflicts.
DROP TRIGGER store_revision_project_changes_insert;
DROP TRIGGER store_revision_project_changes_update;
DROP TRIGGER store_revision_project_changes_delete;
DROP INDEX idx_project_changes_pending;
ALTER TABLE project_changes RENAME TO retained_project_changes;
CREATE TABLE project_changes (
    order_effects_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(order_effects_json)),
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    field TEXT GENERATED ALWAYS AS (COALESCE(json_extract(value_json,'$.field'),'task_order')) VIRTUAL NOT NULL CHECK(field IN ('project_name','project_summary','project_description','workflow','krs','metric_targets','project_status','task_order')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
INSERT INTO project_changes(order_effects_json,seq,id,project_id,value_json,base_json,conflict_json,attempted,error,acknowledged_revision,acknowledged)
SELECT order_effects_json,seq,id,project_id,
    CASE WHEN field='task_order' THEN value_json ELSE json_object('field',CASE field WHEN 'name' THEN 'project_name' WHEN 'summary' THEN 'project_summary' WHEN 'description' THEN 'project_description' WHEN 'workflow' THEN 'workflow' WHEN 'krs' THEN 'krs' WHEN 'metric_targets' THEN 'metric_targets' WHEN 'status' THEN 'project_status' END,'value',json(value_json)) END,
    CASE WHEN base_json IS NULL OR field='task_order' THEN base_json  ELSE json_set(base_json,'$.value',json_object('field',CASE field WHEN 'name' THEN 'project_name' WHEN 'summary' THEN 'project_summary' WHEN 'description' THEN 'project_description' WHEN 'workflow' THEN 'workflow' WHEN 'krs' THEN 'krs' WHEN 'metric_targets' THEN 'metric_targets' WHEN 'status' THEN 'project_status' END,'value',json(base_json -> '$.value'))) END,
    CASE WHEN conflict_json IS NULL OR field='task_order' THEN conflict_json  ELSE json_set(conflict_json,'$.value',json_object('field',CASE field WHEN 'name' THEN 'project_name' WHEN 'summary' THEN 'project_summary' WHEN 'description' THEN 'project_description' WHEN 'workflow' THEN 'workflow' WHEN 'krs' THEN 'krs' WHEN 'metric_targets' THEN 'metric_targets' WHEN 'status' THEN 'project_status' END,'value',json(conflict_json -> '$.value'))) END,
    attempted,error,acknowledged_revision,acknowledged FROM retained_project_changes;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='retained_project_changes'),0))
WHERE name='project_changes';
DROP TABLE retained_project_changes;
CREATE INDEX idx_project_changes_pending ON project_changes(project_id,field,seq);
CREATE TRIGGER store_revision_project_changes_insert AFTER INSERT ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_update AFTER UPDATE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_delete AFTER DELETE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

DROP TRIGGER store_revision_task_changes_insert;
DROP TRIGGER store_revision_task_changes_update;
DROP TRIGGER store_revision_task_changes_delete;
DROP INDEX idx_task_changes_pending;
ALTER TABLE task_changes RENAME TO retained_task_changes;
CREATE TABLE task_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    field TEXT GENERATED ALWAYS AS (COALESCE(json_extract(value_json,'$.field'),'deleted')) VIRTUAL NOT NULL CHECK(field IN ('task_title','task_description','task_assignee','task_project','deleted')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
INSERT INTO task_changes(seq,id,task_id,value_json,base_json,conflict_json,attempted,error,acknowledged_revision,acknowledged)
SELECT seq,id,task_id,
    CASE WHEN field='deleted' THEN value_json ELSE json_object('field',CASE field WHEN 'name' THEN 'task_title' WHEN 'description' THEN 'task_description' WHEN 'assignee' THEN 'task_assignee' WHEN 'project_id' THEN 'task_project' END,'value',json(value_json)) END,
    CASE WHEN base_json IS NULL OR field='deleted' THEN base_json WHEN field='project_id' AND json_type(base_json,'$.value')='null' THEN base_json ELSE json_set(base_json,'$.value',json_object('field',CASE field WHEN 'name' THEN 'task_title' WHEN 'description' THEN 'task_description' WHEN 'assignee' THEN 'task_assignee' WHEN 'project_id' THEN 'task_project' END,'value',json(base_json -> '$.value'))) END,
    CASE WHEN conflict_json IS NULL OR field='deleted' THEN conflict_json WHEN field='project_id' AND json_type(conflict_json,'$.value')='null' THEN conflict_json ELSE json_set(conflict_json,'$.value',json_object('field',CASE field WHEN 'name' THEN 'task_title' WHEN 'description' THEN 'task_description' WHEN 'assignee' THEN 'task_assignee' WHEN 'project_id' THEN 'task_project' END,'value',json(conflict_json -> '$.value'))) END,
    attempted,error,acknowledged_revision,acknowledged FROM retained_task_changes;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='retained_task_changes'),0))
WHERE name='task_changes';
DROP TABLE retained_task_changes;
CREATE INDEX idx_task_changes_pending ON task_changes(task_id,field,seq);
CREATE TRIGGER store_revision_task_changes_insert AFTER INSERT ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_update AFTER UPDATE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_delete AFTER DELETE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Creation recovery keeps the original captured values, not today's Work.
-- Provider inputs, IDs, attempt flags and the sequence boundary remain intact.
CREATE TEMP TABLE creation_baseline_conversion(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO creation_baseline_conversion
SELECT json_type(export_json,'$.model')='object'
    AND json_type(export_json,'$.input')='object'
    AND json_type(export_json,'$.model.status')='text'
FROM project_transitions WHERE export_json IS NOT NULL;
INSERT INTO creation_baseline_conversion
SELECT json_type(export_json,'$.model')='object'
    AND json_type(export_json,'$.input')='object'
FROM task_creation_intents WHERE export_json IS NOT NULL;
DROP TABLE creation_baseline_conversion;

UPDATE project_transitions SET export_json=json_remove(json_set(export_json,
    '$.task_state',NULL,
    '$.baseline',json((SELECT json_group_array(json_object(
        'field',CASE key
            WHEN 'name' THEN 'project_name' WHEN 'summary' THEN 'project_summary'
            WHEN 'description' THEN 'project_description' WHEN 'status' THEN 'project_status'
            WHEN 'workflow' THEN 'workflow' WHEN 'krs' THEN 'krs'
            WHEN 'metric_targets' THEN 'metric_targets' END,
        'value',json(export_json -> ('$.model.' || key))))
        FROM json_each(export_json,'$.model')
        WHERE key IN ('name','summary','description','status','workflow','krs','metric_targets')))),
    '$.model')
WHERE export_json IS NOT NULL;

UPDATE task_creation_intents SET export_json=json_remove(json_set(export_json,
    '$.task_state',json_extract(export_json,'$.model.state'),
    '$.baseline',json((SELECT json_group_array(json_object(
        'field',CASE key
            WHEN 'name' THEN 'task_title' WHEN 'description' THEN 'task_description'
            WHEN 'assignee' THEN 'task_assignee' WHEN 'due_date' THEN 'task_due_date'
            WHEN 'project_id' THEN 'task_project' END,
        'value',json(CASE WHEN key='project_id'
            THEN json_quote(COALESCE((SELECT id FROM projects
                WHERE id=entry.value OR external_project_id=entry.value),entry.value))
            ELSE export_json -> ('$.model.' || key) END)))
        FROM json_each(export_json,'$.model') entry
        WHERE key IN ('name','description','assignee','due_date','project_id')))),
    '$.model')
WHERE export_json IS NOT NULL;

-- Local-only Git planning. Linear effects keep their existing local owners.
CREATE TABLE planning_peer_changes (
    id TEXT PRIMARY KEY,
    record_table TEXT NOT NULL CHECK(record_table IN ('waves','projects','tasks','task_comments')),
    mutation TEXT NOT NULL CHECK(json_valid(mutation)),
    object_id TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.target')) VIRTUAL NOT NULL,
    field TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.change.field')) VIRTUAL NOT NULL,
    clock INTEGER GENERATED ALWAYS AS (json_extract(mutation,'$.clock')) VIRTUAL NOT NULL CHECK(clock>=0)
);
CREATE INDEX planning_peer_changes_object ON planning_peer_changes(object_id,field);
CREATE TABLE planning_peer_observed (
    id TEXT PRIMARY KEY REFERENCES planning_peer_changes(id)
);
CREATE TABLE planning_user (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    user_key TEXT NOT NULL UNIQUE
);
CREATE TABLE planning_destinations (
    repo TEXT NOT NULL,
    id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    reference TEXT NOT NULL,
    fetched_revision TEXT,
    acquisition_error TEXT,
    publication_revision TEXT,
    publication_state TEXT CHECK(publication_state IN ('pending','unconfirmed','confirmed')),
    publication_digest TEXT,
    publication_error TEXT,
    PRIMARY KEY(repo,id)
);
CREATE TABLE planning_active (
    repo TEXT PRIMARY KEY,
    destination TEXT NOT NULL,
    FOREIGN KEY(repo,destination) REFERENCES planning_destinations(repo,id)
);
CREATE TABLE planning_members (
    kind TEXT NOT NULL CHECK(kind IN ('waves','projects','tasks','task_comments')),
    object_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    PRIMARY KEY(kind,object_id),
    FOREIGN KEY(repo,destination) REFERENCES planning_destinations(repo,id)
);
CREATE INDEX planning_members_destination ON planning_members(repo,destination);
CREATE TABLE planning_peer_imports (
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    revision TEXT NOT NULL,
    PRIMARY KEY(repo,destination)
);
CREATE TABLE planning_peer_conflicts (
    repo TEXT NOT NULL,
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    PRIMARY KEY(kind,object_id,reason)
);
CREATE TRIGGER store_revision_planning_destinations_insert AFTER INSERT ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_destinations_update AFTER UPDATE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_destinations_delete AFTER DELETE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_active_insert AFTER INSERT ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_active_update AFTER UPDATE ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_active_delete AFTER DELETE ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_members_insert AFTER INSERT ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_members_update AFTER UPDATE ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_members_delete AFTER DELETE ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_insert AFTER INSERT ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_update AFTER UPDATE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_delete AFTER DELETE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_insert AFTER INSERT ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_update AFTER UPDATE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_delete AFTER DELETE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
