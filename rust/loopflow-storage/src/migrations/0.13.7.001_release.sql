-- draft: project_readiness
-- Preserve the mixed historical name representation before using Linear names verbatim.
-- Only these rows are eligible for one name/slug replacement. The original body
-- includes provider revision; membership and archival evidence remain independent.
CREATE TABLE pm_project_name_cutover (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL CHECK(provider = 'linear'),
    id TEXT NOT NULL,
    body TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    archived INTEGER NOT NULL,
    membership_unresolved INTEGER NOT NULL,
    converted_at INTEGER,
    PRIMARY KEY(repo, provider, id)
);
INSERT INTO pm_project_name_cutover
    (repo, provider, id, body, observed_at, archived, membership_unresolved)
SELECT repo, provider, id, body, observed_at, archived, membership_unresolved
FROM pm_projects WHERE provider = 'linear';

-- Selection references the accepted local Project; provider UUID reservations stay
-- in transitions until provider facts have been accepted.
ALTER TABLE waves ADD COLUMN current_project_id TEXT REFERENCES projects(id);
CREATE TABLE project_binding_imports (
    wave_id TEXT PRIMARY KEY REFERENCES waves(id),
    original_yaml TEXT,
    imported_at INTEGER NOT NULL
);

-- Mutation recovery only: the Wave row owns current selection.
CREATE TABLE project_transitions (
    wave_id TEXT NOT NULL REFERENCES waves(id),
    successor_id TEXT NOT NULL,
    predecessor_id TEXT,
    reset_name TEXT,
    create_successor INTEGER CHECK(create_successor IN (0, 1)),
    created_at INTEGER NOT NULL,
    settled_at INTEGER,
    PRIMARY KEY (wave_id, successor_id)
);
CREATE UNIQUE INDEX project_transitions_pending
    ON project_transitions(wave_id) WHERE settled_at IS NULL;

-- Freeze selected transfer membership independently of later provider membership.
CREATE TABLE project_transition_items (
    wave_id TEXT NOT NULL,
    successor_id TEXT NOT NULL,
    issue_id TEXT NOT NULL,
    PRIMARY KEY (wave_id, successor_id, issue_id),
    FOREIGN KEY (wave_id, successor_id)
        REFERENCES project_transitions(wave_id, successor_id)
);

-- This is an association to the real activation process, not another lifecycle.
ALTER TABLE waves ADD COLUMN project_activation_exec_id TEXT REFERENCES execs(id);

CREATE TRIGGER store_revision_project_transitions_insert AFTER INSERT ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_project_transitions_update AFTER UPDATE ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_project_transitions_delete AFTER DELETE ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_project_transition_items_insert AFTER INSERT ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_project_transition_items_update AFTER UPDATE ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_project_transition_items_delete AFTER DELETE ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER selected_project_wave_update BEFORE UPDATE OF current_project_id ON waves
WHEN NEW.current_project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects WHERE id=NEW.current_project_id AND wave_id=NEW.id
) BEGIN SELECT RAISE(ABORT, 'selected Project belongs to another Wave'); END;
CREATE TRIGGER selected_project_owner_update BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM waves WHERE current_project_id=NEW.id AND id!=NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'selected Project cannot change Wave ownership'); END;
