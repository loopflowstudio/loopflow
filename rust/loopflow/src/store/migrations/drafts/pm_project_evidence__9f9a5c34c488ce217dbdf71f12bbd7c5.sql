-- name: pm_project_evidence
-- id: 9f9a5c34c488ce217dbdf71f12bbd7c5
-- depends_on: normalize_pm_planning

-- These are planning evidence, independent of execution and acquisition age.
ALTER TABLE pm_projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1));
ALTER TABLE pm_projects ADD COLUMN membership_unresolved INTEGER NOT NULL DEFAULT 0 CHECK(membership_unresolved IN (0,1));
