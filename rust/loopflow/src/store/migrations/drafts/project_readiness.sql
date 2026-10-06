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

-- Mutation recovery only: the Home-local Wave binding owns current selection.
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
