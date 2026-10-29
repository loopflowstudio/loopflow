//! Finite pull-request delivery reconciliation over durable landing intents.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fs2::FileExt;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::config::load_config_or_default;
use crate::git::current_branch;
use crate::pr_landing::{
    LandingPlacement, LandingSupervisor, NewPrLanding, PrLanding, PrLandingState,
    SUPERVISOR_STALE_AFTER,
};
use crate::store::sqlite::ci_incidents::RepairAttempt;
use crate::store::{open_store, storage_config_from_env, SharedStore};
use crate::work::task::{CiCheck, CiIncident};

use super::error::{OpsError, OpsResult};
use super::land::LandOptions;
use super::pr::{
    merge_gate_state, merge_request_removal, observe_pr_merge, pr_requires_resync, ManualRemoval,
    MergeGateReading, MergeRequest, MergeRequestRemoval, PrInfo, PrMergeObservation, RepairWindow,
};

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "status", content = "summary", rename_all = "snake_case")]
enum RepairConclusion {
    /// The summary stays in the run's Session.
    Published(serde::de::IgnoredAny),
    Blocked(String),
}

/// The conclusion's shape, enforced natively where the provider supports it.
fn repair_conclusion_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "status": {"type": "string", "enum": ["published", "blocked"]},
            "summary": {"type": "string"},
        },
        "required": ["status", "summary"],
        "additionalProperties": false,
    })
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
    /// A person removed the PR's merge request.
    Withdrawn {
        head_sha: String,
        removal: ManualRemoval,
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
            Self::Withdrawn { head_sha, .. }
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
pub(crate) type RepairCi =
    Arc<dyn Fn(&PrLanding, &CiIncident, &[CiCheck]) -> OpsResult<()> + Send + Sync>;

/// Before attempt `n` (n ≥ 2) of the same incident starts: 1, 2, 4… minutes, at most 30.
pub(crate) fn repair_backoff(attempt: u32) -> i64 {
    (60_i64 << attempt.saturating_sub(2).min(5)).min(30 * 60)
}

fn observe_github(landing: &PrLanding) -> OpsResult<LandingObservation> {
    let number = u64::from(landing.pr_number);
    let observation = observe_pr_merge(&landing.worktree, number)?;
    decide_merge(
        &observation,
        &landing.observed_head_sha,
        || read_merge_request_removal(&landing.worktree, &landing.repo, landing.pr_number),
        |head| merge_gate_state(&landing.worktree, number, head),
    )
}

/// Why this PR has no merge request, judged against its recorded CI repairs.
pub(crate) fn read_merge_request_removal(
    worktree: &Path,
    repo: &str,
    pr_number: u32,
) -> OpsResult<MergeRequestRemoval> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(landing_store())?;
    merge_request_removal(
        worktree,
        u64::from(pr_number),
        &repair_windows(&store, repo, pr_number)?,
    )
}

/// The lifetimes of this PR's CI repairs. A repair with no recorded end is
/// still repairing only while its Process lives.
fn repair_windows(store: &SharedStore, repo: &str, pr_number: u32) -> OpsResult<Vec<RepairWindow>> {
    let at = |seconds: i64| chrono::DateTime::from_timestamp(seconds, 0);
    Ok(store
        .sqlite
        .pr_repair_lifetimes(repo, pr_number)
        .map_err(repair_error)?
        .into_iter()
        .filter_map(|(process, started, ended)| {
            let start = at(started)?;
            match ended {
                Some(ended) => Some(RepairWindow {
                    start,
                    end: Some(at(ended)?),
                }),
                None => (crate::journal::process_evidence(&store.sqlite, &process)
                    != crate::journal::ProcessIdentityEvidence::Dead)
                    .then_some(RepairWindow { start, end: None }),
            }
        })
        .collect())
}

fn ci_fix(
    release: Option<Arc<(super::release_lock::ReleaseLock, crate::git::WorktreeLease)>>,
) -> RepairCi {
    Arc::new(move |landing, incident, checks| {
        admit_ci_fix(landing, incident, checks, release.as_deref())
    })
}

