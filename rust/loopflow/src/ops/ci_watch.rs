//! `lf ci watch` — poll one repository's open pull requests and start a ci-fix
//! when a Task's armed PR fails its required checks.
//!
//! Steady-state reads are REST with `If-None-Match`: an unchanged response is a
//! 304 and costs no quota. A detected failure goes through the landing check in
//! `pr_landing`, which confirms it against GitHub, records the incident and
//! admits one repair per incident. Nothing depends on this process running; it
//! only makes a repair start sooner.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::pr_landing::{PrLanding, PrLandingState};
use crate::store::SharedStore;
use crate::work::task::CiCheck;

use super::cron::Launchctl;
use super::error::{OpsError, OpsResult};
use super::pr::{GhCheck, MergeGateReading};

const INTERVAL: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(300);
const STANDBY: Duration = Duration::from_secs(15);
/// Below this many remaining core requests the watcher waits for the reset.
const RATE_FLOOR: u32 = 500;
const REQUIRED_TTL: Duration = Duration::from_secs(3600);
/// A landing with no failed check is still checked this often: a conflicting
/// or stale head and a CI timeout need repair without any check failing.
const LANDING_CHECK: Duration = Duration::from_secs(300);
const REPAIRS_KEPT: usize = 20;

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

// REST transport

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestResponse {
    pub status: u16,
    pub etag: Option<String>,
    pub remaining: Option<u32>,
    pub reset_at: Option<i64>,
    pub body: String,
}

/// Parse `gh api --include` output: a status line, headers, a blank line, the body.
fn parse_rest_response(raw: &str) -> OpsResult<RestResponse> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .unwrap_or((raw, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| error("GitHub response has no HTTP status line"))?;
    let mut response = RestResponse {
        status,
        etag: None,
        remaining: None,
        reset_at: None,
        body: body.to_string(),
    };
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "etag" => response.etag = Some(value.to_string()),
            "x-ratelimit-remaining" => response.remaining = value.parse().ok(),
            "x-ratelimit-reset" => response.reset_at = value.parse().ok(),
            _ => {}
        }
    }
    Ok(response)
}

