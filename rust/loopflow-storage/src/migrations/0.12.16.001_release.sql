-- draft: native_human_session_runtime
DROP TABLE ask_linear_comment_outbox;
DROP TABLE ask_exchanges;

CREATE TABLE work_flow_positions (
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    flow TEXT NOT NULL,
    step TEXT NOT NULL,
    node_id TEXT,
    human INTEGER NOT NULL CHECK (human IN (0, 1)),
    session_run_id TEXT,
    ready_summary TEXT,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (work_kind, work_id)
);

-- draft: restore_completed_task_work
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

-- draft: retire_steers_table
-- Steers are durable Work comments now (`TaskEventKind::Steer` /
-- `ProjectEventKind::Steer`, read via the event streams). No code reads or writes
-- the `steers` table; `stable_work_state` still rebuilds it only because it was
-- authored when steers were a table. Retire it.
DROP TABLE IF EXISTS steers;