/// The one decision over a PR read, its merge-request removal and its head
/// checks. Each reader runs only when the decision needs it.
pub(crate) fn decide_merge(
    observation: &PrMergeObservation,
    fallback_head: &str,
    removal: impl FnOnce() -> OpsResult<MergeRequestRemoval>,
    checks: impl FnOnce(&str) -> OpsResult<Option<MergeGateReading>>,
) -> OpsResult<LandingObservation> {
    let pr = &observation.pr;
    let head_sha = pr
        .head_sha
        .clone()
        .unwrap_or_else(|| fallback_head.to_string());
    match pr.state.as_str() {
        "merged" => {
            let merge_commit = pr.merge_commit.clone().ok_or_else(|| {
                OpsError::Message(format!(
                    "GitHub reports pull request #{} merged without a merge commit",
                    pr.number
                ))
            })?;
            return Ok(LandingObservation::Merged {
                head_sha,
                merge_commit,
            });
        }
        "closed" => return Ok(LandingObservation::Closed { head_sha }),
        _ => {}
    }
    // An open PR without a merge request is waiting on whatever removed it. A
    // removal no person made leaves the head checks to decide; with nothing
    // failing, the missing request itself is the failure a repair re-arms.
    let unarmed = match observation.request {
        // The queue checks an integrated commit. Its membership takes precedence
        // over the original PR head's mergeability and check results.
        Some(MergeRequest::Queued(_)) => return Ok(LandingObservation::Pending { head_sha }),
        Some(_) => None,
        None => match removal()? {
            MergeRequestRemoval::Manual(removal) => {
                return Ok(LandingObservation::Withdrawn { head_sha, removal })
            }
            MergeRequestRemoval::Merged => return Ok(LandingObservation::Pending { head_sha }),
            MergeRequestRemoval::Evicted { reason, run_url } => {
                return Ok(LandingObservation::Failing {
                    head_sha,
                    failing_checks: vec![CiCheck {
                        name: format!("merge-queue ({reason})"),
                        url: run_url,
                    }],
                })
            }
            MergeRequestRemoval::ByLoopflow => Some("repair-disarmed"),
            MergeRequestRemoval::Unknown => Some("unexplained"),
        },
    };
    // Every eviction returned above.
    if pr_requires_resync(observation, || Ok(MergeRequestRemoval::Unknown))? {
        return Ok(LandingObservation::Failing {
            head_sha,
            failing_checks: vec![CiCheck {
                name: "required-integration".into(),
                url: None,
            }],
        });
    }
    match checks(&head_sha)? {
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
        Some(_) => Ok(match unarmed {
            Some(cause) => LandingObservation::Failing {
                head_sha,
                failing_checks: vec![CiCheck {
                    name: format!("merge-request ({cause})"),
                    url: None,
                }],
            },
            None => LandingObservation::Passing { head_sha },
        }),
        None => Ok(LandingObservation::Pending { head_sha }),
    }
}

fn admit_ci_fix(
    landing: &PrLanding,
    incident: &CiIncident,
    checks: &[CiCheck],
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
        if let Some(task_id) = &landing.task_id {
            if !store.sqlite.task_owned(task_id).map_err(repair_error)? {
                return Err(repair_error(
                    "Task is unassigned; CI repair waits for `lf task claim`",
                ));
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
        }
        // Live work in the checkout, such as the Task's own open Session,
        // writes alongside the repair; only a running sync fences an agent.
        let config = load_config_or_default(Some(&landing.worktree));
        let retry = reservation.process.is_some();
        if retry && reservation.retries >= config.automation.retries {
            return Err(repair_error("repair startup exhausted automatic retries"));
        }
        let launcher = crate::journal::current_lf_process_id()
            .ok_or_else(|| repair_error("repair admission requires a recorded Process"))?;
        if !store
            .sqlite
            .reserve_repair(
                &incident.identity,
                landing.generation,
                &launcher,
                retry,
                landing.task_id.as_ref(),
            )
            .map_err(repair_error)?
        {
            return Err(repair_error("landing changed before repair reservation"));
        }
        drop(lock);
        let context = crate::os_process::execution_context().map_err(repair_error)?;
        let argv = vec![
            context.lf_bin.to_string_lossy().to_string(),
            "--batch".into(),
            "--output-schema".into(),
            repair_conclusion_schema().to_string(),
            "skill".into(),
            "ci-fix".into(),
            ci_fix_message(
                landing,
                incident,
                checks,
                &earlier_attempts(&store, &incident.identity)?,
            ),
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
        if !store
            .sqlite
            .handoff_repair(&incident.identity, &launcher, &owner)
            .map_err(repair_error)?
        {
            return Err(repair_error("repair reservation changed during startup"));
        }
        Ok(())
    })
}

