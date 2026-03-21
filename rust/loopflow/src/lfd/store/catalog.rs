use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SqlDialect {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub(crate) enum Query {
    HealthCheck,
    ListWaves,
    ListWavesByRepo,
    UpsertWave,
    GetWaveById,
    GetWaveByName,
    DeleteWaveById,
    ListWaveRunsAll,
    ListWaveRunsByWave,
    ListWaveRunsLimited,
    ListWaveRunsByWaveLimited,
    GetWaveRunById,
    GetActiveWaveRun,
    CountActiveWaveRuns,
    GetLatestWaveRun,
    InsertWaveRun,
    UpdateWaveRun,
    ListStackRuns,
    FailOrphanedRuns,
    GetLivePrState,
    UpsertLivePrState,
    ListTriggers,
    ListTriggersByWave,
    ListTriggersBySignal,
    GetTriggerById,
    InsertTrigger,
    UpdateTrigger,
    DeleteTriggerById,
    ListPendingActivationsByWave,
    InsertPendingActivation,
    UpdatePendingActivation,
    DeletePendingActivationById,
    GetPendingActivationForTrigger,
    InsertActivationLog,
    ListActivationLogByWave,
    GetActivationLogById,
    ListForkRuns,
    UpsertForkRun,
    DeleteForkRuns,
    ListAgentHistoryAll,
    ListAgentHistoryByWorktree,
    ListAgentHistoryByRepo,
    ListAgentHistoryByWorktreeAndRepo,
    ListAgentHistoryLimited,
    ListAgentHistoryByWorktreeLimited,
    ListAgentHistoryByRepoLimited,
    ListAgentHistoryByWorktreeAndRepoLimited,
    GetAgentById,
    GetWaitingAgentForWave,
    InsertAgent,
    UpdateAgentStatus,
    EndAgent,
    GetActiveAgentsForWave,
    EndActiveAgentsForWave,
    GetStuckAgents,
    GetSummaryByWave,
    UpsertSummary,
    ListChatMemoryBlocks,
    UpsertChatMemoryBlock,
    DeleteChatMemoryBlock,
    ListLoopableWaves,
    ListCronWaves,
    GetPendingActivationForWave,
}

impl Query {
    pub(crate) const ALL: [Self; QUERY_COUNT] = [
        Self::HealthCheck,
        Self::ListWaves,
        Self::ListWavesByRepo,
        Self::UpsertWave,
        Self::GetWaveById,
        Self::GetWaveByName,
        Self::DeleteWaveById,
        Self::ListWaveRunsAll,
        Self::ListWaveRunsByWave,
        Self::ListWaveRunsLimited,
        Self::ListWaveRunsByWaveLimited,
        Self::GetWaveRunById,
        Self::GetActiveWaveRun,
        Self::CountActiveWaveRuns,
        Self::GetLatestWaveRun,
        Self::InsertWaveRun,
        Self::UpdateWaveRun,
        Self::ListStackRuns,
        Self::FailOrphanedRuns,
        Self::GetLivePrState,
        Self::UpsertLivePrState,
        Self::ListTriggers,
        Self::ListTriggersByWave,
        Self::ListTriggersBySignal,
        Self::GetTriggerById,
        Self::InsertTrigger,
        Self::UpdateTrigger,
        Self::DeleteTriggerById,
        Self::ListPendingActivationsByWave,
        Self::InsertPendingActivation,
        Self::UpdatePendingActivation,
        Self::DeletePendingActivationById,
        Self::GetPendingActivationForTrigger,
        Self::InsertActivationLog,
        Self::ListActivationLogByWave,
        Self::GetActivationLogById,
        Self::ListForkRuns,
        Self::UpsertForkRun,
        Self::DeleteForkRuns,
        Self::ListAgentHistoryAll,
        Self::ListAgentHistoryByWorktree,
        Self::ListAgentHistoryByRepo,
        Self::ListAgentHistoryByWorktreeAndRepo,
        Self::ListAgentHistoryLimited,
        Self::ListAgentHistoryByWorktreeLimited,
        Self::ListAgentHistoryByRepoLimited,
        Self::ListAgentHistoryByWorktreeAndRepoLimited,
        Self::GetAgentById,
        Self::GetWaitingAgentForWave,
        Self::InsertAgent,
        Self::UpdateAgentStatus,
        Self::EndAgent,
        Self::GetActiveAgentsForWave,
        Self::EndActiveAgentsForWave,
        Self::GetStuckAgents,
        Self::GetSummaryByWave,
        Self::UpsertSummary,
        Self::ListChatMemoryBlocks,
        Self::UpsertChatMemoryBlock,
        Self::DeleteChatMemoryBlock,
        Self::ListLoopableWaves,
        Self::ListCronWaves,
        Self::GetPendingActivationForWave,
    ];
}

const QUERY_COUNT: usize = Query::GetPendingActivationForWave as usize + 1;

#[derive(Debug, Clone, Copy)]
struct QueryDef {
    template: &'static str,
    sqlite_override: Option<&'static str>,
    postgres_override: Option<&'static str>,
}

const QUERY_DEFS: [QueryDef; QUERY_COUNT] = [
    QueryDef {
        template: "SELECT 1",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves\n             WHERE repo = {p1}\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO waves (\n                id, name, repo, direction, area, paused, status, iteration, cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12}, {p13}, {p14})\n            ON CONFLICT(id) DO UPDATE SET\n                name = excluded.name,\n                repo = excluded.repo,\n                direction = excluded.direction,\n                area = excluded.area,\n                paused = excluded.paused,\n                status = excluded.status,\n                iteration = excluded.iteration,\n                cycle_start_iteration = excluded.cycle_start_iteration,\n                created_at = excluded.created_at,\n                workers = excluded.workers,\n                mode = excluded.mode,\n                primary_flow = excluded.primary_flow,\n                cron = excluded.cron",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves\n             WHERE name = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM waves WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             WHERE wave_id = {p1}\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             ORDER BY started_at DESC\n             LIMIT {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             WHERE wave_id = {p1}\n             ORDER BY started_at DESC\n             LIMIT {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             WHERE wave_id = {p1} AND status IN ({p2}, {p3}, {p4})\n             ORDER BY started_at DESC LIMIT 1",
        sqlite_override: None,
        postgres_override: Some(
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             WHERE wave_id = {p1} AND status = ANY({p2})\n             ORDER BY started_at DESC LIMIT 1",
        ),
    },
    QueryDef {
        template: "SELECT COUNT(*) FROM wave_runs WHERE wave_id = {p1} AND status IN ({p2}, {p3}, {p4})",
        sqlite_override: None,
        postgres_override: Some(
            "SELECT COUNT(*) FROM wave_runs WHERE wave_id = {p1} AND status = ANY({p2})",
        ),
    },
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs WHERE wave_id = {p1}\n             ORDER BY started_at DESC LIMIT 1",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO wave_runs (\n                id, wave_id, iteration, step_index, status, worktree, branch,\n                started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                lineage_inferred, target_branch, repair_of\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12}, {p13}, {p14}, {p15}, {p16}, {p17}, {p18}, {p19}, {p20}, {p21}, {p22}, {p23}, {p24}, {p25}, {p26})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE wave_runs\n             SET iteration = {p1}, step_index = {p2}, status = {p3}, worktree = {p4},\n                 branch = {p5}, started_at = {p6}, ended_at = {p7}, error = {p8},\n                 snapshot_repo = {p9}, snapshot_flow = {p10}, snapshot_direction = {p11},\n                 snapshot_area = {p12}, snapshot_pr = {p13}, flow_parents = {p14},\n                 execution_cursor = {p15}, activation_log_id = {p16}, parent_run_id = {p17},\n                 parent_pr_number = {p18}, stack_position = {p19}, stack_group_id = {p20},\n                 stack_status = {p21}, lineage_inferred = {p22}, target_branch = {p23},\n                 repair_of = {p24}\n             WHERE id = {p25}",
        sqlite_override: None,
        postgres_override: None,
    },
    // NOTE: 'ci-fix' literal must match types::CI_FIX_FLOW constant.
    QueryDef {
        template: "SELECT id, wave_id, iteration, step_index, status, worktree, branch,\n                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,\n                    snapshot_area, snapshot_pr, flow_parents, execution_cursor, activation_log_id,\n                    parent_run_id, parent_pr_number, stack_position, stack_group_id, stack_status,\n                    lineage_inferred, target_branch, repair_of\n             FROM wave_runs\n             WHERE wave_id = {p1} AND snapshot_flow <> 'ci-fix'\n             ORDER BY stack_position ASC, started_at ASC, id ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE wave_runs SET status = {p1}, error = {p2}, ended_at = {p3}\n             WHERE status IN ({p4}, {p5}, {p6})",
        sqlite_override: None,
        postgres_override: Some(
            "UPDATE wave_runs SET status = {p1}, error = {p2}, ended_at = {p3}\n             WHERE status = ANY({p4})",
        ),
    },
    QueryDef {
        template: "SELECT repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,\n                    updated_at, merged_at, synced_at\n             FROM live_pr_states\n             WHERE repo_id = {p1} AND pr_number = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO live_pr_states (\n                repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,\n                updated_at, merged_at, synced_at\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10})\n            ON CONFLICT(repo_id, pr_number) DO UPDATE SET\n                state = excluded.state,\n                is_draft = excluded.is_draft,\n                head_ref = excluded.head_ref,\n                head_sha = excluded.head_sha,\n                base_ref = excluded.base_ref,\n                updated_at = excluded.updated_at,\n                merged_at = excluded.merged_at,\n                synced_at = excluded.synced_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, signal, flow, last_main_sha, last_triggered_at, created_at, enabled, source_wave_id, max_iterations\n             FROM triggers ORDER BY created_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, signal, flow, last_main_sha, last_triggered_at, created_at, enabled, source_wave_id, max_iterations\n             FROM triggers WHERE wave_id = {p1} ORDER BY created_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, signal, flow, last_main_sha, last_triggered_at, created_at, enabled, source_wave_id, max_iterations\n             FROM triggers WHERE signal = {p1} ORDER BY created_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, signal, flow, last_main_sha, last_triggered_at, created_at, enabled, source_wave_id, max_iterations\n             FROM triggers WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO triggers (id, wave_id, signal, flow, last_main_sha, last_triggered_at, created_at, enabled, source_wave_id, max_iterations)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE triggers SET\n                signal = {p1}, flow = {p2}, last_main_sha = {p3},\n                last_triggered_at = {p4}, enabled = {p5}, source_wave_id = {p6},\n                max_iterations = {p7}\n             WHERE id = {p8}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM triggers WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, trigger_id, reason, from_sha, to_sha, queued_at, target_branch\n             FROM pending_activations WHERE wave_id = {p1} ORDER BY queued_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO pending_activations (id, wave_id, trigger_id, reason, from_sha, to_sha, queued_at, target_branch)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE pending_activations SET reason = {p1}, from_sha = {p2}, to_sha = {p3}, target_branch = {p4} WHERE id = {p5}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM pending_activations WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, trigger_id, reason, from_sha, to_sha, queued_at, target_branch\n             FROM pending_activations WHERE wave_id = {p1} AND trigger_id = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO activation_log (id, wave_id, trigger_id, reason, outcome, created_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, trigger_id, reason, outcome, created_at\n             FROM activation_log\n             WHERE wave_id = {p1}\n             ORDER BY created_at DESC\n             LIMIT {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, trigger_id, reason, outcome, created_at\n             FROM activation_log\n             WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_run_id, step_index, branch_index, status, worktree\n             FROM fork_runs WHERE wave_run_id = {p1} AND step_index = {p2}\n             ORDER BY branch_index ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO fork_runs (id, wave_run_id, step_index, branch_index, status, worktree)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6})\n             ON CONFLICT(id) DO UPDATE SET\n                 wave_run_id = excluded.wave_run_id,\n                 step_index = excluded.step_index,\n                 branch_index = excluded.branch_index,\n                 status = excluded.status,\n                 worktree = excluded.worktree",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM fork_runs WHERE wave_run_id = {p1} AND step_index = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE worktree = {p1}\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE repo = {p1}\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE worktree = {p1} AND repo = {p2}\n             ORDER BY started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             ORDER BY started_at DESC\n             LIMIT {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE worktree = {p1}\n             ORDER BY started_at DESC\n             LIMIT {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE repo = {p1}\n             ORDER BY started_at DESC\n             LIMIT {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents\n             WHERE worktree = {p1} AND repo = {p2}\n             ORDER BY started_at DESC\n             LIMIT {p3}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents WHERE id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,\n                    a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode\n             FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id\n             WHERE r.wave_id = {p1} AND a.status = {p2}\n             ORDER BY a.started_at DESC LIMIT 1",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO agents (\n                id, step, repo, worktree, wave_run_id, status, started_at,\n                ended_at, pid, container_id, model, run_mode\n            ) VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7}, {p8}, {p9}, {p10}, {p11}, {p12})",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE agents\n             SET status = {p1},\n                 pid = COALESCE({p2}, pid),\n                 container_id = COALESCE({p3}, container_id)\n             WHERE id = {p4}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE agents SET status = {p1}, ended_at = {p2} WHERE id = {p3}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,\n                    a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode\n             FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id\n             WHERE r.wave_id = {p1} AND a.ended_at IS NULL\n             ORDER BY a.started_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "UPDATE agents SET status = {p1}, ended_at = {p2}\n             WHERE wave_run_id IN (SELECT id FROM wave_runs WHERE wave_id = {p3})\n             AND ended_at IS NULL",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, step, repo, worktree, wave_run_id, status,\n                    started_at, ended_at, pid, container_id, model, run_mode\n             FROM agents WHERE ended_at IS NULL AND started_at <= {p1}\n             ORDER BY started_at ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT id, wave_id, content, source_hash, token_budget, model, created_at\n             FROM summaries WHERE wave_id = {p1}",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO summaries (id, wave_id, content, source_hash, token_budget, model, created_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5}, {p6}, {p7})\n             ON CONFLICT(wave_id) DO UPDATE SET\n                 content = excluded.content,\n                 source_hash = excluded.source_hash,\n                 token_budget = excluded.token_budget,\n                 model = excluded.model,\n                 created_at = excluded.created_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "SELECT wave_id, name, content, position, updated_at\n             FROM chat_memory_blocks\n             WHERE wave_id = {p1}\n             ORDER BY position ASC, name ASC",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "INSERT INTO chat_memory_blocks (wave_id, name, content, position, updated_at)\n             VALUES ({p1}, {p2}, {p3}, {p4}, {p5})\n             ON CONFLICT(wave_id, name) DO UPDATE SET\n                 content = excluded.content,\n                 position = excluded.position,\n                 updated_at = excluded.updated_at",
        sqlite_override: None,
        postgres_override: None,
    },
    QueryDef {
        template: "DELETE FROM chat_memory_blocks WHERE wave_id = {p1} AND name = {p2}",
        sqlite_override: None,
        postgres_override: None,
    },
    // ListLoopableWaves
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves\n             WHERE mode = 'loop' AND status != 4\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    // ListCronWaves
    QueryDef {
        template: "SELECT id, name, repo, direction, area, paused, status, iteration,\n                    cycle_start_iteration, created_at, workers, mode, primary_flow, cron\n             FROM waves\n             WHERE mode = 'cron' AND status != 4\n             ORDER BY created_at DESC",
        sqlite_override: None,
        postgres_override: None,
    },
    // GetPendingActivationForWave — match by wave_id where trigger_id IS NULL
    QueryDef {
        template: "SELECT id, wave_id, trigger_id, reason, from_sha, to_sha, queued_at, target_branch\n             FROM pending_activations WHERE wave_id = {p1} AND trigger_id IS NULL",
        sqlite_override: None,
        postgres_override: None,
    },
];

#[derive(Debug)]
struct RenderedCatalog {
    sqlite: Vec<String>,
    postgres: Vec<String>,
}

static RENDERED: LazyLock<RenderedCatalog> = LazyLock::new(|| {
    let mut sqlite = Vec::with_capacity(QUERY_DEFS.len());
    let mut postgres = Vec::with_capacity(QUERY_DEFS.len());

    for query in Query::ALL {
        let def = QUERY_DEFS[query as usize];
        sqlite.push(render_sql(
            def.sqlite_override.unwrap_or(def.template),
            SqlDialect::Sqlite,
        ));
        postgres.push(render_sql(
            def.postgres_override.unwrap_or(def.template),
            SqlDialect::Postgres,
        ));
    }

    RenderedCatalog { sqlite, postgres }
});

pub(crate) fn sql(query: Query, dialect: SqlDialect) -> &'static str {
    let index = query as usize;
    match dialect {
        SqlDialect::Sqlite => RENDERED.sqlite[index].as_str(),
        SqlDialect::Postgres => RENDERED.postgres[index].as_str(),
    }
}

pub(crate) fn list_waves_query(has_repo: bool) -> Query {
    if has_repo {
        Query::ListWavesByRepo
    } else {
        Query::ListWaves
    }
}

pub(crate) fn list_wave_runs_query(has_wave_id: bool, has_limit: bool) -> Query {
    match (has_wave_id, has_limit) {
        (false, false) => Query::ListWaveRunsAll,
        (true, false) => Query::ListWaveRunsByWave,
        (false, true) => Query::ListWaveRunsLimited,
        (true, true) => Query::ListWaveRunsByWaveLimited,
    }
}

pub(crate) fn list_triggers_query(has_wave_id: bool) -> Query {
    if has_wave_id {
        Query::ListTriggersByWave
    } else {
        Query::ListTriggers
    }
}

pub(crate) fn list_agent_history_query(
    has_worktree: bool,
    has_repo: bool,
    has_limit: bool,
) -> Query {
    match (has_worktree, has_repo, has_limit) {
        (false, false, false) => Query::ListAgentHistoryAll,
        (true, false, false) => Query::ListAgentHistoryByWorktree,
        (false, true, false) => Query::ListAgentHistoryByRepo,
        (true, true, false) => Query::ListAgentHistoryByWorktreeAndRepo,
        (false, false, true) => Query::ListAgentHistoryLimited,
        (true, false, true) => Query::ListAgentHistoryByWorktreeLimited,
        (false, true, true) => Query::ListAgentHistoryByRepoLimited,
        (true, true, true) => Query::ListAgentHistoryByWorktreeAndRepoLimited,
    }
}

fn render_sql(template: &str, dialect: SqlDialect) -> String {
    let mut output = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        if let Some((start, end)) = placeholder_bounds(template, idx) {
            let value = &template[start..end];
            match dialect {
                SqlDialect::Sqlite => {
                    output.push('?');
                    output.push_str(value);
                }
                SqlDialect::Postgres => {
                    output.push('$');
                    output.push_str(value);
                }
            }
            idx = end + 1;
            continue;
        }

        output.push(bytes[idx] as char);
        idx += 1;
    }

    output
}

