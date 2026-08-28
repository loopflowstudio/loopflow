//! One watched PR landing lifecycle, shared by the CLI and the Home daemon.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::current_branch;
use crate::engine::load_skill;
use crate::pr_landing::{
    LandingClaim, LandingPlacement, NewPrLanding, PrLanding, PrLandingState, SUPERVISOR_STALE_AFTER,
};
use crate::store::{open_store, storage_config_from_env, SharedStore};
use crate::work::task::{CiCheck, CiIncident, CiObservation, CiState};

use super::error::{OpsError, OpsResult};
use super::land::{arm, LandOptions};
use super::pr::{merge_gate_state, observe_pr_by_number, PrInfo, PrObservation, PrReadFreshness};
use super::progress::{NullProgress, Progress};

const LANDING_POLL_INTERVAL: Duration = Duration::from_secs(30);
const LANDING_DEGRADED_INTERVAL: Duration = Duration::from_secs(60);
const LANDING_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const MAX_REPAIRS: u32 = 3;

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
    fn rearm(&self, landing: &PrLanding) -> OpsResult<PrInfo>;
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

    fn rearm(&self, landing: &PrLanding) -> OpsResult<PrInfo> {
        let options = LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            complete: landing.after_merge == Some(crate::work::task::AfterMerge::CompleteTask),
            next_slug: landing.next_slug.clone(),
            worktree: Some(landing.worktree.display().to_string()),
            commit_message: Some("ci-fix: repair required checks".to_string()),
            pr_title: None,
            pr_body: None,
            agent: None,
        };
        arm(&landing.worktree, &options, &NullProgress)?.ok_or_else(|| {
            OpsError::Message(format!(
                "pull request #{} disappeared while re-arming repaired head",
                landing.pr_number
            ))
        })
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
    let prompt = format!(
        "{skill}\n\nRepair the exact watched landing incident below. Do not land or merge it; leave a material fix in the worktree for the landing supervisor to publish.\n\nRepository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
        incident.repo,
        incident.pr_number,
        landing.branch,
        incident.failed_head_sha,
        checks,
        task_context,
    );
    let config = load_config_or_default(Some(&landing.worktree));
    let launch = AgentConfig {
        task_prompt: prompt,
        agent: Some(config.agent().to_string()),
        cwd: Some(landing.worktree.clone()),
        write_scope: crate::engine::agent::AgentWriteScope::Worktree,
        skip_permissions: true,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: true,
        ..Default::default()
    };
    let result = launch_agent(
        &launch,
        &process,
        &AgentCapabilities {
            chrome: config.chrome,
        },
    )
    .map_err(|error| OpsError::Message(format!("ci-fix provider failed: {error}")))?;
    if result.exit_code != 0 {
        return Err(OpsError::Message(format!(
            "ci-fix provider exited {}: {}",
            result.exit_code,
            result.stderr.trim()
        )));
    }
    Ok(())
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

fn degraded_is_actionable(reason: &str) -> bool {
    let reason = reason.to_ascii_lowercase();
    ![
        "network failure",
        "rate limit",
        "timed out",
        "timeout",
        "could not resolve host",
        "network is unreachable",
        "connection refused",
        "temporary failure",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
    ]
    .iter()
    .any(|marker| reason.contains(marker))
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
) -> OpsResult<PrLanding> {
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
        None,
        Some(reason.clone()),
    )
    .await?;
    Err(OpsError::Message(reason))
}

async fn wait_interval(interval: Duration) {
    if !interval.is_zero() {
        tokio::time::sleep(interval).await;
    }
}

async fn run_driver_operation<T, F>(
    store: &SharedStore,
    landing: &PrLanding,
    label: &'static str,
    operation: F,
) -> OpsResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> OpsResult<T> + Send + 'static,
{
    let mut operation = tokio::task::spawn_blocking(operation);
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
                    operation.abort();
                    return Err(OpsError::Message(format!(
                        "landing {} generation {} lost supervision authority during {label}",
                        landing.id, landing.generation
                    )));
                }
            }
        }
    }
}

