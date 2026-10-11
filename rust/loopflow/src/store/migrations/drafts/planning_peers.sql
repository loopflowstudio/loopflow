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

-- Add prose receipts without changing captured values, attempts or sequence identity.
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
    field TEXT NOT NULL CHECK(field IN ('name','summary','description','workflow','krs','metric_targets','status','task_order')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
INSERT INTO project_changes SELECT * FROM retained_project_changes;
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


-- Local-only Git planning. Linear effects keep their existing local owners.
CREATE TABLE planning_peer_changes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('wave','project','task','comment')),
    object_id TEXT NOT NULL,
    field TEXT NOT NULL,
    value TEXT NOT NULL CHECK(json_valid(value)),
    clock INTEGER NOT NULL CHECK(clock>=0),
    parents TEXT NOT NULL CHECK(json_valid(parents))
);
CREATE INDEX planning_peer_changes_object ON planning_peer_changes(kind,object_id,field);
CREATE TABLE planning_peer_observed (
    object_id TEXT NOT NULL,
    id TEXT NOT NULL REFERENCES planning_peer_changes(id),
    PRIMARY KEY(object_id,id)
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
    kind TEXT NOT NULL CHECK(kind IN ('wave','project','task','comment')),
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
