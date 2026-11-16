-- Members the product no longer reads or writes. Each mark names the Task whose
-- contract drops it once this migration has been on origin/main for the staging
-- window; `scripts/due_removals.py` lists them with their dates.

-- Never had a caller.
-- leaving: tool_responses LOO-943
-- Finished one-time migrations and imports.
-- leaving: pm_project_name_cutover LOO-943
-- leaving: project_transition_items LOO-943
-- leaving: task_issue_identities LOO-943
-- leaving: project_binding_imports LOO-943
-- No writer since 2026-10-09; nothing guards Task paths with it any more.
-- leaving: task_deletions LOO-943
-- Written, never read.
-- leaving: task_pr_repair_incidents LOO-943
-- Pool and on-demand declarations live in `config.yaml`; these held only a
-- name and a creation time.
-- leaving: pools LOO-943
-- leaving: ondemands LOO-943
-- Desktop keeps its own pane layouts, one set per Mac.
-- leaving: work_pane_layouts LOO-943
-- `held_terminals` holds the live viewer state.
-- leaving: processes.viewer_hold LOO-943
-- leaving: processes.viewer_hold_at LOO-943
-- leaving: processes.viewed LOO-943

-- Steering moved to `steers` (#1923).
-- leaving: session_steers LOO-891
-- leaving: steer_batches LOO-891
-- leaving: steer_deliveries LOO-891

-- leaving: processes.native_input LOO-840
-- leaving: desktop_shells.close_requested LOO-840

-- `task_machine_local` replaced these; lf still fills them only for builds
-- released before it, and the contract stops that with the drop.
-- leaving: tasks.worktree LOO-877
-- leaving: tasks.branch LOO-877
-- leaving: tasks.base_commit LOO-877
-- leaving: tasks.parent_pr_id LOO-877
-- leaving: tasks.checkout_machine_id LOO-877
-- leaving: tasks.primary_session_id LOO-877
-- leaving: tasks.ship_lf_process_id LOO-877
-- leaving: tasks.ship_restarts LOO-877
-- leaving: tasks.ship_approved_lf_process_id LOO-877
-- leaving: tasks.ship_stopped_at LOO-877
-- leaving: tasks.ship_stopped_lf_process_id LOO-877
-- leaving: tasks.ship_stopped_person LOO-877
-- leaving: task_ownership.written_here LOO-877
-- leaving: machines.pool LOO-877
-- leaving: machines.cost_usd LOO-877
-- leaving: machines.expires_at LOO-877

-- leaving: tasks.regressed_in LOO-730

-- An owned Machine's pool is a name from `config.yaml`, not a `pools` row.
-- The rebuild keeps every column.
CREATE TABLE pool_machines_rebuilt (
    machine_id TEXT PRIMARY KEY REFERENCES machines(id) ON DELETE CASCADE,
    pool TEXT,
    cost_usd TEXT,
    expires_at INTEGER
);
INSERT INTO pool_machines_rebuilt(machine_id,pool,cost_usd,expires_at)
SELECT machine_id,pool,cost_usd,expires_at FROM pool_machines;
DROP TABLE pool_machines;
ALTER TABLE pool_machines_rebuilt RENAME TO pool_machines;
CREATE INDEX pool_machines_pool ON pool_machines(pool);

-- Dashboards are screen state: an edit no longer wakes planning readers.
DROP TRIGGER store_revision_dashboards_insert;
DROP TRIGGER store_revision_dashboards_update;
DROP TRIGGER store_revision_dashboards_delete;
DROP TRIGGER store_revision_dashboard_arrangements_insert;
DROP TRIGGER store_revision_dashboard_arrangements_update;
DROP TRIGGER store_revision_dashboard_arrangements_delete;
-- Nothing writes pane layouts any more.
DROP TRIGGER store_revision_work_pane_layouts_insert;
DROP TRIGGER store_revision_work_pane_layouts_update;
DROP TRIGGER store_revision_work_pane_layouts_delete;
