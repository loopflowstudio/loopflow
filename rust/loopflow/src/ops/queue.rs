//! One merge-queue reconcile pass, verb-shaped: `lf op queue reconcile`.
//!
//! Runs the same pass the daemon's 60s poller runs (stack-status inference,
//! draft/ready flips, lazy head rebase, queue-block attention writes),
//! sourced from lfdb + gh, then exits. The doorman's poll loop and PR-merged
//! webhook will exec this verb once the collapse lands (wave 3).
//!
//! Queue blocks are attention rows in the ledger — the durable record of the
//! fact. There is no live push; a client reads them by querying attention
//! (`lf status`), the same as every other durable fact in the wave model.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use secrecy::SecretString;

use crate::lfd::config::GitHubConfig;
use crate::lfd::id::LfdId;
use crate::lfd::queue::{
    acquire_reconcile_lock, reconcile_wave_queue_with_ops, QueueOps, RealQueueOps,
};
use crate::lfd::types::Wave;
use crate::lfdb::{open_store, SharedStore};

#[derive(Debug)]
pub struct WaveQueueOutcome {
    pub wave: String,
    pub wave_id: LfdId,
    pub result: Result<(), String>,
}

/// CLI entry: open the local registry, reconcile one wave (by name) or every
/// wave with queue state, print one line per wave.
pub fn reconcile_queue_cmd(wave: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    rt.block_on(async {
        let cfg = crate::lfd::storage_config_from_env()
            .context("failed to resolve local run registry")?;
        let store: SharedStore = Arc::new(
            open_store(&cfg)
                .await
                .map_err(|err| anyhow!("failed to open local run registry: {err}"))?,
        );
        let github = github_config_for_verb(&store).await;
        let outcomes = reconcile_wave_queues(&store, &github, wave)
            .await
            .map_err(|err| anyhow!(err))?;

        if outcomes.is_empty() {
            println!("no waves with queue state");
            return Ok(());
        }

        let mut failures = 0;
        for outcome in &outcomes {
            match &outcome.result {
                Ok(()) => {
                    let block_note = store
                        .list_queue_blocks(&outcome.wave_id)
                        .await
                        .ok()
                        .filter(|blocks| !blocks.is_empty())
                        .map(|blocks| {
                            blocks
                                .iter()
                                .map(|block| block.reason.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                    match block_note {
                        Some(reasons) => println!("{}: blocked ({reasons})", outcome.wave),
                        None => println!("{}: reconciled", outcome.wave),
                    }
                }
                Err(err) => {
                    failures += 1;
                    eprintln!("{}: {err}", outcome.wave);
                }
            }
        }
        if failures > 0 {
            return Err(anyhow!("{failures} wave(s) failed to reconcile"));
        }
        Ok(())
    })
}

/// One reconcile pass. `wave` narrows to a single wave by name; `None` covers
/// every wave that has stack runs (queue state).
pub async fn reconcile_wave_queues(
    store: &SharedStore,
    github: &GitHubConfig,
    wave: Option<&str>,
) -> Result<Vec<WaveQueueOutcome>, String> {
    reconcile_wave_queues_with_ops(store, github, wave, &RealQueueOps).await
}

async fn reconcile_wave_queues_with_ops(
    store: &SharedStore,
    github: &GitHubConfig,
    wave: Option<&str>,
    ops: &dyn QueueOps,
) -> Result<Vec<WaveQueueOutcome>, String> {
    let waves: Vec<Wave> = match wave {
        Some(name) => {
            let wave = store
                .get_wave_by_name(name)
                .await
                .map_err(|err| format!("get_wave_by_name failed: {err}"))?
                .ok_or_else(|| format!("wave not found: {name}"))?;
            vec![wave]
        }
        None => store
            .list_waves(None)
            .await
            .map_err(|err| format!("list_waves failed: {err}"))?,
    };

    let mut outcomes = Vec::new();
    for wave_row in waves {
        let runs = store
            .list_stack_runs(wave_row.id())
            .await
            .map_err(|err| format!("list_stack_runs failed: {err}"))?;
        if runs.is_empty() && wave.is_none() {
            continue;
        }
        let _guard = acquire_reconcile_lock(wave_row.id()).await;
        let result = reconcile_wave_queue_with_ops(store, github, wave_row.id(), ops).await;
        outcomes.push(WaveQueueOutcome {
            wave: wave_row.name().clone(),
            wave_id: wave_row.id().clone(),
            result,
        });
    }
    Ok(outcomes)
}

/// The daemon reads its GitHub token from config/`LFD_GITHUB_TOKEN`; the verb
/// honors the same env var, then falls back to the stored github provider
/// token so a plain shell works. With no token, live-PR lookups fall back to
/// the last synced state in the store.
async fn github_config_for_verb(store: &SharedStore) -> GitHubConfig {
    let env_token = std::env::var("LFD_GITHUB_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let token = match env_token {
        Some(token) => Some(token),
        None => store
            .get_provider_token("github")
            .await
            .ok()
            .flatten()
            .map(|token| token.access_token),
    };
    GitHubConfig {
        webhook_secret: String::new(),
        token: token.map(SecretString::new),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use time::OffsetDateTime;

    use crate::lfd::config::GitHubConfig;
    use crate::lfd::id::LfdId;
    use crate::lfd::queue::{QueueOps, QueueRebaseConflict};
    use crate::lfd::types::{
        LivePrState, LivePullRequestState, PullRequest, QueueBlockReason, Run, RunStackStatus,
        RunStatus, Wave, WaveStatus, DEFAULT_WAVE_FLOW,
    };
    use crate::lfdb::SharedStore;

    use super::{reconcile_wave_queues_with_ops, WaveQueueOutcome};

    #[derive(Debug, Default)]
    struct MockOps {
        scratch_clean: bool,
    }

    impl QueueOps for MockOps {
        fn ensure_branch_checked_out(&self, _worktree: &Path, _branch: &str) -> Result<(), String> {
            Ok(())
        }

        fn mark_ready(&self, _worktree: &Path, _pr_number: u32) -> Result<(), String> {
            Ok(())
        }

        fn mark_draft(&self, _worktree: &Path, _pr_number: u32) -> Result<(), String> {
            Ok(())
        }

        fn rebase_onto_default(
            &self,
            _worktree: &Path,
            _default_branch: &str,
            _parent_landed: bool,
        ) -> Result<(), QueueRebaseConflict> {
            Ok(())
        }

        fn scratch_clean(&self, _worktree: &Path) -> Result<bool, String> {
            Ok(self.scratch_clean)
        }
    }

    async fn sqlite_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lf-queue-verb-test-{}.db", LfdId::new()));
        let config = crate::lfdb::StorageConfig::sqlite(db_path);
        std::sync::Arc::new(
            crate::lfdb::open_store(&config)
                .await
                .expect("sqlite store should initialize"),
        )
    }

    fn make_wave(name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: ".".to_string(),
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

    async fn set_live_state(store: &SharedStore, run: &Run, state: LivePrState, is_draft: bool) {
        let pr_number = run.pr.as_ref().and_then(|pr| pr.number).expect("pr number");
        store
            .upsert_live_pr_state(&LivePullRequestState {
                repo_id: run.repo.clone(),
                pr_number,
                state,
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

    fn assert_ok(outcomes: &[WaveQueueOutcome]) {
        for outcome in outcomes {
            assert!(
                outcome.result.is_ok(),
                "wave {} failed: {:?}",
                outcome.wave,
                outcome.result
            );
        }
    }

    #[tokio::test]
    async fn verb_promotes_head_and_infers_stack_status() {
        let store = sqlite_store().await;
        let wave = make_wave("queue-wave");
        store.create_wave(&wave).await.expect("wave");
        let merged = make_run(&wave, 0, 11);
        let head = make_run(&wave, 1, 12);
        store.create_run(&merged).await.expect("merged run");
        store.create_run(&head).await.expect("head run");
        set_live_state(&store, &merged, LivePrState::Merged, false).await;
        set_live_state(&store, &head, LivePrState::Open, true).await;

        let ops = MockOps {
            scratch_clean: true,
        };
        let outcomes = reconcile_wave_queues_with_ops(&store, &GitHubConfig::default(), None, &ops)
            .await
            .expect("reconcile");
        assert_eq!(outcomes.len(), 1);
        assert_ok(&outcomes);

        // Stack-status inference: the merged live PR flips the durable status.
        let runs = store.list_stack_runs(wave.id()).await.expect("runs");
        let statuses: HashMap<_, _> = runs
            .iter()
            .map(|run| (run.id.clone(), run.stack_status))
            .collect();
        assert_eq!(statuses.get(&merged.id), Some(&RunStackStatus::Merged));
        assert_eq!(statuses.get(&head.id), Some(&RunStackStatus::Active));

        // The head was promoted (ready), so its live state is no longer draft
        // and no queue block was written.
        let head_live = store
            .get_live_pr_state(&head.repo, 12)
            .await
            .expect("live state")
            .expect("head live state");
        assert!(!head_live.is_draft);
        assert!(store
            .list_queue_blocks(wave.id())
            .await
            .expect("blocks")
            .is_empty());
    }

    #[tokio::test]
    async fn verb_writes_queue_block_when_scratch_dirty() {
        let store = sqlite_store().await;
        let wave = make_wave("dirty-wave");
        store.create_wave(&wave).await.expect("wave");
        let run = make_run(&wave, 0, 31);
        store.create_run(&run).await.expect("run");
        set_live_state(&store, &run, LivePrState::Open, true).await;

        let ops = MockOps {
            scratch_clean: false,
        };
        let outcomes = reconcile_wave_queues_with_ops(&store, &GitHubConfig::default(), None, &ops)
            .await
            .expect("reconcile");
        assert_ok(&outcomes);

        let block = store
            .list_queue_blocks(wave.id())
            .await
            .expect("blocks")
            .into_iter()
            .next()
            .expect("block");
        assert_eq!(block.reason, QueueBlockReason::ScratchDirty);
        assert_eq!(block.run_id, run.id);
    }

    #[tokio::test]
    async fn verb_targets_named_wave_only() {
        let store = sqlite_store().await;
        let target = make_wave("target-wave");
        let other = make_wave("other-wave");
        store.create_wave(&target).await.expect("target wave");
        store.create_wave(&other).await.expect("other wave");
        let target_run = make_run(&target, 0, 41);
        let other_run = make_run(&other, 0, 42);
        store.create_run(&target_run).await.expect("target run");
        store.create_run(&other_run).await.expect("other run");
        set_live_state(&store, &target_run, LivePrState::Merged, false).await;
        set_live_state(&store, &other_run, LivePrState::Merged, false).await;

        let ops = MockOps {
            scratch_clean: true,
        };
        let outcomes = reconcile_wave_queues_with_ops(
            &store,
            &GitHubConfig::default(),
            Some("target-wave"),
            &ops,
        )
        .await
        .expect("reconcile");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].wave, "target-wave");

        let target_runs = store.list_stack_runs(target.id()).await.expect("runs");
        assert_eq!(target_runs[0].stack_status, RunStackStatus::Merged);
        // The other wave was not touched: its durable status is still Active.
        let other_runs = store.list_stack_runs(other.id()).await.expect("runs");
        assert_eq!(other_runs[0].stack_status, RunStackStatus::Active);
    }

    #[tokio::test]
    async fn verb_skips_waves_without_queue_state() {
        let store = sqlite_store().await;
        let wave = make_wave("idle-wave");
        store.create_wave(&wave).await.expect("wave");

        let ops = MockOps {
            scratch_clean: true,
        };
        let outcomes = reconcile_wave_queues_with_ops(&store, &GitHubConfig::default(), None, &ops)
            .await
            .expect("reconcile");
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn verb_rejects_unknown_wave_name() {
        let store = sqlite_store().await;
        let ops = MockOps {
            scratch_clean: true,
        };
        let error =
            reconcile_wave_queues_with_ops(&store, &GitHubConfig::default(), Some("ghost"), &ops)
                .await
                .expect_err("unknown wave should fail");
        assert!(error.contains("wave not found"));
    }
}
