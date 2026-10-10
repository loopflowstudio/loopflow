//! Collection is local-only. Missing facts retain a checkout, never imply abandonment.
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::durable::{WorkRef, WorkStatus};
use crate::engine::git::{
    acquire_worktree_lease, current_branch, get_default_branch, is_clean, rev_parse,
};
use crate::engine::worktrees::{is_persistent_worktree, list_porcelain, main_repo_root};
use crate::journal::{process_evidence_at, ProcessIdentityEvidence};
use crate::ops::{OpsError, OpsResult};
use crate::store::{sqlite::SqliteStore, SharedStore};
use crate::work::task::PrPhase;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CleanupAction {
    RemoveCheckout,
    Retain(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupDecision {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub observed_head: Option<String>,
    pub action: CleanupAction,
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

fn normalized(path: &Path) -> PathBuf {
    crate::store::canonicalize_with_missing_tail(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Failure to inspect external execution is unknown, not an empty process set.
fn running_paths() -> OpsResult<HashSet<PathBuf>> {
    let output = crate::ops::read_retry::bounded_output(
        Command::new("lsof").args(["-d", "cwd", "-Fn"]),
        Duration::from_secs(5),
    )?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(error("external process inspection unavailable"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| normalized(&path))
        .collect())
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

fn evidence_roots(store: &SqliteStore) -> OpsResult<Vec<PathBuf>> {
    let mut roots = vec![crate::store::lf_home_dir()];
    if !cfg!(test) {
        roots.push(
            crate::store::production_database_path()
                .parent()
                .expect("database has a parent")
                .to_path_buf(),
        );
    }
    for provider in [
        crate::provider_auth::Provider::Codex,
        crate::provider_auth::Provider::Claude,
    ] {
        roots.push(crate::provider_account::activation::native_home(
            provider, None,
        ));
    }
    roots.extend(
        store
            .list_provider_accounts(None)
            .map_err(error)?
            .into_iter()
            .filter_map(|account| account.home),
    );
    Ok(roots.into_iter().map(|path| normalized(&path)).collect())
}

/// The selected and release registries enforce the same retention policy. Read
/// checkout links and unfinished Processes once per plan, then again under locks.
#[derive(Debug)]
struct RegistryObservations {
    store: SqliteStore,
    home: PathBuf,
    tasks: Vec<crate::store::sqlite::TaskCheckout>,
    open: crate::store::sqlite::task_work::OpenProcesses,
    evidence_roots: Vec<PathBuf>,
}

impl RegistryObservations {
    fn read(store: &SqliteStore) -> OpsResult<Self> {
        let mut tasks = store.task_checkouts().map_err(error)?;
        for task in &mut tasks {
            task.worktree = normalized(&task.worktree);
        }
        Ok(Self {
            store: store.clone(),
            home: store.home_dir().map_err(error)?,
            tasks,
            open: store.open_processes().map_err(error)?,
            evidence_roots: evidence_roots(store)?,
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
        if let Some(process) = self.open.all.iter().find(|process| {
            process
                .cwd
                .as_deref()
                .is_some_and(|cwd| normalized(Path::new(cwd)).starts_with(path))
                && self.execution_blocks(process)
        }) {
            return Ok(Some(format!(
                "Process {} has live or unknown execution",
                process.lfid
            )));
        }
        if self
            .evidence_roots
            .iter()
            .any(|root| root.starts_with(path))
        {
            return Ok(Some("local Session evidence".into()));
        }
        Ok(None)
    }
}

#[derive(Debug)]
struct Observations {
    local: RegistryObservations,
    // A release read failure retains candidates, without obscuring local blockers.
    release: OpsResult<Option<RegistryObservations>>,
    registered: HashSet<PathBuf>,
    default_branch: String,
}

impl Observations {
    fn read(store: &SharedStore, repo: &Path, registered: HashSet<PathBuf>) -> OpsResult<Self> {
        Ok(Self {
            local: RegistryObservations::read(&store.sqlite)?,
            release: release_registry()
                .and_then(|store| store.as_ref().map(RegistryObservations::read).transpose()),
            registered,
            default_branch: get_default_branch(repo)?,
        })
    }
}

fn observe(
    store: &SharedStore,
    repo: &Path,
    decision: &mut CleanupDecision,
    external: &OpsResult<HashSet<PathBuf>>,
) -> OpsResult<()> {
    observe_with_snapshot(
        repo,
        decision,
        external,
        &Observations::read(
            store,
            repo,
            list_porcelain(repo)?
                .into_iter()
                .map(|(path, _)| normalized(&path))
                .collect(),
        )?,
    )
}

fn observe_with_snapshot(
    repo: &Path,
    decision: &mut CleanupDecision,
    external: &OpsResult<HashSet<PathBuf>>,
    snapshot: &Observations,
) -> OpsResult<()> {
    // Each observation stands alone, including a recheck after planning.
    decision.evidence.clear();
    let path = &decision.path;
    if normalized(path) == normalized(repo) {
        retain(decision, "primary checkout");
        return Ok(());
    }
    if !path.try_exists()? {
        // Do not prune another owner's administrative record as a side effect.
        retain(decision, "missing checkout; registration retained");
        return Ok(());
    }
    if normalized(&main_repo_root(path)?) != normalized(repo) || !snapshot.registered.contains(path)
    {
        retain(decision, "checkout registration changed");
        return Ok(());
    }
    decision.branch = current_branch(path)?;
    decision.observed_head = Some(rev_parse(path, "HEAD")?);
    if decision.branch.as_deref() == Some(snapshot.default_branch.as_str()) {
        retain(decision, "default branch");
        return Ok(());
    }
    if is_persistent_worktree(path)? {
        retain(decision, "persistent checkout");
        return Ok(());
    }
    let mut owned = crate::engine::git::absolute_git_dir(path)?
        .join("lf-created")
        .is_file();
    let mut settled = false;
    if let Some(reason) = snapshot.local.blocker(path)? {
        retain(decision, reason);
        return Ok(());
    }
    for task in snapshot.local.tasks_at(path) {
        owned = true;
        for pr in snapshot
            .local
            .store
            .task_prs(&task.task_id)
            .map_err(error)?
        {
            if decision.branch.as_deref() == Some(pr.branch.as_str())
                && pr.phase() == PrPhase::Merged
                && pr.head_sha() == decision.observed_head.as_deref()
            {
                settled = true;
                decision
                    .evidence
                    .push(format!("Task {}: exact merged head", task.issue_identifier));
            }
        }
    }
    if let Some(release) = snapshot.release.as_ref().map_err(error)? {
        // Release facts can veto removal, never settle experimental source.
        if let Some(reason) = release.blocker(path)? {
            retain(decision, format!("{reason} in release registry"));
            return Ok(());
        }
    }
    for landing in snapshot
        .local
        .store
        .merged_landings_at(path)
        .map_err(error)?
    {
        owned = true;
        if decision.branch.as_deref() == Some(landing.branch.as_str())
            && decision.observed_head.as_deref() == Some(landing.observed_head_sha.as_str())
        {
            settled = true;
            decision.evidence.push("landing: exact merged head".into());
        }
    }
    if !owned {
        retain(decision, "unknown Loopflow ownership");
        return Ok(());
    }
    if !settled {
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
    if super::git(path, &["ls-files", "-v"])?.lines().any(|line| {
        line.as_bytes()
            .first()
            .is_some_and(|flag| flag.is_ascii_lowercase() || *flag == b'S')
    }) {
        retain(decision, "tracked files excluded from Git change detection");
        return Ok(());
    }
    if !is_clean(path)? {
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
    decision.action = CleanupAction::RemoveCheckout;
    Ok(())
}

/// A cache tag is an explicit tool contract, not a guess from a directory name.
/// Only wholly ignored directories qualify; tracked source is never an artifact.
fn disposable_artifacts(path: &Path) -> OpsResult<Vec<PathBuf>> {
    let ignored = super::git(
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
            || !super::git(path, &["ls-files", "-z", "--", relative])?.is_empty()
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
    plan_selected(store, repo, None, None)
}

fn plan_selected(
    store: &SharedStore,
    repo: &Path,
    selected: Option<&Path>,
    deadline: Option<Instant>,
) -> OpsResult<Vec<CleanupDecision>> {
    let repo = main_repo_root(repo)?;
    let selected = selected.map(normalized);
    let external = running_paths();
    let registered = list_porcelain(&repo)?;
    let snapshot = Observations::read(
        store,
        &repo,
        registered
            .iter()
            .map(|(path, _)| normalized(path))
            .collect(),
    );
    let mut plan = Vec::new();
    for (path, branch) in registered {
        let path = normalized(&path);
        if selected.as_ref().is_some_and(|selected| *selected != path) {
            continue;
        }
        let mut decision = CleanupDecision {
            path,
            branch,
            observed_head: None,
            action: CleanupAction::Retain("not observed".into()),
            evidence: Vec::new(),
            estimated_bytes: None,
        };
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            retain(&mut decision, "planning budget exhausted");
        } else {
            match &snapshot {
                Ok(snapshot) => {
                    if let Err(error) =
                        observe_with_snapshot(&repo, &mut decision, &external, snapshot)
                    {
                        retain(&mut decision, format!("observation unavailable: {error}"));
                    }
                }
                Err(error) => retain(&mut decision, format!("observation unavailable: {error}")),
            }
        }
        plan.push(decision);
    }
    Ok(plan)
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
        if decision.action != CleanupAction::RemoveCheckout {
            report.deferred.push(decision);
            continue;
        }
        if report.removed.len() >= budget.removals || started.elapsed() >= budget.admission_time {
            retain(&mut decision, "pass budget exhausted");
            report.deferred.push(decision);
            continue;
        }
        let result = (|| {
            let _admission = store.sqlite.lock_checkout(&decision.path).map_err(error)?;
            let release = release_registry()?;
            let _release_admission = release
                .as_ref()
                .map(|release| release.lock_checkout(&decision.path))
                .transpose()
                .map_err(error)?;
            let lease = acquire_worktree_lease(&repo, &decision.path, "checkout cleanup")?;
            let expected_head = decision.observed_head.clone();
            let expected_branch = decision.branch.clone();
            // Refresh all authority and filesystem facts under both locks.
            observe(store, &repo, &mut decision, &running_paths())?;
            if decision.observed_head != expected_head || decision.branch != expected_branch {
                retain(&mut decision, "checkout changed after planning");
            }
            if decision.action != CleanupAction::RemoveCheckout {
                return Ok(false);
            }
            for root in disposable_artifacts(&decision.path)? {
                remove_artifact(&root)?;
            }
            crate::engine::git::worktree_remove_clean_owned(&repo, &decision.path, &lease)?;
            if let (Some(branch), Some(head)) = (&decision.branch, &decision.observed_head) {
                // Compare-and-delete cannot erase commits added since observation.
                if let Err(error) = super::git(
                    &repo,
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
    Ok(report)
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
    std::fs::create_dir_all(&home)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("checkout-cleanup.lock"))?;
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
    let started = Instant::now();
    let plan = plan_selected(store, repo, None, Some(started + budget.admission_time))?;
    apply_cleanup(
        store,
        repo,
        plan,
        CleanupBudget {
            admission_time: budget.admission_time.saturating_sub(started.elapsed()),
            ..budget
        },
    )
}

/// Keep filesystem work off the async lifecycle caller. One worker owns the
/// entire attempt and its locks, even if that caller stops waiting.
pub(crate) async fn cleanup_path(store: &SharedStore, repo: &Path, path: &Path) -> OpsResult<()> {
    let (store, repo, path) = (store.clone(), repo.to_path_buf(), path.to_path_buf());
    tokio::task::spawn_blocking(move || {
        let plan = plan_selected(&store, &repo, Some(&path), None)?;
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
    use std::path::PathBuf;
    use std::sync::Arc;

    use loopflow_test_support::TestRepo;
    use time::OffsetDateTime;

    use super::{
        apply_cleanup, observe, plan_cleanup, CleanupAction, CleanupBudget, CleanupDecision,
    };
    use crate::engine::git::{acquire_worktree_lease, rev_parse};
    use crate::ops::wt::git;
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};

    async fn fixture() -> (TestRepo, tempfile::TempDir, SharedStore, PathBuf) {
        let repo = TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(directory.path().join("store.db")))
                .await
                .unwrap(),
        );
        let path = repo.create_named_worktree("landed").canonicalize().unwrap();
        // The ignore rule is source; ignored customer data is deliberately not source.
        std::fs::write(path.join(".gitignore"), "target/\nprivate/\n").unwrap();
        git(&path, &["add", ".gitignore"]).unwrap();
        git(&path, &["commit", "-m", "ignore generated files"]).unwrap();
        let head = rev_parse(&path, "HEAD").unwrap();
        let landing = PrLanding::new(
            NewPrLanding {
                repo: "test/repo".into(),
                pr_number: 1,
                worktree: path.clone(),
                branch: "landed".into(),
                task_id: None,
                requested_head_sha: head.clone(),
            },
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&landing).await.unwrap();
        let mut landing = store
            .claim_pr_landing(
                &landing.id,
                landing.generation,
                &crate::pr_landing::LandingSupervisor {
                    placement: crate::pr_landing::LandingPlacement::Local,
                    process_id: std::process::id(),
                    heartbeat_at: OffsetDateTime::now_utc(),
                },
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .unwrap()
            .unwrap();
        landing.state = PrLandingState::Merged;
        landing.merge_commit = Some(head);
        assert!(store.update_pr_landing(&landing).await.unwrap());
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
        observe(store, repo.path(), &mut decision, &Ok(HashSet::new())).unwrap();
        decision
    }

    fn retained(decision: &CleanupDecision, reason: &str) {
        assert!(
            matches!(&decision.action, CleanupAction::Retain(message) if message.contains(reason)),
            "{decision:?}"
        );
    }

    struct ExternalInspection {
        _directory: tempfile::TempDir,
        previous_path: std::ffi::OsString,
    }

    impl ExternalInspection {
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
    async fn cleanup_requires_completed_task_delivery_even_with_a_settled_landing() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let task = crate::durable::TaskId::new();
        let wave = crate::id::WaveId::new();
        let project = crate::durable::ProjectId::new();
        let pr = crate::work::task::TaskPrId::new();
        let head = rev_parse(&path, "HEAD").unwrap();
        {
            let conn = rusqlite::Connection::open(_directory.path().join("store.db")).unwrap();
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
            let conn = rusqlite::Connection::open(_directory.path().join("store.db")).unwrap();
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
        )
        .unwrap();
        retained(&item, "inspection failed");
        observe(
            &store,
            repo.path(),
            &mut item,
            &Ok(HashSet::from([path.join("nested")])),
        )
        .unwrap();
        retained(&item, "running external process");
        observe(&store, repo.path(), &mut item, &Ok(HashSet::new())).unwrap();
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

        let plan = super::plan_selected(&store, repo.path(), Some(&path), None).unwrap();
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
        assert_eq!(report.failed.len(), 1);
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
        assert_eq!(report.failed.len(), 1);
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
        let connection = rusqlite::Connection::open(directory.path().join("store.db")).unwrap();
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
    async fn cleanup_planning_budget_retains_unobserved_checkouts() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        let plan = super::plan_selected(&store, repo.path(), None, Some(std::time::Instant::now()))
            .unwrap();
        retained(
            plan.iter().find(|item| item.path == path).unwrap(),
            "planning budget exhausted",
        );
        assert!(path.exists());
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