/// What one ci-fix run repairs and how it answers.
fn ci_fix_message(
    landing: &PrLanding,
    incident: &CiIncident,
    checks: &[CiCheck],
    earlier: &[String],
) -> String {
    // The checks that matched this incident carry its URLs, including the
    // merge-group run of a queue eviction.
    let checks = checks
        .iter()
        .map(|check| match &check.url {
            Some(url) => format!("- {} ({url})", check.name),
            None => format!("- {}", check.name),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let task_context = landing
        .task_id
        .as_ref()
        .map(|task_id| format!("\nTask context: {task_id}"))
        .unwrap_or_default();
    let mut message = format!(
        "Repository: {}\nPull request: #{}\nBranch: {}\nFailed head: {}\nFailing checks:\n{}{}",
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
            message.push_str(&format!("\n{name}: {meaning}"));
        }
    }
    if !earlier.is_empty() {
        message.push_str(&format!(
            "\n\nEarlier repair attempts left this same failure in place:\n{}",
            earlier.join("\n")
        ));
    }
    message.push_str("\n\nReturn your final answer as one JSON object: {\"status\":\"published\",\"summary\":\"Published head, PR URL, auto-merge state, and checks run\"}. If this attempt cannot repair it, return {\"status\":\"blocked\",\"summary\":\"What you tried and what stopped you\"}; the landing tries again after a backoff with your summary.");
    message
}

/// A reservation still held by the caller was never handed to a ci-fix run;
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

/// Record each exited ci-fix run of this PR: the Session it opened and the
/// conclusion its final answer returned.
fn settle_repairs(store: &SharedStore, repo: &str, pr_number: u32) -> OpsResult<()> {
    let current = crate::journal::current_lf_process_id();
    for (identity, owner) in store
        .sqlite
        .pr_repair_processes(repo, pr_number)
        .map_err(repair_error)?
    {
        if Some(&owner) == current.as_ref()
            || crate::journal::process_evidence(&store.sqlite, &owner)
                != crate::journal::ProcessIdentityEvidence::Dead
        {
            continue;
        }
        // The run's Session names the owner that served it.
        let session = match store
            .sqlite
            .transport_session(&owner)
            .map_err(repair_error)?
        {
            Some(id) => store.sqlite.session(&id).map_err(repair_error)?,
            None => None,
        };
        let answer = match session
            .as_ref()
            .and_then(|session| session.input_key().ok())
        {
            Some(input) => store
                .sqlite
                .input_final_answer(input)
                .map_err(repair_error)?,
            None => None,
        };
        let (conclusion, error) = match (&session, answer) {
            (None, _) => (
                None,
                Some(format!(
                    "ci-fix run {owner} exited before opening a Session; inspect transport.log"
                )),
            ),
            (Some(session), None) => (
                None,
                Some(format!(
                    "ci-fix Session {} recorded no final answer",
                    session.id
                )),
            ),
            (Some(_), Some(answer)) => match parse_repair_conclusion(&answer) {
                Ok(RepairConclusion::Published(_)) => (Some("published"), None),
                Ok(RepairConclusion::Blocked(reason)) => (Some("blocked"), Some(reason)),
                Err(error) => (None, Some(error.to_string())),
            },
        };
        if let Some(error) = &error {
            // The next attempt backs off; say why this one ended.
            eprintln!("PR #{pr_number}: ci-fix run ended without a repair: {error}");
        }
        store
            .sqlite
            .finish_repair(
                &identity,
                &owner,
                session
                    .as_ref()
                    .map(|session| (session.id.as_str(), session.captured)),
                error.as_deref(),
                conclusion,
            )
            .map_err(repair_error)?;
    }
    Ok(())
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

/// One observer reconciles a landing at a time; others return and leave it to
/// that observer. It never excludes writers in the checkout.
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
/// check also repairs.
#[derive(Clone)]
pub(crate) struct LandingIo {
    pub observe: ObservePr,
    pub repair: Option<RepairCi>,
}

impl std::fmt::Debug for LandingIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LandingIo")
            .field("acts", &self.repair.is_some())
            .finish()
    }
}

