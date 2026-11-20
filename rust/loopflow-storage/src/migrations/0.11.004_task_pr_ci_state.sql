-- Record the open PR's head commit and its last required-check reading so a
-- waiting Task can be woken into ci-fix when a required check fails, and so
-- `lf status` can render CI state without a live GitHub read.
--
-- github_head_sha: the PR's current head (headRefOid). CI evidence is
--   authoritative only for this head; a reading is stale once it moves.
-- ci_observation: JSON CiObservation { head_sha, state, failing_checks,
--   observed_at }, or NULL until the head has been observed.

ALTER TABLE task_prs ADD COLUMN github_head_sha TEXT;
ALTER TABLE task_prs ADD COLUMN ci_observation TEXT;
