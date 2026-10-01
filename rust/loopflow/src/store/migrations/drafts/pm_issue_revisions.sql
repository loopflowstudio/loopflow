-- depends_on: normalize_pm_planning

-- Signed Linear issue events fence delayed reads, even before an issue is cached.
-- Linear UUIDs identify the same issue across repository views in this store.
CREATE TABLE pm_issue_changes (
    issue_id TEXT PRIMARY KEY,
    revision_ns INTEGER,
    removed INTEGER NOT NULL CHECK(removed IN (0,1))
);
