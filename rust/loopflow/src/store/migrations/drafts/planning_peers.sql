-- depends_on: optional_task_pr
-- Structured Project content replaces the internal Markdown round trip.
-- The migration runner converts the retained description before seeding the journal.
ALTER TABLE projects RENAME COLUMN project_prompt_context TO description;
ALTER TABLE projects ADD COLUMN krs TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(krs));
ALTER TABLE projects ADD COLUMN metric_targets TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(metric_targets));

-- Prose can be absent from older observations. Retain its own revision and age
-- rather than making a newer partial Project observation refresh that evidence.
ALTER TABLE pm_projects ADD COLUMN prose_json TEXT CHECK(prose_json IS NULL OR json_valid(prose_json));

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