impl LandingIo {
    fn github(repair: Option<RepairCi>) -> Self {
        Self {
            observe: Arc::new(observe_github),
            repair,
        }
    }
}

#[cfg(test)]
impl LandingIo {
    /// Scripted reads.
    pub(crate) fn scripted(observe: ObservePr, repair: Option<RepairCi>) -> Self {
        Self { observe, repair }
    }
}

async fn reconcile_claimed(
    store: &SharedStore,
    landing: &mut PrLanding,
    io: LandingIo,
    ownership: &Arc<File>,
) -> OpsResult<()> {
    let LandingIo { observe, repair } = io;
    refresh_joined_request(store, landing).await?;
    settle_repairs(store, &landing.repo, landing.pr_number)?;
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
        LandingObservation::Withdrawn { removal, .. } => {
            let actor = removal.actor.as_deref().unwrap_or("someone");
            let number = landing.pr_number;
            let removed = if removal.queued {
                format!("{actor} removed #{number} from the merge queue")
            } else {
                format!("{actor} disabled auto-merge on #{number}")
            };
            block_landing(
                store,
                landing,
                format!(
                    "{removed} at {}; run lf arm to resume landing",
                    removal.at.to_rfc3339()
                ),
            )
            .await
        }
        LandingObservation::Pending { head_sha } => resume_watching(store, landing, head_sha).await,
        LandingObservation::Failing {
            head_sha,
            failing_checks,
        } => {
            // Publication can expose a new head before its ci-fix run exits.
            // Keep observing that run's landing rather than admitting a new
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
                move || repair(&landing, &incident, &failing_checks)
            })
            .await;
            if let Err(error) = repaired {
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

/// Whether a ci-fix run for this PR is running now, whichever landing reserved it.
pub(crate) fn repair_running(store: &SharedStore, repo: &str, pr_number: u32) -> OpsResult<bool> {
    Ok(store
        .sqlite
        .pr_repair_processes(repo, pr_number)
        .map_err(repair_error)?
        .iter()
        .any(|(_, process)| repair_live(store, Some(process))))
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
/// only unassigning the Task holds its repair. No elapsed time ends the wait.
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
        ci_incident, decide_merge, next_attempt, reconcile_pr_landing, wait_for_landing,
        wait_with_io, LandingIo, LandingObservation, NextAttempt, ObservePr, RepairCi,
    };
    use crate::durable::TaskId;
    use crate::ops::error::{OpsError, OpsResult};
    use crate::ops::pr::{
        ManualRemoval, MergeGateReading, MergeRequest, MergeRequestRemoval, PrInfo,
        PrMergeObservation,
    };
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::task::{CiCheck, LinearWritebackState, Observation, Task, TaskPr, TaskPrId};
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

    fn unexplained() -> OpsResult<MergeRequestRemoval> {
        Ok(MergeRequestRemoval::Unknown)
    }

    fn green(_: &str) -> OpsResult<Option<MergeGateReading>> {
        Ok(Some(MergeGateReading {
            failing: false,
            pending: false,
            failing_leaves: Vec::new(),
        }))
    }

    #[test]
    fn queued_landing_waits_for_integration_instead_of_requiring_sync() {
        let (landing, mut pr) = github_landing_fixture();
        for merge_state in ["behind", "dirty", "clean"] {
            pr.merge_state = Some(merge_state.to_string());
            let observed = decide_merge(
                &PrMergeObservation::fixture(
                    pr.clone(),
                    Some(MergeRequest::Queued("queue-entry".to_string())),
                    true,
                ),
                &landing.observed_head_sha,
                unexplained,
                green,
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
        let observed = decide_merge(
            &PrMergeObservation::fixture(pr, Some(MergeRequest::Auto), false),
            &landing.observed_head_sha,
            unexplained,
            green,
        )
        .unwrap();
        assert!(
            matches!(observed, LandingObservation::Failing { failing_checks, .. }
            if failing_checks[0].name == "required-integration")
        );
    }

    #[test]
    fn queue_membership_never_overrides_a_confirmed_merge_or_closure() {
        let (landing, mut pr) = github_landing_fixture();
        pr.state = "merged".to_string();
        pr.merge_commit = Some("integrated-head".to_string());
        assert_eq!(
            decide_merge(
                &PrMergeObservation::fixture(
                    pr.clone(),
                    Some(MergeRequest::Queued("queue-entry".to_string())),
                    true,
                ),
                &landing.observed_head_sha,
                unexplained,
                green,
            )
            .unwrap(),
            LandingObservation::Merged {
                head_sha: "pr-head".to_string(),
                merge_commit: "integrated-head".to_string()
            }
        );
        pr.state = "closed".to_string();
        assert_eq!(
            decide_merge(
                &PrMergeObservation::fixture(pr, None, true),
                &landing.observed_head_sha,
                unexplained,
                green
            )
            .unwrap(),
            LandingObservation::Closed {
                head_sha: "pr-head".to_string()
            }
        );
    }

    /// Scripted observations and a counted ci-fix in place of GitHub and a ci-fix run.
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
            Arc::new(move |_, _, _| {
                *script.repairs.lock().unwrap() += 1;
                match &script.repair_error {
                    Some(error) => Err(OpsError::Message(error.clone())),
                    None => Ok(()),
                }
            })
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

    /// A landing that belongs to a Task, so its history and Defer apply.
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
            regressed_in: None,
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
    async fn a_persons_dequeue_blocks_naming_them_without_launching_a_repair() {
        let (_directory, store, landing) = fixture().await;
        let script = script(vec![LandingObservation::Withdrawn {
            head_sha: "head".into(),
            removal: ManualRemoval {
                at: "2026-10-22T20:13:13Z".parse().unwrap(),
                actor: Some("jacklionheart".into()),
                queued: true,
            },
        }]);
        assert!(check(store.clone(), landing.clone(), &script)
            .await
            .is_err());
        let blocked = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
        assert_eq!(blocked.state, PrLandingState::Blocked);
        assert_eq!(
            blocked.blocked_reason.as_deref(),
            Some(
                "jacklionheart removed #248 from the merge queue at 2026-10-22T20:13:13+00:00; \
                 run lf arm to resume landing"
            )
        );
        assert_eq!(*script.repairs.lock().unwrap(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn waiting_continues_through_blocks_until_merge() {
        let (_directory, store, landing) = fixture().await;
        let script = Arc::new(Script {
            observations: Mutex::new(
                vec![
                    LandingObservation::Withdrawn {
                        head_sha: "head".into(),
                        removal: ManualRemoval {
                            at: "2026-10-22T20:13:13Z".parse().unwrap(),
                            actor: None,
                            queued: false,
                        },
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
        // The repair's own disarm leaves a green head without a merge request;
        // a check confirms a failure with a second read.
        let disarmed = LandingObservation::Failing {
            head_sha: "head".into(),
            failing_checks: vec![CiCheck {
                name: "merge-request (repair-disarmed)".into(),
                url: None,
            }],
        };
        let script = script(vec![disarmed.clone(), disarmed, merged()]);
        let paused = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(paused.state, PrLandingState::Watching);
        assert!(paused.blocked_reason.is_none());
        let landed = check(store, paused, &script).await.unwrap();
        assert_eq!(landed.state, PrLandingState::Merged);
        assert_eq!(*script.repairs.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn each_merge_group_run_that_evicts_a_head_admits_its_own_repair() {
        let (_directory, store, landing) = fixture().await;
        let evicted = |run: &str| LandingObservation::Failing {
            head_sha: "head".into(),
            failing_checks: vec![CiCheck {
                name: "merge-queue (failed_checks)".into(),
                url: Some(format!("https://github.com/o/r/actions/runs/{run}")),
            }],
        };
        // A check confirms a failure with a second read.
        let script = script(vec![
            evicted("A"),
            evicted("A"),
            evicted("A"),
            evicted("A"),
            evicted("B"),
            evicted("B"),
        ]);
        let repaired = check(store.clone(), landing, &script).await.unwrap();
        assert_eq!(*script.repairs.lock().unwrap(), 1);
        // The same run again is that incident's next attempt, after its backoff.
        let backing_off = check(store.clone(), repaired, &script).await.unwrap();
        assert_eq!(backing_off.state, PrLandingState::Repairing);
        assert_eq!(*script.repairs.lock().unwrap(), 1);
        check(store, backing_off, &script).await.unwrap();
        assert_eq!(*script.repairs.lock().unwrap(), 2);
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
                Some(Arc::new(|_, _, _| {
                    panic!("failed reads cannot start repair")
                })),
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
                Some(Arc::new(move |_, _, _| {
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
