//! Finite pull-request delivery reconciliation over durable landing intents.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::current_branch;
use crate::engine::load_skill;
use crate::pr_landing::{
    LandingPlacement, LandingSupervisor, NewPrLanding, PrLanding, PrLandingState,
    SUPERVISOR_STALE_AFTER,
};
use crate::store::{open_store, storage_config_from_env, SharedStore};
use crate::work::task::{CiCheck, CiIncident, CiObservation, CiState};

use super::error::{OpsError, OpsResult};
use super::land::LandOptions;
use super::pr::{merge_gate_state, observe_pr_by_number, PrInfo, PrObservation, PrReadFreshness};
use super::progress::Progress;

const LANDING_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "status", content = "summary", rename_all = "snake_case")]
enum RepairConclusion {
    Published(String),
    Blocked(String),
}

impl RepairConclusion {
    fn summary(&self) -> &str {
        match self {
            Self::Published(summary) | Self::Blocked(summary) => summary,
        }
    }
}

fn parse_repair_conclusion(answer: &str) -> OpsResult<RepairConclusion> {
    let json = answer.trim();
    let json = json
        .strip_prefix("```json")
        .and_then(|json| json.strip_suffix("```"))
        .unwrap_or(json)
        .trim();
    serde_json::from_str(json).map_err(|error| {
        OpsError::Message(format!(
            "ci-fix returned an unreadable result: {error}\n{answer}"
        ))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LandingObservation {
    Unarmed {
        head_sha: String,
    },
    Pending {
        head_sha: String,
    },
    Passing {
        head_sha: String,
    },
    Failing {
        head_sha: String,
        failing_checks: Vec<CiCheck>,
    },
    Merged {
        head_sha: String,
        merge_commit: String,
    },
    Closed {
        head_sha: String,
    },
    Degraded {
        reason: String,
    },
}

impl LandingObservation {
    fn head_sha(&self) -> Option<&str> {
        match self {
            Self::Unarmed { head_sha }
            | Self::Pending { head_sha }
            | Self::Passing { head_sha }
            | Self::Failing { head_sha, .. }
            | Self::Merged { head_sha, .. }
            | Self::Closed { head_sha } => Some(head_sha),
            Self::Degraded { .. } => None,
        }
    }
}

pub(crate) trait LandingDriver: Send + Sync {
    fn observe(&self, landing: &PrLanding) -> OpsResult<LandingObservation>;
    fn repair(&self, landing: &PrLanding, incident: &CiIncident) -> OpsResult<()>;
}

#[derive(Debug, Clone)]
struct GithubLandingDriver;

impl LandingDriver for GithubLandingDriver {
    fn observe(&self, landing: &PrLanding) -> OpsResult<LandingObservation> {
        match observe_pr_by_number(
            &landing.worktree,
            landing.pr_number,
            &landing.branch,
            PrReadFreshness::Fresh,
        ) {
            PrObservation::Fresh(pr) => {
                if matches!(pr.state.as_str(), "open" | "draft") {
                    match super::pr::auto_merge_enabled(&landing.worktree, pr.number) {
                        Ok(false) => {
                            return Ok(LandingObservation::Unarmed {
                                head_sha: pr
                                    .head_sha
                                    .unwrap_or_else(|| landing.observed_head_sha.clone()),
                            });
                        }
                        Ok(true) => {}
                        Err(error) => {
                            return Ok(LandingObservation::Degraded {
                                reason: error.to_string(),
                            });
                        }
                    }
                }
                classify_github_observation(landing, pr)
            }
            PrObservation::NotFound => Ok(LandingObservation::Degraded {
                reason: format!(
                    "GitHub no longer exposes pull request #{}; merge state is unknown",
                    landing.pr_number
                ),
            }),
            PrObservation::Degraded { reason } => Ok(LandingObservation::Degraded { reason }),
        }
    }

    fn repair(&self, landing: &PrLanding, incident: &CiIncident) -> OpsResult<()> {
        launch_ci_fix(landing, incident)
    }
}

fn classify_github_observation(landing: &PrLanding, pr: PrInfo) -> OpsResult<LandingObservation> {
    let head_sha = pr
        .head_sha
        .unwrap_or_else(|| landing.observed_head_sha.clone());
    match pr.state.as_str() {
        "merged" => {
            let merge_commit = pr.merge_commit.ok_or_else(|| {
                OpsError::Message(format!(
                    "GitHub reports pull request #{} merged without a merge commit",
                    landing.pr_number
                ))
            })?;
            Ok(LandingObservation::Merged {
                head_sha,
                merge_commit,
            })
        }
        "closed" => Ok(LandingObservation::Closed { head_sha }),
        _ => match merge_gate_state(&landing.worktree, &landing.branch) {
            Ok(Some(reading)) if reading.failing => Ok(LandingObservation::Failing {
                head_sha,
                failing_checks: reading
                    .failing_leaves
                    .into_iter()
                    .map(|check| CiCheck {
                        name: check.name,
                        url: check.url,
                    })
                    .collect(),
            }),
            Ok(Some(reading)) if reading.pending => Ok(LandingObservation::Pending { head_sha }),
            Ok(Some(_)) => Ok(LandingObservation::Passing { head_sha }),
            Ok(None) => Ok(LandingObservation::Pending { head_sha }),
            Err(error) => Ok(LandingObservation::Degraded {
                reason: error.to_string(),
            }),
        },
    }
}

fn launch_ci_fix(landing: &PrLanding, incident: &CiIncident) -> OpsResult<()> {
    let skill = load_skill("ci-fix", &landing.worktree)
        .map_err(|error| OpsError::Message(format!("ci-fix skill not found: {error}")))?
        .content
        .ok_or_else(|| OpsError::Message("ci-fix skill has no content".to_string()))?;
    let urls = merge_gate_state(&landing.worktree, &landing.branch)
        .ok()
        .flatten()
        .map(|reading| {
            reading
                .failing_leaves
                .into_iter()
                .filter_map(|check| check.url.map(|url| (check.name, url)))
                .collect::<std::collections::BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let checks = incident
        .failure_set
        .iter()
        .map(|name| match urls.get(name) {
            Some(url) => format!("- {name} ({url})"),
            None => format!("- {name}"),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let task_context = landing
        .task_id
        .as_ref()
        .map(|task_id| format!("\nTask context: {task_id}"))
        .unwrap_or_default();
    let arm_command = repair_arm_command(landing);
    let mut prompt = format!(
        "{skill}\n\nRepair the exact recorded landing incident below. Start with `lf rebase`. Repair and verify, then run `{arm_command}` to publish and enable auto-merge with the requested Task disposition. Do not invoke `lf pr land` or wait for merge; a later finite check observes the result and completes after merge.\n\nRepository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
        incident.repo,
        incident.pr_number,
        landing.branch,
        incident.failed_head_sha,
        checks,
        task_context,
    );
    prompt.push_str(
        "\n\nReturn your final answer as one JSON object: {\"status\":\"published\",\"summary\":\"Published head, PR URL, auto-merge state, and checks run\"}. If an external capability or human decision prevents repair, return {\"status\":\"blocked\",\"summary\":\"Exact blocker and next action\"}. Continue resolving repairable failures yourself. Do not mark an unpublished repair as published. This result belongs only in your final answer; create no result file.",
    );
    let config = load_config_or_default(Some(&landing.worktree));
    let mut launch = AgentConfig {
        task_prompt: prompt,
        agent: Some(config.agent().to_string()),
        cwd: Some(landing.worktree.clone()),
        skip_permissions: true,
        ..Default::default()
    };
    crate::engine::agent::pin_provider_account_id_blocking(&mut launch)?;
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };
    let (harness, model) = crate::engine::parse_agent(launch.agent());
    let capture = crate::run_record::CaptureHandle::begin_with_launch(
        crate::run_record::RunSpec {
            harness,
            model,
            surface: "headless".to_string(),
            cwd: landing.worktree.clone(),
            repo: Some(landing.worktree.clone()),
            worktree: Some(landing.worktree.clone()),
            skill: Some("ci-fix".to_string()),
            subjects: Vec::new(),
            flow: crate::run_record::RunFlowMembership::Independent,
            work: landing.task_id.clone().map(|task| crate::session::RunWork {
                task_id: Some(task),
                wave_id: None,
                source: crate::session::WorkSource::Declared,
            }),
        },
        crate::run_record::RunLaunchRequest::from_prepared(&launch, &capabilities),
    )
    .map_err(|error| OpsError::Message(error.to_string()))?;
    capture.record_input("initial", &launch.task_prompt);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        capture: Some(capture.clone().into()),
        ..Default::default()
    };
    let result = launch_agent(&launch, &process, &capabilities);
    let outcome = if matches!(&result, Ok(result) if result.exit_code == 0) {
        "completed"
    } else {
        "failed"
    };
    capture
        .finish(outcome)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let conclusion = crate::run_record::read_final_answer(&capture.artifact_dir())
        .ok()
        .flatten()
        .map(|answer| answer.text)
        .unwrap_or_else(|| {
            format!(
                "No repair conclusion recorded; inspect lf runs {} --events",
                capture.run_id()
            )
        });
    let result = result.map_err(|error| {
        OpsError::Message(format!("ci-fix provider failed: {error}\n{conclusion}"))
    })?;
    if result.exit_code != 0 {
        return Err(OpsError::Message(format!(
            "ci-fix provider exited {}: {}\n{conclusion}",
            result.exit_code,
            result.stderr.trim()
        )));
    }
    let conclusion = parse_repair_conclusion(&conclusion)?;
    eprintln!("ci-fix: {}", conclusion.summary());
    match conclusion {
        RepairConclusion::Published(_) => Ok(()),
        RepairConclusion::Blocked(reason) => Err(OpsError::Message(reason)),
    }
}

fn repair_arm_command(landing: &PrLanding) -> String {
    if landing.after_merge == Some(crate::work::task::AfterMerge::CompleteTask) {
        "lf pr arm -c".to_string()
    } else if let Some(slug) = &landing.next_slug {
        format!(
            "lf pr arm --next {}",
            crate::engine::process::shell_escape(slug)
        )
    } else {
        "lf pr arm".to_string()
    }
}

fn ci_incident(landing: &PrLanding, checks: &[CiCheck], now: OffsetDateTime) -> CiIncident {
    let mut failure_set = checks
        .iter()
        .map(|check| check.name.clone())
        .collect::<Vec<_>>();
    failure_set.sort();
    failure_set.dedup();
    let mut digest = Sha256::new();
    for check in &failure_set {
        digest.update(check.as_bytes());
        digest.update([0]);
    }
    CiIncident {
        identity: format!(
            "github:ci:{}:{}:{}:{}",
            landing.repo,
            landing.pr_number,
            landing.observed_head_sha,
            hex::encode(digest.finalize())
        ),
        landing_id: Some(landing.id.clone()),
        task_id: landing.task_id.clone(),
        pr_id: None,
        repo: landing.repo.clone(),
        pr_number: landing.pr_number,
        failed_head_sha: landing.observed_head_sha.clone(),
        repaired_head_sha: None,
        failure_set,
        provider_completed_at: None,
        poll_observed_at: Some(now),
        webhook_received_at: None,
        claimed_landing_generation: None,
        responded_at: None,
        green_at: None,
        merged_at: None,
        blocked_at: None,
        blocked_reason: None,
        created_at: now,
        updated_at: now,
    }
}

fn actionable_failure(head_sha: &str, failing_checks: &[CiCheck], now: OffsetDateTime) -> bool {
    CiObservation {
        head_sha: head_sha.to_string(),
        state: CiState::Failing,
        failing_checks: failing_checks.to_vec(),
        observed_at: now,
    }
    .repair_legal()
}

async fn persist_landing_state(
    store: &SharedStore,
    landing: &mut PrLanding,
    state: PrLandingState,
    head_sha: String,
    merge_commit: Option<String>,
    blocked_reason: Option<String>,
) -> OpsResult<()> {
    let now = OffsetDateTime::now_utc();
    let mut candidate = landing.clone();
    candidate.state = state;
    candidate.observed_head_sha = head_sha;
    candidate.merge_commit = merge_commit;
    candidate.blocked_reason = blocked_reason;
    candidate.updated_at = now;
    let persisted = store
        .update_pr_landing(&candidate)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if !persisted {
        return Err(OpsError::Message(format!(
            "landing {} generation {} lost supervision authority",
            landing.id, landing.generation
        )));
    }
    *landing = candidate;
    Ok(())
}

async fn block_landing(
    store: &SharedStore,
    landing: &mut PrLanding,
    reason: String,
) -> OpsResult<()> {
    let now = OffsetDateTime::now_utc();
    store
        .mark_ci_incidents_blocked(&landing.id, landing.generation, now, &reason)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    persist_landing_state(
        store,
        landing,
        PrLandingState::Blocked,
        landing.observed_head_sha.clone(),
        landing.merge_commit.clone(),
        Some(reason.clone()),
    )
    .await?;
    Err(OpsError::Message(reason))
}

async fn run_driver_operation<T, F>(
    store: &SharedStore,
    landing: &PrLanding,
    ownership: &Arc<File>,
    label: &'static str,
    operation: F,
) -> OpsResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> OpsResult<T> + Send + 'static,
{
    // A blocking repair outlives cancellation of its async waiter. Keep the
    // supervisor's lock until the operation itself has returned.
    let ownership = Arc::clone(ownership);
    let mut operation = tokio::task::spawn_blocking(move || {
        let _ownership = ownership;
        operation()
    });
    loop {
        tokio::select! {
            result = &mut operation => {
                return result.map_err(|error| {
                    OpsError::Message(format!("landing {label} panicked: {error}"))
                })?;
            }
            () = tokio::time::sleep(LANDING_HEARTBEAT_INTERVAL) => {
                let now = OffsetDateTime::now_utc();
                if !store
                    .heartbeat_pr_landing(&landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?
                {
                    return Err(OpsError::Message(format!(
                        "landing {} generation {} lost supervision authority during {label}",
                        landing.id, landing.generation
                    )));
                }
            }
        }
    }
}

fn lock_landing(landing: &PrLanding) -> OpsResult<Option<Arc<File>>> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(
            crate::engine::git::absolute_git_dir(&landing.worktree)?.join("lf-pr-landing.lock"),
        )?;
    match FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(Some(Arc::new(lock))),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Inspect once, optionally confirm a repair, and return. The OS supplies the next wake.
pub(crate) async fn reconcile_pr_landing(
    store: SharedStore,
    landing: PrLanding,
    driver: Arc<dyn LandingDriver>,
) -> OpsResult<PrLanding> {
    // Acquire the OS lock before changing the lease. A canceled async repair
    // retains this lock in its blocking worker until that worker really exits.
    let Some(ownership) = lock_landing(&landing)? else {
        return Ok(landing);
    };
    let now = OffsetDateTime::now_utc();
    let claim = LandingSupervisor {
        placement: LandingPlacement::Local,
        process_id: std::process::id(),
        heartbeat_at: now,
    };
    let Some(mut landing) = store
        .claim_pr_landing(
            &landing.id,
            landing.generation,
            &claim,
            now - SUPERVISOR_STALE_AFTER,
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
    else {
        return store
            .get_pr_landing(&landing.id)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message("landing disappeared".into()));
    };
    let result = reconcile_claimed(&store, &mut landing, driver, &ownership).await;
    store
        .release_pr_landing(&landing)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    result?;
    store
        .get_pr_landing(&landing.id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("landing disappeared".into()))
}

async fn reconcile_claimed(
    store: &SharedStore,
    landing: &mut PrLanding,
    driver: Arc<dyn LandingDriver>,
    ownership: &Arc<File>,
) -> OpsResult<()> {
    refresh_joined_request(store, landing).await?;
    let mut observed = if let Some(merge_commit) = &landing.merge_commit {
        LandingObservation::Merged {
            head_sha: landing.observed_head_sha.clone(),
            merge_commit: merge_commit.clone(),
        }
    } else {
        run_driver_operation(store, landing, ownership, "observation", {
            let driver = Arc::clone(&driver);
            let landing = landing.clone();
            move || driver.observe(&landing)
        })
        .await?
    };
    record_observed_head(store, landing, &observed).await?;
    if let LandingObservation::Failing {
        head_sha,
        failing_checks,
    } = &observed
    {
        let now = OffsetDateTime::now_utc();
        if !actionable_failure(head_sha, failing_checks, now) {
            return Ok(());
        }
        let incident = ci_incident(landing, failing_checks, now);
        store
            .observe_ci_incident(&incident)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        observed = run_driver_operation(store, landing, ownership, "CI confirmation", {
            let driver = Arc::clone(&driver);
            let landing = landing.clone();
            move || driver.observe(&landing)
        })
        .await?;
        // Changed failures wait for another check; non-failure evidence can
        // settle immediately without a repair.
        if let LandingObservation::Failing {
            head_sha,
            failing_checks,
        } = &observed
        {
            if head_sha != &incident.failed_head_sha
                || ci_incident(landing, failing_checks, now).identity != incident.identity
            {
                return Ok(());
            }
        }
        record_observed_head(store, landing, &observed).await?;
    }
    let now = OffsetDateTime::now_utc();
    match observed {
        LandingObservation::Merged {
            head_sha,
            merge_commit,
        } => {
            refresh_joined_request(store, landing).await?;
            persist_landing_state(
                store,
                landing,
                PrLandingState::Watching,
                head_sha.clone(),
                Some(merge_commit.clone()),
                None,
            )
            .await?;
            store
                .mark_ci_incidents_merged(&landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
            if landing.task_id.is_some() {
                if let Err(error) = super::task::settle_task_landing(store, landing).await {
                    return block_landing(
                        store,
                        landing,
                        format!("pull request merged but Task settlement is pending: {error}"),
                    )
                    .await;
                }
            }
            persist_landing_state(
                store,
                landing,
                PrLandingState::Merged,
                head_sha,
                Some(merge_commit),
                None,
            )
            .await
        }
        LandingObservation::Closed { head_sha } => {
            persist_landing_state(store, landing, PrLandingState::Closed, head_sha, None, None)
                .await?;
            Err(OpsError::Message(
                "pull request closed without merging".into(),
            ))
        }
        LandingObservation::Passing { .. } => {
            store
                .mark_ci_incidents_green(&landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
            Ok(())
        }
        LandingObservation::Unarmed { .. } => {
            block_landing(
                store,
                landing,
                "pull request has no auto-merge request; run lf pr arm to resume landing".into(),
            )
            .await
        }
        LandingObservation::Pending { .. } => Ok(()),
        LandingObservation::Degraded { reason } => {
            // Failure to read is never evidence of CI failure or merge.
            Err(OpsError::Message(reason))
        }
        LandingObservation::Failing {
            head_sha,
            failing_checks,
        } => {
            if !actionable_failure(&head_sha, &failing_checks, now) {
                return Ok(());
            }
            let incident = ci_incident(landing, &failing_checks, now);
            if !store
                .record_ci_response(&incident.identity, &landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
            {
                return block_landing(
                    store,
                    landing,
                    landing.blocked_reason.clone().unwrap_or_else(|| {
                        "CI incident already received a repair; waiting for changed evidence".into()
                    }),
                )
                .await;
            }
            persist_landing_state(
                store,
                landing,
                PrLandingState::Repairing,
                head_sha.clone(),
                None,
                None,
            )
            .await?;
            let repair = run_driver_operation(store, landing, ownership, "ci-fix", {
                let driver = Arc::clone(&driver);
                let landing = landing.clone();
                move || driver.repair(&landing, &incident)
            })
            .await;
            if let Err(error) = repair {
                return block_landing(store, landing, format!("ci-fix blocked: {error}")).await;
            }
            persist_landing_state(
                store,
                landing,
                PrLandingState::Watching,
                head_sha,
                None,
                None,
            )
            .await
        }
    }
}

async fn record_observed_head(
    store: &SharedStore,
    landing: &mut PrLanding,
    observed: &LandingObservation,
) -> OpsResult<()> {
    if let Some(head) = observed.head_sha() {
        if head != landing.observed_head_sha {
            persist_landing_state(
                store,
                landing,
                PrLandingState::Watching,
                head.to_string(),
                None,
                None,
            )
            .await?;
        }
    }
    Ok(())
}

async fn refresh_joined_request(store: &SharedStore, landing: &mut PrLanding) -> OpsResult<()> {
    let current = store
        .get_pr_landing(&landing.id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message(format!("landing {} disappeared", landing.id)))?;
    if current.generation != landing.generation
        || matches!(
            current.state,
            PrLandingState::Merged | PrLandingState::Closed
        )
    {
        return Err(OpsError::Message(format!(
            "landing {} generation {} is no longer active",
            landing.id, landing.generation
        )));
    }
    landing.requested_head_sha = current.requested_head_sha;
    landing.after_merge = current.after_merge;
    landing.next_slug = current.next_slug;
    Ok(())
}

async fn landing_store() -> OpsResult<SharedStore> {
    let config = storage_config_from_env()
        .map_err(|error| OpsError::Message(format!("resolve landing store: {error}")))?;
    open_store(&config)
        .await
        .map(Arc::new)
        .map_err(|error| OpsError::Message(format!("open landing store: {error}")))
}

async fn create_landing(
    store: &SharedStore,
    repo: &Path,
    options: &LandOptions,
    pr: &PrInfo,
) -> OpsResult<PrLanding> {
    let worktree = options
        .worktree
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.to_path_buf());
    let worktree = std::fs::canonicalize(&worktree).map_err(|error| {
        OpsError::Message(format!(
            "resolve landing worktree {}: {error}",
            worktree.display()
        ))
    })?;
    let branch = current_branch(&worktree)
        .map_err(OpsError::from)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let requested_head_sha = pr.head_sha.clone().ok_or_else(|| {
        OpsError::Message(format!(
            "GitHub omitted the armed head for pull request #{}",
            pr.number
        ))
    })?;
    let repo_id = crate::repository::RepoId::discover(&worktree)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let task = crate::ops::task::task_for_checkout(store, &worktree).await?;
    let (task_id, after_merge, next_slug) = match task {
        Some(task) => {
            let task_pr = store
                .active_task_pr(&task.id)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
                .ok_or_else(|| OpsError::Message("Task has no active PR after arm".to_string()))?;
            let request = task_pr.merge_request().ok_or_else(|| {
                OpsError::Message("Task PR has no exact-head merge request after arm".to_string())
            })?;
            if task_pr.github().map(|github| github.number) != Some(pr.number as u32)
                || request.head_sha != requested_head_sha
            {
                return Err(OpsError::Message(
                    "Task merge request does not match the armed GitHub PR head".to_string(),
                ));
            }
            (
                Some(task.id),
                Some(request.after_merge),
                request.next_slug.clone(),
            )
        }
        None => (None, None, None),
    };
    PrLanding::new(
        NewPrLanding {
            repo: repo_id.as_str().to_string(),
            pr_number: u32::try_from(pr.number).map_err(|_| {
                OpsError::Message(format!(
                    "pull request #{} exceeds supported range",
                    pr.number
                ))
            })?,
            worktree,
            branch,
            task_id,
            requested_head_sha,
            after_merge,
            next_slug,
        },
        OffsetDateTime::now_utc(),
    )
    .map_err(|error| OpsError::Message(error.to_string()))
}

pub(crate) fn record_armed_pr(
    repo: &Path,
    options: &LandOptions,
    pr: &PrInfo,
) -> OpsResult<PrLanding> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = landing_store().await?;
        let landing = create_landing(&store, repo, options, pr).await?;
        store
            .start_or_join_pr_landing(&landing)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))
    })
}

pub fn reconcile_repository(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    let repo = crate::repository::RepoId::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = landing_store().await?;
        let landings = store
            .pending_pr_landings(repo.as_str())
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        let mut errors = Vec::new();
        for landing in landings {
            let number = landing.pr_number;
            match reconcile_pr_landing(store.clone(), landing, Arc::new(GithubLandingDriver)).await
            {
                Ok(landing) => {
                    let detail = landing
                        .blocked_reason
                        .as_deref()
                        .unwrap_or(landing.state.as_str());
                    progress.status(&format!("PR #{number}: {detail}"));
                }
                Err(error) => errors.push(format!("PR #{number}: {error}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(OpsError::Message(errors.join("\n")))
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    use super::{reconcile_pr_landing, LandingDriver, LandingObservation};
    use crate::ops::error::{OpsError, OpsResult};
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::task::{CiCheck, CiIncident};

    struct FakeDriver {
        observations: Mutex<VecDeque<LandingObservation>>,
        repairs: Mutex<u32>,
        repair_error: Option<String>,
    }

    impl LandingDriver for FakeDriver {
        fn observe(&self, _: &PrLanding) -> OpsResult<LandingObservation> {
            self.observations
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| OpsError::Message("observation exhausted".into()))
        }
        fn repair(&self, _: &PrLanding, _: &CiIncident) -> OpsResult<()> {
            *self.repairs.lock().unwrap() += 1;
            match &self.repair_error {
                Some(error) => Err(OpsError::Message(error.clone())),
                None => Ok(()),
            }
        }
    }

    fn driver(observations: Vec<LandingObservation>) -> Arc<FakeDriver> {
        Arc::new(FakeDriver {
            observations: Mutex::new(observations.into()),
            repairs: Mutex::new(0),
            repair_error: None,
        })
    }

    async fn fixture() -> (tempfile::TempDir, SharedStore, PrLanding) {
        let directory = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success());
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".into(),
                pr_number: 248,
                worktree: directory.path().into(),
                branch: "feature".into(),
                task_id: None,
                requested_head_sha: "head".into(),
                after_merge: None,
                next_slug: None,
            },
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&landing).await.unwrap();
        (directory, store, landing)
    }

    fn failure(head: &str) -> LandingObservation {
        LandingObservation::Failing {
            head_sha: head.into(),
            failing_checks: vec![CiCheck {
                name: "rust".into(),
                url: None,
            }],
        }
    }
    fn merged() -> LandingObservation {
        LandingObservation::Merged {
            head_sha: "head".into(),
            merge_commit: "merge".into(),
        }
    }

    #[tokio::test]
    async fn repair_retains_completion_and_rotation_intent() {
        let (_directory, _store, mut landing) = fixture().await;
        landing.after_merge = Some(crate::work::task::AfterMerge::CompleteTask);
        assert_eq!(super::repair_arm_command(&landing), "lf pr arm -c");
        landing.after_merge = Some(crate::work::task::AfterMerge::ContinueTask);
        landing.next_slug = Some("follow-up".into());
        assert_eq!(
            super::repair_arm_command(&landing),
            "lf pr arm --next 'follow-up'"
        );
        landing.next_slug = None;
        assert_eq!(super::repair_arm_command(&landing), "lf pr arm");
    }

    #[tokio::test]
    async fn revoked_merge_intent_blocks_without_launching_a_repair() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![LandingObservation::Unarmed {
            head_sha: "head".into(),
        }]);
        assert!(
            reconcile_pr_landing(store.clone(), landing.clone(), driver.clone())
                .await
                .is_err()
        );
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn finite_checks_return_pending_then_finish_only_after_merge() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![
            LandingObservation::Pending {
                head_sha: "head".into(),
            },
            merged(),
        ]);
        let pending = reconcile_pr_landing(store.clone(), landing, driver.clone())
            .await
            .unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        assert!(pending.supervisor.is_none());
        assert!(pending.merge_commit.is_none());
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
        let landed = reconcile_pr_landing(store.clone(), pending, driver)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(landed.merge_commit.as_deref(), Some("merge"));
        assert!(store
            .pending_pr_landings("loopflowstudio/loopflow")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn unchanged_incident_gets_one_repair_across_ticks_and_still_observes_merge() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![
            failure("head"),
            failure("head"),
            failure("head"),
            failure("head"),
            merged(),
        ]);
        let pending = reconcile_pr_landing(store.clone(), landing, driver.clone())
            .await
            .unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        assert!(
            reconcile_pr_landing(store.clone(), pending.clone(), driver.clone())
                .await
                .is_err()
        );
        let blocked = store.get_pr_landing(&pending.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        assert_eq!(*driver.repairs.lock().unwrap(), 1);
        let merged = reconcile_pr_landing(store.clone(), blocked, driver)
            .await
            .unwrap();
        assert_eq!(merged.state, PrLandingState::Merged);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].incident.responded_at.is_some());
    }

    #[tokio::test]
    async fn changed_confirmation_never_repairs_the_stale_head() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![failure("head"), failure("other")]);
        let pending = reconcile_pr_landing(store, landing, driver.clone())
            .await
            .unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn confirmation_can_settle_a_merge_without_repair() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![failure("head"), merged()]);
        let landed = reconcile_pr_landing(store.clone(), landing, driver.clone())
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(landed.merge_commit.as_deref(), Some("merge"));
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert!(incidents[0].incident.responded_at.is_none());
        assert!(incidents[0].incident.merged_at.is_some());
    }

    #[tokio::test]
    async fn failed_repair_does_not_hide_a_later_merge() {
        let (_directory, store, landing) = fixture().await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(vec![failure("head"), failure("head"), merged()].into()),
            repairs: Mutex::new(0),
            repair_error: Some("provider exited after publish".into()),
        });
        assert!(
            reconcile_pr_landing(store.clone(), landing.clone(), driver.clone())
                .await
                .is_err()
        );
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert!(blocked
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("provider exited"));
        let landed = reconcile_pr_landing(store, blocked, driver).await.unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
    }

    #[tokio::test]
    async fn unavailable_observation_keeps_delivery_pending_and_releases_claim() {
        let (_directory, store, landing) = fixture().await;
        let driver = driver(vec![LandingObservation::Degraded {
            reason: "network failure".into(),
        }]);
        assert!(
            reconcile_pr_landing(store.clone(), landing.clone(), driver.clone())
                .await
                .is_err()
        );
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(saved.state, PrLandingState::Watching);
        assert!(saved.supervisor.is_none());
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn closed_unmerged_never_counts_as_landed() {
        let (_directory, store, landing) = fixture().await;
        assert!(reconcile_pr_landing(
            store.clone(),
            landing.clone(),
            driver(vec![LandingObservation::Closed {
                head_sha: "head".into()
            }])
        )
        .await
        .is_err());
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(saved.state, PrLandingState::Closed);
        assert!(saved.merge_commit.is_none());
    }

    #[tokio::test]
    async fn overlapping_check_returns_while_canceled_repair_retains_its_lock() {
        struct HeldRepair {
            entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
            release: Mutex<std::sync::mpsc::Receiver<()>>,
        }
        impl LandingDriver for HeldRepair {
            fn observe(&self, _: &PrLanding) -> OpsResult<LandingObservation> {
                Ok(failure("head"))
            }
            fn repair(&self, _: &PrLanding, _: &CiIncident) -> OpsResult<()> {
                self.entered
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(())
                    .unwrap();
                self.release.lock().unwrap().recv().unwrap();
                Ok(())
            }
        }
        let (_directory, store, landing) = fixture().await;
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let first = tokio::spawn(reconcile_pr_landing(
            store.clone(),
            landing.clone(),
            Arc::new(HeldRepair {
                entered: Mutex::new(Some(entered)),
                release: Mutex::new(held),
            }),
        ));
        started.await.unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        let competitor = driver(vec![merged()]);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reconcile_pr_landing(store.clone(), landing.clone(), competitor.clone()),
        )
        .await;
        release.send(()).unwrap();
        assert!(result.unwrap().is_ok());
        assert_eq!(*competitor.repairs.lock().unwrap(), 0);
        assert_eq!(competitor.observations.lock().unwrap().len(), 1);
        assert_eq!(
            store
                .get_pr_landing(&landing.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            PrLandingState::Repairing
        );
    }
}
