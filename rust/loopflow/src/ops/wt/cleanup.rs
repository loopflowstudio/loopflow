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
use crate::journal::{process_evidence, ProcessIdentityEvidence};
use crate::ops::{OpsError, OpsResult};
use crate::store::SharedStore;
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

async fn observe(
    store: &SharedStore,
    repo: &Path,
    decision: &mut CleanupDecision,
    external: &OpsResult<HashSet<PathBuf>>,
) -> OpsResult<()> {
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
    if normalized(&main_repo_root(path)?) != normalized(repo)
        || !list_porcelain(repo)?
            .iter()
            .any(|(registered, _)| normalized(registered) == *path)
    {
        retain(decision, "checkout registration changed");
        return Ok(());
    }
    decision.branch = current_branch(path)?;
    decision.observed_head = Some(rev_parse(path, "HEAD")?);
    if decision.branch.as_deref() == Some(get_default_branch(repo)?.as_str()) {
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
    for task in store.list_tasks(None).await.map_err(error)? {
        let Some(worktree) = &task.worktree else {
            continue;
        };
        if normalized(worktree) != normalized(path) {
            continue;
        }
        owned = true;
        let status = store
            .work_status(&WorkRef::Task(task.id.clone()))
            .await
            .map_err(error)?;
        if !matches!(status, WorkStatus::Done | WorkStatus::Abandoned) {
            retain(
                decision,
                format!("unfinished Task {}", task.plan.identifier),
            );
            return Ok(());
        }
        let open = store.sqlite.open_processes().map_err(error)?;
        let work = store
            .sqlite
            .task_open_work(&task.id, &open)
            .map_err(error)?;
        if work.processes.iter().any(|process| {
            process.completed_at.is_none()
                && process_evidence(&store.sqlite, &process.lfid) != ProcessIdentityEvidence::Dead
        }) {
            retain(decision, "Task has live or unknown execution");
            return Ok(());
        }
        for pr in store.task_prs(&task.id).await.map_err(error)? {
            if decision.branch.as_deref() == Some(pr.branch.as_str())
                && pr.phase() == PrPhase::Merged
                && pr.head_sha() == decision.observed_head.as_deref()
            {
                settled = true;
                decision
                    .evidence
                    .push(format!("Task {}: exact merged head", task.plan.identifier));
            }
        }
    }
    // Experimental databases cannot collect release-owned unfinished work.
    // Unit tests use ephemeral registries, never the installed account database.
    #[cfg(not(test))]
    let production = crate::store::production_database_path();
    #[cfg(not(test))]
    if production.exists()
        && crate::store::read_nonterminal_task_worktrees(&production)
            .map_err(error)?
            .iter()
            .any(|root| normalized(root) == normalized(path))
    {
        retain(decision, "unfinished Task in release registry");
        return Ok(());
    }
    for landing in store.sqlite.merged_landings_at(path).map_err(error)? {
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
    for process in store.sqlite.unfinished_processes().map_err(error)? {
        if process
            .cwd
            .as_deref()
            .is_some_and(|cwd| normalized(Path::new(cwd)).starts_with(path))
            && process_evidence(&store.sqlite, &process.lfid) != ProcessIdentityEvidence::Dead
        {
            retain(
                decision,
                format!("Process {} has live or unknown execution", process.lfid),
            );
            return Ok(());
        }
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
        // An explicitly nested LF_HOME is history, even inside a cache-tagged root.
        if normalized(&crate::store::lf_home_dir()).starts_with(&root) {
            return Err(error("local Session evidence inside an artifact root"));
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
    std::fs::remove_file(root.join("CACHEDIR.TAG"))?;
    std::fs::remove_dir(root)?;
    Ok(())
}

pub async fn plan_cleanup(store: &SharedStore, repo: &Path) -> OpsResult<Vec<CleanupDecision>> {
    let repo = main_repo_root(repo)?;
    let external = running_paths();
    let mut plan = Vec::new();
    for (path, branch) in list_porcelain(&repo)? {
        let mut decision = CleanupDecision {
            path: normalized(&path),
            branch,
            observed_head: None,
            action: CleanupAction::Retain("not observed".into()),
            evidence: Vec::new(),
            estimated_bytes: None,
        };
        if let Err(error) = observe(store, &repo, &mut decision, &external).await {
            retain(&mut decision, format!("observation unavailable: {error}"));
        }
        plan.push(decision);
    }
    Ok(plan)
}

pub async fn apply_cleanup(
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
        let result = async {
            let _admission = store.sqlite.lock_checkout(&decision.path).map_err(error)?;
            let lease = acquire_worktree_lease(&repo, &decision.path, "checkout cleanup")?;
            let expected_head = decision.observed_head.clone();
            let expected_branch = decision.branch.clone();
            decision.evidence.clear();
            // Refresh all authority and filesystem facts under both locks.
            observe(store, &repo, &mut decision, &running_paths()).await?;
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
        }
        .await;
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
pub async fn run_cleanup_pass(
    store: &SharedStore,
    repo: &Path,
    budget: CleanupBudget,
) -> OpsResult<CleanupReport> {
    let home = crate::store::lf_home_dir();
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
                deferred: Vec::new(),
                failed: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
    }
    let plan = plan_cleanup(store, repo).await?;
    apply_cleanup(store, repo, plan, budget).await
}

pub(crate) async fn cleanup_path(store: &SharedStore, repo: &Path, path: &Path) -> OpsResult<()> {
    let plan = plan_cleanup(store, repo)
        .await?
        .into_iter()
        .filter(|decision| decision.path == normalized(path))
        .collect();
    let report = apply_cleanup(store, repo, plan, CleanupBudget::default()).await?;
    for decision in report.deferred {
        if let CleanupAction::Retain(reason) = decision.action {
            eprintln!("retained {}: {reason}", decision.path.display());
        }
    }
    for failure in report.failed {
        eprintln!("cleanup {}: {}", failure.path.display(), failure.error);
    }
    Ok(())
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
                after_merge: None,
                next_slug: None,
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

    async fn decision(
        store: &SharedStore,
        repo: &TestRepo,
        path: &std::path::Path,
    ) -> CleanupDecision {
        let mut decision = CleanupDecision {
            path: path.to_path_buf(),
            branch: None,
            observed_head: None,
            action: CleanupAction::Retain("unobserved".into()),
            evidence: Vec::new(),
            estimated_bytes: None,
        };
        observe(store, repo.path(), &mut decision, &Ok(HashSet::new()))
            .await
            .unwrap();
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
    async fn cleanup_retries_unknown_execution_then_removes_once_without_deleting_remote() {
        let _guard = crate::journal::TestLedgerGuard::new();
        let _external = ExternalInspection::idle();
        let (repo, _directory, store, path) = fixture().await;
        git(&path, &["push", "origin", "landed"]).unwrap();
        let mut process = crate::process::Process {
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
        let first = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default())
            .await
            .unwrap();
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
        let second = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default())
            .await
            .unwrap();
        assert!(second.failed.is_empty(), "{:?}", second.failed);
        assert_eq!(second.removed, std::slice::from_ref(&path));
        assert!(!path.exists());
        assert!(git(repo.path(), &["show-ref", "--verify", "refs/heads/landed"]).is_err());
        assert!(
            !git(repo.path(), &["ls-remote", "--heads", "origin", "landed"])
                .unwrap()
                .is_empty()
        );
        let third = super::run_cleanup_pass(&store, repo.path(), CleanupBudget::default())
            .await
            .unwrap();
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
        let plan = plan_cleanup(&store, repo.path()).await.unwrap();
        // A crash after removing one artifact but before Git removal leaves
        // ordinary registered ownership; the next pass needs no cleanup queue.
        std::fs::remove_file(path.join("target/artifact")).unwrap();
        let report = apply_cleanup(&store, repo.path(), plan, CleanupBudget::default())
            .await
            .unwrap();
        assert_eq!(report.removed, std::slice::from_ref(&path), "{report:?}");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn cleanup_retains_ignored_user_data_and_uncommitted_files() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::create_dir(path.join("private")).unwrap();
        std::fs::write(path.join("private/results"), "irreplaceable").unwrap();
        retained(
            &decision(&store, &repo, &path).await,
            "unclassified ignored",
        );
        std::fs::remove_dir_all(path.join("private")).unwrap();
        std::fs::write(path.join("notes"), "uncommitted").unwrap();
        retained(&decision(&store, &repo, &path).await, "uncommitted");
        assert!(path.join("notes").exists());
    }

    #[tokio::test]
    async fn cleanup_requires_valid_cache_tags_and_never_classifies_tracked_source_as_cache() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::create_dir(path.join("target")).unwrap();
        std::fs::write(path.join("target/build"), "regenerable").unwrap();
        std::fs::write(path.join("target/CACHEDIR.TAG"), "invalid").unwrap();
        retained(
            &decision(&store, &repo, &path).await,
            "unclassified ignored",
        );
        std::fs::write(
            path.join("target/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n",
        )
        .unwrap();
        assert_eq!(
            decision(&store, &repo, &path).await.action,
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
            &decision(&store, &repo, &path).await,
            "excluded from Git change detection",
        );
    }

    #[tokio::test]
    async fn cleanup_retains_new_commits_after_merge_even_when_remote_is_gone() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        git(&path, &["commit", "--allow-empty", "-m", "new work"]).unwrap();
        retained(
            &decision(&store, &repo, &path).await,
            "no recorded settlement",
        );
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
        retained(&decision(&store, &repo, &path).await, "persistent");
        let legacy = repo.create_named_worktree("legacy").canonicalize().unwrap();
        retained(
            &decision(&store, &repo, &legacy).await,
            "unknown Loopflow ownership",
        );
    }

    #[tokio::test]
    async fn cleanup_process_inspection_failure_and_running_descendants_retain_checkout() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let mut item = decision(&store, &repo, &path).await;
        observe(
            &store,
            repo.path(),
            &mut item,
            &Err(super::error("inspection failed")),
        )
        .await
        .unwrap();
        retained(&item, "inspection failed");
        observe(
            &store,
            repo.path(),
            &mut item,
            &Ok(HashSet::from([path.join("nested")])),
        )
        .await
        .unwrap();
        retained(&item, "running external process");
        observe(&store, repo.path(), &mut item, &Ok(HashSet::new()))
            .await
            .unwrap();
        assert_eq!(item.action, CleanupAction::RemoveCheckout);
    }

    #[tokio::test]
    async fn cleanup_apply_rechecks_head_and_obeys_checkout_lease() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        let item = decision(&store, &repo, &path).await;
        let lease = acquire_worktree_lease(repo.path(), &path, "other writer").unwrap();
        let report = apply_cleanup(
            &store,
            repo.path(),
            vec![item.clone()],
            CleanupBudget::default(),
        )
        .await
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
        .await
        .unwrap();
        assert_eq!(report.failed.len(), 1);
        assert!(path.exists());
        drop(admission);
        git(
            &path,
            &["commit", "--allow-empty", "-m", "post-preview work"],
        )
        .unwrap();
        let report = apply_cleanup(&store, repo.path(), vec![item], CleanupBudget::default())
            .await
            .unwrap();
        assert!(report.removed.is_empty());
        retained(&report.deferred[0], "changed after planning");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn cleanup_preview_keeps_missing_git_registration_and_reports_unknown_size() {
        let (_guard, (repo, _directory, store, path)) =
            (crate::journal::TestLedgerGuard::new(), fixture().await);
        std::fs::remove_dir_all(&path).unwrap();
        let before = git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap();
        let plan = plan_cleanup(&store, repo.path()).await.unwrap();
        let missing = plan.iter().find(|item| item.path == path).unwrap();
        retained(missing, "missing checkout");
        assert_eq!(missing.estimated_bytes, None);
        assert_eq!(
            git(repo.path(), &["worktree", "list", "--porcelain"]).unwrap(),
            before
        );
    }
}
