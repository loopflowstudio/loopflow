//! Collection is local-only. Missing facts retain a checkout, never imply abandonment.
pub(crate) mod io;
#[doc(hidden)]
pub use io::worker_entry;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::durable::{WorkRef, WorkStatus};
use crate::engine::git::{
    worktree_remove_owned, PreparedWorktreeLease, WorktreeLease, WorktreeRemoval,
};
use crate::journal::{process_evidence_at, ProcessIdentityEvidence};
use crate::ops::{OpsError, OpsResult};
use crate::store::{sqlite::SqliteStore, SharedStore};
use crate::work::task::PrPhase;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CleanupAction {
    RemoveCheckout,
    /// Source is settled; fresh history validation is still required under admission.
    ValidateCheckout,
    Retain(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupDecision {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub observed_head: Option<String>,
    pub action: CleanupAction,
    /// Exact-head settlement facts; an empty list never authorizes removal.
    pub evidence: Vec<String>,
    /// Unknown is not zero. Foreground previews do not recursively measure checkouts.
    pub estimated_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupReport {
    pub planned: Vec<CleanupDecision>,
    pub removed: Vec<PathBuf>,
    pub deferred: Vec<CleanupDecision>,
    pub failed: Vec<CleanupFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupFailure {
    pub path: PathBuf,
    pub error: String,
}

/// Bounded scan continuation carried by cron receipts. Neither scan
/// coverage nor a prior success can substitute for fresh removal evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupProgress {
    pub sequence: u64,
    pub full_scan_at: Option<i64>,
    pub full_scan_started: Option<i64>,
    /// Unattempted members of one setup window (at most 32 registrations).
    /// Drain this continuation before discovering another window.
    pub pending_registrations: Vec<PathBuf>,
    /// Registration setup resumes independently of checkout admission.
    pub registration_after: Option<PathBuf>,
    /// Freeze each registration sweep so arrivals cannot postpone wrap forever.
    pub registration_through: Option<PathBuf>,
    pub registrations_observed: usize,
    /// A sweep with skipped cohort members must wrap again before completing.
    pub full_scan_pending: bool,
    pub observed: usize,
    pub removed: usize,
    pub deferred: usize,
    pub failed: usize,
}

impl CleanupProgress {
    pub(crate) fn initial() -> Self {
        Self {
            sequence: 0,
            full_scan_at: None,
            full_scan_started: None,
            pending_registrations: Vec::new(),
            registration_after: None,
            registration_through: None,
            registrations_observed: 0,
            full_scan_pending: false,
            observed: 0,
            removed: 0,
            deferred: 0,
            failed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CleanupBudget {
    pub removals: usize,
    pub admission_time: Duration,
}

impl Default for CleanupBudget {
    fn default() -> Self {
        Self {
            removals: 8,
            admission_time: Duration::from_secs(30),
        }
    }
}

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

fn normalized(path: &Path) -> OpsResult<PathBuf> {
    crate::store::canonicalize_with_missing_tail(path).map_err(error)
}

/// Failure to inspect external execution is unknown, not an empty process set.
fn running_paths() -> OpsResult<HashSet<PathBuf>> {
    io::read(io::Read::RunningPaths)
}

fn read_running_paths() -> OpsResult<HashSet<PathBuf>> {
    let output = io::output(
        Command::new("lsof").args(["-d", "cwd", "-Fn"]),
        Duration::from_secs(5),
    )?;
    if !output.stderr.is_empty() {
        return Err(error("external process inspection unavailable"));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| normalized(&path))
        .collect()
}

fn retain(decision: &mut CleanupDecision, reason: impl Into<String>) {
    decision.action = CleanupAction::Retain(reason.into());
}

fn release_registry() -> OpsResult<Option<SqliteStore>> {
    // Tests never consult the installed account's registry.
    if cfg!(test) || !crate::store::custom_home_selected() {
        return Ok(None);
    }
    let path = crate::store::production_database_path();
    if !path.try_exists()? {
        return Ok(None);
    }
    SqliteStore::open_read_only(&path).map(Some).map_err(error)
}

fn evidence_homes(
    store: &SqliteStore,
) -> OpsResult<Vec<(crate::provider_auth::Provider, PathBuf)>> {
    let mut homes = Vec::new();
    for provider in [
        crate::provider_auth::Provider::Codex,
        crate::provider_auth::Provider::Claude,
    ] {
        homes.push((
            provider,
            crate::provider_account::activation::native_home(provider, None),
        ));
    }
    for account in store.list_provider_accounts(None).map_err(error)? {
        if let Some(home) = account.home {
            let provider = account
                .provider
                .parse::<crate::provider_auth::Provider>()
                .map_err(error)?;
            homes.push((provider, home));
        }
    }
    let mut seen = HashSet::new();
    homes.retain(|home| seen.insert(home.clone()));
    Ok(homes)
}

/// Runs only in the evidence worker. Every reference is resolved now and none
/// is kept: a positive match retains at once, and only complete coverage may
/// answer false. `advance` reports one consumed row, resolved reference or
/// native entry, never elapsed time.
fn evidence_blocks_checkout(
    store: &SqliteStore,
    checkout: &Path,
    advance: &mut dyn FnMut(),
) -> OpsResult<bool> {
    let mut visit = |path: Option<&Path>| -> OpsResult<bool> {
        let overlaps = match path {
            Some(path) => {
                let root = crate::store::canonicalize_with_missing_tail(path).map_err(error)?;
                root.starts_with(checkout) || checkout.starts_with(root)
            }
            None => false,
        };
        advance();
        Ok(overlaps)
    };
    let mut roots = vec![
        crate::store::lf_home_dir(),
        store.home_dir().map_err(error)?,
    ];
    if !cfg!(test) {
        roots.push(
            crate::store::production_database_path()
                .parent()
                .expect("database has a parent")
                .to_path_buf(),
        );
    }
    let homes = evidence_homes(store)?;
    // Never traverse history beneath an already protected home.
    for root in roots.iter().chain(homes.iter().map(|(_, home)| home)) {
        if visit(Some(root))? {
            return Ok(true);
        }
    }
    // Native layouts carry no change record, so they are walked before the
    // history whose later changes can still be revalidated below.
    for (provider, home) in &homes {
        if crate::ops::human_session::provider_conversation::visit_transcript_evidence(
            *provider,
            home,
            |path| visit(path).map_err(std::io::Error::other),
        )? {
            return Ok(true);
        }
    }
    // References appended or edited behind the stream reappear at a later
    // revision. Read until a snapshot adds nothing; history that never settles
    // stays unknown instead of becoming an old snapshot's negative.
    let mut revision = 0;
    for _ in 0..8 {
        let consumed = revision;
        if store
            .visit_session_evidence(&mut revision, |path| {
                visit(path)
                    .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
            })
            .map_err(error)?
        {
            return Ok(true);
        }
        if revision == consumed {
            if evidence_homes(store)? != homes {
                return Err(error("provider accounts changed during observation"));
            }
            return Ok(false);
        }
    }
    Err(error("Session evidence kept changing during observation"))
}

/// The selected and release registries enforce the same retention policy. Read
/// checkout links and unfinished Processes per candidate, then again under locks.
#[derive(Debug)]
struct RegistryObservations {
    store: SqliteStore,
    home: PathBuf,
    tasks: Vec<crate::store::sqlite::TaskCheckout>,
    open: crate::store::sqlite::task_work::OpenProcesses,
}

impl RegistryObservations {
    fn read(store: &SqliteStore) -> OpsResult<Self> {
        let store = store
            .bounded_reader(Duration::from_secs(2))
            .map_err(error)?;
        let mut tasks = store.task_checkouts().map_err(error)?;
        for task in &mut tasks {
            task.worktree = normalized(&task.worktree)?;
        }
        Ok(Self {
            home: store.home_dir().map_err(error)?,
            tasks,
            open: store.open_processes().map_err(error)?,
            store,
        })
    }

    fn tasks_at<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Iterator<Item = &'a crate::store::sqlite::TaskCheckout> {
        self.tasks.iter().filter(move |task| task.worktree == path)
    }

    fn execution_blocks(&self, process: &crate::process::LfProcess) -> bool {
        process_evidence_at(&self.store, &process.lfid, &self.home) != ProcessIdentityEvidence::Dead
    }

    fn blocker(&self, path: &Path) -> OpsResult<Option<String>> {
        for task in self.tasks_at(path) {
            let status = self
                .store
                .work_status(&WorkRef::Task(task.task_id.clone()))
                .map_err(error)?;
            if !matches!(status, WorkStatus::Done | WorkStatus::Abandoned) {
                return Ok(Some(format!("unfinished Task {}", task.issue_identifier)));
            }
            if status == WorkStatus::Done {
                let Some(pr) = self.store.active_task_pr(&task.task_id).map_err(error)? else {
                    return Ok(Some("completed Task has a PR-less checkout".into()));
                };
                let follow = self
                    .store
                    .task_follow_through(&task.task_id)
                    .map_err(error)?;
                let gate = crate::ops::task::CompletionGate::from_delivery(Some(&pr), &follow);
                if !gate.blockers.is_empty() {
                    return Ok(Some(format!("unresolved delivery: {}", gate.reason())));
                }
            }
            if self
                .open
                .for_task(&task.task_id)
                .iter()
                .any(|process| self.execution_blocks(process))
            {
                return Ok(Some("Task has live or unknown execution".into()));
            }
        }
        for process in &self.open.all {
            if let Some(cwd) = &process.cwd {
                if normalized(Path::new(cwd))?.starts_with(path) && self.execution_blocks(process) {
                    return Ok(Some(format!(
                        "Process {} has live or unknown execution",
                        process.lfid
                    )));
                }
            }
        }

        Ok(None)
    }
}

fn observe(
    store: &SharedStore,
    repo: &Path,
    decision: &mut CleanupDecision,
    external: &OpsResult<HashSet<PathBuf>>,
    validate_removal: bool,
) -> OpsResult<()> {
    let local = store.sqlite.path().map_err(error)?;
    *decision = io::read(io::Read::Observation {
        database: local.clone(),
        repo: repo.to_path_buf(),
        decision: Box::new(decision.clone()),
        external: external.as_ref().map_err(ToString::to_string).cloned(),
        validate_registration: validate_removal,
    })?;
    if validate_removal && decision.action == CleanupAction::ValidateCheckout {
        // The worker exits before its answer is consumed and holds no locks.
        // Nothing it observed outlives this attempt.
        let release: Option<PathBuf> = io::read(io::Read::ReleaseRegistry)?;
        for database in std::iter::once(local).chain(release) {
            if io::observe_evidence(database, decision.path.clone())? {
                retain(decision, "local Session evidence");
                return Ok(());
            }
        }
        decision.action = CleanupAction::RemoveCheckout;
    }
    Ok(())
}

fn observe_source(
    store: &SqliteStore,
    repo: &Path,
    decision: &mut CleanupDecision,
    external: &OpsResult<HashSet<PathBuf>>,
    validate_registration: bool,
) -> OpsResult<()> {
    // Each observation stands alone, including a recheck after planning.
    decision.evidence.clear();
    let path = &decision.path;
    if normalized(path)? == normalized(repo)? {
        retain(decision, "primary checkout");
        return Ok(());
    }
    let missing = match std::fs::symlink_metadata(path) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    let admin = if missing {
        let Some((admin, started)) = interrupted_removal(repo, path)? else {
            retain(decision, "missing checkout; registration retained");
            return Ok(());
        };
        let Some(branch) = &started.branch else {
            retain(
                decision,
                "interrupted detached checkout; registration retained",
            );
            return Ok(());
        };
        // The administrative HEAD and local ref must still name the exact
        // source observed before removal. A missing path alone proves nothing.
        if std::fs::read_to_string(admin.join("HEAD"))?.trim()
            != format!("ref: refs/heads/{branch}")
            || Some(
                read_git(repo, &["rev-parse", &format!("refs/heads/{branch}")])?
                    .trim()
                    .to_string(),
            ) != started.observed_head
        {
            retain(decision, "interrupted checkout registration changed");
            return Ok(());
        }
        decision.branch = started.branch;
        decision.observed_head = started.observed_head;
        admin
    } else {
        if normalized(&main_repo_root(path)?)? != normalized(repo)? {
            retain(decision, "checkout registration changed");
            return Ok(());
        }
        let admin = git_directory(path, "--absolute-git-dir")?;
        if validate_registration {
            // Locked observation validates this registration, not a sibling-wide
            // inventory. A preview's registration cannot authorize removal.
            let common = git_directory(repo, "--git-common-dir")?;
            let backlink = io::checkout_path(&admin)?;
            if admin.parent() != Some(common.join("worktrees").as_path()) || backlink != *path {
                retain(decision, "checkout registration changed");
                return Ok(());
            }
        }
        let branch = read_git(path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        decision.branch = (branch.trim() != "HEAD").then(|| branch.trim().to_string());
        decision.observed_head = Some(read_git(path, &["rev-parse", "HEAD"])?.trim().to_string());
        admin
    };
    let default_branch = read_git(
        repo,
        &[
            "for-each-ref",
            "--format=%(symref:short)",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    let default_branch = default_branch
        .trim()
        .strip_prefix("origin/")
        .unwrap_or("main");
    if decision.branch.as_deref() == Some(default_branch) {
        retain(decision, "default branch");
        return Ok(());
    }
    if let Some(branch) = &decision.branch {
        let persistent = read_git(
            if missing { repo } else { path },
            &[
                "config",
                "--default",
                "false",
                "--bool",
                "--get",
                &format!("branch.{branch}.loopflow-persistent"),
            ],
        )?;
        if persistent.trim() == "true" {
            retain(decision, "persistent checkout");
            return Ok(());
        }
    }
    // Cheap checkout protections need no registry or history scan. Each candidate
    // gets fresh bounded readers here, including the locked removal recheck.
    let local = RegistryObservations::read(store)?;
    let mut owned = admin.join("lf-created").is_file();
    if let Some(reason) = local.blocker(path)? {
        retain(decision, reason);
        return Ok(());
    }
    for task in local.tasks_at(path) {
        owned = true;
        for pr in local.store.task_prs(&task.task_id).map_err(error)? {
            if decision.branch.as_deref() == Some(pr.branch.as_str())
                && pr.phase() == PrPhase::Merged
                && pr.head_sha() == decision.observed_head.as_deref()
            {
                decision
                    .evidence
                    .push(format!("Task {}: exact merged head", task.issue_identifier));
            }
        }
    }
    // A release read failure retains candidates without obscuring local blockers.
    let release = release_registry()?
        .as_ref()
        .map(RegistryObservations::read)
        .transpose()?;
    if let Some(release) = &release {
        // Release facts can veto removal, never settle experimental source.
        if let Some(reason) = release.blocker(path)? {
            retain(decision, format!("{reason} in release registry"));
            return Ok(());
        }
    }
    for landing in local.store.merged_landings_at(path).map_err(error)? {
        owned = true;
        if decision.branch.as_deref() == Some(landing.branch.as_str())
            && decision.observed_head.as_deref() == Some(landing.observed_head_sha.as_str())
        {
            decision.evidence.push("landing: exact merged head".into());
        }
    }
    if !owned {
        retain(decision, "unknown Loopflow ownership");
        return Ok(());
    }
    if decision.evidence.is_empty() {
        retain(decision, "current head has no recorded settlement");
        return Ok(());
    }
    match external {
        Err(reason) => {
            retain(decision, reason.to_string());
            return Ok(());
        }
        Ok(paths) if paths.iter().any(|cwd| cwd.starts_with(path)) => {
            retain(decision, "running external process");
            return Ok(());
        }
        Ok(_) => {}
    }
    if !missing {
        if read_git(path, &["ls-files", "-v"])?.lines().any(|line| {
            line.as_bytes()
                .first()
                .is_some_and(|flag| flag.is_ascii_lowercase() || *flag == b'S')
        }) {
            retain(decision, "tracked files excluded from Git change detection");
            return Ok(());
        }
        if !read_git(path, &["status", "--porcelain"])?.is_empty() {
            retain(decision, "uncommitted or untracked files");
            return Ok(());
        }
        if let Err(reason) = disposable_artifacts(path) {
            retain(decision, reason.to_string());
            return Ok(());
        }
        for root in [".lf/logs", ".lf/runs", ".lf/sessions"] {
            if path.join(root).try_exists()? {
                retain(decision, "local Session evidence");
                return Ok(());
            }
        }
    }
    if missing {
        decision
            .evidence
            .push("interrupted cleanup: exact administrative HEAD".into());
    }
    decision.action = CleanupAction::ValidateCheckout;
    Ok(())
}

/// A removal-intent file lives with Git's registration and disappears with it.
/// Only an absent checkout with this exact registration can use it for repair.
fn interrupted_removal(repo: &Path, path: &Path) -> OpsResult<Option<(PathBuf, CleanupDecision)>> {
    let root = git_directory(repo, "--git-common-dir")?.join("worktrees");
    if !root.try_exists()? {
        return Ok(None);
    }
    for entry in std::fs::read_dir(root)? {
        let admin = entry?.path();
        let marker = admin.join("lf-cleanup.json");
        if !marker.try_exists()? {
            continue;
        }
        if normalized(Path::new(
            std::fs::read_to_string(admin.join("gitdir"))?.trim(),
        ))? != path.join(".git")
        {
            continue;
        }
        let started: CleanupDecision =
            serde_json::from_slice(&std::fs::read(marker)?).map_err(error)?;
        if started.path == path && started.action == CleanupAction::RemoveCheckout {
            return Ok(Some((admin, started)));
        }
    }
    Ok(None)
}

fn record_removal(decision: &CleanupDecision) -> OpsResult<()> {
    let admin = git_directory(&decision.path, "--absolute-git-dir")?;
    let temporary = admin.join("lf-cleanup.tmp");
    crate::ops::cron::write_private_file(
        &temporary,
        &serde_json::to_vec(decision).map_err(error)?,
    )?;
    std::fs::rename(temporary, admin.join("lf-cleanup.json"))?;
    std::fs::File::open(admin)?.sync_all()?;
    Ok(())
}

fn git_directory(repo: &Path, option: &str) -> OpsResult<PathBuf> {
    let path =
        PathBuf::from(read_git(repo, &["rev-parse", "--path-format=absolute", option])?.trim());
    if !path.is_absolute() {
        return Err(error("Git returned a non-absolute administrative path"));
    }
    Ok(path)
}

fn main_repo_root(repo: &Path) -> OpsResult<PathBuf> {
    git_directory(repo, "--git-common-dir")?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| error("Git common directory has no parent"))
}

fn list_porcelain(repo: &Path) -> OpsResult<Vec<(PathBuf, Option<String>)>> {
    io::read(io::Read::Checkouts(repo.to_path_buf()))
}

// Filesystem-sensitive Git reads must not hold up every later candidate.
fn read_git(path: &Path, args: &[&str]) -> OpsResult<String> {
    let mut command = Command::new("git");
    command
        .current_dir(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0");
    let output = io::output(&mut command, Duration::from_secs(2))?;
    // NUL-delimited filenames may start with whitespace. Only scalar readers
    // may trim their output; artifact classification must preserve exact paths.
    String::from_utf8(output.stdout).map_err(error)
}

/// A cache tag is an explicit tool contract, not a guess from a directory name.
/// Only wholly ignored directories qualify; tracked source is never an artifact.
fn disposable_artifacts(path: &Path) -> OpsResult<Vec<PathBuf>> {
    let ignored = read_git(
        path,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
    )?;
    let mut roots = Vec::new();
    for relative in ignored.split('\0').filter(|entry| !entry.is_empty()) {
        let root = path.join(relative);
        if !relative.ends_with('/')
            || relative.starts_with(".lf/")
            || !std::fs::symlink_metadata(&root)?.is_dir()
            || !has_cache_tag(&root)?
            || !read_git(path, &["ls-files", "-z", "--", relative])?.is_empty()
        {
            return Err(error(format!("unclassified ignored content: {relative}")));
        }
        roots.push(root);
    }
    Ok(roots)
}

fn has_cache_tag(root: &Path) -> OpsResult<bool> {
    let tag = root.join("CACHEDIR.TAG");
    if !std::fs::symlink_metadata(&tag).is_ok_and(|meta| meta.is_file()) {
        return Ok(false);
    }
    let mut signature = [0; 43];
    match std::fs::File::open(tag)?.read_exact(&mut signature) {
        Ok(()) => Ok(&signature == b"Signature: 8a477f597d28d172789f06886806bc55"),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Keep the declaration until its contents are gone, so interrupted deletion
/// remains classifiable on the next pass without a second ownership registry.
fn remove_artifact(root: &Path) -> OpsResult<()> {
    if !std::fs::symlink_metadata(root)?.is_dir() || !has_cache_tag(root)? {
        return Err(error("artifact declaration changed before removal"));
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_name() == "CACHEDIR.TAG" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        } else {
            std::fs::remove_file(entry.path())?;
        }
    }
    // Leave the tag for Git's final non-forced removal. A crash at any point
    // leaves a valid declaration, rather than an unclassifiable empty root.
    Ok(())
}

pub fn plan_cleanup(store: &SharedStore, repo: &Path) -> OpsResult<Vec<CleanupDecision>> {
    plan_selected(store, repo, None)
}

/// Allocated bytes, without following symlinks or crossing filesystems. A
/// partial traversal is unknown, never a misleading zero or partial sum.
fn estimate_bytes(path: &Path, deadline: Instant) -> Option<u64> {
    let remaining = deadline.checked_duration_since(Instant::now())?;
    let output = crate::ops::read_retry::bounded_output(
        Command::new("du").args(["-skx"]).arg(path),
        remaining.min(Duration::from_secs(1)),
    )
    .ok()?;
    if !output.stderr.is_empty() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

fn plan_selected(
    store: &SharedStore,
    repo: &Path,
    selected: Option<&Path>,
) -> OpsResult<Vec<CleanupDecision>> {
    let repo = main_repo_root(repo)?;
    let selected = selected
        .map(|path| io::read::<PathBuf>(io::Read::Normalize(path.to_path_buf())))
        .transpose()?;
    let external = running_paths();
    let registered = list_porcelain(&repo)?;
    let mut plan = Vec::new();
    for (path, branch) in registered {
        if selected.as_ref().is_some_and(|selected| *selected != path) {
            continue;
        }
        plan.push(plan_checkout(store, &repo, path, branch, &external));
    }
    Ok(plan)
}

fn plan_checkout(
    store: &SharedStore,
    repo: &Path,
    path: PathBuf,
    branch: Option<String>,
    external: &OpsResult<HashSet<PathBuf>>,
) -> CleanupDecision {
    let mut decision = CleanupDecision {
        path,
        branch,
        observed_head: None,
        action: CleanupAction::Retain("not observed".into()),
        evidence: Vec::new(),
        estimated_bytes: None,
    };
    if let Err(error) = observe(store, repo, &mut decision, external, false) {
        retain(&mut decision, format!("observation unavailable: {error}"));
    }
    decision
}

pub fn apply_cleanup(
    store: &SharedStore,
    repo: &Path,
    plan: Vec<CleanupDecision>,
    budget: CleanupBudget,
) -> OpsResult<CleanupReport> {
    let repo = main_repo_root(repo)?;
    let mut report = CleanupReport {
        planned: plan.clone(),
        removed: Vec::new(),
        deferred: Vec::new(),
        failed: Vec::new(),
    };
    let started = Instant::now();
    for mut decision in plan {
        if !matches!(decision.action, CleanupAction::Retain(_))
            && (report.removed.len() >= budget.removals
                || started.elapsed() >= budget.admission_time)
        {
            retain(&mut decision, "pass budget exhausted");
        }
        apply_checkout(store, &repo, decision, &mut report);
    }
    Ok(report)
}

/// Prepare and open every file before acquiring anything. Workers return only
/// unlocked descriptors; all admission stays in this parent.
fn admit_checkout(
    store: &SharedStore,
    repo: &Path,
    path: &Path,
) -> OpsResult<(Vec<std::fs::File>, WorktreeLease)> {
    let database = store.sqlite.path().map_err(error)?;
    let mut paths: Vec<(PathBuf, bool)> = io::read(io::Read::Admission {
        database,
        path: path.to_path_buf(),
    })?;
    let release: Option<PathBuf> = io::read(io::Read::ReleaseRegistry)?;
    if let Some(database) = release {
        paths.extend(io::read::<Vec<(PathBuf, bool)>>(io::Read::Admission {
            database,
            path: path.to_path_buf(),
        })?);
    }
    let prepared: PreparedWorktreeLease = io::read(io::Read::Lease {
        repo: repo.to_path_buf(),
        path: path.to_path_buf(),
    })?;
    // Finish every opener before taking the first lock. Each open_lock call
    // waits for worker exit; a canceled opener cannot later acquire admission.
    let files = paths
        .into_iter()
        .map(|(path, exclusive)| io::open_lock(path).map(|file| (file, exclusive)))
        .collect::<OpsResult<Vec<_>>>()?;
    let lease_file = io::open_lock(prepared.lock_path.clone())?;
    let mut locks = Vec::new();
    for (file, exclusive) in files {
        locks.push(
            SqliteStore::acquire_checkout_lock(file, exclusive, Duration::ZERO).map_err(error)?,
        );
    }
    let lease = prepared.acquire(lease_file)?;
    Ok((locks, lease))
}

/// Admission budgets belong to the caller. Once admitted, finish this attempt
/// and its locked recheck before observing another checkout.
fn apply_checkout(
    store: &SharedStore,
    repo: &Path,
    mut decision: CleanupDecision,
    report: &mut CleanupReport,
) {
    if matches!(decision.action, CleanupAction::Retain(_)) {
        report.deferred.push(decision);
        return;
    }
    let (_admission, lease) = match admit_checkout(store, repo, &decision.path) {
        Ok(locks) => locks,
        Err(error) => {
            retain(
                &mut decision,
                format!("cleanup admission unavailable: {error}"),
            );
            report.deferred.push(decision);
            return;
        }
    };
    let result = (|| {
        let expected_head = decision.observed_head.clone();
        let expected_branch = decision.branch.clone();
        // Refresh all authority and filesystem facts under both locks.
        if let Err(error) = observe(store, repo, &mut decision, &running_paths(), true) {
            retain(&mut decision, format!("observation unavailable: {error}"));
            return Ok(false);
        }
        if decision.observed_head != expected_head || decision.branch != expected_branch {
            retain(&mut decision, "checkout changed after planning");
        }
        if decision.action != CleanupAction::RemoveCheckout {
            return Ok(false);
        }
        if decision.path.try_exists()? {
            record_removal(&decision)?;
            for root in disposable_artifacts(&decision.path)? {
                remove_artifact(&root)?;
            }
        }
        worktree_remove_owned(repo, &lease, WorktreeRemoval::Clean, &|_| {})?;
        if let (Some(branch), Some(head)) = (&decision.branch, &decision.observed_head) {
            // Compare-and-delete cannot erase commits added since observation.
            if let Err(error) = super::git(
                repo,
                &["update-ref", "-d", &format!("refs/heads/{branch}"), head],
            ) {
                report.failed.push(CleanupFailure {
                    path: decision.path.clone(),
                    error: format!("checkout removed; local ref retained: {error}"),
                });
            }
        }
        Ok::<_, OpsError>(true)
    })();
    match result {
        Ok(true) => report.removed.push(decision.path),
        Ok(false) => report.deferred.push(decision),
        Err(error) => report.failed.push(CleanupFailure {
            path: decision.path,
            error: error.to_string(),
        }),
    }
}

/// Nonblocking machine-wide exclusion; a missed pass is retried by the next tick.
pub fn run_cleanup_pass(
    store: &SharedStore,
    repo: &Path,
    budget: CleanupBudget,
) -> OpsResult<CleanupReport> {
    let home = if cfg!(test) {
        crate::store::lf_home_dir()
    } else {
        crate::store::production_database_path()
            .parent()
            .expect("database has a parent")
            .to_path_buf()
    };
    let lock = io::open_lock(home.join("checkout-cleanup.lock"))?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            return Ok(CleanupReport {
                planned: Vec::new(),
                removed: Vec::new(),
                deferred: vec![CleanupDecision {
                    path: repo.to_path_buf(),
                    branch: None,
                    observed_head: None,
                    action: CleanupAction::Retain("another cleanup pass is running".into()),
                    evidence: Vec::new(),
                    estimated_bytes: None,
                }],
                failed: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
    }
    let repo = main_repo_root(repo)?;
    // A failed/partial page cannot authorize removal; the history reader below
    // reports incomplete coverage. Do not make unrelated checkout facts disappear.
    if let Err(error) = io::schedule(io::Schedule::ProjectEvidence(
        store.sqlite.path().map_err(error)?,
    )) {
        tracing::warn!(%error, "cleanup history backfill deferred");
    }
    let mut receipt = crate::ops::cron::cleanup::CleanupReceipt::begin(&store.sqlite, &repo)?;
    let mut progress = receipt.progress();
    let result = collect_pass(store, &repo, budget, &mut progress, |progress| {
        receipt.save(progress.clone())
    });
    receipt.finish(progress, result.as_ref().err().map(ToString::to_string))?;
    result
}

fn write_attempt(marker: &Path, at: i64) -> OpsResult<()> {
    io::schedule(io::Schedule::Attempt {
        marker: marker.to_path_buf(),
        at,
    })
}

const REGISTRATION_WINDOW: usize = 32;

/// A registration joined to its retry hint, never removal authority.
#[derive(Debug)]
struct CheckoutAttempt {
    admin: PathBuf,
    path: PathBuf,
    branch: Option<String>,
    at: i64,
}

/// Read a bounded window of registration hints, resuming before any per-entry
/// I/O. The receipt cursor survives unreadable hints and interrupted workers.
/// Scheduling hints order this window only; none are checkout authority.
fn checkout_attempts(
    repo: &Path,
    budget: CleanupBudget,
    progress: &mut CleanupProgress,
    save: &mut impl FnMut(&CleanupProgress) -> OpsResult<()>,
    report: &mut CleanupReport,
) -> OpsResult<Vec<CheckoutAttempt>> {
    let resuming = !progress.pending_registrations.is_empty();
    let (window, remaining) = if resuming {
        // Durable candidates need no new discovery. Resumption does not
        // depend on another repository-wide listing.
        (progress.pending_registrations.clone(), None)
    } else {
        let common = git_directory(repo, "--git-common-dir")?;
        let mut admins: Vec<PathBuf> = io::read(io::Read::Registrations(common))?;
        admins.sort();
        if progress.registration_through.is_none() {
            progress.registration_through = admins.last().cloned();
            save(progress)?;
        }
        if let Some(through) = &progress.registration_through {
            admins.retain(|admin| admin <= through);
        }
        let start = progress
            .registration_after
            .as_ref()
            .map_or(0, |after| admins.partition_point(|admin| admin <= after));
        let window = admins[start..]
            .iter()
            .take(REGISTRATION_WINDOW)
            .cloned()
            .collect();
        (window, Some(admins.len() - start))
    };
    // Setup has its own admission window, so slow reads cannot consume every
    // opportunity to apply the healthy candidates already observed. One admitted
    // read/initialization pair finishes; no checkout locks are held here.
    let deadline = Instant::now() + budget.admission_time.min(Duration::from_secs(5));
    let mut attempts = Vec::new();
    progress.registrations_observed = 0;
    for admin in window.iter().take(REGISTRATION_WINDOW) {
        if Instant::now() >= deadline {
            break;
        }
        if !resuming {
            progress.registration_after = Some(admin.clone());
        }
        // A failed read yields its slot until the next registration sweep.
        progress
            .pending_registrations
            .retain(|pending| pending != admin);
        progress.registrations_observed += 1;
        // An interruption may skip this read until the next sweep. Do not report
        // the hourly cohort complete on that incomplete sweep.
        let pending = progress.full_scan_pending;
        progress.full_scan_pending = true;
        save(progress)?;
        progress.full_scan_pending = pending;
        let attempt = match io::read::<io::Attempt>(io::Read::Attempt(admin.clone())) {
            Ok(attempt) => attempt,
            Err(error) => {
                progress.full_scan_pending = true;
                report.failed.push(CleanupFailure {
                    path: admin.clone(),
                    error: format!("cleanup registration observation unavailable: {error}"),
                });
                continue;
            }
        };
        let at = match attempt.at {
            Some(at) => at,
            None => {
                let now = chrono::Utc::now().timestamp_micros();
                if write_attempt(&admin.join("lf-cleanup-attempt"), now).is_ok() {
                    now
                } else {
                    0
                }
            }
        };
        progress.pending_registrations.push(admin.clone());
        attempts.push(CheckoutAttempt {
            admin: admin.clone(),
            path: attempt.path,
            branch: attempt.branch,
            at,
        });
    }
    if remaining == Some(progress.registrations_observed) {
        progress.registration_after = None;
    }
    save(progress)?;
    Ok(attempts)
}

/// Observe and apply one admitted candidate before starting another. Expensive
/// planning can no longer consume the application budget for an entire batch.
fn collect_pass(
    store: &SharedStore,
    repo: &Path,
    budget: CleanupBudget,
    progress: &mut CleanupProgress,
    mut save: impl FnMut(&CleanupProgress) -> OpsResult<()>,
) -> OpsResult<CleanupReport> {
    if budget.removals == 0 || budget.admission_time.is_zero() {
        return Ok(CleanupReport {
            planned: Vec::new(),
            removed: Vec::new(),
            deferred: Vec::new(),
            failed: Vec::new(),
        });
    }
    let now = chrono::Utc::now().timestamp();
    let mut report = CleanupReport {
        planned: Vec::new(),
        removed: Vec::new(),
        deferred: Vec::new(),
        failed: Vec::new(),
    };
    progress.observed = 0;
    progress.removed = 0;
    progress.deferred = 0;
    progress.failed = 0;
    let mut attempts = checkout_attempts(repo, budget, progress, &mut save, &mut report)?;
    let setup_finished = progress.registration_after.is_none();
    if progress.full_scan_started.is_none()
        && progress
            .full_scan_at
            .is_none_or(|last| now.saturating_sub(last) >= 3600)
    {
        // Freeze the discovery cohort. Later arrivals cannot extend this scan
        // forever; settled arrivals still enter the ordinary retry queue.
        progress.full_scan_started = Some(chrono::Utc::now().timestamp_micros());
        save(progress)?;
    }
    let cutoff = progress.full_scan_started;
    let in_scan = |attempt: &CheckoutAttempt| cutoff.is_some_and(|cutoff| attempt.at <= cutoff);
    let mut pending_scan = attempts.iter().filter(|attempt| in_scan(attempt)).count();
    // Within this setup window, persisted last-attempt times put old deferrals
    // ahead of arrivals. The registration cursor covers the other windows;
    // neither that cursor nor these hints can authorize removal.
    attempts.sort_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then_with(|| left.path.cmp(&right.path))
    });
    // Failed publications consume their receipt slot before I/O. Remaining
    // members survive the next pass, so neither a stalled oldest hint nor a new
    // setup window can restart candidate coverage.
    // Setup must not spend the admission window before the first candidate
    // can persist its attempt. Each admitted application still finishes.
    let deadline = Instant::now() + budget.admission_time;
    // Empty cheap ticks need no execution or Session history observations.
    let external = std::cell::OnceCell::new();
    for attempt in &attempts {
        let CheckoutAttempt {
            admin,
            path,
            branch,
            ..
        } = attempt;
        if Instant::now() >= deadline || report.removed.len() >= budget.removals {
            break;
        }
        progress.observed += 1;
        progress
            .pending_registrations
            .retain(|pending| pending != admin);
        let pending = progress.full_scan_pending;
        progress.full_scan_pending |= pending_scan > 0;
        save(progress)?;
        progress.full_scan_pending = pending;
        if in_scan(attempt) {
            pending_scan -= 1;
        }
        if !in_scan(attempt) {
            // This positive-only fast lane uses stored checkout keys, not a
            // filesystem walk of every settled owner. Aliases and missed rows
            // remain covered by the hourly registration scan. A failed lookup
            // yields this candidate, never certifies discovery complete.
            let settled = store.sqlite.path().map_err(error).and_then(|database| {
                io::read::<bool>(io::Read::Settled {
                    database,
                    path: path.clone(),
                })
            });
            match settled {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    progress.full_scan_pending = true;
                    report.failed.push(CleanupFailure {
                        path: path.clone(),
                        error: format!("cleanup settlement discovery unavailable: {error}"),
                    });
                    continue;
                }
            }
        }
        if let Err(error) = write_attempt(
            &admin.join("lf-cleanup-attempt"),
            chrono::Utc::now().timestamp_micros(),
        ) {
            let decision = CleanupDecision {
                path: path.clone(),
                branch: branch.clone(),
                observed_head: None,
                action: CleanupAction::Retain(format!("cleanup scheduling unavailable: {error}")),
                evidence: Vec::new(),
                estimated_bytes: None,
            };
            report.planned.push(decision.clone());
            report.deferred.push(decision);
            progress.deferred = report.deferred.len();
            continue;
        }
        let mut decision = plan_checkout(
            store,
            repo,
            path.clone(),
            branch.clone(),
            external.get_or_init(running_paths),
        );
        if decision.action == CleanupAction::ValidateCheckout {
            // Size is optional; it cannot prevent this admitted removal.
            decision.estimated_bytes = estimate_bytes(&decision.path, deadline);
        }
        report.planned.push(decision.clone());
        apply_checkout(store, repo, decision, &mut report);
        progress.removed = report.removed.len();
        progress.deferred = report.deferred.len();
        progress.failed = report.failed.len();
    }
    progress.full_scan_pending |= pending_scan > 0;
    if setup_finished && progress.pending_registrations.is_empty() {
        progress.registration_through = None;
        if cutoff.is_some() && !progress.full_scan_pending {
            progress.full_scan_at = Some(now);
            progress.full_scan_started = None;
        }
        progress.full_scan_pending = false;
    }
    progress.failed = report.failed.len();
    save(progress)?;
    Ok(report)
}

/// Keep filesystem work off the async lifecycle caller. One worker owns the
/// entire attempt and its locks, even if that caller stops waiting.
pub(crate) async fn cleanup_path(store: &SharedStore, repo: &Path, path: &Path) -> OpsResult<()> {
    let (store, repo, path) = (store.clone(), repo.to_path_buf(), path.to_path_buf());
    tokio::task::spawn_blocking(move || {
        if let Err(error) = io::schedule(io::Schedule::ProjectEvidence(
            store.sqlite.path().map_err(error)?,
        )) {
            tracing::warn!(%error, "cleanup history backfill deferred");
        }
        let plan = plan_selected(&store, &repo, Some(&path))?;
        let report = apply_cleanup(&store, &repo, plan, CleanupBudget::default())?;
        for decision in report.deferred {
            if let CleanupAction::Retain(reason) = decision.action {
                eprintln!("retained {}: {reason}", decision.path.display());
            }
        }
        for failure in report.failed {
            eprintln!("cleanup {}: {}", failure.path.display(), failure.error);
        }
        Ok(())
    })
    .await
    .map_err(error)?
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use loopflow_test_support::TestRepo;
    use time::OffsetDateTime;

    use super::{
        apply_cleanup, observe, plan_cleanup, CleanupAction, CleanupBudget, CleanupDecision,
    };
    use crate::engine::git::{acquire_worktree_lease, rev_parse};
    use crate::ops::wt::git;
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};

    async fn fixture() -> (TestRepo, tempfile::TempDir, SharedStore, PathBuf) {
        let repo = TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(directory.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let path = repo.create_named_worktree("landed").canonicalize().unwrap();
        // The ignore rule is source; ignored customer data is deliberately not source.
        std::fs::write(path.join(".gitignore"), "target/\n target/\nprivate/\n").unwrap();
        git(&path, &["add", ".gitignore"]).unwrap();
        git(&path, &["commit", "-m", "ignore generated files"]).unwrap();
        record_settlement(&directory, &path, "landed");
        (repo, directory, store, path)
    }

    fn decision(store: &SharedStore, repo: &TestRepo, path: &std::path::Path) -> CleanupDecision {
        let mut decision = CleanupDecision {
            path: path.to_path_buf(),
            branch: None,
            observed_head: None,
            action: CleanupAction::Retain("unobserved".into()),
            evidence: Vec::new(),
            estimated_bytes: None,
        };
        observe(store, repo.path(), &mut decision, &Ok(HashSet::new()), true).unwrap();
        decision
    }

    fn retained(decision: &CleanupDecision, reason: &str) {
        assert!(
            matches!(&decision.action, CleanupAction::Retain(message) if message.contains(reason)),
            "{decision:?}"
        );
    }

    fn add_settled(repo: &TestRepo, directory: &tempfile::TempDir, name: &str) -> PathBuf {
        let path = repo.create_named_worktree(name).canonicalize().unwrap();
        record_settlement(directory, &path, name);
        path
    }

    fn record_settlement(directory: &tempfile::TempDir, path: &std::path::Path, branch: &str) {
        let head = rev_parse(path, "HEAD").unwrap();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        conn.execute("INSERT INTO pr_landings(id,repo,pr_number,worktree,branch,requested_head_sha,observed_head_sha,merge_commit,state,generation,created_at,updated_at)
            VALUES(?1,'test/repo',2,?2,?1,?3,?3,?3,'merged',1,1,1)",
            rusqlite::params![branch, path.to_str().unwrap(), head]).unwrap();
    }

    #[tokio::test]
    async fn cleanup_collection_repairs_only_its_interrupted_registration() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let unrelated = add_settled(&repo, &directory, "unrelated");
        let initial = decision(&store, &repo, &path);
        super::record_removal(&initial).unwrap();
        // Simulate interruption after checkout deletion, before Git unregisters it.
        std::fs::remove_dir_all(&path).unwrap();
        std::fs::remove_dir_all(&unrelated).unwrap();
        let before = git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap();
        let plan = plan_cleanup(&store, repo.path()).unwrap();
        assert_eq!(
            git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap(),
            before
        );
        assert_eq!(
            plan.iter().find(|item| item.path == path).unwrap().action,
            CleanupAction::ValidateCheckout
        );
        retained(
            plan.iter().find(|item| item.path == unrelated).unwrap(),
            "missing checkout",
        );
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path.clone()], "{report:?}");
        let after = git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap();
        assert!(!after.contains(path.to_str().unwrap()));
        assert!(after.contains(unrelated.to_str().unwrap()));
    }

    #[tokio::test]
    async fn cleanup_collection_interrupted_registration_rechecks_new_commits() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, _directory, store, path) = fixture().await;
        super::record_removal(&decision(&store, &repo, &path)).unwrap();
        git(&path, &["commit", "--allow-empty", "-m", "new source"]).unwrap();
        std::fs::remove_dir_all(&path).unwrap();
        retained(&decision(&store, &repo, &path), "registration changed");
        assert!(git(repo.path(), &["rev-parse", "landed"]).is_ok());
    }

    #[tokio::test]
    async fn cleanup_collection_passes_advance_past_slow_candidates_and_reconcile_hourly() {
        use std::time::Duration;
        let _guard = crate::journal::TestLedgerGuard::new();
        let external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let slow = add_settled(&repo, &directory, "aaa-slow");
        std::fs::write(slow.join("unfinished"), "keep").unwrap();
        let mut eligible = vec![path];
        for name in ["bbb", "ccc", "ddd"] {
            eligible.push(add_settled(&repo, &directory, name));
        }
        let unowned = repo
            .create_named_worktree("unowned")
            .canonicalize()
            .unwrap();
        // Both a blocked and an eligible candidate outlast admission. The former
        // must yield its place; the latter must finish its admitted removal.
        external.wrap_git(&format!("if {{ [ \"$PWD\" = '{}' ] || [ \"$PWD\" = '{}' ]; }} && [ \"$1\" = status ]; then sleep 0.3; fi", slow.display(), eligible[1].display()));
        let budget = CleanupBudget {
            removals: 1,
            admission_time: Duration::from_millis(200),
        };
        let mut removed = Vec::new();
        for _ in 0..12 {
            let report = super::run_cleanup_pass(&store, repo.path(), budget).unwrap();
            assert!(report.removed.len() <= 1);
            removed.extend(report.removed);
            if eligible.iter().all(|path| !path.exists()) {
                break;
            }
        }
        assert_eq!(removed.len(), eligible.len());
        assert!(slow.join("unfinished").exists());
        assert!(unowned.exists());
        // Drain the saved window, finish discovery, then complete a fresh
        // reconciliation sweep. Short earlier ticks may have stopped in any of
        // those phases; a single pass is not a full-scan completion contract.
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        for _ in 0..4 {
            super::collect_pass(
                &store,
                repo.path(),
                CleanupBudget::default(),
                &mut progress,
                |_| Ok(()),
            )
            .unwrap();
        }
        assert!(progress.full_scan_at.is_some());
        let cheap = super::collect_pass(
            &store,
            repo.path(),
            CleanupBudget::default(),
            &mut progress,
            |_| Ok(()),
        )
        .unwrap();
        assert!(!cheap.planned.iter().any(|item| item.path == unowned));
        progress.full_scan_at = Some(chrono::Utc::now().timestamp() - 3601);
        let hourly = super::collect_pass(
            &store,
            repo.path(),
            CleanupBudget::default(),
            &mut progress,
            |_| Ok(()),
        )
        .unwrap();
        assert!(hourly.planned.iter().any(|item| item.path == unowned));
        receipt.finish(progress, None).unwrap();
    }

    #[tokio::test]
    async fn cleanup_oldest_deferrals_precede_continual_arrivals() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, first) = fixture().await;
        let second = add_settled(&repo, &directory, "z-older-deferred");
        for path in [&first, &second] {
            std::fs::write(path.join("unfinished"), "keep").unwrap();
        }
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        let older: Vec<_> = report
            .deferred
            .iter()
            .filter(|d| d.path == first || d.path == second)
            .map(|d| d.path.clone())
            .collect();
        assert_eq!(older.len(), 2);
        for path in &older {
            std::fs::remove_file(path.join("unfinished")).unwrap();
        }
        let admin = crate::engine::git::absolute_git_dir(&older[0]).unwrap();
        std::fs::write(admin.join("lf-cleanup-attempt"), "interrupted hint").unwrap();
        // Arrival rate exceeds the removal cap; both lexically earlier and
        // later names must wait behind the already deferred checkouts.
        for (tick, expected) in older.iter().enumerate() {
            for prefix in ["a", "zz", "m"] {
                add_settled(&repo, &directory, &format!("{prefix}-arrival-{tick}"));
            }
            let report = super::run_cleanup_pass(
                &store,
                repo.path(),
                CleanupBudget {
                    removals: 1,
                    ..CleanupBudget::default()
                },
            )
            .unwrap();
            assert_eq!(&report.removed, &vec![expected.clone()]);
        }
    }

    fn begin_receipt(
        store: &SharedStore,
        repo: &TestRepo,
    ) -> crate::ops::cron::cleanup::CleanupReceipt {
        // Use maintenance's canonical repository key, not the fixture's alias.
        crate::ops::cron::cleanup::CleanupReceipt::begin(
            &store.sqlite,
            &super::main_repo_root(repo.path()).unwrap(),
        )
        .unwrap()
    }

    fn declared_cache(path: &Path) -> PathBuf {
        let cache = path.join("target");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(
            cache.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
        cache
    }

    fn fifo(path: &Path) {
        assert!(std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success());
    }

    fn assert_no_fifo_reader(path: &Path) {
        use std::os::unix::fs::OpenOptionsExt;
        let error = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .unwrap_err();
        assert_eq!(
            error.raw_os_error(),
            Some(libc::ENXIO),
            "timed-out worker still reads the FIFO"
        );
    }

    #[tokio::test]
    async fn cleanup_setup_leaves_primary_out_of_retry_scheduling() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let primary = repo.path().canonicalize().unwrap();
        let plan = plan_cleanup(&store, repo.path()).unwrap();
        retained(
            plan.iter().find(|item| item.path == primary).unwrap(),
            "primary checkout",
        );

        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
        assert!(report.planned.iter().all(|item| item.path != primary));
        assert!(!crate::engine::git::absolute_git_dir(&primary)
            .unwrap()
            .join("lf-cleanup-attempt")
            .exists());
        assert!(primary.exists());
    }

    #[tokio::test]
    async fn cleanup_setup_resumes_saved_candidates_without_repository_discovery() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        progress.pending_registrations.push(admin.clone());
        progress.registration_after = Some(admin.clone());
        progress.registration_through = Some(admin);
        receipt.save(progress.clone()).unwrap();
        let mut progress = begin_receipt(&store, &repo).progress();
        external.wrap_git("exit 1");
        let mut report = super::CleanupReport {
            planned: Vec::new(),
            removed: Vec::new(),
            deferred: Vec::new(),
            failed: Vec::new(),
        };
        let attempts = super::checkout_attempts(
            repo.path(),
            CleanupBudget::default(),
            &mut progress,
            &mut |_| Ok(()),
            &mut report,
        )
        .unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].path, path);
        assert_eq!(attempts[0].branch.as_deref(), Some("landed"));
        assert_eq!(
            progress.pending_registrations,
            receipt.progress().pending_registrations
        );
        assert!(path.exists(), "resumed setup grants no removal authority");
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_read_releases_workers_and_preserves_neighbors() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "healthy-neighbor");
        let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
        let marker = admin.join("lf-cleanup-attempt");
        fifo(&marker);
        let started = std::time::Instant::now();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        assert_eq!(report.removed, vec![neighbor]);
        assert!(path.exists());
        assert_no_fifo_reader(&marker);
        // The timed-out process neither owns admission nor has a continuation
        // into deletion. Both ownership domains are immediately available.
        drop(store.sqlite.lock_checkout(&path).unwrap());
        drop(
            crate::engine::git::acquire_worktree_lease(repo.path(), &path, "test after timeout")
                .unwrap(),
        );
        std::fs::remove_file(&marker).unwrap();
        assert!(path.exists());
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_registrations_resume_across_interruption_and_arrivals() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, healthy) = fixture().await;
        let mut stalled = Vec::new();
        for index in 0..8 {
            let path = add_settled(&repo, &directory, &format!("a-stalled-{index:02}"));
            let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
            fifo(&admin.join("lf-cleanup-attempt"));
            stalled.push((path, admin));
        }
        let budget = CleanupBudget {
            removals: 1,
            admission_time: std::time::Duration::from_secs(1),
        };
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        let interrupted =
            super::collect_pass(&store, repo.path(), budget, &mut progress, |progress| {
                receipt.save(progress.clone())?;
                if progress.registrations_observed == 1 {
                    return Err(super::error("interrupted before registration read"));
                }
                Ok(())
            });
        assert!(interrupted.is_err());
        drop(receipt);
        let resumed = begin_receipt(&store, &repo).progress();
        assert_eq!(resumed.registration_after, progress.registration_after);
        assert!(resumed.full_scan_pending);
        let mut retried = HashSet::new();
        for tick in 0..24 {
            // More arrivals than removals, on both sides of the setup cursor.
            for prefix in ["0", "m", "zzz"] {
                add_settled(&repo, &directory, &format!("{prefix}-arrival-{tick:02}"));
            }
            let started = std::time::Instant::now();
            let report = super::run_cleanup_pass(&store, repo.path(), budget).unwrap();
            assert!(
                started.elapsed() < std::time::Duration::from_secs(6),
                "a pass must not read all eight two-second FIFOs"
            );
            assert!(report.removed.len() <= 1);
            retried.extend(report.failed.iter().map(|failure| failure.path.clone()));
            assert!(stalled.iter().all(|(path, _)| path.exists()));
            if !healthy.exists() && stalled.iter().all(|(_, admin)| retried.contains(admin)) {
                break;
            }
        }
        assert!(
            !healthy.exists(),
            "healthy collection must pass stalled setup"
        );
        assert!(
            stalled.iter().all(|(_, admin)| retried.contains(admin)),
            "including the registration skipped by interruption"
        );
        for (path, admin) in &stalled {
            assert_no_fifo_reader(&admin.join("lf-cleanup-attempt"));
            drop(store.sqlite.lock_checkout(path).unwrap());
        }
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_hint_writer_yields_first_admission_slot() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, healthy) = fixture().await;
        let blocked = add_settled(&repo, &directory, "aaa-stalled");
        let marker = crate::engine::git::absolute_git_dir(&blocked)
            .unwrap()
            .join("lf-cleanup-attempt");
        std::fs::write(&marker, "0").unwrap();
        std::env::set_var("LF_TEST_CLEANUP_STALL_HINT", &marker);
        let result = (|| {
            for _ in 0..4 {
                super::run_cleanup_pass(
                    &store,
                    repo.path(),
                    CleanupBudget {
                        removals: 1,
                        admission_time: std::time::Duration::from_millis(300),
                    },
                )?;
                if !healthy.exists() {
                    break;
                }
            }
            Ok::<_, super::OpsError>(())
        })();
        std::env::remove_var("LF_TEST_CLEANUP_STALL_HINT");
        result.unwrap();
        assert!(
            !healthy.exists(),
            "a timed-out oldest hint must yield admission"
        );
        assert!(blocked.exists());
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "0");
    }

    #[tokio::test]
    async fn cleanup_setup_candidate_continuation_crosses_windows_with_stalls_and_arrivals() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, first) = fixture().await;
        let mut original = vec![first];
        let mut stalled = Vec::new();
        let mut healthy = Vec::new();
        for prefix in ["a", "b"] {
            for index in 0..32 {
                let path = add_settled(&repo, &directory, &format!("{prefix}{index:02}"));
                let marker = crate::engine::git::absolute_git_dir(&path)
                    .unwrap()
                    .join("lf-cleanup-attempt");
                std::fs::write(&marker, "0").unwrap();
                if index == 0 {
                    stalled.push(marker);
                } else if index == 1 {
                    healthy.push(path.clone());
                } else {
                    // Keep the finite cohort present across multiple sweeps.
                    std::fs::write(path.join("unfinished"), "preserve").unwrap();
                }
                original.push(path);
            }
        }
        let budget = CleanupBudget {
            removals: 1,
            admission_time: std::time::Duration::from_millis(1900),
        };
        std::env::set_var(
            "LF_TEST_CLEANUP_STALL_HINT",
            std::env::join_paths(&stalled).unwrap(),
        );
        let result = (|| {
            let first = super::run_cleanup_pass(&store, repo.path(), budget)?;
            assert!(first
                .deferred
                .iter()
                .any(|decision| decision.branch.as_deref() == Some("a00")));
            assert!(
                first.removed.is_empty(),
                "stalled publication consumes candidate deadline"
            );
            let mut attempted: HashSet<_> = first.planned.into_iter().map(|d| d.path).collect();
            let mut receipt = begin_receipt(&store, &repo);
            let mut progress = receipt.progress();
            assert_eq!(
                progress.registrations_observed, 32,
                "fixture crosses the count cap, not only the setup deadline"
            );
            assert!(!progress.pending_registrations.is_empty());
            assert!(
                progress.registration_after.is_some(),
                "fixture exceeds setup cap"
            );
            let interrupted =
                super::collect_pass(&store, repo.path(), budget, &mut progress, |progress| {
                    receipt.save(progress.clone())?;
                    if progress.observed == 1 {
                        return Err(super::error(
                            "interrupted after candidate continuation publication",
                        ));
                    }
                    Ok(())
                });
            assert!(interrupted.is_err());
            drop(receipt);
            assert_eq!(
                begin_receipt(&store, &repo)
                    .progress()
                    .pending_registrations,
                progress.pending_registrations
            );
            for tick in 0..32 {
                for prefix in ["0", "m", "zzz"] {
                    add_settled(&repo, &directory, &format!("{prefix}-arrival-{tick:02}"));
                }
                let report = super::run_cleanup_pass(&store, repo.path(), budget)?;
                attempted.extend(report.planned.into_iter().map(|decision| decision.path));
                assert!(report.removed.len() <= 1);
                let progress = begin_receipt(&store, &repo).progress();
                assert!(progress.pending_registrations.len() <= 32);
                if healthy.iter().all(|path| !path.exists())
                    && original.iter().all(|path| attempted.contains(path))
                {
                    break;
                }
            }
            assert!(
                healthy.iter().all(|path| !path.exists()),
                "both windows must collect despite interruption and heavier arrivals"
            );
            assert!(
                original.iter().all(|path| attempted.contains(path)),
                "the interrupted candidate and failed publications must retry"
            );
            for marker in &stalled {
                assert_eq!(std::fs::read_to_string(marker).unwrap(), "0");
            }
            Ok::<_, super::OpsError>(())
        })();
        std::env::remove_var("LF_TEST_CLEANUP_STALL_HINT");
        result.unwrap();
    }

    #[tokio::test]
    async fn cleanup_setup_interrupted_hint_write_retries_without_blocking_neighbors() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "healthy-neighbor");
        let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
        let marker = admin.join("lf-cleanup-attempt");
        std::fs::write(&marker, "0").unwrap();
        std::env::set_var("LF_TEST_CLEANUP_STALL_HINT", &marker);
        let result = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default());
        std::env::remove_var("LF_TEST_CLEANUP_STALL_HINT");
        let report = result.unwrap();
        assert_eq!(report.removed, vec![neighbor]);
        retained(
            report
                .deferred
                .iter()
                .find(|decision| decision.path == path)
                .unwrap(),
            "scheduling unavailable",
        );
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "0");
        drop(store.sqlite.lock_checkout(&path).unwrap());
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_setup_deadlines_never_cancel_admitted_removal() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let plan = super::plan_selected(&store, repo.path(), Some(&path)).unwrap();
        external.wrap_git("if [ \"$3\" = worktree ] && [ \"$4\" = remove ]; then sleep 3; fi");
        let started = std::time::Instant::now();
        let report = apply_cleanup(
            &store,
            repo.path(),
            plan,
            CleanupBudget {
                removals: 1,
                admission_time: std::time::Duration::from_millis(100),
            },
        )
        .unwrap();
        assert_eq!(report.removed, vec![path], "{report:?}");
        assert!(started.elapsed() >= std::time::Duration::from_secs(3));
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_receipt_read_does_not_strand_the_next_pass() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let receipt = begin_receipt(&store, &repo);
        let root = crate::ops::cron::receipt_root(&store.sqlite.home_dir().unwrap());
        let recorded = crate::ops::cron::list_cron_receipts(&root, "", None, 1)
            .unwrap()
            .pop()
            .unwrap();
        let file = root
            .join(&recorded.flow)
            .join(format!("{}-{}.json", recorded.started_at, recorded.id));
        let bytes = std::fs::read(&file).unwrap();
        std::fs::remove_file(&file).unwrap();
        fifo(&file);
        let started = std::time::Instant::now();
        assert!(super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert!(path.exists());
        assert_no_fifo_reader(&file);
        drop(store.sqlite.lock_checkout(&path).unwrap());
        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, bytes).unwrap();
        drop(receipt);
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_setup_receipt_interruption_resumes_published_progress() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, directory, store, path) = fixture().await;
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        progress.pending_registrations = vec![path.clone()];
        progress.observed = 7;
        let stall = directory.path().join("receipt-response");
        fifo(&stall);
        std::env::set_var("LF_TEST_CLEANUP_STALL_RECEIPT", &stall);
        let result = receipt.save(progress.clone());
        std::env::remove_var("LF_TEST_CLEANUP_STALL_RECEIPT");
        assert!(result.is_err());
        assert_no_fifo_reader(&stall);
        // A lost acknowledgment cannot lead to a second writer for this receipt.
        assert!(receipt
            .finish(super::CleanupProgress::initial(), None)
            .is_err());
        let next = begin_receipt(&store, &repo);
        let resumed = next.progress();
        assert_eq!(resumed.sequence, progress.sequence + 1);
        assert_eq!(
            resumed.pending_registrations,
            progress.pending_registrations
        );
        assert_eq!(resumed.observed, 7);
        assert!(path.exists());
        drop(store.sqlite.lock_checkout(&path).unwrap());
    }

    #[tokio::test]
    async fn cleanup_unavailable_scheduling_hint_does_not_block_neighbors() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "healthy-neighbor");
        let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
        // A directory cannot be replaced by the atomic hint-file rename.
        std::fs::create_dir(admin.join("lf-cleanup-attempt")).unwrap();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![neighbor]);
        retained(
            report.deferred.iter().find(|d| d.path == path).unwrap(),
            "scheduling unavailable",
        );
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cleanup_failed_hints_beyond_a_window_allow_progress_and_retry_after_interruption() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, first) = fixture().await;
        let mut blocked = vec![first];
        for index in 0..40 {
            blocked.push(add_settled(
                &repo,
                &directory,
                &format!("a-blocked-{index:02}"),
            ));
        }
        for path in &blocked {
            let admin = crate::engine::git::absolute_git_dir(path).unwrap();
            std::fs::create_dir(admin.join("lf-cleanup-attempt")).unwrap();
        }
        let healthy = add_settled(&repo, &directory, "zz-healthy");
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        let interrupted = super::collect_pass(
            &store,
            repo.path(),
            CleanupBudget::default(),
            &mut progress,
            |progress| {
                receipt.save(progress.clone())?;
                if progress.observed == 12 {
                    return Err(super::error("interrupted after durable admission"));
                }
                Ok(())
            },
        );
        assert!(interrupted.is_err());
        drop(receipt);
        let mut attempted = HashSet::new();
        for tick in 0..6 {
            // Arrivals exceed a one-removal pass, but must not restart coverage.
            for prefix in ["0", "m", "zzz"] {
                add_settled(&repo, &directory, &format!("{prefix}-arrival-{tick}"));
            }
            let report =
                super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
            attempted.extend(report.planned.iter().map(|d| d.path.clone()));
            assert!(report.planned.len() <= 32);
            assert!(blocked.iter().all(|path| path.exists()));
            if !healthy.exists() && blocked.iter().all(|path| attempted.contains(path)) {
                break;
            }
        }
        assert!(
            !healthy.exists(),
            "failed hints must not starve a healthy neighbor"
        );
        assert!(
            blocked.iter().all(|path| attempted.contains(path)),
            "failed candidates must also get fair retries"
        );
        let recovered = blocked.last().unwrap();
        let admin = crate::engine::git::absolute_git_dir(recovered).unwrap();
        std::fs::remove_dir(admin.join("lf-cleanup-attempt")).unwrap();
        for _ in 0..4 {
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
            if !recovered.exists() {
                break;
            }
        }
        assert!(!recovered.exists(), "a repaired hint must allow collection");
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_settled_path_does_not_defer_healthy_collection() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let external = ExternalInspection::idle();
        for hourly in [false, true] {
            let (repo, directory, store, healthy) = fixture().await;
            let stalled = add_settled(&repo, &directory, "aaa-stalled");
            let fifo_path = directory.path().join("stalled-status");
            fifo(&fifo_path);
            external.wrap_git(&format!(
                "if [ \"$PWD\" = '{}' ] && [ \"$1\" = status ]; then cat '{}'; fi",
                stalled.display(),
                fifo_path.display()
            ));
            // A historical settled owner outside this repository cannot be
            // normalized. It must not participate in this candidate's lookup.
            let alias = directory.path().join("unresolvable-owner");
            std::os::unix::fs::symlink(&alias, &alias).unwrap();
            let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
            conn.execute("INSERT INTO pr_landings(id,repo,pr_number,worktree,branch,requested_head_sha,observed_head_sha,merge_commit,state,generation,created_at,updated_at)
                VALUES('unrelated','other/repo',3,?1,'other','head','head','head','merged',1,1,1)", [alias.to_str().unwrap()]).unwrap();
            let mut receipt = begin_receipt(&store, &repo);
            let mut progress = receipt.progress();
            if !hourly {
                progress.full_scan_at = Some(chrono::Utc::now().timestamp());
            }
            receipt.save(progress).unwrap();
            let report =
                super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
            assert_eq!(report.removed, vec![healthy], "{report:?}");
            retained(
                report
                    .deferred
                    .iter()
                    .find(|item| item.path == stalled)
                    .unwrap(),
                "observation unavailable",
            );
            assert!(stalled.exists());
            assert_no_fifo_reader(&fifo_path);
            drop(store.sqlite.lock_checkout(&stalled).unwrap());
            external.wrap_git("");
            let report =
                super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
            assert_eq!(report.removed, vec![stalled]);
        }
    }

    #[tokio::test]
    async fn cleanup_setup_settlement_lookup_failure_is_candidate_local() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "healthy-neighbor");
        let mut receipt = begin_receipt(&store, &repo);
        let mut progress = receipt.progress();
        progress.full_scan_at = Some(chrono::Utc::now().timestamp());
        receipt.save(progress).unwrap();
        let fifo_path = directory.path().join("settlement-read");
        fifo(&fifo_path);
        std::env::set_var("LF_TEST_CLEANUP_STALL_SETTLED", &path);
        std::env::set_var("LF_TEST_CLEANUP_STALL_SETTLED_FIFO", &fifo_path);
        let result = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default());
        std::env::remove_var("LF_TEST_CLEANUP_STALL_SETTLED");
        std::env::remove_var("LF_TEST_CLEANUP_STALL_SETTLED_FIFO");
        let report = result.unwrap();
        assert_eq!(report.removed, vec![neighbor]);
        assert!(report.failed.iter().any(|failure| failure.path == path
            && failure.error.contains("settlement discovery unavailable")));
        assert!(path.exists());
        assert_no_fifo_reader(&fifo_path);
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_setup_lock_preparation_timeout_never_owns_admission() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "healthy-neighbor");
        let lease =
            crate::engine::git::PreparedWorktreeLease::discover(repo.path(), &path).unwrap();
        let fifo_path = directory.path().join("lock-ack");
        fifo(&fifo_path);
        // Interrupt both before file preparation and after descriptor transfer.
        for phase in ["before", "after"] {
            std::env::set_var("LF_TEST_CLEANUP_STALL_LOCK", &lease.lock_path);
            std::env::set_var("LF_TEST_CLEANUP_STALL_LOCK_FIFO", &fifo_path);
            std::env::set_var("LF_TEST_CLEANUP_STALL_LOCK_PHASE", phase);
            let result = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default());
            std::env::remove_var("LF_TEST_CLEANUP_STALL_LOCK");
            std::env::remove_var("LF_TEST_CLEANUP_STALL_LOCK_FIFO");
            std::env::remove_var("LF_TEST_CLEANUP_STALL_LOCK_PHASE");
            let report = result.unwrap();
            assert!(path.exists());
            assert!(!neighbor.exists());
            retained(
                report
                    .deferred
                    .iter()
                    .find(|item| item.path == path)
                    .unwrap(),
                "admission unavailable",
            );
            assert_no_fifo_reader(&fifo_path);
            drop(store.sqlite.lock_checkout(&path).unwrap());
            drop(acquire_worktree_lease(repo.path(), &path, "after canceled opener").unwrap());
        }
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_setup_stalled_gitdir_is_local_to_its_registration() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, healthy) = fixture().await;
        std::fs::write(healthy.join("unfinished"), "preserve").unwrap();
        let blocked = add_settled(&repo, &directory, "a-blocked");
        let admin = crate::engine::git::absolute_git_dir(&blocked).unwrap();
        let gitdir = admin.join("gitdir");
        let contents = std::fs::read(&gitdir).unwrap();
        std::fs::remove_file(&gitdir).unwrap();
        fifo(&gitdir);
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(report.failed.iter().any(|failure| failure.path == admin));
        retained(
            report
                .deferred
                .iter()
                .find(|decision| decision.path == healthy)
                .unwrap(),
            "uncommitted",
        );
        assert!(blocked.exists());
        assert_no_fifo_reader(&gitdir);
        std::fs::remove_file(&gitdir).unwrap();
        std::fs::write(&gitdir, contents).unwrap();
        std::fs::remove_file(healthy.join("unfinished")).unwrap();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(report.removed.contains(&healthy));
        assert!(report.removed.contains(&blocked));
    }

    #[tokio::test]
    async fn cleanup_apply_rechecks_registration_backlink() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let neighbor = add_settled(&repo, &directory, "neighbor");
        let plan = super::plan_selected(&store, repo.path(), Some(&path)).unwrap();
        let admin = crate::engine::git::absolute_git_dir(&path).unwrap();
        std::fs::write(
            admin.join("gitdir"),
            neighbor.join(".git").to_str().unwrap(),
        )
        .unwrap();
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "registration changed");
        assert!(path.exists());
        assert!(neighbor.exists());
    }

    #[tokio::test]
    async fn cleanup_setup_does_not_require_aggregate_git_listing() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        external.wrap_git("if [ \"$1\" = worktree ] && [ \"$2\" = list ]; then sleep 30; fi");
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
        // Manual previews still use Git's full listing, in a read-only worker.
        let started = std::time::Instant::now();
        assert!(super::list_porcelain(repo.path()).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert!(repo.path().exists());
    }

    #[test]
    fn cleanup_size_estimate_is_unknown_on_deadline_or_failed_observation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("payload"), vec![1_u8; 8192]).unwrap();
        let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(1);
        assert!(super::estimate_bytes(dir.path(), deadline()).unwrap() >= 8192);
        assert_eq!(
            super::estimate_bytes(dir.path(), std::time::Instant::now()),
            None
        );
        assert_eq!(
            super::estimate_bytes(&dir.path().join("absent"), deadline()),
            None
        );
    }

    struct ExternalInspection {
        _directory: tempfile::TempDir,
        previous_path: std::ffi::OsString,
    }

    impl ExternalInspection {
        fn wrap_git(&self, script: &str) {
            use std::os::unix::fs::PermissionsExt;
            let real_git = std::env::split_paths(&self.previous_path)
                .map(|path| path.join("git"))
                .find(|path| path.is_file())
                .unwrap();
            let path = self._directory.path().join("git");
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\n{script}\nexec '{}' \"$@\"\n",
                    real_git.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn idle() -> Self {
            use std::os::unix::fs::PermissionsExt;
            let directory = tempfile::tempdir().unwrap();
            let script = directory.path().join("lsof");
            std::fs::write(&script, "#!/bin/sh\nprintf 'p1\\nn/\\n'\n").unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            let previous_path = std::env::var_os("PATH").unwrap();
            let mut paths = vec![directory.path().to_path_buf()];
            paths.extend(std::env::split_paths(&previous_path));
            std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
            Self {
                _directory: directory,
                previous_path,
            }
        }
    }

    impl Drop for ExternalInspection {
        fn drop(&mut self) {
            std::env::set_var("PATH", &self.previous_path);
        }
    }

    #[tokio::test]
    async fn cleanup_collection_receipts_are_bounded_and_resume_the_cursor() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, _directory, store, _path) = fixture().await;
        for sequence in 1..=15 {
            let mut receipt = begin_receipt(&store, &repo);
            let mut progress = receipt.progress();
            assert_eq!(progress.sequence, sequence);
            if sequence > 1 {
                assert_eq!(progress.full_scan_started, Some(42));
            }
            progress.full_scan_started = Some(42);
            receipt.finish(progress, None).unwrap();
        }
        let root = crate::ops::cron::receipt_root(&store.sqlite.home_dir().unwrap());
        let receipts = crate::ops::cron::list_cron_receipts(&root, "", None, 1).unwrap();
        assert!(receipts.len() <= 11, "{} receipts", receipts.len());
        assert_eq!(
            receipts
                .iter()
                .filter_map(|r| r.cleanup.as_ref())
                .map(|p| p.sequence)
                .max(),
            Some(15)
        );
    }

    #[tokio::test]
    async fn cleanup_requires_completed_task_delivery_even_with_a_settled_landing() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let task = crate::durable::TaskId::new();
        let wave = crate::id::WaveId::new();
        let project = crate::durable::ProjectId::new();
        let pr = crate::work::task::TaskPrId::new();
        let head = rev_parse(&path, "HEAD").unwrap();
        {
            let conn = rusqlite::Connection::open(_directory.path().join("loopflow.db")).unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'cleanup',?2,1)",
                rusqlite::params![wave, repo.path().to_string_lossy()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)",
                rusqlite::params![project.as_str(), wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,branch,base_commit,created_at,updated_at,issue_title,issue_description,pm_snapshot_synced_at,planning_completed) VALUES(?1,?2,'issue','CLEAN-1',?3,'landed','landed',?4,1,1,'Findings','Accepted findings',1,1)",
                rusqlite::params![task.as_str(), project.as_str(), path.to_string_lossy(), head],
            )
            .unwrap();
        }
        retained(&decision(&store, &repo, &path), "PR-less");
        {
            let conn = rusqlite::Connection::open(_directory.path().join("loopflow.db")).unwrap();
            conn.execute(
                "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,github_number,github_url,github_head_sha,merge_commit,created_at,updated_at) VALUES(?1,?2,1,'landed','landed',?3,1,1,'https://github.com/example/repo/pull/1',?3,?3,1,1)",
                rusqlite::params![pr.as_str(), task.as_str(), head],
            )
            .unwrap();
        }
        retained(&decision(&store, &repo, &path), "unresolved delivery");
        store
            .sqlite
            .finish_follow_through(&task, "No remaining scope", true)
            .unwrap();
        assert_eq!(
            decision(&store, &repo, &path).action,
            CleanupAction::RemoveCheckout
        );
    }

    #[tokio::test]
    async fn cleanup_retries_unknown_execution_then_removes_once_without_deleting_remote() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        git(&path, &["push", "origin", "landed"]).unwrap();
        let mut process = crate::process::LfProcess {
            lfid: crate::id::ProcessLfid::new(),
            pid: None,
            trace_id: crate::id::TraceId::new(),
            parent_process_lfid: None,
            via_agent: None,
            caller_session_id: None,
            caller_provider_generation: None,
            command: Some("unknown execution".into()),
            repo: None,
            cwd: Some(path.to_string_lossy().into_owned()),
            started_at: OffsetDateTime::now_utc().unix_timestamp(),
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        store.sqlite.record_process(&process).unwrap();
        let first = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(first.removed.is_empty());
        retained(
            first
                .deferred
                .iter()
                .find(|item| item.path == path)
                .unwrap(),
            "unknown execution",
        );
        process.completed_at = Some(OffsetDateTime::now_utc().unix_timestamp());
        process.outcome = Some("succeeded".into());
        process.exit_code = Some(0);
        store.sqlite.record_process(&process).unwrap();
        let second =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(second.failed.is_empty(), "{:?}", second.failed);
        assert_eq!(second.removed, std::slice::from_ref(&path));
        assert!(!path.exists());
        assert!(git(repo.path(), &["show-ref", "--verify", "refs/heads/landed"]).is_err());
        assert!(
            !git(repo.path(), &["ls-remote", "--heads", "origin", "landed"])
                .unwrap()
                .is_empty()
        );
        let third = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(third.removed.is_empty());
    }

    #[tokio::test]
    async fn cleanup_removes_only_tagged_artifacts_and_retries_interrupted_artifact_removal() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        std::fs::create_dir(path.join("target")).unwrap();
        std::fs::write(
            path.join("target/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n",
        )
        .unwrap();
        std::fs::write(path.join("target/artifact"), "regenerable").unwrap();
        let plan = plan_cleanup(&store, repo.path()).unwrap();
        // A crash after removing one artifact but before Git removal leaves
        // ordinary registered ownership; the next pass needs no cleanup queue.
        super::remove_artifact(&path.join("target")).unwrap();
        assert!(!path.join("target/artifact").exists());
        assert!(path.join("target/CACHEDIR.TAG").exists());
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, std::slice::from_ref(&path), "{report:?}");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn cleanup_retains_ignored_user_data_and_uncommitted_files() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::create_dir(path.join("private")).unwrap();
        std::fs::write(path.join("private/results"), "irreplaceable").unwrap();
        retained(&decision(&store, &repo, &path), "unclassified ignored");
        std::fs::remove_dir_all(path.join("private")).unwrap();
        std::fs::write(path.join("notes"), "uncommitted").unwrap();
        retained(&decision(&store, &repo, &path), "uncommitted");
        assert!(path.join("notes").exists());
    }

    #[tokio::test]
    async fn cleanup_never_trims_ignored_paths_into_declared_caches() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        declared_cache(&path);
        std::fs::create_dir(path.join(" target")).unwrap();
        std::fs::write(path.join(" target/results"), "irreplaceable").unwrap();

        let plan = plan_cleanup(&store, repo.path()).unwrap();
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == path)
                .unwrap(),
            "unclassified ignored content:  target/",
        );
        assert_eq!(
            std::fs::read_to_string(path.join(" target/results")).unwrap(),
            "irreplaceable"
        );
    }

    #[tokio::test]
    async fn cleanup_requires_valid_cache_tags_and_never_classifies_tracked_source_as_cache() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::create_dir(path.join("target")).unwrap();
        std::fs::write(path.join("target/build"), "regenerable").unwrap();
        std::fs::write(path.join("target/CACHEDIR.TAG"), "invalid").unwrap();
        retained(&decision(&store, &repo, &path), "unclassified ignored");
        std::fs::write(
            path.join("target/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n",
        )
        .unwrap();
        assert_eq!(
            decision(&store, &repo, &path).action,
            CleanupAction::RemoveCheckout
        );
        assert_eq!(
            super::disposable_artifacts(&path).unwrap(),
            [path.join("target")]
        );
        git(&path, &["add", "-f", "target/build"]).unwrap();
        assert!(super::disposable_artifacts(&path).is_err());
    }

    #[tokio::test]
    async fn cleanup_retains_changes_hidden_from_git_status() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, _directory, store, path) = fixture().await;
        git(&path, &["update-index", "--assume-unchanged", ".gitignore"]).unwrap();
        std::fs::write(path.join(".gitignore"), "uncommitted hidden edit").unwrap();
        assert!(crate::engine::git::is_clean(&path).unwrap());
        retained(
            &decision(&store, &repo, &path),
            "excluded from Git change detection",
        );
    }

    #[tokio::test]
    async fn cleanup_retains_new_commits_after_merge_even_when_remote_is_gone() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        git(&path, &["commit", "--allow-empty", "-m", "new work"]).unwrap();
        retained(&decision(&store, &repo, &path), "no recorded settlement");
    }

    #[tokio::test]
    async fn cleanup_preserves_persistent_and_unowned_checkouts() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        git(
            &path,
            &["config", "branch.landed.loopflow-persistent", "true"],
        )
        .unwrap();
        retained(&decision(&store, &repo, &path), "persistent");
        let legacy = repo.create_named_worktree("legacy").canonicalize().unwrap();
        retained(
            &decision(&store, &repo, &legacy),
            "unknown Loopflow ownership",
        );
    }

    #[tokio::test]
    async fn cleanup_process_inspection_failure_and_running_descendants_retain_checkout() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let mut item = decision(&store, &repo, &path);
        observe(
            &store,
            repo.path(),
            &mut item,
            &Err(super::error("inspection failed")),
            true,
        )
        .unwrap();
        retained(&item, "inspection failed");
        observe(
            &store,
            repo.path(),
            &mut item,
            &Ok(HashSet::from([path.join("nested")])),
            true,
        )
        .unwrap();
        retained(&item, "running external process");
        observe(&store, repo.path(), &mut item, &Ok(HashSet::new()), true).unwrap();
        assert_eq!(item.action, CleanupAction::RemoveCheckout);
        assert_eq!(item.evidence, ["landing: exact merged head"]);
    }

    #[tokio::test]
    async fn cleanup_targeted_pass_only_plans_the_selected_checkout() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let neighbor = repo.create_named_worktree("unfinished");
        std::fs::write(neighbor.join("notes"), "keep me").unwrap();

        let plan = super::plan_selected(&store, repo.path(), Some(&path)).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].path, path);
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, [path]);
        assert_eq!(
            std::fs::read_to_string(neighbor.join("notes")).unwrap(),
            "keep me"
        );
    }

    #[tokio::test]
    async fn cleanup_apply_rechecks_head_and_obeys_checkout_lease() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let item = decision(&store, &repo, &path);
        let lease = acquire_worktree_lease(repo.path(), &path, "other writer").unwrap();
        let report = apply_cleanup(
            &store,
            repo.path(),
            vec![item.clone()],
            CleanupBudget::default(),
        )
        .unwrap();
        assert!(report.failed.is_empty());
        retained(&report.deferred[0], "admission unavailable");
        assert!(path.exists());
        drop(lease);
        let admission = store.sqlite.lock_checkout(&path).unwrap();
        let report = apply_cleanup(
            &store,
            repo.path(),
            vec![item.clone()],
            CleanupBudget::default(),
        )
        .unwrap();
        assert!(report.failed.is_empty());
        retained(&report.deferred[0], "admission unavailable");
        assert!(path.exists());
        drop(admission);
        git(
            &path,
            &["commit", "--allow-empty", "-m", "post-preview work"],
        )
        .unwrap();
        let report =
            apply_cleanup(&store, repo.path(), vec![item], CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "changed after planning");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cleanup_retains_provider_and_lf_homes_inside_a_declared_cache() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, directory, store, path) = fixture().await;
        let home = path.join("target/provider");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            path.join("target/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
        std::fs::write(home.join("history.jsonl"), "retained conversation").unwrap();
        // The home itself protects this checkout even when its native layout
        // cannot be traversed. A positive match needs no complete inventory.
        std::fs::write(home.join("sessions"), "unreadable layout").unwrap();
        let connection = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        connection.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,routing_state,created_at,updated_at) VALUES('codex','account',?1,'missing','disabled',1,1)", [home.to_str().unwrap()]).unwrap();
        retained(&decision(&store, &repo, &path), "Session evidence");
        connection
            .execute("DELETE FROM provider_accounts", [])
            .unwrap();
        std::env::set_var("LF_HOME", &home);
        retained(&decision(&store, &repo, &path), "Session evidence");
        assert_eq!(
            std::fs::read_to_string(home.join("history.jsonl")).unwrap(),
            "retained conversation"
        );
    }

    #[tokio::test]
    async fn cleanup_preserves_native_transcripts_reached_through_account_symlinks() {
        let guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        guard.set_db_path(directory.path().join("loopflow.db"));
        std::env::set_var("LF_HOME", directory.path());
        let initial = decision(&store, &repo, &path);
        let cache = declared_cache(&path);
        let id = "0199a213-81c0-7800-8aa1-bbab2a035a54";
        let payload = cache.join("native.jsonl");
        let transcript = format!(
            "{}\n",
            serde_json::json!({"type":"session_meta", "payload":{"id":id,"cwd":repo.path()}})
        );
        std::fs::write(&payload, &transcript).unwrap();
        let home = directory.path().join("native-account");
        let day = home.join("sessions/2026/10/09");
        std::fs::create_dir_all(&day).unwrap();
        std::os::unix::fs::symlink(&payload, day.join(format!("rollout-2026-10-09-{id}.jsonl")))
            .unwrap();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        conn.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,routing_state,created_at,updated_at) VALUES('codex','native-account',?1,'missing','disabled',1,1)", [home.to_str().unwrap()]).unwrap();
        let preview = plan_cleanup(&store, repo.path()).unwrap();
        assert_eq!(
            preview
                .iter()
                .find(|item| item.path == path)
                .unwrap()
                .action,
            CleanupAction::ValidateCheckout
        );
        let report = apply_cleanup(&store, repo.path(), preview, CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == path)
                .unwrap(),
            "Session evidence",
        );
        // A previously fully observed decision grants no stale authority either.
        let report =
            apply_cleanup(&store, repo.path(), vec![initial], CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "Session evidence");
        assert_eq!(std::fs::read_to_string(payload).unwrap(), transcript);
        // No recorded identity exists: admit must discover and read the native
        // rollout to recover its working directory. This does not launch a provider.
        let resumed = crate::ops::human_session::provider_conversation::admit(&store, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resumed.cwd, repo.path());
        assert_eq!(resumed.provider.as_deref(), Some("codex"));
    }

    #[tokio::test]
    async fn cleanup_preserves_historical_payloads_even_after_a_new_capture() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let initial = decision(&store, &repo, &path);
        let cache = declared_cache(&path);
        let payload = cache.join("past-conversation.jsonl");
        std::fs::write(&payload, "retained provider history\n").unwrap();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('past','past','generated',1,1,?1)", [repo.path().to_str().unwrap()]).unwrap();
        for input in [
            "00000000000000000000000000000001",
            "00000000000000000000000000000002",
        ] {
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [input]).unwrap();
        }
        conn.execute(
            "UPDATE agent_sessions SET current_capture=last_insert_rowid() WHERE id='past'",
            [],
        )
        .unwrap();
        let capture =
            crate::session_record::record_dir(directory.path(), "00000000000000000000000000000001")
                .unwrap();
        std::fs::create_dir_all(&capture).unwrap();
        std::os::unix::fs::symlink(&payload, capture.join("provider.jsonl")).unwrap();
        let evidence = serde_json::json!({"input_id":"00000000000000000000000000000001", "source":"runs", "evidence":{"provider_session_path":capture.join("provider.jsonl")}});
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed','00000000000000000000000000000001:runs',1,?1)", [evidence.to_string()]).unwrap();
        let native_id = "0199a213-81c0-7800-8aa1-bbab2a035a53";
        let reference = serde_json::json!({"source":"provider-session:fixture", "evidence": {
            "schema_version":1, "provider_session_id":native_id, "account_id":null
        }});
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed','00000000000000000000000000000001:provider-session:fixture',1,?1)", [reference.to_string()]).unwrap();
        retained(&decision(&store, &repo, &path), "Session evidence");
        // Evidence recorded after planning must also veto the destructive recheck.
        let report =
            apply_cleanup(&store, repo.path(), vec![initial], CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "Session evidence");
        assert_eq!(
            std::fs::read_to_string(&payload).unwrap(),
            "retained provider history\n"
        );
        assert_eq!(
            std::fs::read_to_string(capture.join("provider.jsonl")).unwrap(),
            "retained provider history\n"
        );
        let historical = store
            .sqlite
            .input_provider_session("00000000000000000000000000000001")
            .unwrap()
            .unwrap();
        assert_eq!(historical.agent_session.as_str(), native_id);
        // The native resume lookup still resolves the older provider identity
        // to its Session, even though that Session now has a newer capture.
        let resumed = crate::ops::human_session::provider_conversation::admit(&store, native_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resumed.id, "past");
        assert_eq!(resumed.artifact_key, "00000000000000000000000000000002");
    }

    #[tokio::test]
    async fn cleanup_history_pages_eventually_collect_and_resolve_retargeted_paths() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let disposable = add_settled(&repo, &directory, "disposable");
        let cache = declared_cache(&path);
        let payload = cache.join("history.jsonl");
        std::fs::write(&payload, "preserved history").unwrap();
        let alias = directory.path().join("historical-payload");
        std::os::unix::fs::symlink(directory.path().join("outside"), &alias).unwrap();
        let conn = captures(&directory, 520);
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed','00000000000000000000000000000000:runs',1,?1)",
            [serde_json::json!({"evidence":{"provider_session_path":alias}}).to_string()]).unwrap();
        // Rebuild the derived projection, as after upgrading a populated store.
        conn.execute_batch("BEGIN; DELETE FROM session_evidence; UPDATE session_evidence_backfill SET through_seq=0,target_seq=(SELECT MAX(seq) FROM session_events),complete=0; COMMIT;").unwrap();
        for tick in 0..3 {
            if tick == 1 {
                conn.execute_batch("CREATE TRIGGER interrupt_cleanup_projection BEFORE INSERT ON session_evidence WHEN NEW.event_seq>300 AND NEW.event_seq<600 BEGIN SELECT RAISE(ABORT,'interrupted page'); END;").unwrap();
            }
            let report =
                super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
            assert!(report.removed.is_empty());
            assert!(path.exists() && disposable.exists());
            if tick == 1 {
                assert_eq!(
                    conn.query_row(
                        "SELECT through_seq FROM session_evidence_backfill",
                        [],
                        |row| row.get::<_, i64>(0)
                    )
                    .unwrap(),
                    256
                );
                conn.execute_batch("DROP TRIGGER interrupt_cleanup_projection")
                    .unwrap();
            }
            // Source triggers must project heavier arrivals without extending
            // the frozen historical cohort or concealing the interrupted page.
            let tx = conn.unchecked_transaction().unwrap();
            for index in (10000 + tick * 600)..(10600 + tick * 600) {
                tx.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [format!("{index:032x}")]).unwrap();
            }
            tx.commit().unwrap();
        }
        // The next maintenance page resumes from durable coverage.
        store.sqlite.advance_session_evidence().unwrap();
        let before_retarget = decision(&store, &repo, &path);
        assert_eq!(before_retarget.action, CleanupAction::RemoveCheckout);
        std::fs::remove_file(&alias).unwrap();
        std::os::unix::fs::symlink(&payload, &alias).unwrap();
        let report = apply_cleanup(
            &store,
            repo.path(),
            vec![before_retarget],
            CleanupBudget::default(),
        )
        .unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "Session evidence");
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![disposable]);
        assert_eq!(
            std::fs::read_to_string(payload).unwrap(),
            "preserved history"
        );
    }

    /// Native homes owned by the test, so an observation's amount of work is
    /// the fixture's and never the developer's own provider history.
    struct NativeHomes {
        directory: tempfile::TempDir,
        previous: [Option<std::ffi::OsString>; 2],
    }

    impl NativeHomes {
        const ENV: [&'static str; 2] = ["CODEX_HOME", "CLAUDE_CONFIG_DIR"];

        fn isolated() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let previous = Self::ENV.map(std::env::var_os);
            std::env::set_var("CODEX_HOME", directory.path().join("codex"));
            std::env::set_var("CLAUDE_CONFIG_DIR", directory.path().join("claude"));
            Self {
                directory,
                previous,
            }
        }

        fn codex_day(&self) -> PathBuf {
            let day = self.directory.path().join("codex/sessions/2026/10/09");
            std::fs::create_dir_all(&day).unwrap();
            day
        }
    }

    impl Drop for NativeHomes {
        fn drop(&mut self) {
            for hook in [
                "LF_TEST_CLEANUP_EVIDENCE_GATE",
                "LF_TEST_CLEANUP_EVIDENCE_GATE_AT",
                "LF_TEST_CLEANUP_EVIDENCE_DELAY_MICROS",
            ] {
                std::env::remove_var(hook);
            }
            for (name, value) in Self::ENV.iter().zip(&self.previous) {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn captures(directory: &tempfile::TempDir, count: usize) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('past','past','generated',1,1,'/')", []).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for index in 0..count {
            tx.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [format!("{index:032x}")]).unwrap();
        }
        tx.commit().unwrap();
        conn
    }

    #[tokio::test]
    async fn cleanup_evidence_healthy_history_outlasts_the_stall_deadline() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let homes = NativeHomes::isolated();
        let (repo, directory, store, path) = fixture().await;
        let payload = declared_cache(&path).join("native.jsonl");
        std::fs::write(&payload, "preserved transcript").unwrap();
        let outside = directory.path().join("outside.jsonl");
        std::fs::write(&outside, "elsewhere").unwrap();
        // Complete history and a symlink-heavy native layout: about 3,000
        // units of healthy work, each slower than a millisecond.
        let _history = captures(&directory, 300);
        let day = homes.codex_day();
        for index in 0..200 {
            std::os::unix::fs::symlink(&outside, day.join(format!("rollout-{index:03}.jsonl")))
                .unwrap();
        }
        // The one positive reference is far behind the old whole-set deadline.
        let late = day.join("rollout-zzz.jsonl");
        std::os::unix::fs::symlink(&payload, &late).unwrap();
        std::env::set_var("LF_TEST_CLEANUP_EVIDENCE_DELAY_MICROS", "1000");
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == path)
                .unwrap(),
            "Session evidence",
        );
        assert_eq!(
            std::fs::read_to_string(&payload).unwrap(),
            "preserved transcript"
        );
        std::fs::remove_file(&late).unwrap();
        std::os::unix::fs::symlink(&outside, &late).unwrap();
        let started = std::time::Instant::now();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        // Same data, same two-second stall deadline: only useful work renewed it.
        assert!(started.elapsed() > std::time::Duration::from_secs(3));
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_evidence_stall_or_interruption_retains_and_a_later_pass_collects() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let _homes = NativeHomes::isolated();
        let (repo, directory, store, path) = fixture().await;
        let payload = declared_cache(&path).join("build-output");
        std::fs::write(&payload, "regenerable").unwrap();
        let _history = captures(&directory, 40);
        let gate = directory.path().join("evidence-gate");
        fifo(&gate);
        std::env::set_var("LF_TEST_CLEANUP_EVIDENCE_GATE", &gate);
        std::env::set_var("LF_TEST_CLEANUP_EVIDENCE_GATE_AT", "60");
        // A real blocked open after useful work: nothing answers the FIFO.
        let started = std::time::Instant::now();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "observation unavailable");
        // The stream ends cleanly but without its completed observation.
        let writer = std::thread::spawn({
            let gate = gate.clone();
            move || std::fs::write(gate, "exit").unwrap()
        });
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        writer.join().unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "observation was interrupted");
        assert_eq!(std::fs::read_to_string(&payload).unwrap(), "regenerable");
        // The released attempt left admission available and no negative fact.
        std::env::remove_var("LF_TEST_CLEANUP_EVIDENCE_GATE_AT");
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    async fn cleanup_evidence_revalidates_references_changed_behind_the_stream() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let _homes = NativeHomes::isolated();
        let (repo, directory, store, path) = fixture().await;
        let payload = declared_cache(&path).join("past-conversation.jsonl");
        std::fs::write(&payload, "retained provider history").unwrap();
        let conn = captures(&directory, 40);
        let reference = |target: &Path| {
            serde_json::json!({"evidence":{"provider_session_path":target}}).to_string()
        };
        let first = format!("{:032x}", 0);
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed',?1,1,?2)",
            rusqlite::params![format!("{first}:runs"), reference(&directory.path().join("outside"))]).unwrap();
        // Rebuild the projection so the edited reference leads the stream.
        conn.execute_batch("BEGIN; DELETE FROM session_evidence; INSERT INTO session_evidence(event_seq,capture_key,raw_paths) SELECT seq,capture_key,raw_paths FROM session_evidence_source ORDER BY seq DESC; COMMIT;").unwrap();
        let gate = directory.path().join("evidence-gate");
        fifo(&gate);
        std::env::set_var("LF_TEST_CLEANUP_EVIDENCE_GATE", &gate);
        std::env::set_var("LF_TEST_CLEANUP_EVIDENCE_GATE_AT", "120");
        let statements: [(&str, String); 2] = [
            (
                "UPDATE session_events SET payload=?2 WHERE receipt_key=?1",
                format!("{first}:runs"),
            ),
            (
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed',?1,1,?2)",
                format!("{first}:terminal.json"),
            ),
        ];
        for (statement, key) in statements {
            // The worker is past the early rows when this write commits.
            let report = std::thread::scope(|scope| {
                let writer = scope.spawn(|| {
                    let open = std::fs::OpenOptions::new().write(true).open(&gate).unwrap();
                    rusqlite::Connection::open(directory.path().join("loopflow.db"))
                        .unwrap()
                        .execute(statement, rusqlite::params![key, reference(&payload)])
                        .unwrap();
                    drop(open);
                });
                let report =
                    super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
                writer.join().unwrap();
                report
            });
            assert!(report.removed.is_empty());
            retained(&report.deferred[0], "Session evidence");
            conn.execute(
                "UPDATE session_events SET payload=?2 WHERE receipt_key=?1",
                rusqlite::params![key, reference(&directory.path().join("outside"))],
            )
            .unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&payload).unwrap(),
            "retained provider history"
        );
        std::env::remove_var("LF_TEST_CLEANUP_EVIDENCE_GATE_AT");
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert_eq!(report.removed, vec![path]);
    }

    #[tokio::test]
    #[ignore = "opt-in evidence cost probe; creates a large disposable history"]
    async fn cleanup_evidence_cost_probe() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let homes = NativeHomes::isolated();
        let (_repo, directory, store, path) = fixture().await;
        let _history = captures(&directory, 65536);
        let day = homes.codex_day();
        for index in 0..65536 {
            std::fs::write(day.join(format!("rollout-{index}.jsonl")), "").unwrap();
        }
        let started = std::time::Instant::now();
        let blocked =
            super::io::observe_evidence(store.sqlite.path().unwrap(), path.clone()).unwrap();
        eprintln!(
            "history rows=65536 native entries=65536 observation={:?}",
            started.elapsed()
        );
        assert!(!blocked);
    }

    #[tokio::test]
    async fn cleanup_preview_defers_unreadable_history_without_granting_removal() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        // Incomplete raw coverage and an unreadable native layout both belong
        // to locked application, not to a foreground recursive preview.
        conn.execute("UPDATE session_evidence_backfill SET complete=0", [])
            .unwrap();
        let home = directory.path().join("native-account");
        std::fs::create_dir(&home).unwrap();
        std::fs::write(home.join("sessions"), "not a directory").unwrap();
        conn.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,routing_state,created_at,updated_at) VALUES('codex','native-account',?1,'missing','disabled',1,1)", [home.to_str().unwrap()]).unwrap();
        for complete in [false, true] {
            conn.execute(
                "UPDATE session_evidence_backfill SET complete=?1",
                [complete],
            )
            .unwrap();
            let plan = plan_cleanup(&store, repo.path()).unwrap();
            assert_eq!(
                plan.iter().find(|item| item.path == path).unwrap().action,
                CleanupAction::ValidateCheckout
            );
            let report =
                apply_cleanup(&store, repo.path(), plan, CleanupBudget::default()).unwrap();
            assert!(report.removed.is_empty());
            retained(
                report
                    .deferred
                    .iter()
                    .find(|item| item.path == path)
                    .unwrap(),
                "observation unavailable",
            );
            assert!(path.exists());
        }
    }

    #[tokio::test]
    async fn cleanup_collection_incomplete_history_never_settles_source() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let dirty = add_settled(&repo, &directory, "dirty");
        std::fs::write(dirty.join("notes"), "unfinished work").unwrap();
        let unowned = repo
            .create_named_worktree("unowned")
            .canonicalize()
            .unwrap();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES('past','past','generated',1,1,?1)", [repo.path().to_str().unwrap()]).unwrap();
        for input in [
            "00000000000000000000000000000001",
            "00000000000000000000000000000002",
        ] {
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','captured',?1,1,'{}')", [input]).unwrap();
        }
        // A later malformed reference invalidates the entire history reading,
        // even after an earlier capture yielded usable paths.
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('past','observed','00000000000000000000000000000002:runs',1,?1)",
            [serde_json::json!({"evidence":{"provider_session_path":17}}).to_string()]).unwrap();
        let report =
            super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default()).unwrap();
        assert!(report.removed.is_empty());
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == path)
                .unwrap(),
            "observation unavailable",
        );
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == unowned)
                .unwrap(),
            "unknown Loopflow ownership",
        );
        retained(
            report
                .deferred
                .iter()
                .find(|item| item.path == dirty)
                .unwrap(),
            "uncommitted",
        );
        assert_eq!(
            std::fs::read_to_string(dirty.join("notes")).unwrap(),
            "unfinished work"
        );
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cleanup_collection_exhausted_budget_keeps_unobserved_candidates_waiting() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, _directory, store, path) = fixture().await;
        let mut progress = super::CleanupProgress::initial();
        let report = super::collect_pass(
            &store,
            repo.path(),
            CleanupBudget {
                removals: 1,
                admission_time: std::time::Duration::ZERO,
            },
            &mut progress,
            |_| Ok(()),
        )
        .unwrap();
        assert!(report.planned.is_empty());
        assert!(report.removed.is_empty());
        assert!(progress.full_scan_at.is_none());
        assert!(progress.full_scan_started.is_none());
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cleanup_apply_budget_preserves_the_plan_and_defers_remaining_checkouts() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, directory, store, path) = fixture().await;
        let next = add_settled(&repo, &directory, "next");
        let plan = vec![
            decision(&store, &repo, &path),
            decision(&store, &repo, &next),
        ];
        let report = apply_cleanup(
            &store,
            repo.path(),
            plan.clone(),
            CleanupBudget {
                removals: 1,
                ..CleanupBudget::default()
            },
        )
        .unwrap();
        assert_eq!(report.planned, plan);
        assert_eq!(report.removed, [path]);
        assert!(report.failed.is_empty());
        assert_eq!(report.deferred.len(), 1);
        assert_eq!(report.deferred[0].path, next);
        retained(&report.deferred[0], "pass budget exhausted");
        assert!(next.exists());
    }

    #[tokio::test]
    async fn cleanup_release_registry_protects_unknown_process_and_admission() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let (repo, directory, _store, path) = fixture().await;
        let release =
            crate::store::sqlite::SqliteStore::open_ephemeral(&directory.path().join("release.db"))
                .unwrap();
        let connection = rusqlite::Connection::open(directory.path().join("release.db")).unwrap();
        connection.execute("INSERT INTO processes(lfid,trace_id,cwd,started_at) VALUES('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002',?1,unixepoch())", [path.to_str().unwrap()]).unwrap();
        assert!(super::RegistryObservations::read(&release)
            .unwrap()
            .blocker(&path)
            .unwrap()
            .unwrap()
            .contains("unknown execution"));
        connection
            .execute(
                "UPDATE processes SET completed_at=2,outcome='succeeded',exit_code=0",
                [],
            )
            .unwrap();
        assert!(super::RegistryObservations::read(&release)
            .unwrap()
            .blocker(&path)
            .unwrap()
            .is_none());
        let reader =
            crate::store::sqlite::SqliteStore::open_read_only(&directory.path().join("release.db"))
                .unwrap();
        let admission = release.lock_checkout(&path).unwrap();
        assert!(reader.lock_checkout(&path).is_err());
        drop(admission);
        assert!(reader.lock_checkout(&path).is_ok());
        assert!(repo.path().exists());
    }

    #[tokio::test]
    async fn cleanup_preview_keeps_missing_git_registration_and_reports_unknown_size() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::remove_dir_all(&path).unwrap();
        let before = git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap();
        let plan = plan_cleanup(&store, repo.path()).unwrap();
        let missing = plan.iter().find(|item| item.path == path).unwrap();
        retained(missing, "missing checkout");
        assert_eq!(missing.estimated_bytes, None);
        assert_eq!(
            git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap(),
            before
        );
    }
}