fn gh_rest(repo: &Path, path: &str, etag: Option<&str>) -> OpsResult<RestResponse> {
    let mut command = Command::new("gh");
    command.args([
        "api",
        "--include",
        "-H",
        "Accept: application/vnd.github+json",
    ]);
    if let Some(etag) = etag {
        command.args(["-H", &format!("If-None-Match: {etag}")]);
    }
    // gh exits non-zero for 304 and 404 but still prints the response head.
    let output = command.arg(path).current_dir(repo).output()?;
    let raw = String::from_utf8_lossy(&output.stdout);
    if raw.trim().is_empty() {
        return Err(OpsError::CommandFailed {
            command: format!("gh api {path}"),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    parse_rest_response(&raw)
}

type Fetch<'a> = &'a dyn Fn(&str, Option<&str>) -> OpsResult<RestResponse>;

/// Bodies by request path, revalidated with their ETag on every read.
#[derive(Debug, Default)]
struct RestCache {
    entries: HashMap<String, (String, String)>,
    touched: HashSet<String>,
    remaining: Option<u32>,
    reset_at: Option<i64>,
}

impl RestCache {
    /// `None` is a 404: the resource does not exist.
    fn read(&mut self, fetch: Fetch<'_>, path: &str) -> OpsResult<Option<String>> {
        self.touched.insert(path.to_string());
        let etag = self.entries.get(path).map(|(etag, _)| etag.as_str());
        let response = fetch(path, etag)?;
        if response.remaining.is_some() {
            self.remaining = response.remaining;
            self.reset_at = response.reset_at;
        }
        match response.status {
            304 => Ok(self.entries.get(path).map(|(_, body)| body.clone())),
            200 => {
                if let Some(etag) = response.etag {
                    self.entries
                        .insert(path.to_string(), (etag, response.body.clone()));
                }
                Ok(Some(response.body))
            }
            404 => Ok(None),
            status => Err(error(format!("GitHub read {path} failed: HTTP {status}"))),
        }
    }

    /// Drop bodies the last pass did not ask for: old heads and closed PRs.
    fn end_pass(&mut self) {
        let touched = std::mem::take(&mut self.touched);
        self.entries.retain(|path, _| touched.contains(path));
    }
}

// GitHub REST shapes. External responses, read tolerantly; not wire DTOs.

#[derive(Debug, Deserialize)]
struct RestPull {
    number: u32,
    head: RestRef,
    base: RestRef,
}

#[derive(Debug, Deserialize)]
struct RestRef {
    #[serde(rename = "ref")]
    name: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct RestCheckRuns {
    total_count: usize,
    check_runs: Vec<RestCheckRun>,
}

#[derive(Debug, Deserialize)]
struct RestCheckRun {
    name: String,
    status: String,
    conclusion: Option<String>,
    details_url: Option<String>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    started_at: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    completed_at: Option<OffsetDateTime>,
    check_suite: Option<RestCheckSuite>,
}

#[derive(Debug, Deserialize)]
struct RestCheckSuite {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct RestCombinedStatus {
    statuses: Vec<RestStatus>,
}

#[derive(Debug, Deserialize)]
struct RestStatus {
    context: String,
    state: String,
    target_url: Option<String>,
}

fn parse<T: serde::de::DeserializeOwned>(path: &str, body: &str) -> OpsResult<T> {
    serde_json::from_str(body)
        .map_err(|cause| OpsError::Parse(format!("could not parse GitHub {path}: {cause}")))
}

/// Required check names from a `rules/branches/{branch}` or classic
/// `protection/required_status_checks` response.
fn required_contexts(body: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return names;
    };
    let mut collect = |checks: &serde_json::Value| {
        for check in checks.as_array().into_iter().flatten() {
            if let Some(name) = check.as_str().or_else(|| check["context"].as_str()) {
                names.insert(name.to_string());
            }
        }
    };
    for rule in value.as_array().into_iter().flatten() {
        if rule["type"] == "required_status_checks" {
            collect(&rule["parameters"]["required_status_checks"]);
        }
    }
    collect(&value["contexts"]);
    collect(&value["checks"]);
    names
}

// Gate reading

/// One head's required-check state as the watcher reads it over REST.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Gate {
    Passing,
    Pending,
    Failing {
        checks: Vec<CiCheck>,
        /// When the last failing check completed at the provider.
        completed_at: Option<OffsetDateTime>,
    },
    /// No required check has reported for this head yet.
    Unreported,
}

impl Gate {
    fn label(&self) -> &'static str {
        match self {
            Self::Passing => "passing",
            Self::Pending => "pending",
            Self::Failing { .. } => "failing",
            Self::Unreported => "unreported",
        }
    }
}

fn read_gate(
    mut runs: Vec<RestCheckRun>,
    statuses: Vec<RestStatus>,
    required: &BTreeSet<String>,
) -> Gate {
    // A rerun keeps its suite: the newest run of a name within a suite wins.
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
    let mut seen = HashSet::new();
    let mut completed = HashMap::new();
    let mut required_checks = Vec::new();
    let mut full = Vec::new();
    let mut push = |check: GhCheck, name: &str| {
        if required.contains(name) {
            required_checks.push(check.clone());
        }
        full.push(check);
    };
    for run in runs {
        let suite = run.check_suite.as_ref().map(|suite| suite.id);
        if !seen.insert((run.name.clone(), suite)) {
            continue;
        }
        let state = if run.status == "completed" {
            run.conclusion.unwrap_or_default()
        } else {
            run.status
        };
        let check = GhCheck::new(
            run.name.clone(),
            &state.to_ascii_uppercase(),
            run.details_url,
        );
        if check.failed() {
            if let Some(at) = run.completed_at {
                let latest = completed.entry(run.name.clone()).or_insert(at);
                *latest = (*latest).max(at);
            }
        }
        push(check, &run.name);
    }
    for status in statuses {
        let check = GhCheck::new(
            status.context.clone(),
            &status.state.to_ascii_uppercase(),
            status.target_url,
        );
        push(check, &status.context);
    }
    if required_checks.is_empty() {
        return Gate::Unreported;
    }
    let reading = MergeGateReading::from_checks(required_checks, full);
    if reading.failing {
        let completed_at = reading
            .failing_leaves
            .iter()
            .filter_map(|check| completed.get(&check.name).copied())
            .max();
        Gate::Failing {
            checks: reading
                .failing_leaves
                .into_iter()
                .map(|check| CiCheck {
                    name: check.name,
                    url: check.url,
                })
                .collect(),
            completed_at,
        }
    } else if reading.pending {
        Gate::Pending
    } else {
        Gate::Passing
    }
}

/// What a reading asks of the watcher. A recorded landing is the request to
/// deliver a PR, so only a landing is repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Response {
    /// Run the landing check, which may start a ci-fix.
    CheckLanding,
    /// Run the landing check every `LANDING_CHECK`.
    CheckLandingOccasionally,
    Report(&'static str),
    Nothing,
}

fn respond(gate: &Gate, has_task: bool, has_landing: bool) -> Response {
    match (gate, has_landing, has_task) {
        (Gate::Failing { .. }, true, _) => Response::CheckLanding,
        (_, true, _) => Response::CheckLandingOccasionally,
        (Gate::Failing { .. }, false, true) => {
            Response::Report("not armed; `lf arm` or `lf land` hands it to repair")
        }
        (Gate::Failing { .. }, false, false) => Response::Report("no Task; reported, not repaired"),
        _ => Response::Nothing,
    }
}

// Pacing

/// 60 s between passes with jitter, backing off after a degraded pass and
/// waiting out a low rate limit.
#[derive(Debug, Default)]
struct Pacer {
    degraded_passes: u32,
}

impl Pacer {
    /// `jitter` is a fraction in `0.0..1.0`.
    fn next_delay(
        &mut self,
        clean: bool,
        remaining: Option<u32>,
        reset_at: Option<i64>,
        now: i64,
        jitter: f64,
    ) -> Duration {
        if clean {
            self.degraded_passes = 0;
        } else {
            self.degraded_passes += 1;
        }
        let base = INTERVAL
            .saturating_mul(1 << self.degraded_passes.min(4))
            .min(MAX_BACKOFF);
        let paced = base.mul_f64(0.85 + 0.3 * jitter.clamp(0.0, 1.0));
        match (remaining, reset_at) {
            (Some(remaining), Some(reset_at)) if remaining < RATE_FLOOR => {
                paced.max(Duration::from_secs((reset_at - now).clamp(0, 3600) as u64))
            }
            _ => paced,
        }
    }
}

// Watcher state, readable by `lf ci watch --status` and Desktop

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiWatchState {
    pub pid: u32,
    pub repo: String,
    pub started_at: i64,
    pub last_poll_at: Option<i64>,
    pub next_poll_at: Option<i64>,
    pub rate_remaining: Option<u32>,
    /// Why the last pass was incomplete, when it was.
    pub degraded: Option<String>,
    pub prs: Vec<WatchedPr>,
    /// Recent ci-fix starts, newest first.
    pub repairs: Vec<WatchRepair>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedPr {
    pub number: u32,
    pub task: Option<String>,
    pub head_sha: String,
    pub state: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRepair {
    pub at: i64,
    pub pr_number: u32,
    pub task: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiWatchStatus {
    /// Whether a watcher holds this repository's watch lock now.
    pub running: bool,
    /// Whether the launchd service is installed for this repository.
    pub installed: bool,
    pub state: Option<CiWatchState>,
}

fn lock_path(root: &Path) -> OpsResult<PathBuf> {
    Ok(crate::engine::git::absolute_git_dir(root)?.join("lf-ci-watch.lock"))
}

fn state_path(root: &Path) -> OpsResult<PathBuf> {
    Ok(crate::engine::git::absolute_git_dir(root)?
        .join("loopflow")
        .join("ci-watch.json"))
}

/// The kernel lock is the claim; a dead watcher releases it without cleanup.
fn try_lock(root: &Path) -> OpsResult<Option<File>> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path(root)?)?;
    match FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(Some(lock)),
        Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(cause) => Err(cause.into()),
    }
}

fn read_state(root: &Path) -> Option<CiWatchState> {
    serde_json::from_slice(&std::fs::read(state_path(root).ok()?).ok()?).ok()
}

fn write_state(root: &Path, state: &CiWatchState) -> OpsResult<()> {
    let path = state_path(root)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(state).map_err(error)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub fn status(repo: &Path) -> OpsResult<CiWatchStatus> {
    let root = crate::engine::worktrees::main_repo_root(repo).map_err(error)?;
    let running = try_lock(&root)?.is_none();
    let installed = service_path(&super::cron::default_launch_agents_dir()?, &root).exists();
    Ok(CiWatchStatus {
        running,
        installed,
        state: read_state(&root),
    })
}

// The watcher

struct Watcher {
    /// GitHub `owner/name`, as REST paths spell it.
    nwo: String,
    /// The repository id landings are recorded under.
    repo_id: String,
    cache: RestCache,
    required: HashMap<String, (Instant, BTreeSet<String>)>,
    /// Failures whose repair already finished; unchanged evidence is not rechecked.
    surfaced: HashSet<(u32, String, Vec<String>)>,
    /// When each quiet landing was last checked, and what that check said.
    landing_checked: HashMap<u32, (Instant, Option<String>)>,
    state: CiWatchState,
}

impl Watcher {
    fn new(root: &Path) -> OpsResult<Self> {
        let (owner, name) = crate::engine::worktrees::github_repo_nwo(root)
            .ok_or_else(|| error("the origin remote is not a GitHub repository"))?;
        let nwo = format!("{owner}/{name}");
        let repo_id = crate::repository::RepoId::discover(root)
            .map_err(error)?
            .as_str()
            .to_string();
        Ok(Self {
            state: CiWatchState {
                pid: std::process::id(),
                repo: nwo.clone(),
                started_at: OffsetDateTime::now_utc().unix_timestamp(),
                last_poll_at: None,
                next_poll_at: None,
                rate_remaining: None,
                degraded: None,
                prs: Vec::new(),
                repairs: Vec::new(),
            },
            nwo,
            repo_id,
            cache: RestCache::default(),
            required: HashMap::new(),
            surfaced: HashSet::new(),
            landing_checked: HashMap::new(),
        })
    }

    fn open_pulls(&mut self, fetch: Fetch<'_>) -> OpsResult<Vec<RestPull>> {
        let mut pulls = Vec::new();
        for page in 1..=10 {
            let path = format!(
                "repos/{}/pulls?state=open&per_page=100&page={page}",
                self.nwo
            );
            let body = self
                .cache
                .read(fetch, &path)?
                .ok_or_else(|| error(format!("GitHub repository {} was not found", self.nwo)))?;
            let batch: Vec<RestPull> = parse(&path, &body)?;
            let last = batch.len() < 100;
            pulls.extend(batch);
            if last {
                break;
            }
        }
        Ok(pulls)
    }

    /// Required check names for a base branch, reread hourly.
    fn required(&mut self, fetch: Fetch<'_>, base: &str) -> OpsResult<BTreeSet<String>> {
        if let Some((read_at, names)) = self.required.get(base) {
            if read_at.elapsed() < REQUIRED_TTL {
                return Ok(names.clone());
            }
        }
        let mut names = BTreeSet::new();
        for path in [
            format!("repos/{}/rules/branches/{base}", self.nwo),
            format!(
                "repos/{}/branches/{base}/protection/required_status_checks",
                self.nwo
            ),
        ] {
            // The cache would answer 304 forever; an hourly read must be fresh.
            self.cache.entries.remove(&path);
            if let Some(body) = self.cache.read(fetch, &path)? {
                names.extend(required_contexts(&body));
            }
        }
        self.required
            .insert(base.to_string(), (Instant::now(), names.clone()));
        Ok(names)
    }

    fn gate(&mut self, fetch: Fetch<'_>, pull: &RestPull) -> OpsResult<Gate> {
        let required = self.required(fetch, &pull.base.name)?;
        if required.is_empty() {
            return Ok(Gate::Unreported);
        }
        let mut runs = Vec::new();
        for page in 1..=10 {
            let path = format!(
                "repos/{}/commits/{}/check-runs?per_page=100&page={page}",
                self.nwo, pull.head.sha
            );
            let Some(body) = self.cache.read(fetch, &path)? else {
                break;
            };
            let batch: RestCheckRuns = parse(&path, &body)?;
            runs.extend(batch.check_runs);
            if runs.len() >= batch.total_count {
                break;
            }
        }
        let path = format!(
            "repos/{}/commits/{}/status?per_page=100",
            self.nwo, pull.head.sha
        );
        let statuses = match self.cache.read(fetch, &path)? {
            Some(body) => parse::<RestCombinedStatus>(&path, &body)?.statuses,
            None => Vec::new(),
        };
        Ok(read_gate(runs, statuses, &required))
    }

    /// Read every open PR once and respond to what changed. `Ok(false)` means
    /// some read was degraded and the next pass should back off.
    async fn pass(&mut self, store: &SharedStore, fetch: Fetch<'_>) -> OpsResult<bool> {
        let pulls = self.open_pulls(fetch)?;
        let landings: HashMap<u32, PrLanding> = store
            .pending_pr_landings(&self.repo_id)
            .await
            .map_err(error)?
            .into_iter()
            .map(|landing| (landing.pr_number, landing))
            .collect();
        let mut clean = true;
        let mut watched = Vec::with_capacity(pulls.len());
        for pull in pulls {
            let task = store
                .get_task_by_branch(&pull.head.name)
                .await
                .map_err(error)?
                .map(|task| task.plan.identifier);
            let landing = landings.get(&pull.number);
            let (state, detail) = match self.gate(fetch, &pull) {
                Err(cause) => {
                    clean = false;
                    ("unknown".to_string(), Some(cause.to_string()))
                }
                Ok(gate) => {
                    let detail = match (respond(&gate, task.is_some(), landing.is_some()), landing)
                    {
                        (Response::CheckLanding, Some(landing)) => {
                            let task = task.as_deref();
                            let (ok, detail) =
                                self.check_landing(store, landing, &pull, &gate, task).await;
                            clean &= ok;
                            detail
                        }
                        (Response::CheckLandingOccasionally, Some(landing)) => {
                            let due = self
                                .landing_checked
                                .get(&pull.number)
                                .is_none_or(|(at, _)| at.elapsed() >= LANDING_CHECK);
                            if due {
                                let task = task.as_deref();
                                let (ok, detail) =
                                    self.check_landing(store, landing, &pull, &gate, task).await;
                                clean &= ok;
                                self.landing_checked
                                    .insert(pull.number, (Instant::now(), detail));
                            }
                            self.landing_checked
                                .get(&pull.number)
                                .and_then(|(_, detail)| detail.clone())
                        }
                        (Response::Report(detail), _) => Some(detail.to_string()),
                        _ => None,
                    };
                    (gate.label().to_string(), detail)
                }
            };
            watched.push(WatchedPr {
                number: pull.number,
                task,
                head_sha: pull.head.sha,
                state,
                detail,
            });
        }
        for pr in &watched {
            let before = self.state.prs.iter().find(|old| old.number == pr.number);
            if before.map(|old| (&old.state, &old.detail)) != Some((&pr.state, &pr.detail))
                && (pr.detail.is_some() || before.is_some())
            {
                println!(
                    "PR #{} {}: {}{}",
                    pr.number,
                    pr.task.as_deref().unwrap_or("(no Task)"),
                    pr.state,
                    pr.detail
                        .as_deref()
                        .map(|detail| format!(" — {detail}"))
                        .unwrap_or_default()
                );
            }
        }
        let open: HashSet<u32> = watched.iter().map(|pr| pr.number).collect();
        self.surfaced.retain(|(number, _, _)| open.contains(number));
        self.landing_checked
            .retain(|number, _| open.contains(number));
        self.state.prs = watched;
        self.cache.end_pass();
        Ok(clean)
    }

    /// Hand one landing to the shared landing check. Returns whether GitHub was
    /// readable, and what happened.
    async fn check_landing(
        &mut self,
        store: &SharedStore,
        landing: &PrLanding,
        pull: &RestPull,
        gate: &Gate,
        task: Option<&str>,
    ) -> (bool, Option<String>) {
        let failure = match gate {
            Gate::Failing {
                checks,
                completed_at,
            } => {
                let mut names: Vec<String> = checks.iter().map(|c| c.name.clone()).collect();
                names.sort();
                names.dedup();
                Some((names, *completed_at))
            }
            _ => None,
        };
        let key = failure
            .as_ref()
            .map(|(names, _)| (pull.number, pull.head.sha.clone(), names.clone()));
        if key.as_ref().is_some_and(|key| self.surfaced.contains(key)) {
            return (
                true,
                Some("repair finished; waiting for changed evidence".to_string()),
            );
        }
        let running = |store: &SharedStore| {
            super::pr_landing::repair_running(store, landing).unwrap_or(false)
        };
        if running(store) {
            return (true, Some("ci-fix running".to_string()));
        }
        let result = super::pr_landing::repair_landing(store.clone(), landing.clone()).await;
        if let Some((_, Some(completed_at))) = &failure {
            if let Err(cause) = store.sqlite.record_ci_provider_completion(
                &landing.repo,
                landing.pr_number,
                &pull.head.sha,
                *completed_at,
            ) {
                eprintln!(
                    "PR #{}: provider completion not recorded: {cause}",
                    pull.number
                );
            }
        }
        match result {
            Ok(_) if running(store) => {
                let reason = failure
                    .map(|(names, _)| names.join(", "))
                    .unwrap_or_else(|| "required integration or CI timeout".to_string());
                self.state.repairs.insert(
                    0,
                    WatchRepair {
                        at: OffsetDateTime::now_utc().unix_timestamp(),
                        pr_number: pull.number,
                        task: task.map(str::to_string),
                        reason: reason.clone(),
                    },
                );
                self.state.repairs.truncate(REPAIRS_KEPT);
                (true, Some(format!("ci-fix started: {reason}")))
            }
            // Queued, pending, or changed since the REST read: nothing to repair.
            Ok(landing) => (
                true,
                failure.map(|_| format!("landing {}; no repair needed", landing.state.as_str())),
            ),
            Err(cause) => {
                let cause = cause.to_string();
                let blocked = store
                    .get_pr_landing(&landing.id)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|saved| {
                        saved.state == PrLandingState::Blocked
                            && saved.blocked_reason.as_deref() == Some(cause.as_str())
                    });
                if blocked && cause.contains("already completed a repair") {
                    self.surfaced.extend(key);
                }
                // A recorded block is an answer; anything else was a failed read.
                (blocked, Some(cause))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    /// Run one pass and return instead of watching.
    pub once: bool,
    /// Stop when this process exits; Desktop passes its own pid.
    pub parent_pid: Option<u32>,
}

fn parent_gone(options: &WatchOptions) -> bool {
    options
        .parent_pid
        .is_some_and(|pid| !super::cron::process_alive(pid))
}

/// Sleep in short steps so a vanished parent stops the watcher promptly.
async fn wait(delay: Duration, options: &WatchOptions) -> bool {
    let deadline = tokio::time::Instant::now() + delay;
    while tokio::time::Instant::now() < deadline {
        if parent_gone(options) {
            return false;
        }
        tokio::time::sleep(Duration::from_secs(2).min(delay)).await;
    }
    !parent_gone(options)
}

pub fn watch(repo: &Path, options: WatchOptions) -> OpsResult<()> {
    let root = crate::engine::worktrees::main_repo_root(repo).map_err(error)?;
    if !super::pr::gh_available() {
        return Err(error("lf ci watch needs the gh CLI"));
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let mut announced = false;
        let _lock = loop {
            if let Some(lock) = try_lock(&root)? {
                break lock;
            }
            if options.once {
                println!("another watcher is live for this repository; nothing to do");
                return Ok(());
            }
            if !announced {
                let pid = read_state(&root).map(|state| state.pid);
                println!(
                    "another watcher is live for this repository{}; standing by",
                    pid.map(|pid| format!(" (pid {pid})")).unwrap_or_default()
                );
                announced = true;
            }
            if !wait(STANDBY, &options).await {
                return Ok(());
            }
        };
        let mut watcher = Watcher::new(&root)?;
        let store = super::pr_landing::landing_store().await?;
        println!("watching CI for {}", watcher.nwo);
        let fetch = |path: &str, etag: Option<&str>| gh_rest(&root, path, etag);
        let mut pacer = Pacer::default();
        loop {
            let outcome = watcher.pass(&store, &fetch).await;
            let now = OffsetDateTime::now_utc();
            let clean = matches!(outcome, Ok(true));
            watcher.state.degraded = match &outcome {
                Ok(true) => None,
                Ok(false) => Some("some pull requests could not be read".to_string()),
                Err(cause) => Some(cause.to_string()),
            };
            if let Err(cause) = &outcome {
                eprintln!("pass degraded: {cause}");
            }
            let delay = pacer.next_delay(
                clean,
                watcher.cache.remaining,
                watcher.cache.reset_at,
                now.unix_timestamp(),
                f64::from(now.nanosecond()) / 1e9,
            );
            watcher.state.last_poll_at = Some(now.unix_timestamp());
            watcher.state.rate_remaining = watcher.cache.remaining;
            watcher.state.next_poll_at =
                (!options.once).then(|| now.unix_timestamp() + delay.as_secs() as i64);
            write_state(&root, &watcher.state)?;
            if options.once {
                return outcome.map(|_| ());
            }
            if !wait(delay, &options).await {
                return Ok(());
            }
        }
    });
    // An in-flight landing check keeps its own lock until its worker exits.
    runtime.shutdown_background();
    result
}

// Optional launchd service: the same command, kept alive by launchd

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSpec {
    pub repo: PathBuf,
    pub lf_path: PathBuf,
    pub lf_home: PathBuf,
    pub db_path: PathBuf,
    pub path_env: String,
}

fn service_label(repo: &Path) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "loopflow.ci-watch.{}",
        &hex::encode(Sha256::digest(repo.as_os_str().as_encoded_bytes()))[..24]
    )
}

fn service_path(launch_agents_dir: &Path, repo: &Path) -> PathBuf {
    launch_agents_dir.join(format!("{}.plist", service_label(repo)))
}

fn render_service(spec: &ServiceSpec) -> String {
    let escape = super::cron::xml_escape;
    let log = spec.repo.join(".lf/logs/ci-watch.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{lf_path}</string>
        <string>repo</string>
        <string>ci</string>
        <string>watch</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path_env}</string>
        <key>LF_HOME</key>
        <string>{lf_home}</string>
        <key>LF_DB_PATH</key>
        <string>{db_path}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>30</integer>
    <key>WorkingDirectory</key>
    <string>{repo}</string>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        label = escape(&service_label(&spec.repo)),
        lf_path = escape(&spec.lf_path.to_string_lossy()),
        path_env = escape(&spec.path_env),
        lf_home = escape(&spec.lf_home.to_string_lossy()),
        db_path = escape(&spec.db_path.to_string_lossy()),
        repo = escape(&spec.repo.to_string_lossy()),
        log = escape(&log.to_string_lossy()),
    )
}

pub fn install_service(
    launch_agents_dir: &Path,
    spec: &ServiceSpec,
    launchctl: &dyn Launchctl,
) -> OpsResult<PathBuf> {
    std::fs::create_dir_all(launch_agents_dir)?;
    std::fs::create_dir_all(spec.repo.join(".lf/logs"))?;
    let path = service_path(launch_agents_dir, &spec.repo);
    if path.exists() {
        let _ = launchctl.unload(&path);
    }
    super::cron::write_private_file(&path, render_service(spec).as_bytes())?;
    launchctl.load(&path)?;
    Ok(path)
}

/// Returns whether a service was installed.
pub fn uninstall_service(
    launch_agents_dir: &Path,
    repo: &Path,
    launchctl: &dyn Launchctl,
) -> OpsResult<bool> {
    let path = service_path(launch_agents_dir, repo);
    if !path.exists() {
        return Ok(false);
    }
    launchctl.unload(&path)?;
    std::fs::remove_file(&path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::Arc;

    use super::*;
    use crate::store::StorageConfig;

    fn ok(body: &str, etag: &str) -> RestResponse {
        RestResponse {
            status: 200,
            etag: Some(etag.to_string()),
            remaining: Some(4000),
            reset_at: Some(0),
            body: body.to_string(),
        }
    }

    fn run(name: &str, conclusion: Option<&str>, completed: Option<&str>) -> RestCheckRun {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "status": if conclusion.is_some() { "completed" } else { "in_progress" },
            "conclusion": conclusion,
            "details_url": format!("https://github.com/o/r/runs/{name}"),
            "started_at": "2026-10-01T10:00:00Z",
            "completed_at": completed,
            "check_suite": {"id": 1},
        }))
        .unwrap()
    }

    fn required() -> BTreeSet<String> {
        BTreeSet::from(["tests-result".to_string()])
    }

    #[test]
    fn gh_include_output_yields_status_etag_and_rate_limit() {
        let response = parse_rest_response(
            "HTTP/2.0 200 OK\r\nEtag: W/\"abc\"\r\nX-Ratelimit-Remaining: 4994\r\nX-Ratelimit-Reset: 1790884668\r\n\r\n[{\"a\":1}]",
        )
        .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.etag.as_deref(), Some("W/\"abc\""));
        assert_eq!(response.remaining, Some(4994));
        assert_eq!(response.reset_at, Some(1790884668));
        assert_eq!(response.body, "[{\"a\":1}]");
        assert_eq!(
            parse_rest_response("HTTP/2.0 304 Not Modified\r\nEtag: \"abc\"\r\n\r\n")
                .unwrap()
                .status,
            304
        );
    }

    #[test]
    fn unchanged_reads_send_the_etag_and_reuse_the_body() {
        let sent = RefCell::new(Vec::new());
        let fetch = |path: &str, etag: Option<&str>| {
            sent.borrow_mut()
                .push((path.to_string(), etag.map(str::to_string)));
            Ok(match etag {
                None => ok("first", "\"v1\""),
                Some(_) => RestResponse {
                    status: 304,
                    etag: None,
                    remaining: None,
                    reset_at: None,
                    body: String::new(),
                },
            })
        };
        let mut cache = RestCache::default();
        assert_eq!(cache.read(&fetch, "a").unwrap().as_deref(), Some("first"));
        assert_eq!(cache.read(&fetch, "a").unwrap().as_deref(), Some("first"));
        assert_eq!(
            sent.borrow().as_slice(),
            [
                ("a".to_string(), None),
                ("a".to_string(), Some("\"v1\"".to_string()))
            ]
        );
        // A 304 carries no quota header; the last known budget stands.
        assert_eq!(cache.remaining, Some(4000));
        cache.end_pass();
        cache.end_pass();
        assert!(cache.entries.is_empty(), "unread bodies are dropped");
    }

    #[test]
    fn required_checks_come_from_rulesets_or_classic_protection() {
        let rules = r#"[{"type":"deletion"},{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"tests-result","integration_id":15368}]}}]"#;
        assert_eq!(required_contexts(rules), required());
        let classic = r#"{"contexts":["tests-result"],"checks":[{"context":"lint","app_id":1}]}"#;
        assert_eq!(
            required_contexts(classic),
            BTreeSet::from(["lint".to_string(), "tests-result".to_string()])
        );
        assert!(required_contexts(r#"{"message":"Branch not protected"}"#).is_empty());
    }

    #[test]
    fn a_failed_gate_names_its_leaf_and_when_the_provider_finished() {
        let gate = read_gate(
            vec![
                run(
                    "tests-result",
                    Some("failure"),
                    Some("2026-10-01T10:09:00Z"),
                ),
                run("rust-test", Some("failure"), Some("2026-10-01T10:08:00Z")),
                run("lint", Some("success"), Some("2026-10-01T10:02:00Z")),
            ],
            vec![],
            &required(),
        );
        let Gate::Failing {
            checks,
            completed_at,
        } = gate
        else {
            panic!("expected a failing gate, got {gate:?}");
        };
        assert_eq!(
            checks.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["rust-test"],
            "the aggregate gives way to the leaf that broke"
        );
        assert_eq!(
            completed_at.unwrap(),
            OffsetDateTime::parse(
                "2026-10-01T10:08:00Z",
                &time::format_description::well_known::Rfc3339
            )
            .unwrap()
        );
    }

    #[test]
    fn the_gate_follows_only_required_checks() {
        // A red leaf does not fail a head whose required check has not finished.
        assert_eq!(
            read_gate(
                vec![
                    run("tests-result", None, None),
                    run("rust-test", Some("failure"), None)
                ],
                vec![],
                &required()
            ),
            Gate::Pending
        );
        assert_eq!(
            read_gate(
                vec![run("tests-result", Some("success"), None)],
                vec![],
                &required()
            ),
            Gate::Passing
        );
        assert_eq!(
            read_gate(
                vec![run("rust-test", Some("failure"), None)],
                vec![],
                &required()
            ),
            Gate::Unreported
        );
        let status = RestStatus {
            context: "tests-result".into(),
            state: "failure".into(),
            target_url: None,
        };
        assert!(matches!(
            read_gate(vec![], vec![status], &required()),
            Gate::Failing { .. }
        ));
    }

    #[test]
    fn only_a_recorded_landing_is_repaired() {
        let failing = Gate::Failing {
            checks: vec![],
            completed_at: None,
        };
        assert_eq!(respond(&failing, true, true), Response::CheckLanding);
        assert_eq!(respond(&failing, false, true), Response::CheckLanding);
        assert!(matches!(
            respond(&failing, true, false),
            Response::Report(_)
        ));
        assert_eq!(
            respond(&failing, false, false),
            Response::Report("no Task; reported, not repaired")
        );
        for quiet in [Gate::Pending, Gate::Passing, Gate::Unreported] {
            assert_eq!(
                respond(&quiet, true, true),
                Response::CheckLandingOccasionally,
                "a conflicting head or a CI timeout fails no check"
            );
            assert_eq!(respond(&quiet, true, false), Response::Nothing);
        }
    }

    #[test]
    fn pacing_jitters_backs_off_and_waits_out_a_low_rate_limit() {
        let mut pacer = Pacer::default();
        let low = pacer.next_delay(true, Some(4000), Some(0), 0, 0.0);
        let high = pacer.next_delay(true, Some(4000), Some(0), 0, 1.0);
        assert_eq!(low, Duration::from_secs(51));
        assert_eq!(high, Duration::from_secs(69));

        assert_eq!(
            pacer.next_delay(false, None, None, 0, 0.5),
            Duration::from_secs(120)
        );
        assert_eq!(
            pacer.next_delay(false, None, None, 0, 0.5),
            Duration::from_secs(240)
        );
        assert_eq!(
            pacer.next_delay(false, None, None, 0, 0.5),
            MAX_BACKOFF,
            "backoff stops at five minutes"
        );
        assert_eq!(
            pacer.next_delay(true, None, None, 0, 0.5),
            INTERVAL,
            "a clean pass restores the minute"
        );

        assert_eq!(
            pacer.next_delay(true, Some(RATE_FLOOR - 1), Some(1900), 1000, 0.5),
            Duration::from_secs(900),
            "a low budget waits for its reset"
        );
        assert_eq!(
            pacer.next_delay(true, Some(RATE_FLOOR), Some(1900), 1000, 0.5),
            INTERVAL
        );
    }

    #[test]
    fn a_second_copy_defers_to_the_live_watcher() {
        let directory = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success());
        let first = try_lock(directory.path()).unwrap();
        assert!(first.is_some());
        assert!(try_lock(directory.path()).unwrap().is_none());
        drop(first);
        assert!(
            try_lock(directory.path()).unwrap().is_some(),
            "the claim ends with its holder"
        );
    }

    struct FakeLaunchctl;

    impl Launchctl for FakeLaunchctl {
        fn load(&self, _: &Path) -> OpsResult<()> {
            Ok(())
        }
        fn unload(&self, _: &Path) -> OpsResult<()> {
            Ok(())
        }
        fn is_loaded(&self, _: &str) -> OpsResult<bool> {
            Ok(true)
        }
        fn trigger(&self, _: &str) -> OpsResult<()> {
            Ok(())
        }
    }

    #[test]
    fn the_service_runs_the_same_command_and_uninstalls_cleanly() {
        let directory = tempfile::tempdir().unwrap();
        let agents = directory.path().join("LaunchAgents");
        let spec = ServiceSpec {
            repo: directory.path().join("repo & co"),
            lf_path: PathBuf::from("/usr/local/bin/lf"),
            lf_home: PathBuf::from("/home/.lf"),
            db_path: PathBuf::from("/home/.lf/loopflow.db"),
            path_env: "/usr/bin".into(),
        };
        let path = install_service(&agents, &spec, &FakeLaunchctl).unwrap();
        let plist = std::fs::read_to_string(&path).unwrap();
        assert!(plist.contains(
            "<string>/usr/local/bin/lf</string>\n        <string>repo</string>\n        <string>ci</string>\n        <string>watch</string>"
        ));
        assert!(plist.contains("<key>KeepAlive</key>\n    <true/>"));
        assert!(plist.contains("repo &amp; co</string>"));
        assert!(uninstall_service(&agents, &spec.repo, &FakeLaunchctl).unwrap());
        assert!(!path.exists());
        assert!(!uninstall_service(&agents, &spec.repo, &FakeLaunchctl).unwrap());
    }

    /// GitHub as the watcher sees it: one open PR whose required check failed.
    fn github(path: &str, _: Option<&str>) -> OpsResult<RestResponse> {
        let body = if path.contains("/pulls?") {
            r#"[{"number":248,"head":{"ref":"feature","sha":"head"},"base":{"ref":"main","sha":"base"}}]"#
        } else if path.contains("/rules/branches/main") {
            r#"[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"tests-result"}]}}]"#
        } else if path.contains("/check-runs") {
            r#"{"total_count":1,"check_runs":[{"name":"tests-result","status":"completed","conclusion":"failure","details_url":null,"started_at":"2026-10-01T10:00:00Z","completed_at":"2026-10-01T10:09:00Z","check_suite":{"id":1}}]}"#
        } else if path.contains("/status") {
            r#"{"statuses":[]}"#
        } else {
            return Ok(RestResponse {
                status: 404,
                etag: None,
                remaining: None,
                reset_at: None,
                body: String::new(),
            });
        };
        Ok(ok(body, "\"v1\""))
    }

    fn watcher() -> Watcher {
        Watcher {
            nwo: "loopflowstudio/loopflow".into(),
            repo_id: "loopflowstudio/loopflow".into(),
            cache: RestCache::default(),
            required: HashMap::new(),
            surfaced: HashSet::new(),
            landing_checked: HashMap::new(),
            state: CiWatchState {
                pid: 1,
                repo: "loopflowstudio/loopflow".into(),
                started_at: 0,
                last_poll_at: None,
                next_poll_at: None,
                rate_remaining: None,
                degraded: None,
                prs: Vec::new(),
                repairs: Vec::new(),
            },
        }
    }

    #[tokio::test]
    async fn a_failing_pr_with_no_task_is_reported_and_never_repaired() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let mut watcher = watcher();
        assert!(watcher.pass(&store, &github).await.unwrap());
        assert_eq!(
            watcher.state.prs,
            [WatchedPr {
                number: 248,
                task: None,
                head_sha: "head".into(),
                state: "failing".into(),
                detail: Some("no Task; reported, not repaired".into()),
            }]
        );
        assert!(watcher.state.repairs.is_empty());
    }
}
