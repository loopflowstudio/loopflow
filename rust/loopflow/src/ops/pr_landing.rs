//! Finite pull-request delivery reconciliation over durable landing intents.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::agent::{run_agent, AgentConfig, ProcessConfig};
use crate::config::load_config_or_default;
use crate::git::current_branch;
use crate::graph::load_skill;
use crate::pr_landing::{
    LandingPlacement, LandingSupervisor, NewPrLanding, PrLanding, PrLandingState,
    SUPERVISOR_STALE_AFTER,
};
use crate::session_record::{
    AgentProcessRequest, CaptureHandle, SessionCaptureSpec, SessionGraphMembership,
};
use crate::store::sqlite::ci_incidents::RepairAttempt;
use crate::store::{open_store, storage_config_from_env, SharedStore};
use crate::work::task::{CiCheck, CiIncident, TaskEventKind};

use super::error::{OpsError, OpsResult};
use super::land::LandOptions;
use super::pr::{
    enable_auto_merge, enqueue_pr, merge_gate_state, merge_needs_integration, observe_pr_merge,
    published_pr_copy, MergeRequest, PrInfo,
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
    /// GitHub dropped the merge request for a reason a re-arm answers: a queue
    /// timeout, a manual removal or disable, or no recorded event.
    Dequeued {
        head_sha: String,
        reason: Option<String>,
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
}

impl LandingObservation {
    fn head_sha(&self) -> &str {
        match self {
            Self::Dequeued { head_sha, .. }
            | Self::Pending { head_sha }
            | Self::Passing { head_sha }
            | Self::Failing { head_sha, .. }
            | Self::Merged { head_sha, .. }
            | Self::Closed { head_sha } => head_sha,
        }
    }
}

/// How one check reads its PR.
pub(crate) type ObservePr = Arc<dyn Fn(&PrLanding) -> OpsResult<LandingObservation> + Send + Sync>;
/// How one check starts a ci-fix. A check without one (`lf pr reconcile`) still
/// records the failure and leaves the repair to the landing's own process.
pub(crate) type RepairCi = Arc<dyn Fn(&PrLanding, &CiIncident) -> OpsResult<()> + Send + Sync>;
/// How one check asks GitHub to merge the requested head again.
pub(crate) type RearmPr = Arc<dyn Fn(&PrLanding, &str) -> OpsResult<()> + Send + Sync>;

/// Before attempt `n` (n ≥ 2) of the same incident starts: 1, 2, 4… minutes, at most 30.
pub(crate) fn repair_backoff(attempt: u32) -> i64 {
    (60_i64 << attempt.saturating_sub(2).min(5)).min(30 * 60)
}

fn observe_github(landing: &PrLanding) -> OpsResult<LandingObservation> {
    let read = || {
        let observation = observe_pr_merge(&landing.worktree, u64::from(landing.pr_number))?;
        classify_github_observation(
            landing,
            observation.pr,
            observation.request,
            observation.merge_exit,
        )
    };
    match read()? {
        // GitHub can answer before a fresh arm is visible. An unexplained
        // missing request is read again before anything acts on it.
        LandingObservation::Dequeued { reason: None, .. } => {
            std::thread::sleep(Duration::from_secs(5));
            read()
        }
        observed => Ok(observed),
    }
}

/// Put exactly the requested head back on its way to merge: re-enter the queue
/// when auto-merge survived the removal, otherwise re-arm with its published copy.
fn rearm_github(landing: &PrLanding, head_sha: &str) -> OpsResult<()> {
    let observation = observe_pr_merge(&landing.worktree, u64::from(landing.pr_number))?;
    match observation.request {
        Some(MergeRequest::AwaitingQueue) => {
            enqueue_pr(&landing.worktree, &observation.node_id, head_sha)
        }
        Some(MergeRequest::Auto | MergeRequest::Queued(_)) => Ok(()),
        None => {
            let copy = published_pr_copy(&landing.worktree, head_sha)?;
            enable_auto_merge(
                &landing.worktree,
                u64::from(landing.pr_number),
                copy.as_ref(),
                head_sha,
                &|_| {},
            )
        }
    }
}

fn ci_fix(
    release: Option<Arc<(super::release_lock::ReleaseLock, crate::git::WorktreeLease)>>,
) -> RepairCi {
    Arc::new(move |landing, incident| admit_ci_fix(landing, incident, release.as_deref()))
}

fn classify_github_observation(
    landing: &PrLanding,
    pr: PrInfo,
    request: Option<MergeRequest>,
    merge_exit: Option<String>,
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
            Ok(LandingObservation::Pending { head_sha })
        }
        // A queue removal can leave auto-merge on; GitHub then never re-enters.
        _ if request.is_none()
            || (matches!(request, Some(MergeRequest::AwaitingQueue)) && merge_exit.is_some()) =>
        {
            Ok(classify_dequeue(
                landing,
                head_sha,
                pr.merge_state.as_deref(),
                merge_exit,
            ))
        }
        _ if merge_needs_integration(pr.merge_state.as_deref(), request.as_ref()) => {
            Ok(LandingObservation::Failing {
                head_sha,
                failing_checks: vec![CiCheck {
                    name: "required-integration".into(),
                    url: None,
                }],
            })
        }
        _ => match merge_gate_state(&landing.worktree, u64::from(landing.pr_number), &head_sha)? {
            Some(reading) if reading.failing => Ok(LandingObservation::Failing {
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
            Some(reading) if reading.pending => Ok(LandingObservation::Pending { head_sha }),
            Some(_) => Ok(LandingObservation::Passing { head_sha }),
            None => Ok(LandingObservation::Pending { head_sha }),
        },
    }
}

/// GitHub dropped the merge request. A failed or conflicting queue run, and a
/// head nobody armed, need a repair; anything else is re-armed as it was.
fn classify_dequeue(
    landing: &PrLanding,
    head_sha: String,
    merge_state: Option<&str>,
    reason: Option<String>,
) -> LandingObservation {
    let dirty = merge_state.is_some_and(|state| state.eq_ignore_ascii_case("dirty"));
    let check = match reason.as_deref() {
        Some("failed_checks") => Some("merge-queue"),
        Some("merge_conflict") => Some("required-integration"),
        _ if dirty => Some("required-integration"),
        _ if head_sha != landing.requested_head_sha => Some("auto-merge"),
        _ => None,
    };
    match check {
        Some(name) => LandingObservation::Failing {
            head_sha,
            failing_checks: vec![CiCheck {
                name: name.into(),
                url: None,
            }],
        },
        None => LandingObservation::Dequeued { head_sha, reason },
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
                format!("repair attempt finished at {finished}; the next check starts another")
            })));
        }
        if repair_live(&store, reservation.process.as_ref()) {
            return Ok(());
        }
        if let Some(session) = &reservation.session {
            // An attached Session's AgentProcess is judged by its own OS identity.
            let bound = store
                .sqlite
                .session_attachment(session)
                .map_err(repair_error)?
                .is_some();
            let agent = store
                .sqlite
                .agent_process_identity(session)
                .map_err(repair_error)?;
            if bound
                && agent.is_some_and(|(pid, started)| {
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
            if store.sqlite.task_state(task_id).map_err(repair_error)?
                == crate::durable::TaskState::Paused
            {
                return Err(repair_error("Task is paused; CI repair is held"));
            }
            let local = store.local_machine().await.map_err(repair_error)?;
            let machine = store
                .sqlite
                .task_checkout(task_id)
                .map_err(repair_error)?
                .and_then(|checkout| checkout.machine_id)
                .ok_or_else(|| repair_error("Task checkout Machine is unknown"))?;
            if machine != local.id {
                return Err(repair_error(format!(
                    "Task checkout belongs to Machine {machine}"
                )));
            }
            if let Some(reason) =
                super::task_execution::task_execution_blockers(&store.sqlite, &task.id)?
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
                id: format!("session_{}", uuid::Uuid::new_v4().simple()).into(),
                captured: None,
                artifact_key: Some(crate::session_record::new_artifact_key()),
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
                graph_lf_process_id: None,
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
        let argv = vec![
            context.lf_bin.to_string_lossy().to_string(),
            "task".into(),
            "__repair".into(),
            incident.identity.clone(),
            launcher.to_string(),
        ];
        let work = landing
            .task_id
            .as_ref()
            .map(|id| format!("task:{id}"))
            .unwrap_or_default();
        let environment = [
            (crate::lf::WORK_DECLARATION_ENV, work.as_str()),
            ("LF_USER_NAME", ""),
        ];
        let owner = crate::os_process::start_lf_session(
            &landing.worktree,
            &argv,
            &environment,
            &|command| {
                if let Some((lock, lease)) = release {
                    lock.inherit(command);
                    lease.inherit(command);
                }
            },
        )
        .map_err(repair_error)?;
        loop {
            let owner_exited = crate::journal::process_evidence(&store.sqlite, &owner)
                == crate::journal::ProcessIdentityEvidence::Dead;
            let saved = store
                .sqlite
                .repair_reservation(&incident.identity)
                .map_err(repair_error)?;
            if saved.process.as_ref() != Some(&launcher) {
                return Ok(());
            }
            if owner_exited {
                return Err(repair_error(
                    "repair owner exited before claiming its reservation; inspect transport.log",
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

/// A taskless checkout's work is the Processes started in it.
fn checkout_execution_blockers(store: &SharedStore, worktree: &Path) -> OpsResult<Vec<String>> {
    let processes: Vec<_> = store
        .sqlite
        .processes_since(0)
        .map_err(repair_error)?
        .into_iter()
        .filter(|process| process.cwd.as_deref() == worktree.to_str())
        .collect();
    super::task_execution::execution_blockers(&store.sqlite, &processes)
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
        let observation = observe_github(&landing)?;
        let matching = match observation {
            LandingObservation::Failing {
                head_sha,
                failing_checks,
            } => {
                head_sha == incident.failed_head_sha
                    && ci_incident(&landing, &failing_checks, OffsetDateTime::now_utc()).identity
                        == attempt_base(&incident.identity)
            }
            _ => false,
        };
        if !matching {
            return Err(repair_error(
                "PR evidence changed before repair; next check will reconcile it",
            ));
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
        "{skill}\n\nRepair the exact recorded landing incident below. Start with `lf sync`. Repair and verify, then run `lf arm` to publish and enable auto-merge. Do not invoke `lf pr land` or wait for merge; a later finite check observes the merge; Task completion remains with its Workflow or conversation.\n\nRepository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
        incident.repo,
        incident.pr_number,
        landing.branch,
        incident.failed_head_sha,
        checks,
        task_context,
    );
    for (name, meaning) in [
        ("merge-queue", "GitHub removed the PR from its merge queue because checks failed on the integrated commit. Inspect that run with `gh run list --event merge_group`; rerun a flake by re-arming, or fix a real failure."),
        ("auto-merge", "The PR head is published but not armed. Verify it, then `lf arm`."),
    ] {
        if incident.failure_set.iter().any(|check| check == name) {
            prompt.push_str(&format!("\n{name}: {meaning}"));
        }
    }
    let earlier = earlier_attempts(store, &incident.identity)?;
    if !earlier.is_empty() {
        prompt.push_str(&format!(
            "\n\nEarlier repair attempts left this same failure in place:\n{}",
            earlier.join("\n")
        ));
    }
    prompt.push_str(
        "\n\nReturn your final answer as one JSON object: {\"status\":\"published\",\"summary\":\"Published head, PR URL, auto-merge state, and checks run\"}. If this attempt cannot repair it, return {\"status\":\"blocked\",\"summary\":\"What you tried and what stopped you\"}; the landing tries again after a backoff with your summary. No one is available to answer questions. Continue resolving repairable failures yourself. Do not mark an unpublished repair as published. This result belongs only in your final answer; create no result file.",
    );
    let config = load_config_or_default(Some(&landing.worktree));
    let mut launch = AgentConfig {
        chrome: config.chrome,
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
                    launch.conversation_context = request.conversation_context;
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

    let (harness, model) = crate::config::parse_agent(launch.agent());
    let session = store
        .sqlite
        .session(session_id)
        .map_err(repair_error)?
        .ok_or_else(|| repair_error("reserved Session disappeared"))?;
    let request = AgentProcessRequest::from_prepared(&launch);
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
            graph: SessionGraphMembership::Independent,
            work: landing
                .task_id
                .clone()
                .map(|task| crate::session::SessionWork {
                    task_id: Some(task),
                    wave_id: None,
                    source: crate::session::WorkSource::Declared,
                }),
        },
        session.input_key().map_err(repair_error)?.clone(),
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
    let result = run_agent(&launch, &process);
    let outcome = if matches!(&result, Ok(result) if result.exit_code == 0) {
        "completed"
    } else {
        "failed"
    };
    capture
        .finish(outcome)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let conclusion = capture.final_answer().ok().flatten().unwrap_or_else(|| {
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

/// One line per earlier attempt of this incident, oldest first.
fn earlier_attempts(store: &SharedStore, identity: &str) -> OpsResult<Vec<String>> {
    let base = attempt_base(identity);
    let current = attempt_number(identity);
    (1..current)
        .map(|attempt| {
            let reservation = store
                .sqlite
                .repair_reservation(&attempt_identity(base, attempt))
                .map_err(repair_error)?;
            Ok(format!(
                "- attempt {attempt}: {}{}",
                reservation.conclusion.as_deref().unwrap_or("failed"),
                reservation
                    .error
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ))
        })
        .collect()
}

/// Attempt `n` of an incident is a row of its own: `<identity>#n` for n ≥ 2.
fn attempt_identity(base: &str, attempt: u32) -> String {
    if attempt <= 1 {
        base.to_string()
    } else {
        format!("{base}#{attempt}")
    }
}

fn attempt_base(identity: &str) -> &str {
    identity.rsplit_once('#').map_or(identity, |(base, _)| base)
}

fn attempt_number(identity: &str) -> u32 {
    identity
        .rsplit_once('#')
        .and_then(|(_, attempt)| attempt.parse().ok())
        .unwrap_or(1)
}

#[derive(Debug)]
enum NextAttempt {
    Due(Box<CiIncident>),
    BackingOff { attempt: u32, at: i64 },
}

/// The first attempt of this incident that has not finished, once its backoff
/// has elapsed. A finished attempt never hands the landing back to a person.
fn next_attempt(
    store: &SharedStore,
    landing: &PrLanding,
    base: CiIncident,
    now: OffsetDateTime,
) -> OpsResult<NextAttempt> {
    let retries = load_config_or_default(Some(&landing.worktree))
        .automation
        .retries;
    let mut ended = None;
    for attempt in 1.. {
        let mut incident = base.clone();
        incident.identity = attempt_identity(&base.identity, attempt);
        match store
            .sqlite
            .repair_attempt(&incident.identity, retries)
            .map_err(repair_error)?
        {
            Some(RepairAttempt::Finished { ended_at }) => ended = Some(ended_at),
            Some(RepairAttempt::Open) => return Ok(NextAttempt::Due(Box::new(incident))),
            None => {
                let at = ended.map_or(0, |ended| ended + repair_backoff(attempt));
                return Ok(if at <= now.unix_timestamp() {
                    NextAttempt::Due(Box::new(incident))
                } else {
                    NextAttempt::BackingOff { attempt, at }
                });
            }
        }
    }
    unreachable!("attempt numbers are unbounded")
}

fn ci_incident(landing: &PrLanding, checks: &[CiCheck], now: OffsetDateTime) -> CiIncident {
    let mut attempts = checks
        .iter()
        .map(|check| (&check.name, &check.url))
        .collect::<Vec<_>>();
    attempts.sort();
    attempts.dedup();
    let mut failure_set = attempts
        .iter()
        .map(|(name, _)| (*name).clone())
        .collect::<Vec<_>>();
    failure_set.dedup();
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

async fn run_landing_operation<T, F>(
    ownership: &Arc<File>,
    label: &'static str,
    operation: F,
) -> OpsResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> OpsResult<T> + Send + 'static,
{
    let ownership = Arc::clone(ownership);
    tokio::task::spawn_blocking(move || {
        let _ownership = ownership;
        operation()
    })
    .await
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
    io: LandingIo,
) -> OpsResult<PrLanding> {
    // External cleanup can remove the checkout. Retain the request until its
    // local evidence is available again, without claiming an outcome.
    if !landing.worktree.try_exists()? {
        return Ok(landing);
    }
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
    let result = reconcile_claimed(&store, &mut landing, io, &ownership).await;
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

/// What one check may do. An observing check reads and records; an acting
/// check also repairs and re-arms.
#[derive(Clone)]
pub(crate) struct LandingIo {
    pub observe: ObservePr,
    pub act: Option<(RepairCi, RearmPr)>,
}

impl std::fmt::Debug for LandingIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LandingIo")
            .field("acts", &self.act.is_some())
            .finish()
    }
}

impl LandingIo {
    fn github(repair: Option<RepairCi>) -> Self {
        Self {
            observe: Arc::new(observe_github),
            act: repair.map(|repair| (repair, Arc::new(rearm_github) as RearmPr)),
        }
    }
}

#[cfg(test)]
impl LandingIo {
    /// Scripted reads; an acting check's re-arm succeeds without GitHub.
    pub(crate) fn scripted(observe: ObservePr, repair: Option<RepairCi>) -> Self {
        Self {
            observe,
            act: repair
                .map(|repair| (repair, Arc::new(|_: &PrLanding, _: &str| Ok(())) as RearmPr)),
        }
    }
}

async fn reconcile_claimed(
    store: &SharedStore,
    landing: &mut PrLanding,
    io: LandingIo,
    ownership: &Arc<File>,
) -> OpsResult<()> {
    let LandingIo { observe, act } = io;
    let (repair, rearm) = act.unzip();
    refresh_joined_request(store, landing).await?;
    let mut observed = if let Some(merge_commit) = &landing.merge_commit {
        LandingObservation::Merged {
            head_sha: landing.observed_head_sha.clone(),
            merge_commit: merge_commit.clone(),
        }
    } else {
        run_landing_operation(ownership, "observation", {
            let observe = Arc::clone(&observe);
            let landing = landing.clone();
            move || observe(&landing)
        })
        .await?
    };
    record_observed_head(store, landing, &observed).await?;
    if let LandingObservation::Failing { failing_checks, .. } = &observed {
        let now = OffsetDateTime::now_utc();
        let incident = ci_incident(landing, failing_checks, now);
        store
            .observe_ci_incident(&incident)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        if repair.is_none() {
            return Ok(());
        }
        observed = run_landing_operation(ownership, "CI confirmation", {
            let observe = Arc::clone(&observe);
            let landing = landing.clone();
            move || observe(&landing)
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
        LandingObservation::Passing { head_sha } => {
            store
                .mark_ci_incidents_green(&landing.id, landing.generation, now)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
            resume_watching(store, landing, head_sha).await
        }
        LandingObservation::Dequeued { head_sha, reason } => {
            // A repair revokes the request while it moves the head.
            if repair_running(store, &landing.repo, landing.pr_number)? {
                return Ok(());
            }
            let Some(rearm) = rearm else {
                return Ok(());
            };
            if !rearm_landing(store, landing, ownership, rearm, &head_sha, "re-arm").await? {
                return Ok(());
            }
            let summary = format!(
                "PR #{}: {} at {}; re-armed {head_sha}",
                landing.pr_number,
                describe_drop(reason.as_deref()),
                now.format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_else(|_| now.unix_timestamp().to_string())
            );
            eprintln!("{summary}");
            if let Some(task_id) = &landing.task_id {
                store
                    .append_task_event(task_id, &TaskEventKind::Progress { summary })
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
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
        LandingObservation::Pending { head_sha } => resume_watching(store, landing, head_sha).await,
        LandingObservation::Failing {
            head_sha,
            failing_checks,
        } => {
            // Publication can expose a new head before its repair worker exits.
            // Keep observing that worker's landing rather than admitting a new
            // incident against the checkout it still owns.
            if repair_running(store, &landing.repo, landing.pr_number)? {
                return Ok(());
            }
            let incident = match next_attempt(
                store,
                landing,
                ci_incident(landing, &failing_checks, now),
                now,
            )? {
                NextAttempt::Due(incident) => *incident,
                NextAttempt::BackingOff { attempt, at } => {
                    eprintln!(
                        "PR #{}: repair attempt {attempt} starts in {}s",
                        landing.pr_number,
                        at - now.unix_timestamp()
                    );
                    return persist_landing_state(
                        store,
                        landing,
                        PrLandingState::Repairing,
                        head_sha,
                        None,
                        None,
                    )
                    .await;
                }
            };
            store
                .observe_ci_incident(&incident)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?;
            // Most failed queue runs are flakes: re-enter once before a repair.
            if let Some(rearm) = rearm.filter(|_| {
                attempt_number(&incident.identity) == 1 && incident.failure_set == ["merge-queue"]
            }) {
                if !rearm_landing(store, landing, ownership, rearm, &head_sha, "re-queue").await? {
                    return Ok(());
                }
                store
                    .record_ci_response(&incident.identity, &landing.id, landing.generation, now)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
                eprintln!(
                    "PR #{}: its merge queue run failed; re-queued {head_sha} once before repairing",
                    landing.pr_number
                );
                return persist_landing_state(
                    store,
                    landing,
                    PrLandingState::Watching,
                    head_sha,
                    None,
                    None,
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
            // A check without a ci-fix returned when it recorded the failure.
            let Some(repair) = repair else {
                return Ok(());
            };
            let repaired = run_landing_operation(ownership, "ci-fix", {
                let landing = landing.clone();
                move || repair(&landing, &incident)
            })
            .await;
            if let Err(error) = repaired {
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

/// Ask GitHub to merge `head_sha` again, returning whether it was asked. Pause
/// is the one hold on this; a held or failed request blocks the landing until
/// the next check.
async fn rearm_landing(
    store: &SharedStore,
    landing: &mut PrLanding,
    ownership: &Arc<File>,
    rearm: RearmPr,
    head_sha: &str,
    label: &'static str,
) -> OpsResult<bool> {
    let paused = match &landing.task_id {
        Some(task_id) => {
            store.sqlite.task_state(task_id).map_err(repair_error)?
                == crate::durable::TaskState::Paused
        }
        None => false,
    };
    if paused {
        let reason = "Task is paused; landing continues after `lf task resume`";
        return block_landing(store, landing, reason.into())
            .await
            .map(|()| false);
    }
    let requested = run_landing_operation(ownership, label, {
        let landing = landing.clone();
        let head_sha = head_sha.to_string();
        move || rearm(&landing, &head_sha)
    })
    .await;
    if let Err(error) = requested {
        return block_landing(store, landing, format!("{label} failed: {error}"))
            .await
            .map(|()| false);
    }
    Ok(true)
}

/// Why GitHub stopped merging the PR, as the Task history reads it.
fn describe_drop(reason: Option<&str>) -> String {
    match reason {
        Some("manually_disabled") => "auto-merge was turned off on GitHub".into(),
        Some(reason) => format!("GitHub removed it from the merge queue ({reason})"),
        None => "GitHub dropped its merge request".into(),
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
    let head = observed.head_sha();
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
    if !landing.worktree.try_exists()? {
        return Ok(());
    }
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
        eprintln!("Task {} has merged; retained its checkout until completion. Inspect remaining work with `lf task status {}`.", task.identifier, task.identifier);
        return Ok(());
    }
    // Only a Graph still being driven needs the checkout; a stopped one is history.
    if store
        .sqlite
        .graphs_at(&landing.worktree)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .iter()
        .any(|graph| {
            graph.process.completed_at.is_none()
                && crate::journal::process_evidence(&store.sqlite, graph.id())
                    != crate::journal::ProcessIdentityEvidence::Dead
        })
    {
        eprintln!("PR merged; retained its checkout for the Graph that landed it.");
        return Ok(());
    }
    if crate::git::worktrees::is_persistent_worktree(&landing.worktree)? {
        match crate::ops::sync::restart_landed_branch(&landing.worktree) {
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
            LandingIo::github(Some(ci_fix(release))),
        )
        .await
    })
}

/// Whether a repair worker for this PR is running now, whichever landing reserved it.
pub(crate) fn repair_running(store: &SharedStore, repo: &str, pr_number: u32) -> OpsResult<bool> {
    Ok(store
        .sqlite
        .pr_repair_processes(repo, pr_number)
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

/// Check every 15 s until the PR merges or closes. A blocked check is a
/// recorded state, not an answer: the next check acts on fresh evidence, and
/// only Pause holds it. No elapsed time ends the wait.
async fn wait_for_landing(
    store: &SharedStore,
    id: &crate::pr_landing::PrLandingId,
) -> OpsResult<()> {
    let io = LandingIo::github(Some(ci_fix(None)));
    #[cfg(test)]
    let io = LANDING_IO.try_with(Clone::clone).unwrap_or(io);
    let mut reported = None;
    loop {
        let landing = store
            .get_pr_landing(id)
            .await
            .map_err(repair_error)?
            .ok_or_else(|| repair_error("landing disappeared while waiting"))?;
        let checked = reconcile_pr_landing(store.clone(), landing, io.clone()).await;
        let saved = store
            .get_pr_landing(id)
            .await
            .map_err(repair_error)?
            .ok_or_else(|| repair_error("landing disappeared while waiting"))?;
        match saved.state {
            PrLandingState::Merged => return Ok(()),
            PrLandingState::Closed => {
                return Err(OpsError::DeliveryHeld(
                    "Pull request closed without merging; Task completion is pending".into(),
                ))
            }
            PrLandingState::Watching | PrLandingState::Repairing | PrLandingState::Blocked => {}
        }
        let report = checked.err().map(|error| error.to_string());
        if report.is_some() && report != reported {
            eprintln!(
                "PR #{}: {}; checking again",
                saved.pr_number,
                report.as_deref().unwrap_or_default()
            );
        }
        reported = report;
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}

#[cfg(test)]
tokio::task_local! {
    static LANDING_IO: LandingIo;
}

#[cfg(test)]
pub(super) async fn wait_with_io(
    store: &SharedStore,
    id: &crate::pr_landing::PrLandingId,
    io: LandingIo,
) -> OpsResult<()> {
    LANDING_IO.scope(io, wait_for_landing(store, id)).await
}

pub fn reconcile_repository(repo: &Path) -> OpsResult<DeliveryCheck> {
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let store = landing_store().await?;
        let mut report = DeliveryCheck {
            checked_at: OffsetDateTime::now_utc().unix_timestamp(),
            errors: Vec::new(),
        };
        reconcile_repository_async(repo, &store, &mut report.errors).await?;
        Ok(report)
    });
    runtime.shutdown_background();
    result
}

async fn reconcile_repository_async(
    repo: &Path,
    store: &SharedStore,
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
        let number = landing.pr_number;
        let id = landing.id.clone();
        match reconcile_pr_landing(store.clone(), landing, LandingIo::github(None)).await {
            Ok(landing) => {
                if landing.state == PrLandingState::Merged {
                    if let Err(error) = cleanup_landed_pr(store, &landing).await {
                        errors.push(format!("PR #{number}: {error}"));
                    }
                }
            }
            Err(error) => {
                let saved = store.get_pr_landing(&id).await.map_err(repair_error)?;
                if !saved.is_some_and(|landing| {
                    landing.state == PrLandingState::Blocked
                        && landing.blocked_reason.as_deref() == Some(&error.to_string())
                }) {
                    errors.push(format!("PR #{number}: {error}"));
                }
            }
        }
    }
    for mut task in repository_tasks(store, &repo).await? {
        if let Err(error) = super::task::reconcile_delivered_task(store, &mut task).await {
            errors.push(format!("{}: {error}", task.identifier));
        }
    }
    Ok(())
}

/// The Tasks whose repository is `repo`.
async fn repository_tasks(
    store: &SharedStore,
    repo: &crate::repository::RepoId,
) -> OpsResult<Vec<crate::work::task::Task>> {
    let mut tasks = Vec::new();
    for task in store.list_tasks(None).await.map_err(repair_error)? {
        let task_repo = super::task::repository_for_task(store, &task)?;
        if crate::repository::RepoId::discover(&task_repo)
            .ok()
            .as_ref()
            == Some(repo)
        {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use time::OffsetDateTime;

    use super::{
        ci_incident, classify_github_observation, next_attempt, reconcile_pr_landing,
        wait_for_landing, wait_with_io, LandingIo, LandingObservation, NextAttempt, ObservePr,
        RearmPr, RepairCi,
    };
    use crate::durable::TaskId;
    use crate::ops::error::{OpsError, OpsResult};
    use crate::ops::pr::{MergeRequest, PrInfo};
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::task::{
        CiCheck, LinearWritebackState, Observation, Task, TaskEventKind, TaskPr, TaskPrId,
    };
    use std::path::{Path, PathBuf};

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
                None,
            )
            .unwrap();
            assert_eq!(
                observed,
                LandingObservation::Pending {
                    head_sha: "pr-head".to_string()
                }
            );
        }
    }

    #[test]
    fn dequeued_landing_resumes_ordinary_integration_and_authorization_checks() {
        let (landing, pr) = github_landing_fixture();
        let observed =
            classify_github_observation(&landing, pr.clone(), Some(MergeRequest::Auto), None)
                .unwrap();
        assert!(
            matches!(observed, LandingObservation::Failing { failing_checks, .. }
            if failing_checks[0].name == "required-integration")
        );
        assert_eq!(
            classify_github_observation(&landing, pr, None, None).unwrap(),
            LandingObservation::Dequeued {
                head_sha: "pr-head".to_string(),
                reason: None,
            }
        );
    }

    #[test]
    fn dequeue_reasons_decide_between_repair_and_rearm() {
        let (landing, mut pr) = github_landing_fixture();
        pr.merge_state = Some("blocked".to_string());
        let failing = |observed: LandingObservation| match observed {
            LandingObservation::Failing { failing_checks, .. } => {
                Some(failing_checks[0].name.clone())
            }
            _ => None,
        };
        for (reason, check) in [
            ("failed_checks", Some("merge-queue")),
            ("merge_conflict", Some("required-integration")),
            ("checks_timed_out", None),
            ("manual", None),
            ("manually_disabled", None),
        ] {
            let observed =
                classify_github_observation(&landing, pr.clone(), None, Some(reason.into()))
                    .unwrap();
            assert_eq!(failing(observed), check.map(String::from), "{reason}");
        }
        // A head nobody armed goes to a repair that verifies and arms it.
        pr.head_sha = Some("pushed-head".to_string());
        let observed =
            classify_github_observation(&landing, pr, None, Some("manual".into())).unwrap();
        assert_eq!(failing(observed).as_deref(), Some("auto-merge"));
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
                Some(MergeRequest::Queued("queue-entry".to_string())),
                Some("merged".to_string()),
            )
            .unwrap(),
            LandingObservation::Merged {
                head_sha: "pr-head".to_string(),
                merge_commit: "integrated-head".to_string()
            }
        );
        pr.state = "closed".to_string();
        assert_eq!(
            classify_github_observation(&landing, pr, None, None).unwrap(),
            LandingObservation::Closed {
                head_sha: "pr-head".to_string()
            }
        );
    }

    /// Scripted observations and a counted ci-fix in place of GitHub and a repair worker.
    struct Script {
        observations: Mutex<VecDeque<LandingObservation>>,
        repairs: Mutex<u32>,
        repair_error: Option<String>,
    }

    impl Script {
        fn observe(self: &Arc<Self>) -> ObservePr {
            let script = Arc::clone(self);
            Arc::new(move |_| {
                script
                    .observations
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| OpsError::Message("observation exhausted".into()))
            })
        }

        fn repair(self: &Arc<Self>) -> RepairCi {
            let script = Arc::clone(self);
            Arc::new(move |_, _| {
                *script.repairs.lock().unwrap() += 1;
                match &script.repair_error {
                    Some(error) => Err(OpsError::Message(error.clone())),
                    None => Ok(()),
                }
            })
        }

        /// An acting check whose re-arms record the heads they asked for.
        fn rearming(self: &Arc<Self>) -> (LandingIo, Arc<Mutex<Vec<String>>>) {
            let rearmed = Arc::new(Mutex::new(Vec::new()));
            let rearm: RearmPr = {
                let rearmed = Arc::clone(&rearmed);
                Arc::new(move |_: &PrLanding, head: &str| {
                    rearmed.lock().unwrap().push(head.to_string());
                    Ok(())
                })
            };
            let io = LandingIo {
                observe: self.observe(),
                act: Some((self.repair(), rearm)),
            };
            (io, rearmed)
        }
    }

    fn script(observations: Vec<LandingObservation>) -> Arc<Script> {
        Arc::new(Script {
            observations: Mutex::new(observations.into()),
            repairs: Mutex::new(0),
            repair_error: None,
        })
    }

    /// One check that may start a ci-fix.
    async fn check(
        store: SharedStore,
        landing: PrLanding,
        script: &Arc<Script>,
    ) -> OpsResult<PrLanding> {
        reconcile_pr_landing(
            store,
            landing,
            LandingIo::scripted(script.observe(), Some(script.repair())),
        )
        .await
    }

    async fn fixture() -> (tempfile::TempDir, SharedStore, PrLanding) {
        fixture_for(false).await
    }

    /// A landing that belongs to a Task, so its history and Pause apply.
    async fn fixture_for(with_task: bool) -> (tempfile::TempDir, SharedStore, PrLanding) {
        let directory = tempfile::tempdir().unwrap();
        let worktree = directory.path().join("checkout");
        std::fs::create_dir(&worktree).unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&worktree)
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
        let task_id = if with_task {
            Some(seed_task(&store, &worktree).await)
        } else {
            None
        };
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".into(),
                pr_number: 248,
                worktree,
                branch: "feature".into(),
                task_id,
                requested_head_sha: "head".into(),
            },
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&landing).await.unwrap();
        (directory, store, landing)
    }

    async fn seed_task(store: &SharedStore, worktree: &Path) -> TaskId {
        let now = OffsetDateTime::now_utc();
        let task = Task {
            repository_id: store
                .sqlite
                .ensure_repository(&worktree.display().to_string())
                .unwrap(),
            id: TaskId::new(),
            revision: 0,
            linear_id: None,
            identifier: "LAND-1".into(),
            title: "Land it".into(),
            description: String::new(),
            pm_snapshot_synced_at: None,
            pm_writeback: LinearWritebackState::Current,
            wave_id: None,
            project_id: None,
            worktree: Some(worktree.to_path_buf()),
            workspace_slug: "land-it".into(),
            branch: "feature".into(),
            base_commit: "deadbeef".into(),
            parent_pr_id: None,
            agent: None,
            created_at: now,
            updated_at: now,
            rank: 0,
            assignee: None,
            due_date: None,
            state: None,
            completed_at: None,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: task.branch.clone(),
            base_commit: task.base_commit.clone(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.seed_task(&task, &pr).await.unwrap().id
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
    async fn deleted_checkout_preserves_landing_until_it_can_be_observed() {
        let (_directory, store, landing) = fixture().await;
        let landing = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        std::fs::remove_dir_all(&landing.worktree).unwrap();
        let script = script(vec![merged()]);
        let skipped = check(store.clone(), landing.clone(), &script)
            .await
            .unwrap();
        assert_eq!(skipped, landing);
        assert_eq!(
            store.get_pr_landing(&landing.id).await.unwrap(),
            Some(landing.clone())
        );
        assert!(!landing.worktree.exists());
        super::cleanup_landed_pr(&store, &landing).await.unwrap();

        std::fs::create_dir(&landing.worktree).unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&landing.worktree)
            .status()
            .unwrap()
            .success());
        let observed = check(store, landing, &script).await.unwrap();
        assert_eq!(observed.state, PrLandingState::Merged);
    }

    #[tokio::test]
    async fn landing_wait_missing_record_is_a_failure_not_a_hold() {
        let (_directory, store, _) = fixture().await;
        let missing = crate::pr_landing::PrLandingId::from_raw("missing");
        let result = wait_for_landing(&store, &missing).await;
        assert!(matches!(result, Err(OpsError::Message(reason)) if reason.contains("disappeared")));
    }

    #[tokio::test]
    async fn dropped_merge_request_is_rearmed_without_a_repair() {
        let (_directory, store, landing) = fixture().await;
        let script = script(vec![LandingObservation::Dequeued {
            head_sha: "head".into(),
            reason: Some("checks_timed_out".into()),
        }]);
        let (io, rearmed) = script.rearming();
        let watching = reconcile_pr_landing(store, landing, io).await.unwrap();
        assert_eq!(watching.state, PrLandingState::Watching);
        assert_eq!(*rearmed.lock().unwrap(), ["head"]);
        assert_eq!(*script.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_rearm_is_recorded_in_task_history() {
        let (_directory, store, landing) = fixture_for(true).await;
        let task_id = landing.task_id.clone().unwrap();
        let script = script(vec![LandingObservation::Dequeued {
            head_sha: "head".into(),
            reason: Some("manually_disabled".into()),
        }]);
        let (io, rearmed) = script.rearming();
        reconcile_pr_landing(store.clone(), landing, io)
            .await
            .unwrap();
        assert_eq!(*rearmed.lock().unwrap(), ["head"]);
        let summaries: Vec<_> = store
            .task_events_after(&task_id, 0)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.kind {
                TaskEventKind::Progress { summary } => Some(summary),
                _ => None,
            })
            .collect();
        assert_eq!(summaries.len(), 1, "{summaries:?}");
        assert!(
            summaries[0].starts_with("PR #248: auto-merge was turned off on GitHub at ")
                && summaries[0].ends_with("; re-armed head"),
            "{}",
            summaries[0]
        );
    }

    #[tokio::test]
    async fn a_paused_task_holds_its_rearm() {
        let (_directory, store, landing) = fixture_for(true).await;
        let task_id = landing.task_id.clone().unwrap();
        store.sqlite.set_task_paused(&task_id, true).unwrap();
        let script = script(vec![LandingObservation::Dequeued {
            head_sha: "head".into(),
            reason: Some("checks_timed_out".into()),
        }]);
        let (io, rearmed) = script.rearming();
        let held = reconcile_pr_landing(store, landing, io).await;
        assert!(
            matches!(&held, Err(OpsError::DeliveryHeld(reason)) if reason.contains("lf task resume")),
            "{held:?}"
        );
        assert!(rearmed.lock().unwrap().is_empty());
    }

    #[test]
    fn a_queue_removal_that_leaves_auto_merge_on_is_a_drop() {
        let (landing, mut pr) = github_landing_fixture();
        pr.merge_state = Some("blocked".to_string());
        let awaiting = || Some(MergeRequest::AwaitingQueue);
        assert_eq!(
            classify_github_observation(&landing, pr.clone(), awaiting(), Some("manual".into()))
                .unwrap(),
            LandingObservation::Dequeued {
                head_sha: "pr-head".into(),
                reason: Some("manual".into()),
            }
        );
        // Without a removal, auto-merge waiting for the queue reads the PR's checks.
        assert!(!matches!(
            classify_github_observation(&landing, pr, awaiting(), None),
            Ok(LandingObservation::Dequeued { .. })
        ));
    }

    #[tokio::test]
    async fn a_failed_queue_run_is_requeued_once_before_a_repair() {
        let (_directory, store, landing) = fixture().await;
        let queue_failure = || LandingObservation::Failing {
            head_sha: "head".into(),
            failing_checks: vec![CiCheck {
                name: "merge-queue".into(),
                url: None,
            }],
        };
        let script = script(vec![
            queue_failure(),
            queue_failure(),
            queue_failure(),
            queue_failure(),
        ]);
        let (io, rearmed) = script.rearming();
        let requeued = reconcile_pr_landing(store.clone(), landing, io.clone())
            .await
            .unwrap();
        assert_eq!(requeued.state, PrLandingState::Watching);
        assert_eq!(
            (
                rearmed.lock().unwrap().len(),
                *script.repairs.lock().unwrap()
            ),
            (1, 0)
        );
        // The same failure again is a repair's, after its backoff.
        let backing_off = reconcile_pr_landing(store, requeued, io).await.unwrap();
        assert_eq!(backing_off.state, PrLandingState::Repairing);
        assert_eq!(
            (
                rearmed.lock().unwrap().len(),
                *script.repairs.lock().unwrap()
            ),
            (1, 0)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_continues_through_blocks_until_merge() {
        let (_directory, store, landing) = fixture().await;
        let script = Arc::new(Script {
            observations: Mutex::new(
                vec![
                    LandingObservation::Dequeued {
                        head_sha: "head".into(),
                        reason: Some("manually_disabled".into()),
                    },
                    failure("head"),
                    failure("head"),
                    merged(),
                ]
                .into(),
            ),
            repairs: Mutex::new(0),
            repair_error: Some("Task checkout Machine is unknown".into()),
        });
        wait_with_io(
            &store,
            &landing.id,
            LandingIo::scripted(script.observe(), Some(script.repair())),
        )
        .await
        .unwrap();
        assert_eq!(*script.repairs.lock().unwrap(), 1);
        assert!(script.observations.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_finished_repair_starts_the_next_attempt_after_its_backoff() {
        let (_directory, store, landing) = fixture().await;
        let now = OffsetDateTime::now_utc();
        let base = super::ci_incident(
            &landing,
            &[CiCheck {
                name: "rust".into(),
                url: None,
            }],
            now,
        );
        let NextAttempt::Due(first) = next_attempt(&store, &landing, base.clone(), now).unwrap()
        else {
            panic!("the first attempt is due at once");
        };
        assert_eq!(first.identity, base.identity);
        store.observe_ci_incident(&first).await.unwrap();
        store
            .record_ci_response(&first.identity, &landing.id, landing.generation, now)
            .await
            .unwrap();
        assert!(matches!(
            next_attempt(&store, &landing, base.clone(), now).unwrap(),
            NextAttempt::BackingOff { attempt: 2, .. }
        ));
        let later = now + time::Duration::seconds(61);
        let NextAttempt::Due(second) = next_attempt(&store, &landing, base.clone(), later).unwrap()
        else {
            panic!("the second attempt is due after one minute");
        };
        assert_eq!(second.identity, format!("{}#2", base.identity));
        assert_eq!(super::repair_backoff(2), 60);
        assert_eq!(super::repair_backoff(4), 240);
        assert_eq!(super::repair_backoff(40), 30 * 60);
    }

    #[tokio::test]
    async fn repair_disarm_keeps_landing_until_the_repair_rearms_and_merges() {
        let (_directory, store, landing) = fixture().await;
        let incident = ci_incident(
            &landing,
            &[CiCheck {
                name: "rust".into(),
                url: None,
            }],
            OffsetDateTime::now_utc(),
        );
        store.observe_ci_incident(&incident).await.unwrap();
        // A reservation whose Process has no recorded death is a live repair.
        {
            let conn = store.sqlite.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES('00000000-0000-4000-8000-000000000577','fixture',?1)",
                [OffsetDateTime::now_utc().unix_timestamp()],
            )
            .unwrap();
            conn.execute(
                "UPDATE ci_incidents SET repair_lf_process_id='00000000-0000-4000-8000-000000000577' WHERE identity=?1",
                [&incident.identity],
            )
            .unwrap();
        }
        let script = script(vec![
            LandingObservation::Dequeued {
                head_sha: "head".into(),
                reason: None,
            },
            merged(),
        ]);
        let paused = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(paused.state, PrLandingState::Watching);
        assert!(paused.blocked_reason.is_none());
        let landed = check(store, paused, &script).await.unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(*script.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn finite_checks_return_pending_then_finish_only_after_merge() {
        let (_directory, store, landing) = fixture().await;
        let script = script(vec![
            LandingObservation::Pending {
                head_sha: "new-head".into(),
            },
            LandingObservation::Merged {
                head_sha: "new-head".into(),
                merge_commit: "merge".into(),
            },
        ]);
        let pending = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        assert!(pending.supervisor.is_none());
        assert!(pending.merge_commit.is_none());
        assert_eq!(pending.observed_head_sha, "new-head");
        assert_eq!(pending.requested_head_sha, "head");
        assert_eq!(*script.repairs.lock().unwrap(), 0);
        let landed = check(store.clone(), pending, &script).await.unwrap();
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
        // Order and duplicates cannot change an incident; repository-specific
        // checks receive the same repair admission as any other required check.
        let failures = |names: &[&str]| LandingObservation::Failing {
            head_sha: "head".into(),
            failing_checks: names
                .iter()
                .map(|name| CiCheck {
                    name: (*name).into(),
                    url: None,
                })
                .collect(),
        };
        let script = script(vec![
            failures(&["rust", "scratch-clear", "rust"]),
            failures(&["scratch-clear", "rust"]),
            failures(&["rust", "scratch-clear"]),
            failures(&["scratch-clear", "scratch-clear", "rust"]),
            merged(),
        ]);
        let pending = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        // The unchanged failure waits out the next attempt's backoff.
        let backing_off = check(store.clone(), pending.clone(), &script)
            .await
            .unwrap();
        assert_eq!(backing_off.state, PrLandingState::Repairing);
        assert_eq!(*script.repairs.lock().unwrap(), 1);
        let merged = check(store.clone(), backing_off, &script).await.unwrap();
        assert_eq!(merged.state, PrLandingState::Merged);
        let incidents = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].incident.failure_set, ["rust", "scratch-clear"]);
        assert!(incidents[0].incident.responded_at.is_some());
    }

    #[tokio::test]
    async fn an_observing_check_records_the_failure_and_leaves_the_repair() {
        let (_directory, store, landing) = fixture().await;
        // The scheduled repository check sees the failure; the watcher repairs it.
        let fake = script(vec![failure("head"), failure("head"), failure("head")]);
        let watching = reconcile_pr_landing(
            store.clone(),
            landing,
            LandingIo::scripted(fake.observe(), None),
        )
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
        check(store.clone(), watching, &fake).await.unwrap();
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
        let script = Arc::new(Script {
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
        assert!(check(store.clone(), landing.clone(), &script)
            .await
            .is_err());
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        let recovered = check(store, blocked, &script).await.unwrap();
        assert_eq!(recovered.state, PrLandingState::Watching);
        assert!(recovered.blocked_reason.is_none());
        assert_eq!(*script.repairs.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn changed_confirmation_never_repairs_the_stale_head() {
        let (_directory, store, landing) = fixture().await;
        let script = script(vec![failure("head"), failure("other")]);
        let pending = check(store, landing, &script).await.unwrap();
        assert_eq!(pending.state, PrLandingState::Watching);
        assert_eq!(*script.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn confirmation_can_settle_a_merge_without_repair() {
        let (_directory, store, landing) = fixture().await;
        let script = script(vec![failure("head"), merged()]);
        let landed = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(landed.merge_commit.as_deref(), Some("merge"));
        assert_eq!(*script.repairs.lock().unwrap(), 0);
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
        let script = Arc::new(Script {
            observations: Mutex::new(vec![failure("head"), failure("head"), merged()].into()),
            repairs: Mutex::new(0),
            repair_error: Some("provider exited after publish".into()),
        });
        assert!(check(store.clone(), landing.clone(), &script)
            .await
            .is_err());
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert!(blocked
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("provider exited"));
        let landed = check(store, blocked, &script).await.unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
    }

    #[tokio::test]
    async fn unavailable_observation_keeps_delivery_pending_and_releases_claim() {
        let (_directory, store, landing) = fixture().await;
        let error = reconcile_pr_landing(
            store.clone(),
            landing.clone(),
            LandingIo::scripted(
                Arc::new(|_| Err(OpsError::Message("network failure".into()))),
                Some(Arc::new(|_, _| panic!("failed reads cannot start repair"))),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "network failure");
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(saved.state, PrLandingState::Watching);
        assert!(saved.supervisor.is_none());
    }

    #[tokio::test]
    async fn closed_unmerged_never_counts_as_landed() {
        let (_directory, store, landing) = fixture().await;
        let closed = script(vec![LandingObservation::Closed {
            head_sha: "head".into(),
        }]);
        assert!(check(store.clone(), landing.clone(), &closed)
            .await
            .is_err());
        let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(saved.state, PrLandingState::Closed);
        assert!(saved.merge_commit.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn overlapping_check_returns_while_canceled_repair_retains_its_lock() {
        let (_directory, store, landing) = fixture().await;
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel::<()>();
        let entered = Mutex::new(Some(entered));
        let held = Mutex::new(held);
        let first = tokio::spawn(reconcile_pr_landing(
            store.clone(),
            landing.clone(),
            LandingIo::scripted(
                Arc::new(|_| Ok(failure("head"))),
                Some(Arc::new(move |_, _| {
                    entered.lock().unwrap().take().unwrap().send(()).unwrap();
                    held.lock().unwrap().recv().unwrap();
                    Ok(())
                })),
            ),
        ));
        started.await.unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        let competitor = script(vec![merged()]);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            check(store.clone(), landing.clone(), &competitor),
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
