-- depends_on: optional_task_pr
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
