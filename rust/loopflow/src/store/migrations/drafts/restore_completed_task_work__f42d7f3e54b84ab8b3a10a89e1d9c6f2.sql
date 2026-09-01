-- name: restore_completed_task_work
-- id: f42d7f3e54b84ab8b3a10a89e1d9c6f2
-- depends_on: stable_work_state

-- stable_work_state shipped without its terminal-state backfill. Recover Task
-- completion from the serial PR fact that survived the release migration. The
-- legacy sentinel came only from the former terminal `merged` Session state.
WITH latest_task_prs AS (
    SELECT pr.*
    FROM task_prs pr
    WHERE pr.sequence = (
        SELECT MAX(candidate.sequence)
        FROM task_prs candidate
        WHERE candidate.task_id = pr.task_id
    )
)
UPDATE tasks
SET work_state = 'done',
    work_terminal_at = (
        SELECT COALESCE(pr.merged_at, pr.updated_at)
        FROM latest_task_prs pr
        WHERE pr.task_id = tasks.id
    )
WHERE work_state = 'ready'
  AND EXISTS (
      SELECT 1
      FROM latest_task_prs pr
      WHERE pr.task_id = tasks.id
        AND pr.merge_commit IS NOT NULL
        AND (
            pr.after_merge = 'complete_task'
            OR pr.merge_commit = 'legacy-unknown'
        )
  );