fn placeholder_bounds(template: &str, idx: usize) -> Option<(usize, usize)> {
    let bytes = template.as_bytes();
    if idx + 2 >= bytes.len() || bytes[idx] != b'{' || bytes[idx + 1] != b'p' {
        return None;
    }

    let start = idx + 2;
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start || end >= bytes.len() || bytes[end] != b'}' {
        panic!("invalid placeholder in SQL template: {template}");
    }

    Some((start, end))
}

#[cfg(test)]
fn extract_placeholder_numbers(template: &str) -> Vec<usize> {
    let bytes = template.as_bytes();
    let mut numbers = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if let Some((start, end)) = placeholder_bounds(template, idx) {
            let value = template[start..end]
                .parse::<usize>()
                .expect("placeholder number should parse");
            numbers.push(value);
            idx = end + 1;
            continue;
        }

        idx += 1;
    }

    numbers
}

#[cfg(test)]
fn placeholders_are_contiguous(template: &str) -> bool {
    placeholder_numbers_are_contiguous(extract_placeholder_numbers(template))
}

#[cfg(test)]
fn extract_rendered_placeholder_numbers(sql: &str, marker: u8) -> Vec<usize> {
    let bytes = sql.as_bytes();
    let mut numbers = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == marker {
            let start = idx + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }

            if end > start {
                if let Ok(value) = sql[start..end].parse::<usize>() {
                    numbers.push(value);
                }
                idx = end;
                continue;
            }
        }
        idx += 1;
    }

    numbers
}

