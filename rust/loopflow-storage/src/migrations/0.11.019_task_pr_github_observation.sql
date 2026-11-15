-- Coalesce repeated GitHub reads across short-lived `lf` processes. The Task PR
-- row remains the only durable PR state; this JSON stores only the last refresh
-- attempt (`GithubObservation { checked_at, result }`) and its degraded reason.
--
-- Ordinals .016-.018 are owned by Linear observations, migration provenance,
-- and session body provenance. This observation cache follows that canonical
-- chain.

ALTER TABLE task_prs ADD COLUMN github_observation TEXT;
