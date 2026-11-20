-- LOO-935 stops reading and writing what the product no longer uses. Released
-- lf still reads some of these members, so they stay until LOO-954's contract
-- drops them, exporting what they hold.

-- Still read: a later Task drops each once its last read goes.
-- leaving: agent_sessions.work_source LOO-836
-- Released peers still exchange it through planning.
-- leaving: tasks.regressed_in LOO-730
-- leaving: provider_accounts LOO-928
-- leaving: provider_session_accounts LOO-928
-- leaving: provider_account_limits LOO-928
-- leaving: provider_account_switches LOO-928

-- Never had a caller, finished one-time migrations and imports, write-only
-- records, and screen state Desktop now keeps in its own files.
-- leaving: tool_responses LOO-954
-- leaving: pm_project_name_cutover LOO-954
-- leaving: project_transition_items LOO-954
-- leaving: task_issue_identities LOO-954
-- leaving: task_deletions LOO-954
-- leaving: task_pr_repair_incidents LOO-954
-- leaving: project_binding_imports LOO-954
-- leaving: work_pane_layouts LOO-954
-- Steering moved to `steers` (#1923).
-- leaving: session_steers LOO-954
-- leaving: steer_deliveries LOO-954
-- leaving: steer_batches LOO-954

-- Pool and on-demand declarations live in `config.yaml`; these held only a
-- name and a creation time. An owned Machine's pool is that name, so
-- `pool_machines` no longer refers to `pools`. `pool_machines` replaced the
-- three `machines` columns.
-- leaving: pools LOO-954
-- leaving: ondemands LOO-954
-- leaving: machines.pool LOO-954
-- leaving: machines.cost_usd LOO-954
-- leaving: machines.expires_at LOO-954
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

-- `held_terminals` holds the live viewer state; Sessions take input natively.
-- leaving: processes.viewer_hold LOO-954
-- leaving: processes.viewer_hold_at LOO-954
-- leaving: processes.viewed LOO-954
-- leaving: processes.native_input LOO-954
-- A Close ends a held shell directly; no owner polls for it.
-- leaving: desktop_shells.close_requested LOO-954
-- Only the dropped repair incidents read whether repair tracking was complete.
-- leaving: task_prs.repair_tracking_complete LOO-954
-- `task_ownership.writer_machine_id` replaced `written_here`, which stays
-- filled for released lf until the drop.
-- leaving: task_ownership.written_here LOO-954

-- `task_checkouts`, `task_primary_sessions` and `task_ship_events` replaced
-- these Task columns (`task_machine_local`).
-- leaving: tasks.worktree LOO-954
-- leaving: tasks.branch LOO-954
-- leaving: tasks.base_commit LOO-954
-- leaving: tasks.parent_pr_id LOO-954
-- leaving: tasks.checkout_machine_id LOO-954
-- leaving: tasks.primary_session_id LOO-954
-- leaving: tasks.ship_lf_process_id LOO-954
-- leaving: tasks.ship_restarts LOO-954
-- leaving: tasks.ship_approved_lf_process_id LOO-954
-- leaving: tasks.ship_stopped_at LOO-954
-- leaving: tasks.ship_stopped_lf_process_id LOO-954
-- leaving: tasks.ship_stopped_person LOO-954
