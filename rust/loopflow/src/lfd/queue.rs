use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use time::OffsetDateTime;

use crate::engine::git;
use crate::engine::worktrees::main_repo_root;
use crate::lfd::config::GitHubConfig;
use crate::lfd::github;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    LivePrState, LivePullRequestState, QueueBlock, QueueMergeEvent, WaveRun, WaveRunStackStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueTrigger {
    RunCompleted,
    WebhookMerged,
    Poll,
}

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
    pub block_reason: Option<String>,
    pub blocked_at: Option<OffsetDateTime>,
    pub next_action: QueueNextAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRebaseConflict {
    pub files: Vec<String>,
}

trait QueueOps: Send + Sync {
    fn ensure_branch_checked_out(&self, worktree: &Path, branch: &str) -> Result<(), String>;
    fn mark_ready(&self, worktree: &Path, pr_number: u32) -> Result<(), String>;
    fn mark_draft(&self, worktree: &Path, pr_number: u32) -> Result<(), String>;
    fn rebase_onto_default(
        &self,
        worktree: &Path,
        default_branch: &str,
    ) -> Result<(), QueueRebaseConflict>;
    fn scratch_clean(&self, worktree: &Path) -> Result<bool, String>;
}

#[derive(Debug, Clone, Copy)]
struct RealQueueOps;

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
    ) -> Result<(), QueueRebaseConflict> {
        let main_repo = main_repo_root(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        git::fetch(&main_repo, "origin", default_branch).map_err(|err| QueueRebaseConflict {
            files: vec![err.to_string()],
        })?;
        let rebase_result = git::rebase(worktree, &format!("origin/{default_branch}"), None)
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

pub async fn reconcile_wave_queue(
    store: &SharedStore,
    github_config: &GitHubConfig,
    wave_id: &LfdId,
    trigger: QueueTrigger,
) -> Result<(), String> {
    reconcile_wave_queue_with_ops(store, github_config, wave_id, trigger, &RealQueueOps).await
}

async fn reconcile_wave_queue_with_ops(
    store: &SharedStore,
    github_config: &GitHubConfig,
    wave_id: &LfdId,
    trigger: QueueTrigger,
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

    refresh_live_states(store, github_config, &runs).await;
    let mut live_states = load_live_states(store, &runs).await;

    let mut status_changed = false;
    for run in &mut runs {
        let Some(live_state) = live_for_run(&live_states, run) else {
            continue;
        };
        let inferred = inferred_stack_status(run.stack_status, Some(&live_state));
        if inferred != run.stack_status {
            run.stack_status = inferred;
            store
                .update_wave_run(run)
                .await
                .map_err(|err| format!("update_wave_run failed: {err}"))?;
            status_changed = true;
        }
    }
    if status_changed {
        runs = store
            .list_stack_runs(wave_id)
            .await
            .map_err(|err| format!("list_stack_runs refresh failed: {err}"))?;
        live_states = load_live_states(store, &runs).await;
    }

    let head_index = find_queue_head_index(&runs, &live_states);
    let Some(head_index) = head_index else {
        tracing::debug!(wave_id = %wave_id_for_log, ?trigger, "queue reconcile: no active queue head");
        return Ok(());
    };

    for (index, run) in runs.iter().enumerate() {
        if index == head_index {
            continue;
        }
        let Some(pr_number) = pr_number(run) else {
            continue;
        };
        let Some(state) = live_for_run(&live_states, run) else {
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
            live_states.insert(run_pr_key(run), updated);
        }
    }

    let head = runs[head_index].clone();
    let Some(head_pr_number) = pr_number(&head) else {
        set_queue_block(store, &head, "missing_pr", Vec::new(), None).await?;
        return Ok(());
    };
    let Some(head_live_state) = live_for_run(&live_states, &head) else {
        set_queue_block(store, &head, "missing_pr", Vec::new(), None).await?;
        return Ok(());
    };
    if head_live_state.state != LivePrState::Open {
        return Ok(());
    }

    if let Some(active_run) = store
        .get_active_wave_run(wave_id)
        .await
        .map_err(|err| format!("get_active_wave_run failed: {err}"))?
    {
        if active_run.id != head.id {
            set_queue_block(store, &head, "wave_running", Vec::new(), None).await?;
            return Ok(());
        }
    }

    let worktree = Path::new(&head.worktree);
    if !ops.scratch_clean(worktree)? {
        set_queue_block(store, &head, "scratch_dirty", Vec::new(), None).await?;
        return Ok(());
    }

    if head.stack_position > 0 {
        let main_repo = main_repo_root(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        let default_branch =
            git::get_default_branch(&main_repo).unwrap_or_else(|_| "main".to_string());
        if let Err(conflict) = ops
            .ensure_branch_checked_out(worktree, &head.branch)
            .map_err(|err| QueueRebaseConflict { files: vec![err] })
            .and_then(|_| ops.rebase_onto_default(worktree, &default_branch))
        {
            set_queue_block(
                store,
                &head,
                "rebase_conflict",
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
            set_queue_block(store, &head, "promotion_failed", Vec::new(), Some(err)).await?;
            return Ok(());
        }

        let mut promoted_state = head_live_state.clone();
        promoted_state.is_draft = false;
        promoted_state.synced_at = OffsetDateTime::now_utc();
        let _ = store.upsert_live_pr_state(&promoted_state).await;
    }

    let _ = store.delete_queue_block(wave_id, &head.id).await;
    Ok(())
}

pub async fn handle_pr_merged(
    store: &SharedStore,
    github_config: &GitHubConfig,
    wave_id: &LfdId,
    merged_pr_number: u32,
    merged_at: OffsetDateTime,
) -> Result<bool, String> {
    let event = QueueMergeEvent {
        wave_id: wave_id.clone(),
        pr_number: merged_pr_number,
        merged_at,
        processed_at: OffsetDateTime::now_utc(),
    };
    let inserted = store
        .record_merge_event(&event)
        .await
        .map_err(|err| format!("record_merge_event failed: {err}"))?;
    if !inserted {
        return Ok(false);
    }

    if let Some(mut run) = store
        .list_stack_runs(wave_id)
        .await
        .map_err(|err| format!("list_stack_runs failed: {err}"))?
        .into_iter()
        .find(|run| pr_number(run) == Some(merged_pr_number))
    {
        run.stack_status = WaveRunStackStatus::Merged;
        let _ = store.update_wave_run(&run).await;
    }

    if let Err(err) =
        reconcile_wave_queue(store, github_config, wave_id, QueueTrigger::WebhookMerged).await
    {
        tracing::warn!(wave_id = %wave_id, error = %err, "queue reconcile after merge failed");
    }
    Ok(true)
}

pub fn project_queue_views<F>(
    runs: &[WaveRun],
    mut live_state_for: F,
    blocks: &HashMap<LfdId, QueueBlock>,
) -> HashMap<LfdId, QueueRunView>
where
    F: FnMut(&WaveRun) -> Option<LivePullRequestState>,
{
    let mut live_by_run = HashMap::new();
    for run in runs {
        live_by_run.insert(run.id.clone(), live_state_for(run));
    }

    let head_index = runs.iter().position(|run| {
        let live = live_by_run.get(&run.id).and_then(|value| value.as_ref());
        inferred_stack_status(run.stack_status, live) == WaveRunStackStatus::Active
    });

    let mut result = HashMap::with_capacity(runs.len());
    for (index, run) in runs.iter().enumerate() {
        let block = blocks.get(&run.id);
        let live = live_by_run.get(&run.id).and_then(|value| value.as_ref());
        let role = match inferred_stack_status(run.stack_status, live) {
            WaveRunStackStatus::Merged => QueueRole::Merged,
            WaveRunStackStatus::Superseded => QueueRole::Superseded,
            WaveRunStackStatus::Active => {
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
                block_reason: block.map(|value| value.reason.clone()),
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
        QueueRole::Blocked => {
            let reason = block.map(|value| value.reason.as_str()).unwrap_or_default();
            if reason.contains("scratch") || reason.contains("conflict") {
                QueueNextAction::ResolveConflict
            } else if reason == "missing_pr" {
                QueueNextAction::OpenPr
            } else {
                QueueNextAction::AwaitMerge
            }
        }
    }
}

fn find_queue_head_index(
    runs: &[WaveRun],
    live_states: &HashMap<String, LivePullRequestState>,
) -> Option<usize> {
    runs.iter().position(|run| {
        let live = live_for_run(live_states, run);
        inferred_stack_status(run.stack_status, live.as_ref()) == WaveRunStackStatus::Active
    })
}

fn inferred_stack_status(
    durable: WaveRunStackStatus,
    live: Option<&LivePullRequestState>,
) -> WaveRunStackStatus {
    if durable != WaveRunStackStatus::Active {
        return durable;
    }
    match live.map(|state| state.state) {
        Some(LivePrState::Merged) => WaveRunStackStatus::Merged,
        Some(LivePrState::Closed) => WaveRunStackStatus::Superseded,
        _ => WaveRunStackStatus::Active,
    }
}

fn run_pr_key(run: &WaveRun) -> String {
    let repo = run.snapshot.repo.clone();
    let pr = pr_number(run).unwrap_or_default();
    format!("{repo}:{pr}")
}

fn pr_number(run: &WaveRun) -> Option<u32> {
    run.snapshot.pr.as_ref()?.number
}

fn live_for_run(
    live_states: &HashMap<String, LivePullRequestState>,
    run: &WaveRun,
) -> Option<LivePullRequestState> {
    live_states.get(&run_pr_key(run)).cloned()
}

async fn load_live_states(
    store: &SharedStore,
    runs: &[WaveRun],
) -> HashMap<String, LivePullRequestState> {
    let mut states = HashMap::new();
    for run in runs {
        let Some(pr_number) = pr_number(run) else {
            continue;
        };
        if let Ok(Some(state)) = store.get_live_pr_state(&run.snapshot.repo, pr_number).await {
            states.insert(run_pr_key(run), state);
        }
    }
    states
}

async fn set_queue_block(
    store: &SharedStore,
    run: &WaveRun,
    reason: &str,
    conflict_files: Vec<String>,
    error: Option<String>,
) -> Result<(), String> {
    let block = QueueBlock {
        wave_id: run.wave_id.clone(),
        run_id: run.id.clone(),
        reason: reason.to_string(),
        attempted_at: OffsetDateTime::now_utc(),
        conflict_files,
        error,
    };
    store
        .upsert_queue_block(&block)
        .await
        .map_err(|err| format!("upsert_queue_block failed: {err}"))
}

async fn refresh_live_states(store: &SharedStore, github_config: &GitHubConfig, runs: &[WaveRun]) {
    let token = github_config
        .token
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if token.is_empty() {
        return;
    }

    let mut repo_cache: HashMap<String, Option<String>> = HashMap::new();
    for run in runs {
        let Some(pr_number) = pr_number(run) else {
            continue;
        };
        let repo_path = run.snapshot.repo.clone();
        let repo_name = if let Some(existing) = repo_cache.get(&repo_path) {
            existing.clone()
        } else {
            let repo_path_clone = repo_path.clone();
            let full_name = tokio::task::spawn_blocking(move || {
                github::github_repo_from_local(Path::new(&repo_path_clone))
            })
            .await
            .ok()
            .flatten();
            repo_cache.insert(repo_path.clone(), full_name.clone());
            full_name
        };
        let Some(repo_name) = repo_name else {
            continue;
        };
        let fetched = github::fetch_pull_request(&repo_name, pr_number, &token).await;
        let Ok(Some(pull_request)) = fetched else {
            continue;
        };
        let live_state = LivePullRequestState {
            repo_id: run.snapshot.repo.clone(),
            pr_number: pull_request.number,
            state: pull_request.state,
            is_draft: pull_request.is_draft,
            head_ref: pull_request.head_ref,
            head_sha: pull_request.head_sha,
            base_ref: pull_request.base_ref,
            updated_at: pull_request.updated_at,
            merged_at: pull_request.merged_at,
            synced_at: OffsetDateTime::now_utc(),
        };
        let _ = store.upsert_live_pr_state(&live_state).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        PullRequest, Wave, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStatus, WaveStatus,
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
        let config = crate::lfd::store::StorageConfig::sqlite(db_path);
        Arc::new(crate::lfd::store::open_store(&config).await.expect("sqlite store should initialize"))
    }

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "queue-wave".to_string(),
            repo: repo.to_string(),
            flow: "ship".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }

    fn make_run(wave: &Wave, stack_position: u32, pr_number: u32) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: wave.flow.clone(),
                direction: wave.direction.clone(),
                area: wave.area.clone(),
                pr: Some(PullRequest {
                    url: format!("https://example.test/pr/{pr_number}"),
                    number: Some(pr_number),
                    state: Some("open".to_string()),
                    title: Some(format!("run-{pr_number}")),
                    branch: Some(format!("feature-{pr_number}")),
                }),
            },
            iteration: stack_position,
            step_index: 0,
            status: WaveRunStatus::Completed,
            worktree: ".".to_string(),
            branch: format!("feature-{pr_number}"),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: Some(OffsetDateTime::now_utc()),
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position,
            stack_group_id: wave.id.to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        }
    }

    async fn set_live_open(store: &SharedStore, run: &WaveRun, is_draft: bool) {
        let pr_number = pr_number(run).expect("pr number");
        store
            .upsert_live_pr_state(&LivePullRequestState {
                repo_id: run.snapshot.repo.clone(),
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
        store.create_wave_run(&run1).await.expect("run1");
        store.create_wave_run(&run2).await.expect("run2");
        set_live_open(&store, &run1, true).await;
        set_live_open(&store, &run2, true).await;

        let ops = MockOps {
            scratch_clean: true,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(
            &store,
            &GitHubConfig::default(),
            &wave.id,
            QueueTrigger::RunCompleted,
            &ops,
        )
        .await
        .expect("reconcile");

        let blocks = store
            .list_queue_blocks(&wave.id)
            .await
            .expect("list queue blocks")
            .into_iter()
            .map(|block| (block.run_id.clone(), block))
            .collect::<HashMap<_, _>>();
        let runs = store.list_stack_runs(&wave.id).await.expect("runs");
        let live_states = load_live_states(&store, &runs).await;
        let projected = project_queue_views(
            &runs,
            |run| live_for_run(&live_states, run),
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
    async fn handle_pr_merged_is_idempotent() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run1 = make_run(&wave, 0, 21);
        let run2 = make_run(&wave, 1, 22);
        store.create_wave_run(&run1).await.expect("run1");
        store.create_wave_run(&run2).await.expect("run2");
        set_live_open(&store, &run1, false).await;
        set_live_open(&store, &run2, true).await;

        let merged_at = OffsetDateTime::now_utc();
        let first = handle_pr_merged(&store, &GitHubConfig::default(), &wave.id, 21, merged_at)
            .await
            .expect("first merge");
        let second = handle_pr_merged(&store, &GitHubConfig::default(), &wave.id, 21, merged_at)
            .await
            .expect("second merge");
        assert!(first);
        assert!(!second);
    }

    #[tokio::test]
    async fn scratch_dirty_marks_blocked_with_resolve_conflict_action() {
        let store = sqlite_store().await;
        let wave = make_wave(".");
        store.create_wave(&wave).await.expect("wave");
        let run = make_run(&wave, 0, 31);
        store.create_wave_run(&run).await.expect("run");
        set_live_open(&store, &run, true).await;

        let ops = MockOps {
            scratch_clean: false,
            ..Default::default()
        };
        reconcile_wave_queue_with_ops(
            &store,
            &GitHubConfig::default(),
            &wave.id,
            QueueTrigger::RunCompleted,
            &ops,
        )
        .await
        .expect("reconcile");

        let block = store
            .list_queue_blocks(&wave.id)
            .await
            .expect("blocks")
            .into_iter()
            .next()
            .expect("block");
        assert_eq!(block.reason, "scratch_dirty");

        let live_states = load_live_states(&store, &[run.clone()]).await;
        let projection = project_queue_views(
            &[run.clone()],
            |r| live_for_run(&live_states, r),
            &HashMap::from([(run.id.clone(), block)]),
        );
        assert_eq!(
            projection.get(&run.id).map(|view| view.next_action),
            Some(QueueNextAction::ResolveConflict)
        );
    }
}