#[cfg(test)]
fn rendered_placeholders_are_contiguous(sql: &str, marker: u8) -> bool {
    placeholder_numbers_are_contiguous(extract_rendered_placeholder_numbers(sql, marker))
}

#[cfg(test)]
fn placeholder_numbers_are_contiguous(mut numbers: Vec<usize>) -> bool {
    if numbers.is_empty() {
        return true;
    }

    numbers.sort_unstable();
    numbers.dedup();
    numbers
        .iter()
        .enumerate()
        .all(|(idx, number)| *number == idx + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_catalog_definitions_cover_every_variant() {
        assert_eq!(QUERY_DEFS.len(), QUERY_COUNT, "query defs length mismatch");
        assert_eq!(
            Query::ALL.len(),
            QUERY_COUNT,
            "query enum list missing variants"
        );

        let mut seen = vec![false; QUERY_COUNT];
        for query in Query::ALL {
            let index = query as usize;
            assert!(
                index < QUERY_DEFS.len(),
                "query index out of range for {query:?}"
            );
            assert!(
                !seen[index],
                "duplicate query variant in Query::ALL for {query:?}"
            );
            seen[index] = true;

            let def = QUERY_DEFS[index];
            assert!(
                !def.template.trim().is_empty(),
                "empty SQL template for {query:?}"
            );
        }

        assert!(
            seen.into_iter().all(|value| value),
            "missing query definitions"
        );
    }

    #[test]
    fn every_query_renders_for_both_dialects_with_valid_placeholders() {
        for query in Query::ALL {
            let sqlite = std::panic::catch_unwind(|| sql(query, SqlDialect::Sqlite))
                .unwrap_or_else(|_| panic!("sqlite rendering panicked for {query:?}"));
            let postgres = std::panic::catch_unwind(|| sql(query, SqlDialect::Postgres))
                .unwrap_or_else(|_| panic!("postgres rendering panicked for {query:?}"));

            assert!(
                !sqlite.trim().is_empty(),
                "sqlite rendering is empty for {query:?}"
            );
            assert!(
                !postgres.trim().is_empty(),
                "postgres rendering is empty for {query:?}"
            );
            assert!(
                !sqlite.contains("{p"),
                "sqlite rendering still has placeholders for {query:?}"
            );
            assert!(
                !postgres.contains("{p"),
                "postgres rendering still has placeholders for {query:?}"
            );
            assert!(
                rendered_placeholders_are_contiguous(sqlite, b'?'),
                "sqlite placeholders not contiguous for {query:?}: {sqlite}"
            );
            assert!(
                rendered_placeholders_are_contiguous(postgres, b'$'),
                "postgres placeholders not contiguous for {query:?}: {postgres}"
            );
        }
    }

    #[test]
    fn catalog_placeholders_are_contiguous() {
        for query in Query::ALL {
            let def = QUERY_DEFS[query as usize];
            assert!(
                placeholders_are_contiguous(def.template),
                "template has non-contiguous placeholders for {query:?}"
            );
            if let Some(sqlite_override) = def.sqlite_override {
                assert!(
                    placeholders_are_contiguous(sqlite_override),
                    "sqlite override has non-contiguous placeholders for {query:?}"
                );
            }
            if let Some(postgres_override) = def.postgres_override {
                assert!(
                    placeholders_are_contiguous(postgres_override),
                    "postgres override has non-contiguous placeholders for {query:?}"
                );
            }
        }
    }
}
