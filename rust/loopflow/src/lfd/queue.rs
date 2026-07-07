use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use once_cell::sync::Lazy;
use time::OffsetDateTime;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::engine::git;
use crate::engine::identity::WaveId;
use crate::engine::worktrees::main_repo_root;
use crate::lfd::attention::{
    attention_id_for_queue_block, queue_block_attention_item_from_existing,
};
use crate::lfd::config::GitHubConfig;
use crate::lfd::id::LfdId;
use crate::lfd::live_pr::{build_live_pr_snapshot, run_live_pr_key, LivePrSnapshot};
use crate::lfd::types::{
    AttentionStatus, LivePrState, LivePullRequestState, QueueBlock, QueueBlockReason, Run,
    RunStackStatus,
};
use crate::lfdb::SharedStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueRole {
    Ready,
    Draft,
    Blocked,
    Merged,
    Superseded,
}

impl QueueRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Draft => "draft",
            Self::Blocked => "blocked",
            Self::Merged => "merged",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueNextAction {
    OpenPr,
    ResolveConflict,
    CombinePrs,
    AwaitMerge,
}

impl QueueNextAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenPr => "open_pr",
            Self::ResolveConflict => "resolve_conflict",
            Self::CombinePrs => "combine_prs",
            Self::AwaitMerge => "await_merge",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRunView {
    pub role: QueueRole,
    pub block_reason: Option<QueueBlockReason>,
    pub blocked_at: Option<OffsetDateTime>,
    pub next_action: QueueNextAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRebaseConflict {
    pub files: Vec<String>,
}

pub(crate) trait QueueOps: Send + Sync {
    fn ensure_branch_checked_out(&self, worktree: &Path, branch: &str) -> Result<(), String>;
    fn mark_ready(&self, worktree: &Path, pr_number: u32) -> Result<(), String>;
    fn mark_draft(&self, worktree: &Path, pr_number: u32) -> Result<(), String>;
    fn rebase_onto_default(
        &self,
        worktree: &Path,
        default_branch: &str,
        parent_landed: bool,
    ) -> Result<(), QueueRebaseConflict>;
    fn scratch_clean(&self, worktree: &Path) -> Result<bool, String>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RealQueueOps;

static QUEUE_RECONCILE_LOCKS: Lazy<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

impl QueueOps for RealQueueOps {
    fn ensure_branch_checked_out(&self, worktree: &Path, branch: &str) -> Result<(), String> {
        git::checkout(worktree, branch).map_err(|err| err.to_string())
    }

    fn mark_ready(&self, worktree: &Path, _pr_number: u32) -> Result<(), String> {
        crate::ops::mark_ready(worktree).map_err(|err| err.to_string())
    }

    fn mark_draft(&self, worktree: &Path, pr_number: u32) -> Result<(), String> {
        let output = Command::new("gh")
            .arg("pr")
            .arg("ready")
            .arg("--undo")
            .arg(pr_number.to_string())
            .current_dir(worktree)
            .output()
            .map_err(|err| err.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.to_ascii_lowercase().contains("already a draft") {
            return Ok(());
        }
        Err(if stderr.is_empty() {
            "failed to mark PR draft".to_string()
        } else {
            stderr
        })
    }

    fn rebase_onto_default(
        &self,
        worktree: &Path,
        default_branch: &str,
        parent_landed: bool,
    ) -> Result<(), QueueRebaseConflict> {
        let main_repo = main_repo_root(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        git::fetch(&main_repo, "origin", default_branch).map_err(|err| QueueRebaseConflict {
            files: vec![err.to_string()],
        })?;
        // When this child's stack parent has landed, drop the parent's commits
        // by forking off its tip. `parent_landed` carries the daemon's lfdb
        // signal (the parent PR merged, content-independent) so a *reworked*
        // parent is handled too; without it the lazy rebase replays the parent's
        // commits against the default branch and blocks the queue with a
        // spurious RebaseConflict.
        let branch = git::current_branch(worktree)
            .ok()
            .flatten()
            .unwrap_or_default();
        let merged_parent =
            crate::ops::merged_parent_fork_point(worktree, &branch, default_branch, parent_landed);
        let fork_point = merged_parent
            .as_ref()
            .map(|(fork_point, _)| fork_point.clone());
        let rebase_result = git::rebase(
            worktree,
            &format!("origin/{default_branch}"),
            fork_point.as_deref(),
        )
        .map_err(|err| QueueRebaseConflict {
            files: vec![err.to_string()],
        })?;
        if !rebase_result.success {
            return Err(QueueRebaseConflict {
                files: rebase_result
                    .conflicts
                    .unwrap_or_default()
                    .into_iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
            });
        }
        git::push(worktree, true).map_err(|err| QueueRebaseConflict {
            files: vec![err.to_string()],
        })?;
        // The child re-parented onto the default branch; prune the merged
        // parent's lingering local ref so it stops resolving as an open base.
        if let Some((_, Some(local_parent))) = merged_parent {
            let _ = git::delete_local_branch(worktree, &local_parent);
        }
        Ok(())
    }

    fn scratch_clean(&self, worktree: &Path) -> Result<bool, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["status", "--porcelain", "--", "scratch/"])
            .output()
            .map_err(|err| err.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }
}

pub async fn remove_reconcile_lock(wave_id: &LfdId) {
    let mut locks = QUEUE_RECONCILE_LOCKS.lock().await;
    locks.remove(&wave_id.to_string());
}

pub(crate) async fn acquire_reconcile_lock(wave_id: &LfdId) -> OwnedMutexGuard<()> {
    let wave_key = wave_id.to_string();
    let lock = {
        let mut locks = QUEUE_RECONCILE_LOCKS.lock().await;
        locks
            .entry(wave_key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    lock.lock_owned().await
}

pub(crate) async fn reconcile_wave_queue_with_ops(
    store: &SharedStore,
    github_config: &GitHubConfig,
    wave_id: &LfdId,
    ops: &dyn QueueOps,
) -> Result<(), String> {
    let wave_id_for_log = wave_id.clone();
    let mut runs = store
        .list_stack_runs(wave_id)
        .await
        .map_err(|err| format!("list_stack_runs failed: {err}"))?;
    if runs.is_empty() {
        return Ok(());
    }

    let mut live_snapshot = build_live_pr_snapshot(store, github_config, &runs)
        .await
        .map_err(|err| format!("build_live_pr_snapshot failed: {err}"))?;

    let mut status_changed = false;
    for run in &mut runs {
        let Some(live_state) = live_snapshot.state_for_run(run) else {
            continue;
        };
        let inferred = inferred_stack_status(run.stack_status, Some(live_state));
        if inferred != run.stack_status {
            run.stack_status = inferred;
            store
                .update_run(run)
                .await
                .map_err(|err| format!("update_run failed: {err}"))?;
            status_changed = true;
        }
    }
    if status_changed {
        runs = store
            .list_stack_runs(wave_id)
            .await
            .map_err(|err| format!("list_stack_runs refresh failed: {err}"))?;
        live_snapshot = build_live_pr_snapshot(store, github_config, &runs)
            .await
            .map_err(|err| format!("build_live_pr_snapshot refresh failed: {err}"))?;
    }

    let head_index = find_queue_head_index(&runs, &live_snapshot);
    let Some(head_index) = head_index else {
        tracing::debug!(wave_id = %wave_id_for_log, "queue reconcile: no active queue head");
        return Ok(());
    };

    for (index, run) in runs.iter().enumerate() {
        if index == head_index {
            continue;
        }
        let Some(pr_number) = pr_number(run) else {
            continue;
        };
        let Some(state) = live_snapshot.state_for_run(run) else {
            continue;
        };
        if state.state == LivePrState::Open
            && !state.is_draft
            && ops
                .ensure_branch_checked_out(Path::new(&run.worktree), &run.branch)
                .and_then(|_| ops.mark_draft(Path::new(&run.worktree), pr_number))
                .is_ok()
        {
            let mut updated = state.clone();
            updated.is_draft = true;
            updated.synced_at = OffsetDateTime::now_utc();
            let _ = store.upsert_live_pr_state(&updated).await;
            if let Some(key) = run_live_pr_key(run) {
                live_snapshot.live_states.insert(key, updated);
            }
        }
    }

    let head = runs[head_index].clone();
    let Some(head_pr_number) = pr_number(&head) else {
        set_queue_block(store, &head, QueueBlockReason::MissingPr, Vec::new(), None).await?;
        return Ok(());
    };
    let Some(head_live_state) = live_snapshot.state_for_run(&head) else {
        set_queue_block(store, &head, QueueBlockReason::MissingPr, Vec::new(), None).await?;
        return Ok(());
    };
    if head_live_state.state != LivePrState::Open {
        return Ok(());
    }

    if let Some(active_run) = store
        .get_active_run(wave_id)
        .await
        .map_err(|err| format!("get_active_run failed: {err}"))?
    {
        if active_run.id != head.id {
            set_queue_block(
                store,
                &head,
                QueueBlockReason::WaveRunning,
                Vec::new(),
                None,
            )
            .await?;
            return Ok(());
        }
    }

    let worktree = Path::new(&head.worktree);
    if !ops.scratch_clean(worktree)? {
        set_queue_block(
            store,
            &head,
            QueueBlockReason::ScratchDirty,
            Vec::new(),
            None,
        )
        .await?;
        return Ok(());
    }

    if head.stack_position > 0 {
        let main_repo = main_repo_root(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        let default_branch =
            git::get_default_branch(&main_repo).unwrap_or_else(|_| "main".to_string());
        // The head is the oldest unmerged run, so its stack parent has usually
        // landed — but only re-parent onto main when that parent's PR is
        // actually Merged (not merely closed/superseded, whose changes never
        // reached main). This lfdb signal is content-independent, so a reworked
        // parent is caught where a git content-check would miss it.
        let parent_landed = head_stack_parent_merged(&runs, &head, &live_snapshot);
        if let Err(conflict) = ops
            .ensure_branch_checked_out(worktree, &head.branch)
            .map_err(|err| QueueRebaseConflict { files: vec![err] })
            .and_then(|_| ops.rebase_onto_default(worktree, &default_branch, parent_landed))
        {
            set_queue_block(
                store,
                &head,
                QueueBlockReason::RebaseConflict,
                conflict.files.clone(),
                Some("lazy rebase failed".to_string()),
            )
            .await?;
            return Ok(());
        }
    }

    if head_live_state.is_draft {
        ops.ensure_branch_checked_out(worktree, &head.branch)?;
        if let Err(err) = ops.mark_ready(worktree, head_pr_number) {
            set_queue_block(
                store,
                &head,
                QueueBlockReason::PromotionFailed,
                Vec::new(),
                Some(err),
            )
            .await?;
            return Ok(());
        }

        let mut promoted_state = head_live_state.clone();
        promoted_state.is_draft = false;
        promoted_state.synced_at = OffsetDateTime::now_utc();
        let _ = store.upsert_live_pr_state(&promoted_state).await;
    }

    clear_queue_block(store, wave_id, &head.id).await?;
    Ok(())
}

pub fn project_queue_views<F>(
    runs: &[Run],
    mut live_state_for: F,
    blocks: &HashMap<LfdId, QueueBlock>,
) -> HashMap<LfdId, QueueRunView>
where
    F: FnMut(&Run) -> Option<LivePullRequestState>,
{
    let mut live_by_run = HashMap::new();
    for run in runs {
        live_by_run.insert(run.id.clone(), live_state_for(run));
    }

    let head_index = runs.iter().position(|run| {
        let live = live_by_run.get(&run.id).and_then(|value| value.as_ref());
        inferred_stack_status(run.stack_status, live) == RunStackStatus::Active
    });

    let mut result = HashMap::with_capacity(runs.len());
    for (index, run) in runs.iter().enumerate() {
        let block = blocks.get(&run.id);
        let live = live_by_run.get(&run.id).and_then(|value| value.as_ref());
        let role = match inferred_stack_status(run.stack_status, live) {
            RunStackStatus::Merged => QueueRole::Merged,
            RunStackStatus::Superseded => QueueRole::Superseded,
            RunStackStatus::Active => {
                if block.is_some() {
                    QueueRole::Blocked
                } else if head_index == Some(index) {
                    QueueRole::Ready
                } else {
                    QueueRole::Draft
                }
            }
        };
        let next_action = queue_next_action(role, block, pr_number(run).is_some());
        result.insert(
            run.id.clone(),
            QueueRunView {
                role,
                block_reason: block.map(|value| value.reason),
                blocked_at: block.map(|value| value.attempted_at),
                next_action,
            },
        );
    }

    result
}

fn queue_next_action(role: QueueRole, block: Option<&QueueBlock>, has_pr: bool) -> QueueNextAction {
    match role {
        QueueRole::Ready | QueueRole::Merged => QueueNextAction::AwaitMerge,
        QueueRole::Superseded => QueueNextAction::CombinePrs,
        QueueRole::Draft => {
            if has_pr {
                QueueNextAction::AwaitMerge
            } else {
                QueueNextAction::OpenPr
            }
        }
        QueueRole::Blocked => match block.map(|value| value.reason) {
            Some(QueueBlockReason::ScratchDirty | QueueBlockReason::RebaseConflict) => {
                QueueNextAction::ResolveConflict
            }
            Some(QueueBlockReason::MissingPr) => QueueNextAction::OpenPr,
            _ => QueueNextAction::AwaitMerge,
        },
    }
}

/// True when the head's dotted stack parent has a run whose inferred status is
/// `Merged` — the content-independent "parent PR merged" signal that tells the
/// child to re-parent onto the default branch (even for a reworked parent).
fn head_stack_parent_merged(runs: &[Run], head: &Run, live_snapshot: &LivePrSnapshot) -> bool {
    // The head branch is author-scoped (`jack/wave.child`), so `WaveId::parse`
    // recovers the user from the branch itself; the fallback is never used.
    let Some(parent) = WaveId::parse(&head.branch, "user").and_then(|id| id.parent()) else {
        return false;
    };
    runs.iter().any(|run| {
        run.branch == parent
            && inferred_stack_status(run.stack_status, live_snapshot.state_for_run(run))
                == RunStackStatus::Merged
    })
}

fn find_queue_head_index(runs: &[Run], live_snapshot: &LivePrSnapshot) -> Option<usize> {
    runs.iter().position(|run| {
        let live = live_snapshot.state_for_run(run);
        inferred_stack_status(run.stack_status, live) == RunStackStatus::Active
    })
}

fn inferred_stack_status(
    durable: RunStackStatus,
    live: Option<&LivePullRequestState>,
) -> RunStackStatus {
    if durable != RunStackStatus::Active {
        return durable;
    }
    match live.map(|state| state.state) {
        Some(LivePrState::Merged) => RunStackStatus::Merged,
        Some(LivePrState::Closed) => RunStackStatus::Superseded,
        _ => RunStackStatus::Active,
    }
}

fn pr_number(run: &Run) -> Option<u32> {
    run.pr.as_ref()?.number
}

async fn set_queue_block(
    store: &SharedStore,
    run: &Run,
    reason: QueueBlockReason,
    conflict_files: Vec<String>,
    error: Option<String>,
) -> Result<(), String> {
    let block = QueueBlock {
        wave_id: run.wave_id.clone(),
        run_id: run.id.clone(),
        reason,
        attempted_at: OffsetDateTime::now_utc(),
        conflict_files,
        error,
    };
    let attention_id = attention_id_for_queue_block(&block.run_id);
    let existing = store
        .get_attention_item(&attention_id)
        .await
        .map_err(|err| format!("get queue attention item failed: {err}"))?;
    let item = queue_block_attention_item_from_existing(&block, existing.as_ref());
    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("upsert_queue_block failed: {err}"))?;

    Ok(())
}

async fn clear_queue_block(
    store: &SharedStore,
    _wave_id: &LfdId,
    run_id: &LfdId,
) -> Result<(), String> {
    let attention_id = attention_id_for_queue_block(run_id);
    let Some(mut item) = store
        .get_attention_item(&attention_id)
        .await
        .map_err(|err| format!("get queue attention item failed: {err}"))?
    else {
        return Ok(());
    };
    if item.status == AttentionStatus::Resolved {
        return Ok(());
    }
    item.status = AttentionStatus::Resolved;
    item.resolved_at = Some(OffsetDateTime::now_utc());
    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("resolve queue attention item failed: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        PullRequest, QueueBlockReason, Run, RunStatus, Wave, WaveStatus, DEFAULT_WAVE_FLOW,
    };

    #[derive(Debug, Default)]
    struct MockOps {
        scratch_clean: bool,
        rebase_fail_for_branch: Option<String>,
        ready_calls: Mutex<Vec<String>>,
    }

    impl QueueOps for MockOps {
        fn ensure_branch_checked_out(&self, _worktree: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }

        fn mark_ready(&self, _worktree: &Path, _pr_number: u32) -> Result<(), String> {
            self.ready_calls
                .lock()
                .expect("mutex")
                .push("ready".to_string());
            Ok(())
        }

        fn mark_draft(&self, _worktree: &Path, _pr_number: u32) -> Result<(), String> {
            Ok(())
        }

        fn rebase_onto_default(
            &self,
            _worktree: &Path,
            default_branch: &str,
            _parent_landed: bool,
        ) -> Result<(), QueueRebaseConflict> {
            if self
                .rebase_fail_for_branch
                .as_ref()
                .is_some_and(|value| value == default_branch)
            {
                return Err(QueueRebaseConflict {
                    files: vec!["src/lib.rs".to_string()],
                });
            }
            Ok(())
        }

        fn scratch_clean(&self, _worktree: &Path) -> Result<bool, String> {
            Ok(self.scratch_clean)
        }
    }

    async fn sqlite_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-queue-test-{}.db", LfdId::new()));
        let config = crate::lfdb::StorageConfig::sqlite(db_path);
        Arc::new(
            crate::lfdb::open_store(&config)
                .await
                .expect("sqlite store should initialize"),
        )
    }

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "queue-wave".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_run(wave: &Wave, stack_position: u32, pr_number: u32) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: wave.repo().to_string(),
            flow: DEFAULT_WAVE_FLOW.to_string(),
            task: None,
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            iteration: stack_position,
            step_index: 0,
            status: RunStatus::Completed,
            worktree: ".".to_string(),
            branch: format!("feature-{pr_number}"),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: Some(OffsetDateTime::now_utc()),
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position,
            stack_group_id: wave.id().to_string(),
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: Some(PullRequest {
                url: format!("https://example.test/pr/{pr_number}"),
                number: Some(pr_number),
                state: Some("open".to_string()),
                title: Some(format!("run-{pr_number}")),
                branch: Some(format!("feature-{pr_number}")),
            }),
        }
    }

    async fn set_live_open(store: &SharedStore, run: &Run, is_draft: bool) {
        let pr_number = pr_number(run).expect("pr number");
        store
            .upsert_live_pr_state(&LivePullRequestState {
                repo_id: run.repo.clone(),
                pr_number,
                state: LivePrState::Open,
                is_draft,
                head_ref: run.branch.clone(),
                head_sha: "abc123".to_string(),
                base_ref: "main".to_string(),
                updated_at: OffsetDateTime::now_utc(),
                merged_at: None,
                synced_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("live state");
    }

    #[tokio::test]
    async fn reconcile_promotes_only_oldest_unmerged() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run1 = make_run(&wave, 0, 11);
        let run2 = make_run(&wave, 1, 12);
        store.create_run(&run1).await.expect("run1");
        store.create_run(&run2).await.expect("run2");
        set_live_open(&store, &run1, true).await;
        set_live_open(&store, &run2, true).await;

        let ops = MockOps {
            scratch_clean: true,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &ops)
            .await
            .expect("reconcile");

        let blocks = store
            .list_queue_blocks(wave.id())
            .await
            .expect("list queue blocks")
            .into_iter()
            .map(|block| (block.run_id.clone(), block))
            .collect::<HashMap<_, _>>();
        let runs = store.list_stack_runs(wave.id()).await.expect("runs");
        let live_snapshot = build_live_pr_snapshot(&store, &GitHubConfig::default(), &runs)
            .await
            .expect("live snapshot");
        let projected = project_queue_views(
            &runs,
            |run| live_snapshot.state_for_run(run).cloned(),
            &blocks,
        );
        let ready_count = projected
            .values()
            .filter(|view| view.role == QueueRole::Ready)
            .count();
        assert_eq!(ready_count, 1);
        assert_eq!(
            projected.get(&run1.id).map(|view| view.role),
            Some(QueueRole::Ready)
        );
        assert_eq!(
            projected.get(&run2.id).map(|view| view.role),
            Some(QueueRole::Draft)
        );
    }

    #[tokio::test]
    async fn scratch_dirty_marks_blocked_with_resolve_conflict_action() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run = make_run(&wave, 0, 31);
        store.create_run(&run).await.expect("run");
        set_live_open(&store, &run, true).await;

        let ops = MockOps {
            scratch_clean: false,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &ops)
            .await
            .expect("reconcile");

        let block = store
            .list_queue_blocks(wave.id())
            .await
            .expect("blocks")
            .into_iter()
            .next()
            .expect("block");
        assert_eq!(block.reason, QueueBlockReason::ScratchDirty);

        let live_snapshot =
            build_live_pr_snapshot(&store, &GitHubConfig::default(), std::slice::from_ref(&run))
                .await
                .expect("live snapshot");
        let projection = project_queue_views(
            std::slice::from_ref(&run),
            |r| live_snapshot.state_for_run(r).cloned(),
            &HashMap::from([(run.id.clone(), block)]),
        );
        assert_eq!(
            projection.get(&run.id).map(|view| view.next_action),
            Some(QueueNextAction::ResolveConflict)
        );
    }

    #[tokio::test]
    async fn repeated_queue_block_preserves_age() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run = make_run(&wave, 0, 41);
        store.create_run(&run).await.expect("run");
        set_live_open(&store, &run, true).await;

        let ops = MockOps {
            scratch_clean: false,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &ops)
            .await
            .expect("first reconcile");

        let attention_id = attention_id_for_queue_block(&run.id);
        let first_item = store
            .get_attention_item(&attention_id)
            .await
            .expect("get attention item")
            .expect("queue block attention item created");

        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &ops)
            .await
            .expect("second reconcile");

        // A repeated block keeps the one row and its original age — reconcile
        // doesn't reset surfaced_at on every pass.
        let queue_item = store
            .get_attention_item(&attention_id)
            .await
            .expect("get attention item")
            .expect("attention item exists");
        assert_eq!(
            queue_item.surfaced_at.unix_timestamp(),
            first_item.surfaced_at.unix_timestamp()
        );
    }

    #[tokio::test]
    async fn clearing_queue_block_resolves_attention() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run = make_run(&wave, 0, 51);
        store.create_run(&run).await.expect("run");
        set_live_open(&store, &run, true).await;

        let blocked_ops = MockOps {
            scratch_clean: false,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &blocked_ops)
            .await
            .expect("block reconcile");

        let cleared_ops = MockOps {
            scratch_clean: true,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(&store, &GitHubConfig::default(), wave.id(), &cleared_ops)
            .await
            .expect("clear reconcile");

        // Clearing the block resolves the attention row in the ledger — the
        // durable truth, no push.
        let attention_id = attention_id_for_queue_block(&run.id);
        let item = store
            .get_attention_item(&attention_id)
            .await
            .expect("get attention item")
            .expect("attention item exists");
        assert_eq!(item.status, AttentionStatus::Resolved);
    }

    #[tokio::test]
    async fn reconcile_lock_serializes_per_wave() {
        let wave_id = LfdId::new();
        let guard = acquire_reconcile_lock(&wave_id).await;

        let locked = Arc::new(Mutex::new(false));
        let locked_clone = Arc::clone(&locked);
        let wave_id_clone = wave_id.clone();
        let waiter = tokio::spawn(async move {
            let _guard = acquire_reconcile_lock(&wave_id_clone).await;
            *locked_clone.lock().expect("mutex") = true;
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!*locked.lock().expect("mutex"));

        drop(guard);
        waiter.await.expect("waiter task");
        assert!(*locked.lock().expect("mutex"));
    }

    fn git_out(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn queue_rebase_drops_squash_merged_parent_commits() {
        use loopflow_test_support::TestRepo;

        // Parent `a.b` builds shared.txt over two commits; child `a.b.c` extends
        // it. A plain `git rebase origin/main` (fork-point None) would replay the
        // parent's commits and conflict — the queue's old behavior, which
        // surfaces as a spurious RebaseConflict block. The fork-point fix must
        // drop the parent's commits and rebase cleanly.
        let repo = TestRepo::new();

        repo.create_branch("jack/a.b");
        repo.create_file("shared.txt", "a-line-1\na-line-2\n");
        repo.stage_all();
        repo.commit("p1");
        repo.create_file("shared.txt", "a-line-1\na-line-2\na-line-3\n");
        repo.stage_all();
        repo.commit("p2");

        repo.create_branch("jack/a.b.c");
        repo.create_file("shared.txt", "a-line-1\na-line-2\na-line-3\nb-line\n");
        repo.stage_all();
        repo.commit("child work");
        repo.push_new_branch("jack/a.b.c");

        // Squash-merge the parent into main; leave the local `jack/a.b` ref dangling.
        repo.checkout("main");
        git_out(repo.path(), &["merge", "--squash", "jack/a.b"]);
        git_out(repo.path(), &["commit", "-m", "squash merge a.b"]);
        repo.push();

        repo.checkout("jack/a.b.c");
        RealQueueOps
            .rebase_onto_default(repo.path(), "main", true)
            .expect("queue rebase should drop merged parent and succeed");

        // The child carries only its own change relative to main.
        let commits_beyond = git_out(repo.path(), &["rev-list", "--count", "origin/main..HEAD"]);
        assert_eq!(
            commits_beyond, "1",
            "only the child's commit sits above main"
        );
        let diff = git_out(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]);
        assert_eq!(diff, "shared.txt");
        // The merged parent's lingering local ref was pruned.
        let branches = git_out(repo.path(), &["branch", "--list", "jack/a.b"]);
        assert!(branches.is_empty(), "merged local parent should be pruned");
    }

    #[test]
    fn queue_rebase_surfaces_conflict_when_reworked_parent_overlaps() {
        use loopflow_test_support::TestRepo;

        // The parent landed REWORKED (lfdb says merged; main's content diverges)
        // and the child's own commit overlaps the divergent lines. The queue
        // must surface a RebaseConflict — not silently rebase onto the stale
        // parent, not auto-heal.
        let repo = TestRepo::new();

        repo.create_branch("jack/a.b");
        repo.create_file("feature.txt", "v1\n");
        repo.stage_all();
        repo.commit("feature v1");
        repo.push_new_branch("jack/a.b");

        repo.create_branch("jack/a.b.c");
        repo.create_file("feature.txt", "v1\nchild addition\n");
        repo.stage_all();
        repo.commit("child extends feature");
        repo.push_new_branch("jack/a.b.c");

        repo.checkout("main");
        repo.create_file("feature.txt", "v2 reworked\n");
        repo.stage_all();
        repo.commit("feature v2");
        repo.push();
        git_out(repo.path(), &["push", "origin", "--delete", "jack/a.b"]);

        repo.checkout("jack/a.b.c");
        // parent_landed = true: lfdb reports the parent PR merged.
        let result = RealQueueOps.rebase_onto_default(repo.path(), "main", true);
        assert!(
            result.is_err(),
            "reworked overlap must block the queue with a conflict"
        );
    }
}
