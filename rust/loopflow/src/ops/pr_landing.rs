//! One watched PR landing lifecycle, shared by the CLI and the Home daemon.

use std::collections::HashSet;
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

const LANDING_POLL_INTERVAL: Duration = Duration::from_secs(30);
const LANDING_DEGRADED_INTERVAL: Duration = Duration::from_secs(60);
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
    fn repair(
        &self,
        landing: &PrLanding,
        incident: &CiIncident,
        previous: &str,
    ) -> OpsResult<String>;
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

    fn repair(
        &self,
        landing: &PrLanding,
        incident: &CiIncident,
        previous: &str,
    ) -> OpsResult<String> {
        launch_ci_fix(landing, incident, previous)
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

fn launch_ci_fix(landing: &PrLanding, incident: &CiIncident, previous: &str) -> OpsResult<String> {
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
        "{skill}\n\nRepair the exact watched landing incident below. Start with `lf rebase`. Repair and verify, then run `{arm_command}` to publish and enable auto-merge with the requested Task disposition. Do not invoke `lf pr land` or wait for merge; the landing supervisor only observes the result and completes after merge.\n\nRepository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
        incident.repo,
        incident.pr_number,
        landing.branch,
        incident.failed_head_sha,
        checks,
        task_context,
    );
    if !previous.is_empty() {
        prompt.push_str(&format!(
            "\n\nPrevious repair conclusion (GitHub still requires recovery):\n{previous}\nContinue from the existing checkout and resolve the remaining failure."
        ));
    }
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
        RepairConclusion::Published(summary) => Ok(summary),
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

async fn lock_supervisor(store: &SharedStore, landing: &PrLanding) -> OpsResult<Arc<File>> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(
            crate::engine::git::absolute_git_dir(&landing.worktree)?.join("lf-pr-landing.lock"),
        )?;
    loop {
        if !store
            .heartbeat_pr_landing(&landing.id, landing.generation, OffsetDateTime::now_utc())
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            return Err(OpsError::Message(format!(
                "landing {} generation {} lost supervision authority",
                landing.id, landing.generation
            )));
        }
        match FileExt::try_lock_exclusive(&lock) {
            Ok(()) => return Ok(Arc::new(lock)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(error) => return Err(error.into()),
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
    let ownership = lock_supervisor(&store, &landing).await?;
    let mut previous_repair = String::new();
    let mut repair_error = None;
    let mut repaired_incident: Option<CiIncident> = None;
    let mut next_observation = None;
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
        let observed = match next_observation.take() {
            Some(observed) => Ok(observed),
            None => {
                run_driver_operation(&store, &landing, &ownership, "observation", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    move || driver.observe(&landing)
                })
                .await
            }
        };
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

        // Provider failure can follow successful publication or even merge.
        // Reconcile GitHub before deciding whether the repair blocked delivery.
        if !matches!(observed, LandingObservation::Degraded { .. }) {
            if let Some(error) = repair_error.take() {
                if matches!(observed, LandingObservation::Unarmed { .. })
                    || matches!(&observed, LandingObservation::Failing { head_sha, .. } if head_sha == &landing.observed_head_sha)
                {
                    return block_landing(&store, &mut landing, error).await;
                }
            }
        }
        if let Some(head) = observed.head_sha() {
            if let Some(incident) = repaired_incident.as_ref() {
                if head != incident.failed_head_sha {
                    store
                        .mark_ci_incident_repaired(
                            &incident.identity,
                            &landing.id,
                            landing.generation,
                            head,
                            now,
                        )
                        .await
                        .map_err(|error| OpsError::Message(error.to_string()))?;
                    repaired_incident = None;
                }
            }
            if head != landing.observed_head_sha {
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Watching,
                    head.to_string(),
                    None,
                    None,
                )
                .await?;
            }
        }

        match observed {
            LandingObservation::Merged {
                head_sha,
                merge_commit,
            } => {
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
                let reason = format!(
                    "pull request #{} has no auto-merge request; run lf pr arm to resume landing",
                    landing.pr_number
                );
                return block_landing(&store, &mut landing, reason).await;
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
                let incident = ci_incident(&landing, &failing_checks, now);
                store
                    .observe_ci_incident(&incident)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                let confirmation =
                    run_driver_operation(&store, &landing, &ownership, "CI confirmation", {
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
                    } if head_sha == &incident.failed_head_sha => {
                        ci_incident(&landing, failing_checks, now).identity == incident.identity
                    }
                    _ => false,
                };
                if !still_current {
                    next_observation = Some(confirmation);
                    continue;
                }
                store
                    .record_ci_response(&incident.identity, &landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Repairing,
                    incident.failed_head_sha.clone(),
                    None,
                    None,
                )
                .await?;
                let repair = run_driver_operation(&store, &landing, &ownership, "ci-fix", {
                    let driver = Arc::clone(&driver);
                    let landing = landing.clone();
                    let incident = incident.clone();
                    let previous = previous_repair.clone();
                    move || driver.repair(&landing, &incident, &previous)
                })
                .await;
                repaired_incident = Some(incident);
                match repair {
                    Ok(conclusion) => previous_repair = conclusion,
                    Err(error) => {
                        repair_error = Some(format!(
                            "ci-fix blocked for pull request #{}: {error}",
                            landing.pr_number
                        ));
                    }
                }
                persist_landing_state(
                    &store,
                    &mut landing,
                    PrLandingState::Watching,
                    head_sha,
                    None,
                    None,
                )
                .await?;
                wait_interval(poll_interval).await;
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
    let home = crate::store::lf_home_dir();
    let mut shown = HashSet::new();
    let mut report_at = std::time::Instant::now();
    loop {
        let landing = store
            .get_pr_landing(landing_id)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message(format!("landing {landing_id} disappeared")))?;
        if landing.state.is_terminal() || std::time::Instant::now() >= report_at {
            match repair_conclusions(&home, &landing, &mut shown) {
                Ok(conclusions) => {
                    for conclusion in conclusions {
                        eprintln!("ci-fix: {conclusion}");
                    }
                }
                Err(error) => tracing::warn!(%error, "could not read CI repair output"),
            }
            report_at = std::time::Instant::now() + LANDING_POLL_INTERVAL;
        }
        match landing.state {
            PrLandingState::Merged => return Ok(landing),
            PrLandingState::Closed | PrLandingState::Blocked => {
                return Err(OpsError::Message(landing.blocked_reason.unwrap_or_else(
                    || "pull request closed without merging".to_string(),
                )))
            }
            PrLandingState::Watching | PrLandingState::Repairing => {
                let now = OffsetDateTime::now_utc();
                let claim = LandingSupervisor {
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

fn repair_conclusions(
    home: &Path,
    landing: &PrLanding,
    shown: &mut HashSet<String>,
) -> std::io::Result<Vec<String>> {
    let runs = crate::run_record::scan_runs_since(home, landing.created_at.unix_timestamp())?;
    let mut conclusions = Vec::new();
    for run in runs.into_iter().rev().filter(|run| {
        run.skill.as_deref() == Some("ci-fix")
            && run.worktree.as_deref().map(Path::new) == Some(landing.worktree.as_path())
            && run.ended.is_some()
    }) {
        if shown.contains(&run.id) {
            continue;
        }
        let (dir, _) = crate::run_record::resolve_manifest(home, &run.id)?;
        let conclusion = crate::run_record::read_final_answer(&dir)?
            .map(|answer| {
                parse_repair_conclusion(&answer.text)
                    .map(|conclusion| conclusion.summary().to_string())
                    .unwrap_or(answer.text)
            })
            .unwrap_or_else(|| {
                format!(
                    "Run {} finished; inspect lf runs {} --events",
                    run.id, run.id
                )
            });
        conclusions.push(conclusion);
        shown.insert(run.id);
    }
    Ok(conclusions)
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

    use super::{
        ci_incident, repair_arm_command, supervise_pr_landing, LandingDriver, LandingObservation,
    };
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use time::OffsetDateTime;

    use crate::ops::error::{OpsError, OpsResult};
    use crate::pr_landing::{
        LandingPlacement, LandingSupervisor, NewPrLanding, PrLanding, PrLandingState,
        SUPERVISOR_STALE_AFTER,
    };
    use crate::store::SharedStore;
    use crate::store::StorageConfig;
    use crate::work::task::{AfterMerge, CiCheck, CiIncident};

    struct FakeDriver {
        observations: Mutex<VecDeque<LandingObservation>>,
        repairs: Mutex<u32>,
        repair_results: Mutex<VecDeque<OpsResult<String>>>,
    }

    impl LandingDriver for FakeDriver {
        fn observe(&self, _landing: &PrLanding) -> OpsResult<LandingObservation> {
            self.observations
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| OpsError::Message("fake observation exhausted".to_string()))
        }

        fn repair(
            &self,
            _landing: &PrLanding,
            _incident: &CiIncident,
            _previous: &str,
        ) -> OpsResult<String> {
            *self.repairs.lock().unwrap() += 1;
            self.repair_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok("Repair published".to_string()))
        }
    }

    async fn store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(path))
                .await
                .unwrap(),
        );
        (directory, store)
    }

    async fn claimed(store: &SharedStore, worktree: &std::path::Path) -> PrLanding {
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(worktree)
            .status()
            .unwrap()
            .success());
        let now = OffsetDateTime::now_utc();
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: worktree.to_path_buf(),
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
                &LandingSupervisor {
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
    async fn pr_landing_observes_ci_fix_publication_and_finishes_only_after_merge() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
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
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        let landed = supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(*driver.repairs.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn ci_fix_can_land_without_changing_the_head() {
        for result in [
            LandingObservation::Pending {
                head_sha: "failed-head".to_string(),
            },
            LandingObservation::Passing {
                head_sha: "failed-head".to_string(),
            },
            LandingObservation::Merged {
                head_sha: "failed-head".to_string(),
                merge_commit: "merge-head".to_string(),
            },
        ] {
            let (_directory, store) = store().await;
            let landing = claimed(&store, _directory.path()).await;
            let failure = failed_check();
            let driver = Arc::new(FakeDriver {
                observations: Mutex::new(VecDeque::from([
                    failure.clone(),
                    failure,
                    result,
                    LandingObservation::Merged {
                        head_sha: "failed-head".to_string(),
                        merge_commit: "merge-head".to_string(),
                    },
                ])),
                repairs: Mutex::new(0),
                repair_results: Mutex::new(VecDeque::new()),
            });
            let landed =
                supervise_pr_landing(store.clone(), landing, driver.clone(), Duration::ZERO)
                    .await
                    .unwrap();
            assert_eq!(landed.state, PrLandingState::Merged);
            assert_eq!(*driver.repairs.lock().unwrap(), 1);
            let incidents = store
                .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
                .await
                .unwrap();
            assert!(incidents[0].incident.merged_at.is_some());
            assert!(incidents[0].incident.repaired_head_sha.is_none());
        }
    }

    fn failed_check() -> LandingObservation {
        LandingObservation::Failing {
            head_sha: "failed-head".to_string(),
            failing_checks: vec![CiCheck {
                name: "rust".to_string(),
                url: None,
            }],
        }
    }

    #[tokio::test]
    async fn landing_finishes_when_ci_confirmation_observes_merge() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                failed_check(),
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        let landed = supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn landing_waiter_reads_completed_repair_output_once() {
        use crate::chat::types::{ConversationEvent, ConversationItem};
        use crate::run_record::{CaptureHandle, RunSpec};

        let (home, store) = store().await;
        let landing = claimed(&store, home.path()).await;
        let mut shown = std::collections::HashSet::new();
        let mut active = Vec::new();
        for (worktree, skill, completed) in [
            (landing.worktree.clone(), "ci-fix", true),
            (landing.worktree.clone(), "ci-fix", false),
            (PathBuf::from("/another/worktree"), "ci-fix", true),
            (landing.worktree.clone(), "implement", true),
        ] {
            let capture = CaptureHandle::begin_at(
                home.path(),
                RunSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: worktree.clone(),
                    repo: None,
                    worktree: Some(worktree),
                    skill: Some(skill.into()),
                    subjects: Vec::new(),
                    flow: crate::run_record::RunFlowMembership::Independent,
                },
            )
            .unwrap();
            capture.record_conversation(ConversationEvent::ItemCompleted {
                turn_id: "turn".into(),
                item: ConversationItem::Message {
                    id: "answer".into(),
                    text: r#"{"status":"blocked","summary":"GitHub credential revoked; reconnect it before retrying."}"#.into(),
                    phase: Some("final_answer".into()),
                },
            });
            if completed {
                capture.record_conversation(ConversationEvent::TurnCompleted {
                    turn_id: "turn".into(),
                    status: crate::chat::types::Lifecycle::Completed,
                });
                capture.finish("completed").unwrap();
            } else {
                active.push(capture);
            }
        }
        assert_eq!(
            super::repair_conclusions(home.path(), &landing, &mut shown).unwrap(),
            ["GitHub credential revoked; reconnect it before retrying."]
        );
        assert!(super::repair_conclusions(home.path(), &landing, &mut shown)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn landing_takeover_waits_for_the_old_repair_to_finish() {
        struct HeldRepair {
            entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
            release: Mutex<std::sync::mpsc::Receiver<()>>,
        }
        impl LandingDriver for HeldRepair {
            fn observe(&self, _: &PrLanding) -> OpsResult<LandingObservation> {
                Ok(failed_check())
            }
            fn repair(&self, _: &PrLanding, _: &CiIncident, _: &str) -> OpsResult<String> {
                self.entered
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(())
                    .unwrap();
                self.release.lock().unwrap().recv().unwrap();
                Ok("Repair published".into())
            }
        }

        let (directory, store) = store().await;
        let landing = claimed(&store, directory.path()).await;
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let old = tokio::spawn(supervise_pr_landing(
            store.clone(),
            landing.clone(),
            Arc::new(HeldRepair {
                entered: Mutex::new(Some(entered)),
                release: Mutex::new(held),
            }),
            Duration::ZERO,
        ));
        started.await.unwrap();
        old.abort();
        assert!(old.await.unwrap_err().is_cancelled());
        let now = OffsetDateTime::now_utc() + SUPERVISOR_STALE_AFTER;
        let successor = store
            .claim_pr_landing(
                &landing.id,
                landing.generation,
                &LandingSupervisor {
                    placement: LandingPlacement::Local,
                    process_id: 42,
                    heartbeat_at: now,
                },
                now,
            )
            .await
            .unwrap()
            .unwrap();
        let mut recovered = tokio::spawn(supervise_pr_landing(
            store,
            successor,
            Arc::new(FakeDriver {
                observations: Mutex::new(VecDeque::from([LandingObservation::Merged {
                    head_sha: "failed-head".into(),
                    merge_commit: "merged".into(),
                }])),
                repairs: Mutex::new(0),
                repair_results: Mutex::new(VecDeque::new()),
            }),
            Duration::ZERO,
        ));
        let while_repairing =
            tokio::time::timeout(Duration::from_millis(200), &mut recovered).await;
        // Release before asserting so a regression cannot strand the blocking thread.
        release.send(()).unwrap();
        assert!(while_repairing.is_err());
        let landed = tokio::time::timeout(Duration::from_secs(5), recovered)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
    }

    #[tokio::test]
    async fn landing_keeps_repairing_a_previously_serviced_incident() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
        let mut observations = VecDeque::from(vec![failed_check(); 8]);
        observations.push_back(LandingObservation::Merged {
            head_sha: "failed-head".to_string(),
            merge_commit: "merge-head".to_string(),
        });
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(observations),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        let landed = supervise_pr_landing(store.clone(), landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(*driver.repairs.lock().unwrap(), 4);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].incident.responded_at.is_some());
        assert!(incidents[0].incident.merged_at.is_some());
    }

    #[tokio::test]
    async fn repair_error_does_not_hide_a_merge() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                failed_check(),
                failed_check(),
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::from([Err(OpsError::Message(
                "provider disconnected after publication".to_string(),
            ))])),
        });
        let landed = supervise_pr_landing(store, landing, driver, Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
    }

    #[tokio::test]
    async fn repair_error_explains_a_block_and_same_head_retry_can_merge() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                failed_check(),
                failed_check(),
                failed_check(),
            ])),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::from([Err(OpsError::Message(
                "provider credential revoked".to_string(),
            ))])),
        });
        let error = supervise_pr_landing(store.clone(), landing.clone(), driver, Duration::ZERO)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("provider credential revoked"));
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);

        let resumed = claimed(&store, _directory.path()).await;
        assert_eq!(resumed.id, landing.id);
        assert!(resumed.generation > landing.generation);
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([
                failed_check(),
                failed_check(),
                LandingObservation::Merged {
                    head_sha: "failed-head".to_string(),
                    merge_commit: "merge-head".to_string(),
                },
            ])),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        let landed = supervise_pr_landing(store.clone(), resumed, driver, Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert!(incidents[0].incident.blocked_at.is_some());
        assert!(incidents[0].incident.merged_at.is_some());
    }

    #[tokio::test]
    async fn ci_fix_arm_preserves_task_completion_and_rotation() {
        let (_directory, store) = store().await;
        let mut landing = claimed(&store, _directory.path()).await;
        assert_eq!(repair_arm_command(&landing), "lf pr arm");
        landing.after_merge = Some(AfterMerge::CompleteTask);
        assert_eq!(repair_arm_command(&landing), "lf pr arm -c");
        landing.after_merge = Some(AfterMerge::ContinueTask);
        landing.next_slug = Some("parser-proof".to_string());
        assert_eq!(
            repair_arm_command(&landing),
            "lf pr arm --next 'parser-proof'"
        );
    }

    #[tokio::test]
    async fn pending_ci_never_launches_a_provider() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
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
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn missing_merge_request_blocks_without_mutating_the_pr() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(VecDeque::from([LandingObservation::Unarmed {
                head_sha: "failed-head".to_string(),
            }])),
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        let error = supervise_pr_landing(
            store.clone(),
            landing.clone(),
            driver.clone(),
            Duration::ZERO,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("no auto-merge request"));
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn terminal_preflight_failure_never_launches_a_provider() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
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
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn stale_ci_incident_is_not_repaired_after_the_head_moves() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
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
            repairs: Mutex::new(0),
            repair_results: Mutex::new(VecDeque::new()),
        });
        supervise_pr_landing(store, landing, driver.clone(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(*driver.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn landing_adopts_an_unclaimed_incident_from_before_the_migration() {
        let (_directory, store) = store().await;
        let landing = claimed(&store, _directory.path()).await;
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
            .record_ci_response(&incident.identity, &landing.id, landing.generation, now)
            .await
            .unwrap());
    }
}