/// Run the claimed landing generation until GitHub reports a terminal state.
pub(crate) async fn supervise_pr_landing(
    store: SharedStore,
    mut landing: PrLanding,
    driver: Arc<dyn LandingDriver>,
    poll_interval: Duration,
) -> OpsResult<PrLanding> {
    loop {
        let now = OffsetDateTime::now_utc();
        if !store
            .heartbeat_pr_landing(&landing.id, landing.generation, now)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            return Err(OpsError::Message(format!(
                "landing {} generation {} lost supervision authority",
                landing.id, landing.generation
            )));
        }
        refresh_joined_request(&store, &mut landing).await?;
        let observed = run_driver_operation(&store, &landing, "observation", {
            let driver = Arc::clone(&driver);
            let landing = landing.clone();
            move || driver.observe(&landing)
        })
        .await;
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                let reason = format!(
                    "pull request #{} observation blocked: {error}",
                    landing.pr_number
                );
                return block_landing(&store, &mut landing, reason).await;
            }
        };

        if let Some(head) = observed.head_sha() {
            if head != landing.observed_head_sha
                && !matches!(observed, LandingObservation::Merged { .. })
            {
                landing.observed_head_sha = head.to_string();
                refresh_joined_request(&store, &mut landing).await?;
                let armed = run_driver_operation(&store, &landing, "re-arm", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    move || driver.rearm(&landing)
                })
                .await;
                let armed = match armed {
                    Ok(armed) => armed,
                    Err(error) => {
                        let reason = format!(
                            "pull request #{} changed heads but re-arm failed: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                };
                let Some(armed_head) = armed.head_sha else {
                    let reason = format!(
                        "GitHub omitted the head after re-arming pull request #{}",
                        landing.pr_number
                    );
                    return block_landing(&store, &mut landing, reason).await;
                };
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Watching,
                    armed_head,
                    None,
                    None,
                )
                .await?;
                continue;
            }
        }

        match observed {
            LandingObservation::Merged {
                head_sha,
                merge_commit,
            } => {
                landing.observed_head_sha = head_sha.clone();
                refresh_joined_request(&store, &mut landing).await?;
                store
                    .mark_ci_incidents_merged(&landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                if landing.task_id.is_some() {
                    if let Err(error) = super::task::settle_task_landing(&store, &landing).await {
                        let reason = format!(
                            "pull request #{} merged but Task settlement blocked: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                }
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Merged,
                    head_sha,
                    Some(merge_commit),
                    None,
                )
                .await?;
                return Ok(landing);
            }
            LandingObservation::Closed { head_sha } => {
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Closed,
                    head_sha,
                    None,
                    None,
                )
                .await?;
                return Err(OpsError::Message(format!(
                    "pull request #{} closed without merging",
                    landing.pr_number
                )));
            }
            LandingObservation::Passing { .. } => {
                store
                    .mark_ci_incidents_green(&landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                wait_interval(poll_interval).await;
            }
            LandingObservation::Unarmed { .. } => {
                refresh_joined_request(&store, &mut landing).await?;
                let armed = run_driver_operation(&store, &landing, "re-arm", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    move || driver.rearm(&landing)
                })
                .await;
                let armed = match armed {
                    Ok(armed) => armed,
                    Err(error) => {
                        let reason = format!(
                            "pull request #{} lost its merge request and re-arm failed: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                };
                let Some(armed_head) = armed.head_sha else {
                    let reason = format!(
                        "GitHub omitted the head after re-arming pull request #{}",
                        landing.pr_number
                    );
                    return block_landing(&store, &mut landing, reason).await;
                };
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Watching,
                    armed_head,
                    None,
                    None,
                )
                .await?;
            }
            LandingObservation::Pending { .. } => wait_interval(poll_interval).await,
            LandingObservation::Degraded { reason } if degraded_is_actionable(&reason) => {
                return block_landing(&store, &mut landing, reason).await;
            }
            LandingObservation::Degraded { .. } => {
                wait_interval(poll_interval.max(LANDING_DEGRADED_INTERVAL)).await;
            }
            LandingObservation::Failing {
                head_sha,
                failing_checks,
            } if !actionable_failure(&head_sha, &failing_checks, now) => {
                wait_interval(poll_interval).await;
            }
            LandingObservation::Failing {
                head_sha,
                failing_checks,
            } => {
                if landing.repair_count >= MAX_REPAIRS {
                    let reason = format!(
                        "pull request #{} exhausted its {MAX_REPAIRS} repair attempts",
                        landing.pr_number
                    );
                    return block_landing(&store, &mut landing, reason).await;
                }
                landing.observed_head_sha = head_sha;
                let incident = ci_incident(&landing, &failing_checks, now);
                store
                    .observe_ci_incident(&incident)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                let confirmation = run_driver_operation(&store, &landing, "CI confirmation", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    move || driver.observe(&landing)
                })
                .await;
                let confirmation = match confirmation {
                    Ok(confirmation) => confirmation,
                    Err(error) => {
                        let reason = format!(
                            "pull request #{} CI confirmation blocked: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                };
                let still_current = match &confirmation {
                    LandingObservation::Failing {
                        head_sha,
                        failing_checks,
                    } if head_sha == &incident.failed_head_sha
                        && actionable_failure(
                            head_sha,
                            failing_checks,
                            OffsetDateTime::now_utc(),
                        ) =>
                    {
                        ci_incident(&landing, failing_checks, now).identity == incident.identity
                    }
                    _ => false,
                };
                if !still_current {
                    if let LandingObservation::Degraded { reason } = confirmation {
                        if degraded_is_actionable(&reason) {
                            return block_landing(&store, &mut landing, reason).await;
                        }
                    }
                    wait_interval(poll_interval).await;
                    continue;
                }
                if !store
                    .claim_ci_incident(&incident.identity, &landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?
                {
                    return block_landing(
                        &store,
                        &mut landing,
                        format!(
                            "CI incident {} already consumed its one repair without advancing the head",
                            incident.identity
                        ),
                    )
                    .await;
                }
                landing.repair_count += 1;
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Repairing,
                    incident.failed_head_sha.clone(),
                    None,
                    None,
                )
                .await?;
                let repair = run_driver_operation(&store, &landing, "ci-fix", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    let incident = incident.clone();
                    move || driver.repair(&landing, &incident)
                })
                .await;
                match repair {
                    Ok(()) => {}
                    Err(error) => {
                        let reason = format!(
                            "ci-fix blocked for pull request #{}: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                }
                refresh_joined_request(&store, &mut landing).await?;
                let armed = run_driver_operation(&store, &landing, "re-arm", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    move || driver.rearm(&landing)
                })
                .await;
                let armed = match armed {
                    Ok(armed) => armed,
                    Err(error) => {
                        let reason = format!(
                            "pull request #{} repair could not be re-armed: {error}",
                            landing.pr_number
                        );
                        return block_landing(&store, &mut landing, reason).await;
                    }
                };
                let Some(repaired_head) = armed.head_sha else {
                    let reason = format!(
                        "GitHub omitted the repaired head for pull request #{}",
                        landing.pr_number
                    );
                    return block_landing(&store, &mut landing, reason).await;
                };
                if repaired_head == incident.failed_head_sha {
                    let reason = format!(
                        "ci-fix did not advance pull request #{} past failed head {}",
                        landing.pr_number, incident.failed_head_sha
                    );
                    return block_landing(&store, &mut landing, reason).await;
                }
                if !store
                    .mark_ci_incident_repaired(
                        &incident.identity,
                        &landing.id,
                        landing.generation,
                        &repaired_head,
                        OffsetDateTime::now_utc(),
                    )
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?
                {
                    return Err(OpsError::Message(format!(
                        "landing {} lost authority while recording repaired head",
                        landing.id
                    )));
                }
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Watching,
                    repaired_head,
                    None,
                    None,
                )
                .await?;
            }
        }
    }
}

async fn refresh_joined_request(store: &SharedStore, landing: &mut PrLanding) -> OpsResult<()> {
    let current = store
        .get_pr_landing(&landing.id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message(format!("landing {} disappeared", landing.id)))?;
    if current.generation != landing.generation || current.state.is_terminal() {
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
    let task = store
        .get_task_by_worktree(&worktree.display().to_string())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
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

async fn watch_armed(repo: &Path, options: &LandOptions, pr: PrInfo) -> OpsResult<PrLanding> {
    let store = landing_store().await?;
    let candidate = create_landing(&store, repo, options, &pr).await?;
    let landing = store
        .start_or_join_pr_landing(&candidate)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if landing.state.is_terminal() {
        return if landing.state == PrLandingState::Merged {
            Ok(landing)
        } else {
            Err(OpsError::Message(
                landing
                    .blocked_reason
                    .clone()
                    .unwrap_or_else(|| "landing is terminal without a merge".to_string()),
            ))
        };
    }

    let local_home = store
        .local_home()
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let _ = crate::lfd::claim_pr_landing(&local_home.id, &landing.id, landing.generation).await;
    wait_for_landing(&store, &landing.id).await
}

async fn wait_for_landing(
    store: &SharedStore,
    landing_id: &crate::pr_landing::PrLandingId,
) -> OpsResult<PrLanding> {
    loop {
        let landing = store
            .get_pr_landing(landing_id)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message(format!("landing {landing_id} disappeared")))?;
        match landing.state {
            PrLandingState::Merged => return Ok(landing),
            PrLandingState::Closed | PrLandingState::Blocked => {
                return Err(OpsError::Message(landing.blocked_reason.unwrap_or_else(
                    || "pull request closed without merging".to_string(),
                )))
            }
            PrLandingState::Watching | PrLandingState::Repairing => {
                let now = OffsetDateTime::now_utc();
                let claim = LandingClaim {
                    placement: LandingPlacement::Local,
                    process_id: std::process::id(),
                    heartbeat_at: now,
                };
                if let Some(claimed) = store
                    .claim_pr_landing(
                        &landing.id,
                        landing.generation,
                        &claim,
                        now - SUPERVISOR_STALE_AFTER,
                    )
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?
                {
                    let driver = github_landing_driver();
                    return supervise_pr_landing(
                        Arc::clone(store),
                        claimed,
                        driver,
                        LANDING_POLL_INTERVAL,
                    )
                    .await;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

pub(crate) fn watch_armed_pr(
    repo: &Path,
    options: &LandOptions,
    pr: PrInfo,
    progress: &impl Progress,
) -> OpsResult<PrLanding> {
    progress.status(&format!(
        "Watching pull request #{} through merge...",
        pr.number
    ));
    let runtime = tokio::runtime::Runtime::new()?;
    let landing = runtime.block_on(watch_armed(repo, options, pr))?;
    progress.status(&format!(
        "Pull request #{} merged as {}.",
        landing.pr_number,
        landing.merge_commit.as_deref().unwrap_or("unknown")
    ));
    Ok(landing)
}

pub(crate) fn github_landing_driver() -> Arc<dyn LandingDriver> {
    Arc::new(GithubLandingDriver)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;
    use crate::store::migrations::migration_sql_for_test;
    use crate::store::{open_store, StorageConfig};

    struct FakeDriver {
        observations: Mutex<VecDeque<LandingObservation>>,
        repaired_head: String,
        repairs: Mutex<u32>,
        rearms: Mutex<u32>,
    }

    impl LandingDriver for FakeDriver {
        fn observe(&self, _landing: &PrLanding) -> OpsResult<LandingObservation> {
            self.observations
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| OpsError::Message("fake observation exhausted".to_string()))
        }

        fn repair(&self, _landing: &PrLanding, _incident: &CiIncident) -> OpsResult<()> {
            *self.repairs.lock().unwrap() += 1;
            Ok(())
        }

        fn rearm(&self, landing: &PrLanding) -> OpsResult<PrInfo> {
            *self.rearms.lock().unwrap() += 1;
            Ok(PrInfo {
                url: format!(
                    "https://github.com/{}/pull/{}",
                    landing.repo, landing.pr_number
                ),
                number: u64::from(landing.pr_number),
                state: "open".to_string(),
                branch: landing.branch.clone(),
                merge_commit: None,
                merged_at: None,
                head_sha: Some(self.repaired_head.clone()),
            })
        }
    }

    async fn store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("registry.db");
        open_store(&StorageConfig::sqlite(path.clone()))
            .await
            .unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        let migrated = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='pr_landings')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !migrated {
            conn.execute_batch(&migration_sql_for_test("pr_landings"))
                .unwrap();
        }
        let store = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        (directory, store)
    }

    async fn claimed(store: &SharedStore) -> PrLanding {
        let now = OffsetDateTime::now_utc();
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: PathBuf::from("/tmp/landing"),
                branch: "jack/landing".to_string(),
                task_id: None,
                requested_head_sha: "failed-head".to_string(),
                after_merge: None,
                next_slug: None,
            },
            now,
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&landing).await.unwrap();
        store
            .claim_pr_landing(
                &landing.id,
                landing.generation,
                &LandingClaim {
                    placement: LandingPlacement::Local,
                    process_id: 41,
                    heartbeat_at: now,
                },
                now - SUPERVISOR_STALE_AFTER,
            )
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn pr_landing_runs_ci_fix_rearms_and_finishes_only_after_merge() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                LandingObservation::Failing {
                    head_sha: "failed-head".to_string(),
                    failing_checks: vec![CiCheck {
                        name: "rust".to_string(),
                        url: Some("https://example.com/rust".to_string()),
                    }],
                },
                LandingObservation::Failing {
                    head_sha: "failed-head".to_string(),
                    failing_checks: vec![CiCheck {
                        name: "rust".to_string(),
                        url: Some("https://example.com/rust".to_string()),
                    }],
                },
                LandingObservation::Pending {
                    head_sha: "repaired-head".to_string(),
                },
                LandingObservation::Merged {
                    head_sha: "repaired-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repaired_head: "repaired-head".to_string(),
            repairs: Mutex::new(0),
            rearms: Mutex::new(0),
        });
        let landed = supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(landed.repair_count, 1);
        assert_eq!(*driver.repairs.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn pending_ci_never_launches_a_provider() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                LandingObservation::Pending {
                    head_sha: "failed-head".to_string(),
                },
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repaired_head: "unused".to_string(),
            repairs: Mutex::new(0),
            rearms: Mutex::new(0),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn missing_merge_request_is_rearmed_without_a_repair() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                LandingObservation::Unarmed {
                    head_sha: "failed-head".to_string(),
                },
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repaired_head: "failed-head".to_string(),
            repairs: Mutex::new(0),
            rearms: Mutex::new(0),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
        assert_eq!(*driver.rearms.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn terminal_preflight_failure_never_launches_a_provider() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                LandingObservation::Failing {
                    head_sha: "failed-head".to_string(),
                    failing_checks: vec![CiCheck {
                        name: "scratch-clear".to_string(),
                        url: None,
                    }],
                },
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repaired_head: "unused".to_string(),
            repairs: Mutex::new(0),
            rearms: Mutex::new(0),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn stale_ci_incident_is_not_repaired_after_the_head_moves() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                LandingObservation::Failing {
                    head_sha: "failed-head".to_string(),
                    failing_checks: vec![CiCheck {
                        name: "rust".to_string(),
                        url: None,
                    }],
                },
                LandingObservation::Failing {
                    head_sha: "next-head".to_string(),
                    failing_checks: vec![CiCheck {
                        name: "rust".to_string(),
                        url: None,
                    }],
                },
                LandingObservation::Pending {
                    head_sha: "next-head".to_string(),
                },
                LandingObservation::Merged {
                    head_sha: "next-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repaired_head: "next-head".to_string(),
            repairs: Mutex::new(0),
            rearms: Mutex::new(0),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn landing_adopts_an_unclaimed_incident_from_before_the_migration() {
        let (_directory, store) = store().await;
        let landing = claimed(&store).await;
        let now = OffsetDateTime::now_utc();
        let checks = vec![CiCheck {
            name: "rust".to_string(),
            url: None,
        }];
        let incident = ci_incident(&landing, &checks, now);
        let mut legacy = incident.clone();
        legacy.landing_id = None;
        store.observe_ci_incident(&legacy).await.unwrap();
        store.observe_ci_incident(&incident).await.unwrap();

        assert!(store
            .claim_ci_incident(&incident.identity, &landing.id, landing.generation, now)
            .await
            .unwrap());
    }
}
