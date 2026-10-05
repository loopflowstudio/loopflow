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
