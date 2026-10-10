//! Finite pull-request delivery reconciliation over durable landing intents.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::agent::{run_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::config::load_config_or_default;
use crate::flow::load_skill;
use crate::git::current_branch;
use crate::pr_landing::{
    LandingPlacement, LandingSupervisor, NewPrLanding, PrLanding, PrLandingState,
    SUPERVISOR_STALE_AFTER,
};
use crate::session_record::{
    AgentProcessRequest, CaptureHandle, SessionCaptureSpec, SessionFlowMembership,
};
use crate::store::{open_store, storage_config_from_env, SharedStore};
use crate::work::task::{CiCheck, CiIncident, CiObservation, CiState};

use super::error::{OpsError, OpsResult};
use super::land::LandOptions;
use super::pr::{
    merge_gate_state, merge_needs_integration, observe_pr_merge, MergeRequest, PrInfo,
};

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
        attempt: Option<String>,
    },
    Queued {
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
            | Self::Pending { head_sha, .. }
            | Self::Queued { head_sha }
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
    /// Whether this check may start a ci-fix. A check that may not still records
    /// the failure and leaves the repair to `lf ci watch`.
    fn repairs(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
struct GithubLandingDriver {
    repairs: bool,
    release: Option<Arc<(super::release_lock::ReleaseLock, crate::git::WorktreeLease)>>,
}

impl LandingDriver for GithubLandingDriver {
    fn observe(&self, landing: &PrLanding) -> OpsResult<LandingObservation> {
        match observe_pr_merge(&landing.worktree, u64::from(landing.pr_number)) {
            Ok(observation) => {
                classify_github_observation(landing, observation.pr, observation.request)
            }
            Err(error) => Ok(LandingObservation::Degraded {
                reason: error.to_string(),
            }),
        }
    }

    fn repair(&self, landing: &PrLanding, incident: &CiIncident) -> OpsResult<()> {
        admit_ci_fix(landing, incident, self.release.as_deref())
    }

    fn repairs(&self) -> bool {
        self.repairs
    }
}

fn classify_github_observation(
    landing: &PrLanding,
    pr: PrInfo,
    request: Option<MergeRequest>,
) -> OpsResult<LandingObservation> {
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
        // The queue checks an integrated commit. Its membership takes precedence
        // over the original PR head's mergeability and check results.
        _ if matches!(request, Some(MergeRequest::Queued(_))) => {
            Ok(LandingObservation::Queued { head_sha })
        }
        _ if request.is_none() => Ok(LandingObservation::Unarmed { head_sha }),
        _ if merge_needs_integration(pr.merge_state.as_deref(), request.as_ref()) => {
            Ok(LandingObservation::Failing {
                head_sha,
                failing_checks: vec![CiCheck {
                    name: "required-integration".into(),
                    url: None,
                }],
            })
        }
        _ => match merge_gate_state(&landing.worktree, u64::from(landing.pr_number), &head_sha) {
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
            Ok(Some(reading)) if reading.pending => Ok(LandingObservation::Pending {
                head_sha,
                attempt: Some(reading.attempt),
            }),
            Ok(Some(_)) => Ok(LandingObservation::Passing { head_sha }),
            Ok(None) => Ok(LandingObservation::Pending {
                head_sha,
                attempt: None,
            }),
            Err(error) => Ok(LandingObservation::Degraded {
                reason: error.to_string(),
            }),
        },
    }
}

fn admit_ci_fix(
    landing: &PrLanding,
    incident: &CiIncident,
    release: Option<&(super::release_lock::ReleaseLock, crate::git::WorktreeLease)>,
) -> OpsResult<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = landing_store().await?;
        let lock = store
            .sqlite
            .lock_task_checkouts(&[&landing.worktree], landing.task_id.as_ref())
            .map_err(repair_error)?;
        let reservation = store
            .sqlite
            .repair_reservation(&incident.identity)
            .map_err(repair_error)?;
        if let Some(finished) = reservation
            .finished
            .filter(|_| reservation.conclusion.is_some())
        {
            return Err(OpsError::Message(reservation.error.unwrap_or_else(|| {
                format!("repair completed at {finished}; waiting for changed evidence")
            })));
        }
        if repair_live(&store, reservation.process.as_ref()) {
            return Ok(());
        }
        if let Some(session) = &reservation.session {
            // A bound driver's engine is judged by its own OS identity.
            let bound = store
                .sqlite
                .session_attachment(session)
                .map_err(repair_error)?
                .is_some();
            let engine = store
                .sqlite
                .session_provider_process(session)
                .map_err(repair_error)?;
            if bound
                && engine.is_some_and(|(pid, started)| {
                    crate::journal::process_identity_evidence(pid, started)
                        != crate::journal::ProcessIdentityEvidence::Dead
                })
            {
                return Err(OpsError::Message(format!(
                    "repair Session {session} has a live or unresolved provider"
                )));
            }
        }
        if let Some(task_id) = &landing.task_id {
            let task = store
                .get_task(task_id)
                .await
                .map_err(repair_error)?
                .ok_or_else(|| repair_error("landing Task is missing"))?;
            if store
                .sqlite
                .task_automation(task_id)
                .map_err(repair_error)?
                .enabled
                == Some(false)
            {
                return Err(repair_error("Task automation is held"));
            }
            let local = store.local_machine().await.map_err(repair_error)?;
            let placement = store
                .placement(&crate::durable::WorkRef::Task(task_id.clone()))
                .await
                .map_err(repair_error)?;
            if placement.machine_id != local.id {
                return Err(repair_error(format!(
                    "Task is placed on Machine {}",
                    placement.machine_id
                )));
            }
            if let Some(reason) =
                super::task_automation::task_execution_blockers(&store.sqlite, &task.id)?
                    .into_iter()
                    .next()
            {
                return Err(OpsError::CheckoutBusy(reason));
            }
        } else if let Some(reason) = checkout_execution_blockers(&store, &landing.worktree)?
            .into_iter()
            .next()
        {
            return Err(OpsError::CheckoutBusy(reason));
        }
        let config = load_config_or_default(Some(&landing.worktree));
        let retry = reservation.process.is_some();
        if retry && reservation.retries >= config.automation.retries {
            return Err(repair_error("repair startup exhausted automatic retries"));
        }
        let launcher = crate::journal::current_lf_process_id()
            .ok_or_else(|| repair_error("repair admission requires a recorded Process"))?;
        let session = if let Some(id) = &reservation.session {
            store
                .sqlite
                .session(id)
                .map_err(repair_error)?
                .ok_or_else(|| repair_error("reserved repair Session disappeared"))?
        } else {
            let (provider, model) = crate::config::parse_agent(config.agent());
            crate::session::LfSession {
                id: format!("session_{}", uuid::Uuid::new_v4().simple()),
                captured: None,
                artifact_key: crate::session_record::new_artifact_key(),
                caller_artifact_key: None,
                input_published: false,
                cwd: landing.worktree.clone(),
                skill: Some("ci-fix".into()),
                provider: Some(provider),
                model,
                node: None,
                iterations: None,
                task_id: landing.task_id.clone(),
                wave_id: None,
                flow_lf_process_id: None,
                work_source: landing
                    .task_id
                    .as_ref()
                    .map(|_| crate::session::WorkSource::Declared),
                bound_at: None,
                interactive: false,
                repo: None,
                title: format!("Repair PR #{}", landing.pr_number),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: OffsetDateTime::now_utc().unix_timestamp(),
            }
        };
        if !store
            .sqlite
            .reserve_repair(
                &incident.identity,
                landing.generation,
                &launcher,
                retry,
                session,
            )
            .map_err(repair_error)?
        {
            return Err(repair_error("landing changed before repair reservation"));
        }
        drop(lock);
        let context = crate::os_process::execution_context().map_err(repair_error)?;
        let bin = crate::os_process::pin_control_binary(&context.lf_bin);
        let argv = vec![
            bin.to_string_lossy().to_string(),
            "task".into(),
            "__repair".into(),
            incident.identity.clone(),
            launcher.to_string(),
        ];
        let name = format!(
            "lf-repair-{}-{}",
            &hex::encode(Sha256::digest(incident.identity.as_bytes()))[..16],
            reservation.retries + u32::from(retry)
        );
        let work = landing
            .task_id
            .as_ref()
            .map(|id| format!("task:{id}"))
            .unwrap_or_default();
        let environment = [
            (crate::lf::WORK_DECLARATION_ENV, work.as_str()),
            ("LF_USER_NAME", ""),
        ];
        if let Some((lock, lease)) = release {
            crate::os_process::start_lf_session_inheriting(
                &name,
                &landing.worktree,
                &argv,
                &environment,
                &|command| {
                    lock.inherit(command);
                    lease.inherit(command);
                },
            )
            .await
            .map_err(repair_error)?;
        } else {
            crate::os_process::start_lf_session_with_env(
                &name,
                &landing.worktree,
                &argv,
                &[
                    (crate::lf::WORK_DECLARATION_ENV, &work),
                    ("LF_USER_NAME", ""),
                ],
            )
            .await
            .map_err(repair_error)?;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let saved = store
                .sqlite
                .repair_reservation(&incident.identity)
                .map_err(repair_error)?;
            if saved.process.as_ref() != Some(&launcher) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(repair_error(
                    "repair startup acknowledgement timed out; reservation retained",
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}

/// A reservation still held by the caller was never handed to a repair worker;
/// a long-lived watcher must be able to retry its own unacknowledged launch.
fn repair_live(store: &SharedStore, process: Option<&crate::id::LfProcessId>) -> bool {
    process.is_some_and(|process| {
        Some(process) != crate::journal::current_lf_process_id().as_ref()
            && crate::journal::process_evidence(&store.sqlite, process)
                != crate::journal::ProcessIdentityEvidence::Dead
    })
}

fn repair_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Checks pending past the deadline become one synthetic failing check.
const CI_TIMEOUT: &str = "ci-timeout";

fn is_ci_timeout(incident: &CiIncident) -> bool {
    incident.failure_set == [CI_TIMEOUT]
}

fn ci_timeout_failure(head_sha: &str) -> LandingObservation {
    LandingObservation::Failing {
        head_sha: head_sha.to_string(),
        failing_checks: vec![CiCheck {
            name: CI_TIMEOUT.into(),
            url: None,
        }],
    }
}

/// A taskless checkout's work is the Processes started in it.
fn checkout_execution_blockers(store: &SharedStore, worktree: &Path) -> OpsResult<Vec<String>> {
    let processes: Vec<_> = store
        .sqlite
        .processes_since(0)
        .map_err(repair_error)?
        .into_iter()
        .filter(|process| process.cwd.as_deref() == worktree.to_str())
        .collect();
    super::task_automation::execution_blockers(&store.sqlite, &processes)
}

pub fn run_repair(identity: &str, launcher: &str) -> OpsResult<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let (store, landing, incident, captured) = runtime.block_on(async {
        let store = landing_store().await?;
        let id = store
            .sqlite
            .incident_landing(identity)
            .map_err(repair_error)?
            .ok_or_else(|| repair_error("repair incident has no landing"))?;
        let landing = store
            .get_pr_landing(&crate::pr_landing::PrLandingId::from_raw(id))
            .await
            .map_err(repair_error)?
            .ok_or_else(|| repair_error("repair landing disappeared"))?;
        let incident = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, Some(&landing.repo))
            .await
            .map_err(repair_error)?
            .into_iter()
            .find(|row| row.incident.identity == identity)
            .ok_or_else(|| repair_error("repair incident disappeared"))?
            .incident;
        let process = crate::journal::current_lf_process_id()
            .ok_or_else(|| repair_error("repair worker has no Process"))?;
        if !store
            .sqlite
            .handoff_repair(
                identity,
                &crate::id::LfProcessId::parse(launcher).map_err(repair_error)?,
                &process,
            )
            .map_err(repair_error)?
        {
            return Err(repair_error("repair reservation changed before startup"));
        }
        let session = store
            .sqlite
            .repair_reservation(identity)
            .map_err(repair_error)?
            .session
            .ok_or_else(|| repair_error("repair reservation has no Session"))?;
        let captured = store
            .sqlite
            .session(&session)
            .map_err(repair_error)?
            .ok_or_else(|| repair_error("repair Session disappeared"))?
            .captured;
        Ok::<_, OpsError>((store, landing, incident, captured))
    })?;
    let result = (|| {
        let observation = GithubLandingDriver {
            repairs: true,
            release: None,
        }
        .observe(&landing)?;
        let matching = match observation {
            LandingObservation::Failing {
                head_sha,
                failing_checks,
            } => {
                head_sha == incident.failed_head_sha
                    && ci_incident(&landing, &failing_checks, OffsetDateTime::now_utc()).identity
                        == incident.identity
            }
            LandingObservation::Pending { head_sha, .. } => {
                head_sha == incident.failed_head_sha && is_ci_timeout(&incident)
            }
            _ => false,
        };
        if !matching {
            return Err(repair_error(
                "PR evidence changed before repair; next check will reconcile it",
            ));
        }
        if is_ci_timeout(&incident) {
            store
                .sqlite
                .consume_timeout_rerun(
                    landing.id.as_str(),
                    OffsetDateTime::now_utc().unix_timestamp(),
                )
                .map_err(repair_error)?;
        }
        process_ci_fix(&store, &landing, &incident)
    })();
    let (conclusion, failure) = match &result {
        Ok(RepairConclusion::Published(_)) => (Some("published"), None),
        Ok(RepairConclusion::Blocked(reason)) => (Some("blocked"), Some(reason.clone())),
        Err(error) => (None, Some(error.to_string())),
    };
    store
        .sqlite
        .finish_repair(
            identity,
            &crate::journal::current_lf_process_id().expect("repair has a process"),
            failure.as_deref(),
            conclusion,
            captured,
        )
        .map_err(repair_error)?;
    if let Some(failure) = failure {
        Err(repair_error(failure))
    } else {
        Ok(())
    }
}

fn process_ci_fix(
    store: &SharedStore,
    landing: &PrLanding,
    incident: &CiIncident,
) -> OpsResult<RepairConclusion> {
    let skill = load_skill("ci-fix", &landing.worktree)
        .map_err(|error| OpsError::Message(format!("ci-fix skill not found: {error}")))?
        .content
        .ok_or_else(|| OpsError::Message("ci-fix skill has no content".to_string()))?;
    let urls = merge_gate_state(
        &landing.worktree,
        u64::from(landing.pr_number),
        &incident.failed_head_sha,
    )
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
    let mut prompt = format!(
        "{skill}\n\nRepair the exact recorded landing incident below. Start with `lf sync`. Repair and verify, then run `lf arm` to publish and enable auto-merge. Do not invoke `lf pr land` or wait for merge; a later finite check observes the merge; Task follow-through remains with its finishing Flow or operator.\n\nRepository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
        incident.repo,
        incident.pr_number,
        landing.branch,
        incident.failed_head_sha,
        checks,
        task_context,
    );
    if is_ci_timeout(incident) {
        prompt.push_str("\nThe recorded CI attempt exceeded 30 minutes. Diagnose pending or missing expected checks. This incident authorizes at most one timeout-only rerun; do not loop reruns or manufacture an empty commit. If no eligible run exists or the rerun still blocks, report the blocker.");
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
    let reservation = store
        .sqlite
        .repair_reservation(&incident.identity)
        .map_err(repair_error)?;
    let session_id = reservation
        .session
        .as_deref()
        .ok_or_else(|| repair_error("repair has no Session"))?;
    if reservation.retries > 0 {
        for artifact in store
            .sqlite
            .session_inputs(session_id)
            .map_err(repair_error)?
            .iter()
            .rev()
        {
            if let Ok((_, manifest)) =
                crate::session_record::resolve_manifest(&crate::store::lf_home_dir(), artifact)
            {
                if let Some(request) = manifest.process {
                    launch.agent = Some(request.agent);
                    launch.provider_account_id = request.account_id;
                    launch.system_prompt = request.system_prompt;
                    launch.task_prompt = request.task_prompt;
                    launch.skill_invocation = request.skill_invocation;
                    launch.max_turns = request.max_turns;
                    launch.write_scope = request.write_scope;
                    launch.execution_boundary = request.execution_boundary;
                    break;
                }
            }
        }
    }
    crate::agent::pin_provider_account_id_blocking(&mut launch)?;
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };
    let (harness, model) = crate::config::parse_agent(launch.agent());
    let session = store
        .sqlite
        .session(session_id)
        .map_err(repair_error)?
        .ok_or_else(|| repair_error("reserved Session disappeared"))?;
    let request = AgentProcessRequest::from_prepared(&launch, &capabilities);
    let context = crate::trace::PreparedTurnContext::from_prompts(
        &request.system_prompt,
        &request.task_prompt,
    );
    let capture = CaptureHandle::begin_reserved_with_context(
        SessionCaptureSpec {
            harness,
            model,
            surface: "headless".to_string(),
            cwd: landing.worktree.clone(),
            repo: Some(landing.worktree.clone()),
            worktree: Some(landing.worktree.clone()),
            skill: Some("ci-fix".to_string()),
            subjects: Vec::new(),
            flow: SessionFlowMembership::Independent,
            work: landing
                .task_id
                .clone()
                .map(|task| crate::session::SessionWork {
                    task_id: Some(task),
                    wave_id: None,
                    source: crate::session::WorkSource::Declared,
                }),
        },
        session.artifact_key.clone(),
        Some(request),
        &context,
        |_| store.sqlite.publish_capture(&session.id, session.captured),
    )
    .map_err(|error| OpsError::Message(error.to_string()))?;
    capture.record_input("initial", &launch.task_prompt);
    let process = ProcessConfig {
        auto: true,
        stream: true,
        capture: Some(capture.clone().into()),
        ..Default::default()
    };
    let result = run_agent(&launch, &process, &capabilities);
    let outcome = if matches!(&result, Ok(result) if result.exit_code == 0) {
        "completed"
    } else {
        "failed"
    };
    capture
        .finish(outcome)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let conclusion = capture
        .final_answer()
        .ok()
        .flatten()
        .map(|answer| answer.text)
        .unwrap_or_else(|| {
            format!(
                "No repair conclusion recorded for input {}; retained artifacts: {}",
                capture.artifact_key(),
                capture.artifact_dir().display()
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
    Ok(conclusion)
}

fn ci_incident(landing: &PrLanding, checks: &[CiCheck], now: OffsetDateTime) -> CiIncident {
    let mut failure_set = checks
        .iter()
        .map(|check| check.name.clone())
        .collect::<Vec<_>>();
    failure_set.sort();
    failure_set.dedup();
    let mut attempts = checks
        .iter()
        .map(|check| (&check.name, &check.url))
        .collect::<Vec<_>>();
    attempts.sort();
    attempts.dedup();
    let mut digest = Sha256::new();
    for (name, url) in attempts {
        digest.update(name.as_bytes());
        digest.update([0]);
        if let Some(url) = url {
            digest.update(url.as_bytes());
        }
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
    Err(OpsError::DeliveryHeld(reason))
}

async fn run_driver_operation<T, F>(
    ownership: &Arc<File>,
    label: &'static str,
    operation: F,
) -> OpsResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> OpsResult<T> + Send + 'static,
{
    let ownership = Arc::clone(ownership);
    let operation = tokio::task::spawn_blocking(move || {
        let _ownership = ownership;
        operation()
    });
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| OpsError::Message(format!("landing {label} exceeded 10 seconds")))?
        .map_err(|error| OpsError::Message(format!("landing {label} panicked: {error}")))?
}

fn lock_landing(landing: &PrLanding) -> OpsResult<Option<Arc<File>>> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(crate::git::absolute_git_dir(&landing.worktree)?.join("lf-pr-landing.lock"))?;
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
        run_driver_operation(ownership, "observation", {
            let driver = Arc::clone(&driver);
            let landing = landing.clone();
            move || driver.observe(&landing)
        })
        .await?
    };
    record_observed_head(store, landing, &observed).await?;
    if let LandingObservation::Pending { head_sha, attempt } = &observed {
        let allowance = load_config_or_default(Some(&landing.worktree))
            .automation
            .timeout_reruns;
        if let Some(allowed) = store
            .sqlite
            .ci_timeout(
                landing.id.as_str(),
                head_sha,
                attempt.as_deref(),
                OffsetDateTime::now_utc().unix_timestamp(),
                allowance,
            )
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            if !allowed {
                return block_landing(
                    store,
                    landing,
                    "CI timeout rerun allowance exhausted; inspect the pending checks".into(),
                )
                .await;
            }
            observed = ci_timeout_failure(head_sha);
        }
    }
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
        if !driver.repairs() {
            return Ok(());
        }
        observed = run_driver_operation(ownership, "CI confirmation", {
            let driver = Arc::clone(&driver);
            let landing = landing.clone();
            move || driver.observe(&landing)
        })
        .await?;
        if is_ci_timeout(&incident) {
            if let LandingObservation::Pending { head_sha, .. } = &observed {
                observed = ci_timeout_failure(head_sha);
            }
        }
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
        LandingObservation::Passing { head_sha } => {
            store
                .mark_ci_incidents_green(&landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
            resume_watching(store, landing, head_sha).await
        }
        LandingObservation::Unarmed { .. } => {
            block_landing(
                store,
                landing,
                "pull request has no auto-merge request; run lf arm to resume landing".into(),
            )
            .await
        }
        LandingObservation::Pending { head_sha, .. } | LandingObservation::Queued { head_sha } => {
            resume_watching(store, landing, head_sha).await
        }
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
            if store
                .sqlite
                .ci_response_complete(&incident.identity)
                .map_err(|error| OpsError::Message(error.to_string()))?
            {
                return block_landing(
                    store,
                    landing,
                    "CI incident already completed a repair; waiting for changed evidence".into(),
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
            let identity = incident.identity.clone();
            let repair = run_driver_operation(ownership, "ci-fix", {
                let driver = Arc::clone(&driver);
                let landing = landing.clone();
                move || driver.repair(&landing, &incident)
            })
            .await;
            if let Err(error) = repair {
                if matches!(error, OpsError::CheckoutBusy(_)) {
                    // Another observer's busy checkout is not a delivery
                    // failure. In particular, a watcher must not stop a land
                    // command that is itself about to admit this repair.
                    persist_landing_state(
                        store,
                        landing,
                        PrLandingState::Watching,
                        head_sha,
                        None,
                        None,
                    )
                    .await?;
                    return Err(error);
                }
                return block_landing(store, landing, format!("ci-fix blocked: {error}")).await;
            }
            store
                .record_ci_response(&identity, &landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
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

/// An armed head whose checks no longer fail has outlived its recorded block.
async fn resume_watching(
    store: &SharedStore,
    landing: &mut PrLanding,
    head_sha: String,
) -> OpsResult<()> {
    if landing.state != PrLandingState::Blocked {
        return Ok(());
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
    Ok(())
}

pub(crate) async fn landing_store() -> OpsResult<SharedStore> {
    let config = storage_config_from_env()
        .map_err(|error| OpsError::Message(format!("resolve landing store: {error}")))?;
    open_store(&config)
        .await
        .map(Arc::new)
        .map_err(|error| OpsError::Message(format!("open landing store: {error}")))
}

async fn cleanup_landed_pr(store: &SharedStore, landing: &PrLanding) -> OpsResult<()> {
    if let Some(task_id) = &landing.task_id {
        let task = store
            .get_task(task_id)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message(format!("landing Task {task_id} disappeared")))?;
        if crate::ops::task::task_work_status(store, &task).await?
            == crate::durable::WorkStatus::Done
        {
            return crate::ops::task::cleanup_completed_task(store, &task).await;
        }
        eprintln!("Task {} has merged; retained its checkout for follow-through. Inspect remaining work with `lf task status {}`.", task.plan.identifier, task.plan.identifier);
        return Ok(());
    }
    // Only a Flow still being driven needs the checkout; a stopped one is history.
    if store
        .sqlite
        .flows_at(&landing.worktree)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .iter()
        .any(|flow| {
            flow.process.completed_at.is_none()
                && crate::journal::process_evidence(&store.sqlite, flow.id())
                    != crate::journal::ProcessIdentityEvidence::Dead
        })
    {
        eprintln!("PR merged; retained its checkout for the Flow that landed it.");
        return Ok(());
    }
    if crate::git::worktrees::is_persistent_worktree(&landing.worktree)? {
        match crate::ops::sync::restart_landed_persistent(&landing.worktree) {
            Ok(true) => eprintln!("PR merged; restarted persistent branch from the default branch."),
            Ok(false) => eprintln!(
                "PR merged; retained persistent checkout with unmerged commits. Run lf sync before the next publication."
            ),
            Err(error) => eprintln!(
                "PR merged; retained persistent checkout. Run lf sync before the next publication: {error}"
            ),
        }
        return Ok(());
    }
    let _admission = store
        .sqlite
        .lock_checkout(&landing.worktree)
        .map_err(repair_error)?;
    // A conversation is history; only a running Process holds the checkout.
    if let Some(reason) = checkout_execution_blockers(store, &landing.worktree)?
        .into_iter()
        .next()
    {
        eprintln!(
            "PR merged; retained its checkout: {reason}. Use lf wt delete after it finishes."
        );
        return Ok(());
    }
    let repo = crate::git::worktrees::main_repo_root(&landing.worktree)?;
    if std::fs::canonicalize(&repo)? == std::fs::canonicalize(&landing.worktree)? {
        eprintln!("PR merged; retained the primary checkout and branch.");
        return Ok(());
    }
    let deletion =
        crate::ops::wt::prepare_landed_delete(&repo, &landing.branch, &landing.observed_head_sha)?;
    crate::ops::wt::apply_delete(deletion, &crate::ops::NullProgress).map_err(|error| {
        OpsError::Message(format!(
            "PR merged, but cleanup failed: {error}; retry `lf wt delete {}`",
            landing.branch,
        ))
    })
}

async fn create_landing(
    store: &SharedStore,
    repo: &Path,
    options: &LandOptions,
    pr: &PrInfo,
) -> OpsResult<PrLanding> {
    let (worktree, _) = super::land::resolve_repos(repo, options.worktree.as_deref())?;
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
    let task_id = match task {
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
            Some(task.id)
        }
        None => None,
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

pub(crate) fn reconcile_armed_pr(
    repo: &Path,
    options: &LandOptions,
    pr: &PrInfo,
    lock: &super::release_lock::ReleaseLock,
    lease: &crate::git::WorktreeLease,
) -> OpsResult<PrLanding> {
    let landing = record_armed_pr(repo, options, pr)?;
    let release = Some(Arc::new((lock.try_clone()?, lease.try_clone()?)));
    tokio::runtime::Runtime::new()?.block_on(async {
        reconcile_pr_landing(
            landing_store().await?,
            landing,
            Arc::new(GithubLandingDriver {
                repairs: true,
                release,
            }),
        )
        .await
    })
}

/// One landing check that may start a ci-fix: the entry point `lf ci watch`
/// shares with a release's own landing. The landing lock, generation claim and
/// incident reservation keep a second caller from repeating the repair.
pub(crate) async fn repair_landing(store: SharedStore, landing: PrLanding) -> OpsResult<PrLanding> {
    reconcile_pr_landing(
        store,
        landing,
        Arc::new(GithubLandingDriver {
            repairs: true,
            release: None,
        }),
    )
    .await
}

/// Whether a repair worker for this landing is running now.
pub(crate) fn repair_running(store: &SharedStore, landing: &PrLanding) -> OpsResult<bool> {
    Ok(store
        .sqlite
        .landing_repair_processes(&landing.id)
        .map_err(repair_error)?
        .iter()
        .any(|process| repair_live(store, Some(process))))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeliveryCheck {
    pub checked_at: i64,
    pub errors: Vec<String>,
}

/// Observe and repair this PR until it merges; each check releases its claim.
pub fn wait_for_merge(repo: &Path, options: &LandOptions, pr: &PrInfo) -> OpsResult<()> {
    let initial = record_armed_pr(repo, options, pr)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let store = landing_store().await?;
        wait_for_landing(&store, &initial.id).await
    });
    runtime.shutdown_background();
    result
}

async fn wait_for_landing(
    store: &SharedStore,
    id: &crate::pr_landing::PrLandingId,
) -> OpsResult<()> {
    tokio::time::timeout(Duration::from_secs(30 * 60), async {
        loop {
            let landing = store
                .get_pr_landing(id)
                .await
                .map_err(repair_error)?
                .ok_or_else(|| repair_error("landing disappeared while waiting"))?;
            let observed = reconcile_pr_landing(
                store.clone(),
                landing,
                Arc::new(GithubLandingDriver {
                    repairs: true,
                    release: None,
                }),
            )
            .await?;
            match observed.state {
                PrLandingState::Merged => return Ok(()),
                PrLandingState::Closed => {
                    return Err(OpsError::DeliveryHeld(
                        "Pull request closed without merging; follow-through has not run".into(),
                    ))
                }
                PrLandingState::Blocked => {
                    return Err(OpsError::DeliveryHeld(
                        observed
                            .blocked_reason
                            .unwrap_or_else(|| "Landing is blocked".into()),
                    ))
                }
                PrLandingState::Watching | PrLandingState::Repairing => {}
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    })
    .await
    .map_err(|_| {
        OpsError::DeliveryHeld(
            "Landing wait timed out; merge intent retained. Run lf pr reconcile or wait again."
                .into(),
        )
    })?
}

pub fn reconcile_repository(repo: &Path) -> OpsResult<DeliveryCheck> {
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let store = landing_store().await?;
        let mut report = DeliveryCheck {
            checked_at: OffsetDateTime::now_utc().unix_timestamp(),
            errors: Vec::new(),
        };
        reconcile_repository_async(
            repo,
            &store,
            tokio::time::Instant::now() + Duration::from_secs(45),
            &mut report.errors,
        )
        .await?;
        Ok(report)
    });
    // Timed-out observations retain their landing locks until the worker exits.
    runtime.shutdown_background();
    result
}

async fn reconcile_repository_async(
    repo: &Path,
    store: &SharedStore,
    deadline: tokio::time::Instant,
    errors: &mut Vec<String>,
) -> OpsResult<()> {
    let repo = crate::repository::RepoId::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let mut landings = store
        .pending_pr_landings(repo.as_str())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    landings.sort_by_key(|landing| landing.updated_at);
    for landing in landings {
        if tokio::time::Instant::now() >= deadline {
            errors.push("delivery coverage overdue: pass deadline exceeded".into());
            break;
        }
        let number = landing.pr_number;
        let id = landing.id.clone();
        match tokio::time::timeout_at(
            deadline,
            reconcile_pr_landing(
                store.clone(),
                landing,
                Arc::new(GithubLandingDriver {
                    repairs: false,
                    release: None,
                }),
            ),
        )
        .await
        {
            Ok(Ok(landing)) => {
                if landing.state == PrLandingState::Merged {
                    if let Err(error) = cleanup_landed_pr(store, &landing).await {
                        errors.push(format!("PR #{number}: {error}"));
                    }
                }
            }
            Ok(Err(error)) => {
                let saved = store.get_pr_landing(&id).await.map_err(repair_error)?;
                if !saved.is_some_and(|landing| {
                    landing.state == PrLandingState::Blocked
                        && landing.blocked_reason.as_deref() == Some(&error.to_string())
                }) {
                    errors.push(format!("PR #{number}: {error}"));
                }
            }
            Err(_) => {
                errors.push(format!("PR #{number}: observation deadline exceeded"));
                break;
            }
        }
    }
    for mut task in super::task_automation::repository_tasks(store, &repo).await? {
        if tokio::time::Instant::now() >= deadline {
            errors.push("Task delivery coverage overdue: pass deadline exceeded".into());
            break;
        }
        match tokio::time::timeout_at(
            deadline,
            super::task::reconcile_delivered_task(store, &mut task),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(format!("{}: {error}", task.plan.identifier)),
            Err(_) => {
                errors.push(format!(
                    "{}: delivery observation deadline exceeded",
                    task.plan.identifier
                ));
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    use super::{
        classify_github_observation, lock_landing, reconcile_pr_landing, wait_for_landing,
        LandingDriver, LandingObservation,
    };
    use crate::ops::error::{OpsError, OpsResult};
    use crate::ops::pr::{MergeRequest, PrInfo};
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::task::{CiCheck, CiIncident};
    use std::path::PathBuf;

    fn github_landing_fixture() -> (PrLanding, PrInfo) {
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 1323,
                worktree: PathBuf::from("/unused"),
                branch: "jack/landing".to_string(),
                task_id: None,
                requested_head_sha: "pr-head".to_string(),
            },
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        let pr = PrInfo {
            url: "https://github.com/loopflowstudio/loopflow/pull/1323".to_string(),
            number: u64::from(landing.pr_number),
            state: "open".to_string(),
            branch: landing.branch.clone(),
            merge_commit: None,
            merged_at: None,
            head_sha: Some("pr-head".to_string()),
            merge_state: Some("behind".to_string()),
        };
        (landing, pr)
    }

    #[test]
    fn queued_landing_waits_for_integration_instead_of_requiring_sync() {
        let (landing, mut pr) = github_landing_fixture();
        for merge_state in ["behind", "dirty", "clean"] {
            pr.merge_state = Some(merge_state.to_string());
            let observed = classify_github_observation(
                &landing,
                pr.clone(),
                Some(MergeRequest::Queued("queue-entry".to_string())),
            )
            .unwrap();
            assert_eq!(
                observed,
                LandingObservation::Queued {
                    head_sha: "pr-head".to_string()
                }
            );
        }
    }

    #[test]
    fn dequeued_landing_resumes_ordinary_integration_and_authorization_checks() {
        let (landing, pr) = github_landing_fixture();
        let observed =
            classify_github_observation(&landing, pr.clone(), Some(MergeRequest::Auto)).unwrap();
        assert!(
            matches!(observed, LandingObservation::Failing { failing_checks, .. }
            if failing_checks[0].name == "required-integration")
        );
        assert_eq!(
            classify_github_observation(&landing, pr, None).unwrap(),
            LandingObservation::Unarmed {
                head_sha: "pr-head".to_string()
            }
        );
    }

    #[test]
    fn queue_membership_never_overrides_a_confirmed_merge_or_closure() {
        let (landing, mut pr) = github_landing_fixture();
        pr.state = "merged".to_string();
        pr.merge_commit = Some("integrated-head".to_string());
        assert_eq!(
            classify_github_observation(
                &landing,
                pr.clone(),
                Some(MergeRequest::Queued("queue-entry".to_string()))
            )
            .unwrap(),
            LandingObservation::Merged {
                head_sha: "pr-head".to_string(),
                merge_commit: "integrated-head".to_string()
            }
        );
        pr.state = "closed".to_string();
        assert_eq!(
            classify_github_observation(&landing, pr, None).unwrap(),
            LandingObservation::Closed {
                head_sha: "pr-head".to_string()
            }
        );
    }

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

    #[tokio::test(start_paused = true)]
    async fn landing_wait_timeout_holds_without_losing_merge_intent() {
        let (_directory, store, landing) = fixture().await;
        // Another observer owns this landing throughout the wait. No provider
        // call occurs; the real waiting loop expires under Tokio's paused clock.
        let _owner = lock_landing(&landing).unwrap().unwrap();
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        let before = tokio::time::Instant::now();
        let result = wait_for_landing(&store, &landing.id).await;
        assert!(
            matches!(result, Err(OpsError::DeliveryHeld(reason)) if reason.contains("timed out"))
        );
        assert!(before.elapsed() >= std::time::Duration::from_secs(30 * 60));
        assert_eq!(
            store.get_pr_landing(&landing.id).await.unwrap().unwrap(),
            saved
        );
    }

    #[tokio::test]
    async fn landing_wait_missing_record_is_a_failure_not_a_hold() {
        let (_directory, store, _) = fixture().await;
        let missing = crate::pr_landing::PrLandingId::from_raw("missing");
        let result = wait_for_landing(&store, &missing).await;
        assert!(matches!(result, Err(OpsError::Message(reason)) if reason.contains("disappeared")));
    }

    #[tokio::test]
    async fn ci_deadline_tracks_attempts_and_keeps_the_timeout_rerun_bound() {
        let (_directory, store, landing) = fixture().await;
        let id = landing.id.as_str();
        assert_eq!(
            store.sqlite.ci_timeout(id, "head", None, 100, 1).unwrap(),
            None
        );
        assert_eq!(
            store.sqlite.ci_timeout(id, "head", None, 1899, 1).unwrap(),
            None
        );
        assert_eq!(
            store.sqlite.ci_timeout(id, "head", None, 1900, 1).unwrap(),
            Some(true)
        );
        assert_eq!(
            store
                .sqlite
                .ci_timeout(id, "head", Some("run-1/attempt-2"), 1901, 1)
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .sqlite
                .ci_timeout(id, "head", Some("run-1/attempt-2"), 3701, 1)
                .unwrap(),
            Some(true)
        );
        store.sqlite.consume_timeout_rerun(id, 3701).unwrap();
        assert_eq!(
            store
                .sqlite
                .ci_timeout(id, "head", Some("run-1/attempt-3"), 3702, 1)
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .sqlite
                .ci_timeout(id, "head", Some("run-1/attempt-3"), 5502, 1)
                .unwrap(),
            Some(false)
        );
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
                attempt: None,
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

    /// The scheduled repository check: it sees the failure, the watcher repairs it.
    struct ObservingDriver(Arc<FakeDriver>);

    impl LandingDriver for ObservingDriver {
        fn observe(&self, landing: &PrLanding) -> OpsResult<LandingObservation> {
            self.0.observe(landing)
        }
        fn repair(&self, landing: &PrLanding, incident: &CiIncident) -> OpsResult<()> {
            self.0.repair(landing, incident)
        }
        fn repairs(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn an_observing_check_records_the_failure_and_leaves_the_repair() {
        let (_directory, store, landing) = fixture().await;
        let fake = driver(vec![failure("head"), failure("head"), failure("head")]);
        let observing = Arc::new(ObservingDriver(fake.clone()));
        let watching = reconcile_pr_landing(store.clone(), landing, observing)
            .await
            .unwrap();
        assert_eq!(watching.state, PrLandingState::Watching);
        assert_eq!(*fake.repairs.lock().unwrap(), 0);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].incident.responded_at.is_none());

        // The watcher's check then repairs that same incident, once.
        reconcile_pr_landing(store.clone(), watching, fake.clone())
            .await
            .unwrap();
        assert_eq!(*fake.repairs.lock().unwrap(), 1);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].incident.responded_at.is_some());
    }

    #[tokio::test]
    async fn recovered_checks_clear_a_block_without_another_repair() {
        let (_directory, store, landing) = fixture().await;
        let driver = Arc::new(FakeDriver {
            observations: Mutex::new(
                vec![
                    failure("head"),
                    failure("head"),
                    LandingObservation::Passing {
                        head_sha: "head".into(),
                    },
                ]
                .into(),
            ),
            repairs: Mutex::new(0),
            repair_error: Some("needs a secret".into()),
        });
        assert!(
            reconcile_pr_landing(store.clone(), landing.clone(), driver.clone())
                .await
                .is_err()
        );
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        let recovered = reconcile_pr_landing(store, blocked, driver.clone())
            .await
            .unwrap();
        assert_eq!(recovered.state, PrLandingState::Watching);
        assert!(recovered.blocked_reason.is_none());
        assert_eq!(*driver.repairs.lock().unwrap(), 1);
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
