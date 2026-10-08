//! Daemonless local persistence shared by `lf`, Waves, Projects, and Tasks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::id::WaveId;
use crate::profile::{
    AccessProfile, AuthBrowserBinding, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::work::wave::{Wave, WaveLocator};
pub(crate) mod changes;
mod chapters;
mod children;
pub(crate) mod ci_incidents;
mod durable;
mod metrics;
mod migration_catalog;
mod migration_schema;
pub mod migrations;
mod pr_landings;
mod processes;
pub(crate) mod project_transitions;
pub mod rows;
mod sessions;
pub mod sqlite;
mod token_crypto;

/// One Wave's planning view, assembled from shared entities and membership.
#[derive(Debug, Clone, PartialEq)]
pub struct PmSnapshotRow {
    pub wave_id: WaveId,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub snapshot: crate::pm::PmSnapshot,
}

/// A planning observation; neither Project ownership nor execution is required.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PmTaskRecord {
    pub item: crate::pm::PmItem,
    pub project: Option<crate::pm::PmProject>,
    pub observed_at: i64,
}

/// Availability of retained planning facts, independent of execution permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanningState {
    Available,
    Invalid,
    Removed,
    Absent,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PmTaskObservation {
    pub record: Option<PmTaskRecord>,
    pub state: PlanningState,
}

#[derive(Debug, Clone)]
pub(crate) struct WaveLocatorUpdate {
    pub wave_id: WaveId,
    pub expected_repo: String,
    pub expected_slug: String,
    pub target: WaveLocator,
    pub retire_collision: Option<WaveId>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Planning(#[from] crate::planning::PlanningError),
    #[error(transparent)]
    TaskData(#[from] crate::work::task::TaskDataError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("development store is incompatible: {0}")]
    IncompatibleDevelopment(String),
    #[error("invalid control authority: {0}")]
    InvalidAuthority(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskPrMergeEvidenceOutcome {
    Accepted,
    Repeated,
    Missing,
    Conflict { accepted_at: i64 },
    SchemaUnavailable,
}

/// Keep live checkpoints at one-second precision for the longest public usage
/// window. Older Turns retain only their final or latest receipt.
pub const TURN_USAGE_LIVE_RETENTION_SECONDS: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageConfig {
    Sqlite { path: PathBuf },
}

impl StorageConfig {
    pub fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }
}

fn machine_home_dir() -> PathBuf {
    crate::installation::account_home()
        .expect("resolve OS account home directory for the production store guard")
}

pub(crate) fn production_database_path() -> PathBuf {
    machine_home_dir().join(".lf/loopflow.db")
}

pub(crate) fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    sqlite::read_nonterminal_task_worktrees(path)
}

fn default_lf_home_dir() -> PathBuf {
    machine_home_dir().join(".lf")
}

pub(crate) fn lf_home_dir() -> PathBuf {
    std::env::var_os("LF_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_lf_home_dir)
}

pub(crate) fn custom_home_selected() -> bool {
    !same_database_file(&lf_home_dir(), &default_lf_home_dir()).unwrap_or(false)
}

pub fn default_db_path() -> PathBuf {
    lf_home_dir().join("loopflow.db")
}

/// The selected Machine's database. A Machine has exactly one, at a fixed name.
pub fn database_path_from_env() -> Result<PathBuf, std::io::Error> {
    let path = default_db_path();
    guard_development_database(&path, crate::build_info::provenance(), &machine_home_dir())?;
    Ok(path)
}

fn guard_development_database(
    path: &Path,
    provenance: crate::build_info::BuildProvenance,
    home: &Path,
) -> Result<(), std::io::Error> {
    if provenance.is_release() {
        return Ok(());
    }
    let production = home.join(".lf/loopflow.db");
    if !same_database_file(path, &production)? {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "development lf ({}) refuses production database {}; use an installed release lf",
            crate::build_info::source_identity(),
            production.display()
        ),
    ))
}

/// Whether an open is the authorized owner of the shared migration frontier.
///
/// Advancing `~/.lf/loopflow.db` past the frontier the installed `lf` knows must
/// never be a side effect of an ordinary command: on 2026-07-17 a published
/// candidate at `target/release/lf` did exactly that and stranded the installed
/// binary. Only `lf install promote`, under the exclusive promotion lock, opens
/// the store as `Authorized`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrontierAdvance {
    /// An ordinary open: the shared frontier is read, never advanced.
    Forbidden,
    /// The promotion boundary: it may apply the pending migration.
    Authorized,
}

/// Whether this open may apply migrations to `path`.
///
/// A private store (any path that is not the machine's shared `~/.lf/loopflow.db`)
/// may initialize once; subsequent opens require its exact schema. The shared
/// release store is exclusive to the promotion boundary: a
/// validation-only build never writes to it, and an ordinary (`Forbidden`) open
/// neither initializes nor advances it. Bootstrapping a missing or empty shared
/// store to the candidate's head can strand an older installed binary exactly as
/// advancing an existing frontier would, so both belong to `Authorized` alone.
fn may_apply_migrations(
    path: &Path,
    authority: crate::build_info::MigrationAuthority,
    home: &Path,
    advance: FrontierAdvance,
) -> Result<bool, std::io::Error> {
    if !same_database_file(path, &home.join(".lf/loopflow.db"))? {
        return Ok(true);
    }
    if authority != crate::build_info::MigrationAuthority::Published {
        return Ok(false);
    }
    Ok(advance == FrontierAdvance::Authorized)
}

pub(crate) fn same_database_file(left: &Path, right: &Path) -> Result<bool, std::io::Error> {
    if canonicalize_with_missing_tail(left)? == canonicalize_with_missing_tail(right)? {
        return Ok(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if let (Ok(left), Ok(right)) = (left.metadata(), right.metadata()) {
            return Ok(left.dev() == right.dev() && left.ino() == right.ino());
        }
    }
    Ok(false)
}

pub(crate) fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf, std::io::Error> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "database path has no existing root",
            )
        })?;
        missing.push(name.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "database path has no parent")
        })?;
    }
    let mut resolved = existing.canonicalize()?;
    for component in missing.into_iter().rev() {
        if component == "." {
            continue;
        }
        if component == ".." {
            resolved.pop();
        } else {
            resolved.push(component);
        }
    }
    Ok(resolved)
}

pub fn storage_config_from_env() -> Result<StorageConfig, std::io::Error> {
    Ok(StorageConfig::sqlite(database_path_from_env()?))
}

/// Planning and checkout locks retained by every queued writer using this acquisition.
#[derive(Debug, Clone)]
pub struct PlanningLocks {
    locks: Vec<Arc<std::fs::File>>,
}

impl PlanningLocks {
    pub(crate) fn new(wave: std::fs::File) -> Self {
        Self {
            locks: vec![Arc::new(wave)],
        }
    }

    pub(crate) fn with_checkouts(&self, checkouts: &[Arc<std::fs::File>]) -> Self {
        let mut locks = self.locks.clone();
        locks.extend_from_slice(checkouts);
        Self { locks }
    }
}

#[derive(Debug)]
pub struct Store {
    pub(crate) sqlite: sqlite::SqliteStore,
}

pub(crate) async fn run_sqlite<T, F>(store: &sqlite::SqliteStore, func: F) -> StoreResult<T>
where
    T: Send + 'static,
    F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || func(store))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}

#[cfg(test)]
tokio::task_local! {
    pub(crate) static PLANNING_ACCEPTANCE_GATE: (Arc<tokio::sync::Notify>, Arc<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>);
}

// The blocking worker owns acquisition through commit, even if its async caller is canceled.
async fn run_planning_write<T: Send + 'static>(
    store: &sqlite::SqliteStore,
    acquisition: Option<Arc<PlanningLocks>>,
    write: impl FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
) -> StoreResult<T> {
    #[cfg(test)]
    let gate = PLANNING_ACCEPTANCE_GATE.try_with(Clone::clone).ok();
    run_sqlite(store, move |store| {
        let _acquisition = acquisition;
        #[cfg(test)]
        if let Some((entered, release)) = gate {
            entered.notify_one();
            let _ = release.lock().expect("planning test gate poisoned").recv();
        }
        write(store)
    })
    .await
}

impl Store {
    #[cfg(test)]
    pub(crate) fn from_sqlite_for_test(sqlite: sqlite::SqliteStore) -> Self {
        Self { sqlite }
    }

    #[cfg(test)]
    pub(crate) fn apply_migration_for_test(&self, name: &str) -> StoreResult<()> {
        self.sqlite.apply_migration_for_test(name)
    }

    pub async fn put_pm_snapshot(
        &self,
        snapshot: PmSnapshotRow,
        acquisition: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        run_planning_write(&self.sqlite, acquisition, move |store| {
            store.put_pm_snapshot(&snapshot)
        })
        .await
    }

    pub async fn put_pm_project(
        &self,
        wave: &WaveId,
        provider: &str,
        initiative: &str,
        project: crate::pm::PmProject,
        observed_at: i64,
        acquisition: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<crate::pm::PmProject> {
        let wave = wave.clone();
        let provider = provider.to_string();
        let initiative = initiative.to_string();
        run_planning_write(&self.sqlite, acquisition, move |store| {
            store.put_pm_project(&wave, &provider, &initiative, &project, observed_at)
        })
        .await
    }

    pub(crate) async fn reconcile_pm_project_teams(
        &self,
        wave: &WaveId,
        provider: &str,
        initiative: &str,
        project: crate::pm::PmProject,
        observed_at: i64,
        acquisition: Arc<PlanningLocks>,
    ) -> StoreResult<()> {
        let wave = wave.clone();
        let provider = provider.to_string();
        let initiative = initiative.to_string();
        run_planning_write(&self.sqlite, Some(acquisition), move |store| {
            store.reconcile_pm_project_teams(&wave, &provider, &initiative, &project, observed_at)
        })
        .await
    }

    pub async fn put_pm_task(
        &self,
        repo: &str,
        provider: &str,
        record: PmTaskRecord,
        confirmed_wave: Option<(WaveId, String)>,
        acquisition: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        let repo = repo.to_string();
        let provider = provider.to_string();
        run_planning_write(&self.sqlite, acquisition, move |store| {
            store.put_pm_task(
                &repo,
                &provider,
                &record,
                confirmed_wave
                    .as_ref()
                    .map(|(wave, initiative)| (wave, initiative.as_str())),
            )
        })
        .await
    }

    pub async fn pm_task_observation(
        &self,
        repo: &str,
        provider: &str,
        selector: &str,
    ) -> StoreResult<PmTaskObservation> {
        let repo = repo.to_string();
        let provider = provider.to_string();
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.pm_task_observation(&repo, &provider, &selector)
        })
        .await
    }

    pub async fn confirm_pm_project_archival(
        &self,
        repo: &str,
        provider: &str,
        project: crate::pm::PmProject,
        observed_at: i64,
    ) -> StoreResult<()> {
        let repo = repo.to_string();
        let provider = provider.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.confirm_pm_project_archival(&repo, &provider, &project, observed_at)
        })
        .await
    }

    pub async fn observe_pm_issue_change(
        &self,
        issue_id: &str,
        revision: Option<&str>,
        removed: bool,
    ) -> StoreResult<()> {
        let issue_id = issue_id.to_string();
        let revision = revision.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.observe_pm_issue_change(&issue_id, revision.as_deref(), removed)
        })
        .await
    }

    pub async fn invalidate_pm_task(
        &self,
        repo: &str,
        provider: &str,
        expected: PmTaskRecord,
        acquisition: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        let repo = repo.to_string();
        let provider = provider.to_string();
        run_planning_write(&self.sqlite, acquisition, move |store| {
            store.invalidate_pm_task(&repo, &provider, &expected)
        })
        .await
    }

    pub(crate) async fn retain_task_issue_identity(
        &self,
        wave_id: &WaveId,
        issue_id: &str,
        identifier: &str,
    ) -> StoreResult<()> {
        let wave_id = wave_id.clone();
        let issue_id = issue_id.to_string();
        let identifier = identifier.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.retain_task_issue_identity(&wave_id, &issue_id, &identifier)
        })
        .await
    }

    pub(crate) async fn task_issue_identity(
        &self,
        wave_id: &WaveId,
        issue: &str,
    ) -> StoreResult<Option<(String, String)>> {
        let wave_id = wave_id.clone();
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_issue_identity(&wave_id, &issue)
        })
        .await
    }

    pub(crate) async fn task_deletion(
        &self,
        wave_id: &WaveId,
        issue: &str,
    ) -> StoreResult<Option<(String, String)>> {
        let wave_id = wave_id.clone();
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_deletion(&wave_id, &issue)
        })
        .await
    }

    pub(crate) async fn confirm_task_deletion(
        &self,
        wave_id: &WaveId,
        issue_id: &str,
        identifier: &str,
    ) -> StoreResult<()> {
        let wave_id = wave_id.clone();
        let issue_id = issue_id.to_string();
        let identifier = identifier.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.confirm_task_deletion(&wave_id, &issue_id, &identifier)
        })
        .await
    }

    pub async fn deleted_task_issues(
        &self,
        wave_id: &WaveId,
    ) -> StoreResult<std::collections::HashSet<String>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.deleted_task_issues(&wave_id)
        })
        .await
    }

    pub async fn pm_snapshot(&self, wave_id: &WaveId) -> StoreResult<Option<PmSnapshotRow>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.pm_snapshot(&wave_id)).await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let Some(repo) = repo else {
            return run_sqlite(&self.sqlite, move |store| store.list_waves(None)).await;
        };
        let canonical = match crate::repository::CanonicalRepo::discover(Path::new(repo)) {
            Ok(canonical) => canonical,
            Err(_) => {
                let repo = repo.to_string();
                return run_sqlite(&self.sqlite, move |store| store.list_waves(Some(&repo))).await;
            }
        };
        let all = run_sqlite(&self.sqlite, move |store| store.list_waves(None)).await?;
        for wave in all {
            if wave.repo() == canonical.to_string() {
                continue;
            }
            let equivalent = crate::repository::CanonicalRepo::discover(Path::new(wave.repo()))
                .is_ok_and(|stored| stored == canonical);
            if !equivalent {
                continue;
            }
            let wave_id = wave.id().clone();
            let expected_repo = wave.repo().to_string();
            let target_repo = canonical.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.repair_wave_repo(&wave_id, &expected_repo, &target_repo)
            })
            .await?;
        }
        let repo = canonical.to_string();
        run_sqlite(&self.sqlite, move |store| store.list_waves(Some(&repo))).await
    }

    /// A chord's contents: the waves whose `parent_wave_id` is `parent`,
    /// ordered by creation.
    pub async fn list_child_waves(&self, parent: &WaveId) -> StoreResult<Vec<Wave>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.list_child_waves(&parent)).await
    }

    pub async fn get_wave(&self, wave_id: &WaveId) -> StoreResult<Option<Wave>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.get_wave(&wave_id)).await
    }

    pub async fn get_wave_at(&self, locator: &WaveLocator) -> StoreResult<Option<Wave>> {
        let locator = locator.clone();
        if let Some(wave) = run_sqlite(&self.sqlite, {
            let locator = locator.clone();
            move |store| store.get_wave_at(&locator)
        })
        .await?
        {
            return Ok(Some(wave));
        }

        let candidates = self.find_waves_by_slug(locator.slug()).await?;
        let equivalent = candidates
            .into_iter()
            .filter(|wave| {
                crate::repository::CanonicalRepo::discover(Path::new(wave.repo()))
                    .is_ok_and(|repo| &repo == locator.repo())
            })
            .collect::<Vec<_>>();
        let [wave] = equivalent.as_slice() else {
            if equivalent.len() > 1 {
                return Err(StoreError::InvalidData(format!(
                    "multiple stored Wave locators canonicalize to {}/{}",
                    locator.repo(),
                    locator.slug()
                )));
            }
            return Ok(None);
        };
        let wave_id = wave.id().clone();
        let expected_repo = wave.repo().to_string();
        let target_repo = locator.repo().to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.repair_wave_repo(&wave_id, &expected_repo, &target_repo)
        })
        .await?;
        self.get_wave(wave.id()).await
    }

    pub async fn find_waves_by_slug(&self, slug: &str) -> StoreResult<Vec<Wave>> {
        let slug = slug.to_string();
        run_sqlite(&self.sqlite, move |store| store.find_waves_by_slug(&slug)).await
    }

    pub(crate) async fn reconcile_wave_directory(
        &self,
        id: &WaveId,
        name: &str,
        parent: Option<&WaveId>,
    ) -> StoreResult<()> {
        let id = id.clone();
        let name = name.to_string();
        let parent = parent.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.reconcile_wave_directory(&id, &name, parent.as_ref())
        })
        .await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.create_wave(&wave)).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.update_wave(&wave)).await
    }

    pub(crate) async fn relocate_waves(&self, updates: Vec<WaveLocatorUpdate>) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| store.relocate_waves(&updates)).await
    }

    pub(crate) async fn wave_retirement_blockers(
        &self,
        wave_id: &WaveId,
    ) -> StoreResult<Vec<String>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.wave_retirement_blockers(&wave_id)
        })
        .await
    }

    pub async fn delete_wave(&self, wave_id: &WaveId) -> StoreResult<()> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.delete_wave(&wave_id)).await
    }

    pub(crate) async fn provider_auth_snapshot(
        &self,
        provider: Provider,
    ) -> StoreResult<Option<crate::provider_auth::ProviderAuthSnapshot>> {
        run_sqlite(&self.sqlite, move |store| {
            store.provider_auth_snapshot(provider)
        })
        .await
    }

    pub async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        let provider = provider.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.get_provider_token(&provider)
        })
        .await
    }

    pub async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        let token = token.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.upsert_provider_token(&token)
        })
        .await
    }

    /// The file lock moves into the blocking write, surviving caller cancellation.
    pub(crate) async fn replace_provider_token(
        &self,
        expected: &ProviderToken,
        replacement: &ProviderToken,
        lock: std::fs::File,
        deadline: std::time::Instant,
    ) -> StoreResult<ProviderTokenReplacement> {
        let expected = expected.clone();
        let replacement = replacement.clone();
        run_sqlite(&self.sqlite, move |store| {
            let _lock = lock;
            store.replace_provider_token(&expected, &replacement, deadline)
        })
        .await
    }

    pub async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        let provider = provider.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.delete_provider_token(&provider)
        })
        .await
    }

    pub async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        run_sqlite(&self.sqlite, |store| store.list_provider_tokens()).await
    }

    pub async fn upsert_provider_account(&self, account: &ProviderAccount) -> StoreResult<()> {
        let account = account.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.upsert_provider_account(&account)
        })
        .await
    }

    pub async fn record_provider_account_identity(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        email: &str,
        subject: &str,
        plan: Option<&str>,
        credential_digest: Option<&str>,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        let email = email.to_string();
        let subject = subject.to_string();
        let credential_digest = credential_digest.map(str::to_string);
        let plan = plan.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.record_provider_account_identity(
                &provider,
                &account_id,
                &email,
                &subject,
                plan.as_deref(),
                credential_digest.as_deref(),
            )
        })
        .await
    }

    pub async fn get_provider_account(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<Option<ProviderAccount>> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.get_provider_account(&provider, &account_id)
        })
        .await
    }

    pub async fn list_provider_accounts(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<ProviderAccount>> {
        let provider = provider.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.list_provider_accounts(provider.as_deref())
        })
        .await
    }

    pub async fn update_provider_account_lifecycle(
        &self,
        account: &ProviderAccount,
    ) -> StoreResult<()> {
        let account = account.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_provider_account_lifecycle(&account)
        })
        .await
    }

    pub async fn clear_provider_account_cooldown(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.clear_provider_account_cooldown(&provider, &account_id)
        })
        .await
    }

    pub async fn reset_provider_account_health(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reset_provider_account_health(&provider, &account_id)
        })
        .await
    }

    pub async fn update_provider_account_credential_state(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        state: CredentialState,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_provider_account_credential_state(&provider, &account_id, state)
        })
        .await
    }

    pub async fn record_provider_account_credential_invalidated(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        reason: &str,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.record_provider_account_credential_invalidated(&provider, &account_id, &reason)
        })
        .await
    }

    pub async fn record_provider_account_health(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        utilization_percent: Option<u8>,
        cooldown_until: Option<i64>,
        cooldown_reason: Option<&str>,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        let cooldown_reason = cooldown_reason.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.record_provider_account_health(
                &provider,
                &account_id,
                utilization_percent,
                cooldown_until,
                cooldown_reason.as_deref(),
            )
        })
        .await
    }

    pub async fn upsert_provider_account_limits(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        windows: &[AccountLimitWindow],
        source: &str,
    ) -> StoreResult<()> {
        let provider = provider.to_string();
        let account_id = account_id.clone();
        let windows = windows.to_vec();
        let source = source.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.upsert_provider_account_limits(&provider, &account_id, &windows, &source)
        })
        .await
    }

    pub async fn provider_account_limits(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<AccountLimitRow>> {
        let provider = provider.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.provider_account_limits(provider.as_deref())
        })
        .await
    }

    pub async fn upsert_access_profile(&self, profile: &AccessProfile) -> StoreResult<()> {
        let profile = profile.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.upsert_access_profile(&profile)
        })
        .await
    }

    pub async fn get_access_profile(
        &self,
        profile_id: &ProfileId,
    ) -> StoreResult<Option<AccessProfile>> {
        let profile_id = profile_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.get_access_profile(&profile_id)
        })
        .await
    }

    pub async fn list_access_profiles(&self) -> StoreResult<Vec<AccessProfile>> {
        run_sqlite(&self.sqlite, |store| store.list_access_profiles()).await
    }

    pub async fn set_auth_browser_profiles(
        &self,
        provider: Provider,
        account_id: Option<&ProviderAccountId>,
        profile_ids: &[ProfileId],
    ) -> StoreResult<()> {
        let account_id = account_id.cloned();
        let profile_ids = profile_ids.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.set_auth_browser_profiles(provider, account_id.as_ref(), &profile_ids)
        })
        .await
    }

    pub async fn list_auth_browser_profiles(
        &self,
        provider: Option<Provider>,
        account_id: Option<&ProviderAccountId>,
    ) -> StoreResult<Vec<AuthBrowserBinding>> {
        let account_id = account_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_auth_browser_profiles(provider, account_id.as_ref())
        })
        .await
    }

    pub async fn set_provider_route(&self, route: &ProviderRoute) -> StoreResult<()> {
        let route = route.clone();
        run_sqlite(&self.sqlite, move |store| store.set_provider_route(&route)).await
    }

    pub async fn provider_route(
        &self,
        scope: &RouteScope,
        provider: Provider,
    ) -> StoreResult<Option<ProviderRoute>> {
        let scope = scope.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.provider_route(&scope, provider)
        })
        .await
    }

    pub async fn pin_provider_session_route(
        &self,
        provider: Provider,
        provider_session_id: &str,
        account_id: &ProviderAccountId,
        isolated: bool,
    ) -> StoreResult<()> {
        let provider_session_id = provider_session_id.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pin_provider_session_route(provider, &provider_session_id, &account_id, isolated)
        })
        .await
    }

    pub async fn provider_session_isolated(
        &self,
        provider: Provider,
        provider_session_id: &str,
    ) -> StoreResult<Option<bool>> {
        let provider_session_id = provider_session_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.provider_session_isolated(provider, &provider_session_id)
        })
        .await
    }

    pub async fn record_provider_account_switch(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        cause: &'static str,
    ) -> StoreResult<()> {
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.record_provider_account_switch(provider, &account_id, cause)
        })
        .await
    }

    pub async fn provider_account_switched_since(
        &self,
        provider: Provider,
        since: i64,
    ) -> StoreResult<Option<ProviderAccountId>> {
        run_sqlite(&self.sqlite, move |store| {
            store.provider_account_switched_since(provider, since)
        })
        .await
    }

    pub async fn provider_session_account(
        &self,
        provider: Provider,
        provider_session_id: &str,
    ) -> StoreResult<Option<ProviderAccountId>> {
        let provider_session_id = provider_session_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.provider_session_account(provider, &provider_session_id)
        })
        .await
    }

    pub async fn select_provider_account(
        &self,
        provider: Provider,
        candidates: &[ProviderAccountId],
        provider_session_id: Option<&str>,
    ) -> StoreResult<Option<ProviderAccountSelection>> {
        let candidates = candidates.to_vec();
        let provider_session_id = provider_session_id.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.select_provider_account(provider, &candidates, provider_session_id.as_deref())
        })
        .await
    }

    pub async fn health_check(&self) -> StoreResult<()> {
        run_sqlite(&self.sqlite, |store| store.health_check()).await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        run_sqlite(&self.sqlite, |store| store.schema_version()).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth,
    ApiKey,
}

impl CredentialType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "apikey",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "apikey" => Self::ApiKey,
            _ => Self::OAuth,
        }
    }
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub(crate) enum ProviderTokenReplacement {
    Replaced,
    Changed(ProviderToken),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub oauth_client_id: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ProviderAccountId(String);

impl ProviderAccountId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.len() > 63 {
            return Err("account id must be 1-63 characters".to_string());
        }
        let mut chars = value.chars();
        let first = chars
            .next()
            .expect("non-empty account id has a first character");
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err("account id must start with a lowercase letter or number".to_string());
        }
        if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        {
            return Err(
                "account id may contain lowercase letters, numbers, '-' and '_'".to_string(),
            );
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderAccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderAccount {
    pub provider: String,
    pub account_id: ProviderAccountId,
    pub home: Option<PathBuf>,
    pub login_email: Option<EmailAddress>,
    pub observed_email: Option<String>,
    pub observed_subject: Option<String>,
    pub observed_credential_digest: Option<String>,
    pub observed_plan: Option<String>,
    pub credential_state: CredentialState,
    pub routing_state: RoutingState,
    pub plan: Option<String>,
    pub paid_through: Option<time::Date>,
    pub utilization_percent: Option<u8>,
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
    pub last_selected_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProviderAccount {
    pub fn effective_routing_state(&self, today: time::Date) -> RoutingState {
        if self.routing_state == RoutingState::Automatic
            && self.paid_through.is_some_and(|date| date < today)
        {
            RoutingState::ExplicitOnly
        } else {
            self.routing_state
        }
    }

    pub fn eligible_for_automatic_routing(&self, today: time::Date) -> bool {
        self.credential_state == CredentialState::Connected
            && self.effective_routing_state(today) == RoutingState::Automatic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialState {
    Connected,
    Missing,
}

impl CredentialState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Missing => "missing",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "connected" => Ok(Self::Connected),
            "missing" => Ok(Self::Missing),
            other => Err(format!("unknown credential state '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingState {
    Automatic,
    ExplicitOnly,
    Disabled,
}

impl RoutingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::ExplicitOnly => "explicit_only",
            Self::Disabled => "disabled",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "automatic" => Ok(Self::Automatic),
            "explicit_only" => Ok(Self::ExplicitOnly),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!("unknown routing state '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccountSelection {
    pub account: ProviderAccount,
    pub resume_requested_session: bool,
}

/// One observed subscription rate-limit window: how much of the plan's
/// `session`/`weekly`/`weekly:<model>` window an account has consumed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountLimitWindow {
    pub window: String,
    pub used_percent: u8,
    pub resets_at: Option<i64>,
    pub plan: Option<String>,
}

/// A stored window observation for one managed account.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountLimitRow {
    pub provider: String,
    pub account_id: ProviderAccountId,
    pub window: String,
    pub used_percent: u8,
    pub resets_at: Option<i64>,
    pub plan: Option<String>,
    pub observed_at: i64,
    /// 'stream' when a running harness reported it; 'poll' when asked for.
    pub source: String,
}

pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store> {
    let StorageConfig::Sqlite { path } = cfg;
    Ok(Store {
        sqlite: sqlite::SqliteStore::new(path)?,
    })
}

/// Open for reading only. The ordinary open runs first so an incompatible
/// schema is reported the same way; the returned connection cannot write, so a
/// long-lived reader never commits and never wakes itself.
pub(crate) async fn open_read_only_store(cfg: &StorageConfig) -> StoreResult<Store> {
    let StorageConfig::Sqlite { path } = cfg;
    drop(sqlite::SqliteStore::new(path)?);
    Ok(Store {
        sqlite: sqlite::SqliteStore::open_read_only(path)?,
    })
}

/// Test-only: open a hermetic, fully-migrated [`Store`] at `path`. Unlike
/// [`open_store`], it consults **no** ambient env or machine identity
/// (`LF_HOME`, install selection, shared `~/.lf`), so the schema is deterministic
/// under parallel test execution instead of racing on the draft-application
/// decision. See [`sqlite::SqliteStore::open_ephemeral`].
pub async fn open_ephemeral_store(cfg: &StorageConfig) -> StoreResult<Store> {
    let StorageConfig::Sqlite { path } = cfg;
    Ok(Store {
        sqlite: sqlite::SqliteStore::open_ephemeral(path)?,
    })
}

/// Open the machine's shared registry store only if one already exists.
/// Returns `None` if the registry is absent or incompatible; never creates it.
pub async fn open_existing_store() -> Option<Store> {
    let cfg = crate::store::storage_config_from_env().ok()?;
    let StorageConfig::Sqlite { path } = &cfg;
    if !path.exists() {
        return None;
    }
    match open_store(&cfg).await {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::warn!(?path, %err, "local store is incompatible; run lf doctor");
            None
        }
    }
}

/// Why the shared registry could not be opened for a Task authority check.
/// Unlike [`open_existing_store`], this preserves the *reason* so a Task PR
/// entry point can refuse with an actionable error instead of silently
/// degrading to generic PR behavior.
#[derive(Debug, Clone)]
pub enum RegistryUnavailable {
    /// The configured registry path does not exist — no registry has been
    /// created on this machine. For a worktree with no ambient Task id this is
    /// the explicit "ordinary non-Task PR" case (no tasks exist); for a Task
    /// entry point it is missing authority.
    MissingFile { path: PathBuf },
    /// The registry path is configured but cannot be resolved — bad env, the
    /// development guard, or an IO failure before the file is even opened.
    Unresolved { error: String },
    /// The registry file exists but could not be opened: inaccessible, locked,
    /// or schema-incompatible. Actionable via `lf doctor`.
    Incompatible { path: PathBuf, error: String },
}

/// Open the shared registry for a Task authority check, surfacing the reason it
/// could not be opened rather than collapsing every failure to `None`. Callers
/// that must not degrade to generic PR behavior turn the [`RegistryUnavailable`]
/// into an actionable authority error; callers that may treat a missing file as
/// "no tasks on this machine" handle [`RegistryUnavailable::MissingFile`]
/// explicitly.
pub async fn open_registry_for_authority() -> Result<Store, RegistryUnavailable> {
    let path = database_path_from_env().map_err(|error| RegistryUnavailable::Unresolved {
        error: error.to_string(),
    })?;
    if !path.exists() {
        return Err(RegistryUnavailable::MissingFile { path });
    }
    open_store(&StorageConfig::sqlite(path.clone()))
        .await
        .map_err(|error| RegistryUnavailable::Incompatible {
            path,
            error: error.to_string(),
        })
}

pub type SharedStore = Arc<Store>;
#[cfg(test)]
mod tests {
    use super::{
        guard_development_database, may_apply_migrations, read_nonterminal_task_worktrees,
        CredentialState, PmSnapshotRow, ProviderAccount, ProviderAccountId, RoutingState,
        StorageConfig,
    };
    use crate::build_info::{BuildProvenance, MigrationAuthority};
    use crate::child::ChildRef;
    use crate::durable::{Author, WorkRef};
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::profile::EmailAddress;
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        CiIncident, GithubPr, PmWritebackState, PrPhase, PrPresentation, PrPublication, Task,
        TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn reads_nonterminal_task_ownership_without_opening_the_store_for_writes() {
        let temp = tempfile::tempdir().expect("create temp directory");
        let path = temp.path().join("registry.db");
        let connection = rusqlite::Connection::open(&path).expect("open fixture database");
        connection
            .execute_batch(
                "CREATE TABLE tasks (
                    id TEXT PRIMARY KEY,
                    worktree TEXT NOT NULL,
                    abandoned_at INTEGER
                 );
                 CREATE TABLE task_workflows (task_id TEXT PRIMARY KEY, node TEXT NOT NULL, edge INTEGER);
                 INSERT INTO tasks VALUES ('running', '/repo.running', NULL);
                 INSERT INTO tasks VALUES ('waiting', '/repo.waiting', NULL);
                 INSERT INTO tasks VALUES ('completed', '/repo.completed', NULL);
                 INSERT INTO tasks VALUES ('abandoned', '/repo.abandoned', 1);
                 INSERT INTO task_workflows VALUES ('running', 'review', 2);
                 INSERT INTO task_workflows VALUES ('completed', 'end', NULL);",
            )
            .expect("seed task ownership");
        drop(connection);

        let mut paths = read_nonterminal_task_worktrees(&path).expect("read task ownership");
        paths.sort();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/repo.running"),
                PathBuf::from("/repo.waiting")
            ]
        );
    }

    #[test]
    fn development_production_gate_has_no_override() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        let production = home.join(".lf/loopflow.db");
        assert!(
            guard_development_database(&production, BuildProvenance::Development, home,).is_err()
        );
        guard_development_database(&production, BuildProvenance::Release, home).unwrap();
    }

    #[test]
    fn advancing_the_shared_frontier_is_exclusive_to_the_promotion_boundary() {
        use super::FrontierAdvance::{Authorized, Forbidden};
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        let production = home.join(".lf/loopflow.db");
        let published = MigrationAuthority::Published;
        let validation_only = MigrationAuthority::ValidationOnly;

        // A validation-only build never writes migrations to the shared store,
        // boundary or not.
        assert!(!may_apply_migrations(&production, validation_only, home, Forbidden).unwrap());
        assert!(!may_apply_migrations(&production, validation_only, home, Authorized).unwrap());

        // A published build's ordinary open neither initializes nor advances the
        // shared store; only the promotion boundary owns both.
        assert!(!may_apply_migrations(&production, published, home, Forbidden).unwrap());
        assert!(may_apply_migrations(&production, published, home, Authorized).unwrap());

        // A private store may initialize regardless of published authority.
        // Its schema must match exactly on subsequent opens.
        let isolated = home.join(".lf-dev/branch/loopflow.db");
        assert!(may_apply_migrations(&isolated, validation_only, home, Forbidden).unwrap());
        assert!(may_apply_migrations(&isolated, published, home, Forbidden).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn development_production_gate_resolves_symlink_aliases() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let production_home = home.join(".lf");
        std::fs::create_dir_all(&production_home).unwrap();
        let alias = directory.path().join("store-alias");
        symlink(&production_home, &alias).unwrap();

        assert!(guard_development_database(
            &alias.join("loopflow.db"),
            BuildProvenance::Development,
            &home,
        )
        .is_err());
    }

    #[test]
    fn development_production_gate_normalizes_missing_parent_components() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        std::fs::create_dir_all(home.join(".lf")).unwrap();

        assert!(guard_development_database(
            &home.join(".lf/new/../loopflow.db"),
            BuildProvenance::Development,
            &home,
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn development_production_gate_rejects_existing_hard_link_alias() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let production = home.join(".lf/loopflow.db");
        std::fs::create_dir_all(production.parent().unwrap()).unwrap();
        std::fs::write(&production, b"database").unwrap();
        let alias = directory.path().join("alias.db");
        std::fs::hard_link(&production, &alias).unwrap();

        assert!(guard_development_database(&alias, BuildProvenance::Development, &home,).is_err());
    }

    fn make_wave(repo: &str) -> Wave {
        let id = WaveId::new();
        Wave::new(id.clone(), format!("wave-{id}"), repo.to_string())
    }

    fn make_task(wave: &Wave, project: &Project) -> Task {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        let id = TaskId::new();
        Task {
            id: id.clone(),
            plan: TaskPlan {
                revision: 0,
                linear_id: Some(LinearIssueId::new("issue-uuid").unwrap()),
                identifier: "INF-123".to_string(),
                title: "Add hello world".to_string(),
                description: "Ship one command".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: Some(PathBuf::from("/repo.inf-123")),
            workspace_slug: format!("task-{}", &id.as_str()[3..11]),
            agent: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        }
    }

    fn make_task_pr(task: &Task) -> TaskPr {
        TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: format!("jack/{}", task.workspace_slug),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }

    fn select_project(store: &super::Store, project: &Project) {
        let previous = crate::store::sqlite::project_selection::read_project_binding(
            &store.sqlite,
            &project.wave_id,
        )
        .unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            &project.wave_id,
            previous.as_deref(),
            project.plan.linear_id.as_ref().unwrap().as_str(),
            &super::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
    }
    fn make_project(wave: &Wave) -> Project {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                summary: String::new(),
                workflow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                linear_id: Some(
                    LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap(),
                ),
                slug: "developer-efficiency".to_string(),
                name: "Developer Efficiency".to_string(),
                prompt_context: "Definition:\nKeep local work fast.".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn task_planning_snapshot(wave: &Wave, project: &Project, task: &Task) -> PmSnapshotRow {
        let mut snapshot: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot.items.truncate(1);
        snapshot.projects[0].id = project
            .plan
            .linear_id
            .as_ref()
            .unwrap()
            .as_str()
            .to_string();
        snapshot.projects[0].revision = Some("2026-10-05T12:00:00Z".into());
        snapshot.items[0].id = task.plan.linear_id.as_ref().unwrap().as_str().to_string();
        snapshot.items[0].identifier = task.plan.identifier.clone();
        snapshot.items[0].project_id = Some(snapshot.projects[0].id.clone());
        snapshot.items[0].project = Some(snapshot.projects[0].slug.clone());
        snapshot.items[0].revision = Some("2026-10-05T12:00:00Z".into());
        PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: snapshot.projects[0].initiative_ids[0].clone(),
            synced_at: 1,
            snapshot,
        }
    }

    async fn planning_store() -> (tempfile::TempDir, super::Store, Wave) {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        (directory, store, wave)
    }

    #[tokio::test]
    async fn registration_returns_accepted_planning_and_preserves_reserved_identity() {
        for initializing in [false, true] {
            let (directory, store, wave) = planning_store().await;
            let project = make_project(&wave);
            store.create_project(&project).await.unwrap();
            select_project(&store, &project);
            let mut task = make_task(&wave, &project);
            task.worktree = Some(directory.path().join("checkout"));
            let pr = make_task_pr(&task);
            let mut snapshot = task_planning_snapshot(&wave, &project, &task);
            snapshot.synced_at = 17;
            snapshot.snapshot.items[0].completed = false;
            snapshot.snapshot.items[0].completed_at = None;
            snapshot.snapshot.items[0].state = Some("unstarted".into());
            snapshot.snapshot.items[0].identifier = "NEXT-9".into();
            snapshot.snapshot.items[0].name = "Accepted title".into();
            snapshot.snapshot.items[0].description = "Accepted direction".into();
            store.put_pm_snapshot(snapshot, None).await.unwrap();
            let accepted = if initializing {
                store
                    .create_task_with_worktree(&task, &pr, None)
                    .await
                    .unwrap()
            } else {
                store.create_task(&task, &pr, None).await.unwrap()
            };
            let mut expected = task.clone();
            expected.plan.identifier = "NEXT-9".into();
            expected.plan.title = "Accepted title".into();
            expected.plan.description = "Accepted direction".into();
            expected.plan.pm_snapshot_synced_at = Some(17);
            assert_eq!(accepted, expected);
            assert_eq!(store.get_task(&task.id).await.unwrap(), Some(expected));
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
            let events = store.task_events_after(&task.id, 0).await.unwrap();
            assert_eq!(events.len(), usize::from(initializing));
            if initializing {
                assert!(matches!(
                    events[0].kind,
                    TaskEventKind::WorktreeInitializing { .. }
                ));
            }
            assert!(!store.task_started(&task.id).await.unwrap());
        }
    }

    #[tokio::test]
    async fn registration_rejects_changed_issue_ownership_without_reserving_work() {
        for initializing in [false, true] {
            for destination in [Some("other-project"), None] {
                let (directory, store, wave) = planning_store().await;
                let project = make_project(&wave);
                store.create_project(&project).await.unwrap();
                select_project(&store, &project);
                let mut task = make_task(&wave, &project);
                task.worktree = Some(directory.path().join("checkout"));
                let pr = make_task_pr(&task);
                let mut snapshot = task_planning_snapshot(&wave, &project, &task);
                snapshot.snapshot.items[0].project_id = destination.map(str::to_string);
                store.put_pm_snapshot(snapshot, None).await.unwrap();
                let result = if initializing {
                    store.create_task_with_worktree(&task, &pr, None).await
                } else {
                    store.create_task(&task, &pr, None).await
                };
                assert!(result.unwrap_err().to_string().contains("changed Project"));
                assert!(store.get_task(&task.id).await.unwrap().is_none());
                assert!(store.task_prs(&task.id).await.unwrap().is_empty());
                assert!(!task.worktree.as_ref().unwrap().exists());
            }
        }
    }

    #[tokio::test]
    async fn cancelled_projection_retains_rotation_checkout_exclusion_through_commit() {
        let (directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let mut task = make_task(&wave, &project);
        task.worktree = Some(directory.path().join("checkout"));
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let snapshot = task_planning_snapshot(&wave, &project, &task);
        let checkouts = store
            .lock_checkout_roots(vec![task.worktree.as_ref().unwrap().clone()])
            .await
            .unwrap();
        let wave_lock = std::fs::File::create(directory.path().join("wave.lock")).unwrap();
        fs2::FileExt::try_lock_exclusive(&wave_lock).unwrap();
        let acquisition = Arc::new(super::PlanningLocks::new(wave_lock).with_checkouts(&checkouts));
        drop(checkouts);
        let entered = Arc::new(tokio::sync::Notify::new());
        let (release, blocked) = std::sync::mpsc::channel();
        let gate = (entered.clone(), Arc::new(std::sync::Mutex::new(blocked)));
        let writer_store = super::Store::from_sqlite_for_test(store.sqlite.clone());
        let writer = tokio::spawn(super::PLANNING_ACCEPTANCE_GATE.scope(gate, async move {
            writer_store
                .put_pm_snapshot(snapshot, Some(acquisition))
                .await
        }));
        tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        let excluded = store
            .lock_checkout_roots(vec![task.worktree.as_ref().unwrap().clone()])
            .await
            .is_err();
        release.send(()).unwrap();
        let admitted = store
            .lock_checkout_roots(vec![task.worktree.as_ref().unwrap().clone()])
            .await
            .unwrap();
        assert!(
            excluded,
            "caller cancellation released checkout exclusion before commit"
        );
        assert_eq!(
            store
                .get_task(&task.id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .pm_snapshot_synced_at,
            Some(1)
        );
        drop(admitted);
    }

    #[tokio::test]
    async fn cancelled_registration_retains_planning_exclusion_through_commit() {
        for initializing in [false, true] {
            let (directory, store, wave) = planning_store().await;
            let project = make_project(&wave);
            store.create_project(&project).await.unwrap();
            select_project(&store, &project);
            let mut task = make_task(&wave, &project);
            task.worktree = Some(directory.path().join("checkout"));
            let pr = make_task_pr(&task);
            let path = directory.path().join("wave.lock");
            let acquisition = std::fs::File::create(&path).unwrap();
            fs2::FileExt::try_lock_exclusive(&acquisition).unwrap();
            let competing = std::fs::File::open(&path).unwrap();
            let entered = std::sync::Arc::new(tokio::sync::Notify::new());
            let (release, blocked) = std::sync::mpsc::channel();
            let gate = (
                entered.clone(),
                std::sync::Arc::new(std::sync::Mutex::new(blocked)),
            );
            let writer_store = super::Store::from_sqlite_for_test(store.sqlite.clone());
            let input = task.clone();
            let input_pr = pr.clone();
            let writer = tokio::spawn(super::PLANNING_ACCEPTANCE_GATE.scope(gate, async move {
                let acquisition = Some(std::sync::Arc::new(super::PlanningLocks::new(acquisition)));
                if initializing {
                    writer_store
                        .create_task_with_worktree(&input, &input_pr, acquisition)
                        .await
                } else {
                    writer_store
                        .create_task(&input, &input_pr, acquisition)
                        .await
                }
            }));
            tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
                .await
                .unwrap();
            writer.abort();
            assert!(writer.await.unwrap_err().is_cancelled());
            let excluded = fs2::FileExt::try_lock_exclusive(&competing).is_err();
            let absent = store.get_task(&task.id).await.unwrap().is_none();
            release.send(()).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if fs2::FileExt::try_lock_exclusive(&competing).is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(
                excluded,
                "cancellation released the Wave before registration committed"
            );
            assert!(absent, "queued registration partially wrote its Task");
            assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), task);
            assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
            assert!(!store.task_started(&task.id).await.unwrap());
        }
    }

    #[tokio::test]
    async fn interrupted_reteam_preserves_confirmed_identifier_on_detail_refresh() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let mut task = make_task(&wave, &project);
        task.plan.pm_snapshot_synced_at = Some(1);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        snapshot.snapshot.items[0].name = task.plan.title.clone();
        snapshot.snapshot.items[0].description = task.plan.description.clone();
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();

        let mut confirmed = snapshot.snapshot.items[0].clone();
        confirmed.identifier = "NEXT-8".into();
        confirmed.revision = Some("2026-10-05T13:00:00Z".into());
        store
            .put_pm_task(
                "/repo",
                "linear",
                crate::store::PmTaskRecord {
                    item: confirmed,
                    project: Some(snapshot.snapshot.projects[0].clone()),
                    observed_at: 2,
                },
                Some((wave.id().clone(), snapshot.initiative.clone())),
                None,
            )
            .await
            .unwrap();
        let rebound = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(rebound.plan.identifier, "NEXT-8");

        // An earlier detail response arrives after the interrupted operation.
        store
            .put_pm_task(
                "/repo",
                "linear",
                crate::store::PmTaskRecord {
                    item: snapshot.snapshot.items[0].clone(),
                    project: Some(snapshot.snapshot.projects[0].clone()),
                    observed_at: snapshot.synced_at,
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), rebound);
    }

    #[tokio::test]
    async fn planning_projection_retains_independent_detail_entity_ages() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        snapshot.synced_at = 10;
        let mut other = make_task(&wave, &project);
        other.plan.linear_id = Some(LinearIssueId::new("unobserved-issue").unwrap());
        other.plan.identifier = "INF-124".into();
        other.worktree = Some(PathBuf::from("/repo.inf-124"));
        let other_pr = make_task_pr(&other);
        store.create_task(&other, &other_pr, None).await.unwrap();
        snapshot.snapshot.items.push(
            task_planning_snapshot(&wave, &project, &other)
                .snapshot
                .items
                .remove(0),
        );
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();
        let unobserved = store.get_task(&other.id).await.unwrap().unwrap();
        let mut item = snapshot.snapshot.items[0].clone();
        item.name = "New issue title".into();
        item.revision = Some("2026-10-05T12:00:01Z".into());
        let mut older_project = snapshot.snapshot.projects[0].clone();
        older_project.name = "Old Project title".into();
        older_project.revision = Some("2026-10-05T11:59:59Z".into());
        store
            .put_pm_task(
                "/repo",
                "linear",
                crate::store::PmTaskRecord {
                    item,
                    project: Some(older_project),
                    observed_at: 20,
                },
                None,
                None,
            )
            .await
            .unwrap();
        let retained = store.get_project(&project.id).await.unwrap().unwrap();
        let refreshed = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(retained.plan.name, snapshot.snapshot.projects[0].name);
        assert_eq!(retained.plan.pm_snapshot_synced_at, Some(10));
        assert_eq!(refreshed.plan.title, "New issue title");
        assert_eq!(refreshed.plan.pm_snapshot_synced_at, Some(20));
        assert_eq!(refreshed.worktree, task.worktree);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(
            store.get_task(&other.id).await.unwrap().unwrap(),
            unobserved
        );
        assert_eq!(store.task_prs(&other.id).await.unwrap(), vec![other_pr]);
    }

    #[tokio::test]
    async fn planning_projection_failure_rolls_back_accepted_observations() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let snapshot = task_planning_snapshot(&wave, &project, &task);
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();
        let retained = store.get_task(&task.id).await.unwrap().unwrap();
        let other = Wave::new(crate::id::WaveId::new(), "other".into(), "/repo".into());
        store.create_wave(&other).await.unwrap();
        let mut foreign = make_project(&other);
        foreign.plan.linear_id =
            Some(crate::planning::LinearProjectId::new("foreign-project").unwrap());
        store.create_project(&foreign).await.unwrap();
        let mut response = snapshot.clone();
        response.synced_at = 2;
        response.snapshot.items[0].name = "Rejected issue title".into();
        response.snapshot.items[0].revision = Some("2026-10-05T12:00:01Z".into());
        let mut conflict = response.snapshot.projects[0].clone();
        conflict.id = foreign
            .plan
            .linear_id
            .as_ref()
            .unwrap()
            .as_str()
            .to_string();
        response.snapshot.projects.push(conflict);
        assert!(store.put_pm_snapshot(response, None).await.is_err());
        assert_eq!(
            store.pm_snapshot(wave.id()).await.unwrap().unwrap(),
            snapshot
        );
        assert_eq!(store.get_task(&task.id).await.unwrap().unwrap(), retained);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
    }

    #[tokio::test]
    async fn project_projection_retains_accepted_entity_age() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        snapshot.synced_at = 10;
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();
        let original = snapshot.snapshot.projects[0].clone();
        snapshot.synced_at = 20;
        snapshot.snapshot.projects[0].revision = Some("2026-10-05T11:59:59Z".into());
        snapshot.snapshot.projects[0].name = "Older provider title".into();
        store.put_pm_snapshot(snapshot, None).await.unwrap();
        let accepted = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        assert_eq!(accepted.synced_at, 20);
        assert_eq!(accepted.snapshot.projects[0], original);
        let projected = store.get_project(&project.id).await.unwrap().unwrap();
        assert_eq!(projected.plan.name, original.name);
        assert_eq!(projected.plan.pm_snapshot_synced_at, Some(10));
    }

    #[tokio::test]
    async fn delayed_project_projection_preserves_a_newer_task_transfer() {
        let (_directory, store, wave) = planning_store().await;
        let predecessor = make_project(&wave);
        store.create_project(&predecessor).await.unwrap();
        select_project(&store, &predecessor);
        let task = make_task(&wave, &predecessor);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let old = task_planning_snapshot(&wave, &predecessor, &task);
        store.put_pm_snapshot(old, None).await.unwrap();
        // The first refresh has accepted and loaded its response, then pauses.
        let delayed = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        let predecessor_work = WorkRef::Project(predecessor.id.clone());
        let retained_placement = store.placement(&predecessor_work).await.unwrap();
        let next_home = crate::durable::MachineId::new();
        store
            .add_machine(&next_home, "ssh://fixture", "ssh://fixture", ".")
            .await
            .unwrap();
        store
            .place_work(&WorkRef::Wave(wave.id().clone()), &next_home)
            .await
            .unwrap();
        let mut newer = delayed.clone();
        newer.synced_at = 2;
        let mut successor = newer.snapshot.projects[0].clone();
        successor.id = "successor-project".into();
        successor.slug = "successor".into();
        successor.name = "Successor".into();
        newer.snapshot.items[0].project_id = Some(successor.id.clone());
        newer.snapshot.items[0].project = Some(successor.slug.clone());
        newer.snapshot.items[0].revision = Some("2026-10-05T12:00:01Z".into());
        newer.snapshot.projects.push(successor);
        store.put_pm_snapshot(newer, None).await.unwrap();
        let accepted = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        let transferred = store.get_task(&task.id).await.unwrap().unwrap();
        assert_ne!(transferred.project_id, predecessor.id);
        assert_eq!(
            store.placement(&predecessor_work).await.unwrap(),
            retained_placement
        );
        assert_eq!(
            store
                .placement(&WorkRef::Project(transferred.project_id.clone()))
                .await
                .unwrap()
                .machine_id,
            next_home
        );
        // A delayed response omits the now-known successor. Rejection must
        // preserve both normalized facts and the durable Task transfer.
        assert!(store.put_pm_snapshot(delayed, None).await.is_err());
        assert_eq!(
            store.pm_snapshot(wave.id()).await.unwrap().unwrap(),
            accepted
        );
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(
            store.get_task(&task.id).await.unwrap().unwrap(),
            transferred
        );
    }

    #[tokio::test]
    async fn cold_detail_preserves_durable_work_without_confirmed_wave_membership() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        let mut observed_project = snapshot.snapshot.projects.remove(0);
        observed_project.initiative_ids = vec!["elsewhere".into()];
        observed_project.name = "Another Wave's plan".into();
        observed_project.slug = "another-waves-plan".into();
        let mut item = snapshot.snapshot.items.remove(0);
        item.name = "Work now owned elsewhere".into();

        // Durable identity survives a cold planning cache. It does not prove
        // that this freshly observed Initiative still belongs to that Wave.
        store
            .put_pm_task(
                wave.repo(),
                "linear",
                crate::store::PmTaskRecord {
                    item,
                    project: Some(observed_project),
                    observed_at: 10,
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(store.pm_snapshot(wave.id()).await.unwrap().is_none());
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(store.get_project(&project.id).await.unwrap(), Some(project));
        assert_eq!(store.get_task(&task.id).await.unwrap(), Some(task));
    }

    #[tokio::test]
    async fn cold_detail_refreshes_confirmed_work_and_retains_identity() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        let mut observed = snapshot.snapshot.projects.remove(0);
        observed.name = "Refreshed plan".into();
        let mut item = snapshot.snapshot.items.remove(0);
        item.name = "Refreshed work".into();
        store
            .put_pm_task(
                wave.repo(),
                "linear",
                crate::store::PmTaskRecord {
                    item,
                    project: Some(observed),
                    observed_at: 10,
                },
                Some((wave.id().clone(), snapshot.initiative)),
                None,
            )
            .await
            .unwrap();
        let updated_project = store.get_project(&project.id).await.unwrap().unwrap();
        assert_eq!(updated_project.plan.name, "Refreshed plan");
        assert_eq!(updated_project.wave_id, project.wave_id);
        let mut updated_task = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(updated_task.plan.title, "Refreshed work");
        assert_eq!(updated_task.plan.pm_snapshot_synced_at, Some(10));
        updated_task.plan = task.plan.clone();
        assert_eq!(updated_task, task);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert!(store.pm_snapshot(wave.id()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stale_project_readback_cannot_assign_newer_membership_to_a_wave() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        let task = make_task(&wave, &project);
        let snapshot = task_planning_snapshot(&wave, &project, &task);
        let old = snapshot.snapshot.projects[0].clone();
        let mut current = old.clone();
        current.initiative_ids = vec!["elsewhere".into()];
        current.revision = Some("2026-10-05T12:00:01Z".into());
        store
            .put_pm_task(
                wave.repo(),
                "linear",
                crate::store::PmTaskRecord {
                    item: snapshot.snapshot.items[0].clone(),
                    project: Some(current.clone()),
                    observed_at: 10,
                },
                None,
                None,
            )
            .await
            .unwrap();
        assert!(store
            .put_pm_project(wave.id(), "linear", &snapshot.initiative, old, 20, None)
            .await
            .is_err());
        assert!(store.put_pm_snapshot(snapshot, None).await.is_err());
        assert!(store.pm_snapshot(wave.id()).await.unwrap().is_none());
        assert!(store
            .list_projects(Some(wave.id()))
            .await
            .unwrap()
            .is_empty());
        let retained = store
            .pm_task_observation(
                wave.repo(),
                "linear",
                task.plan.linear_id.as_ref().unwrap().as_str(),
            )
            .await
            .unwrap();
        assert_eq!(retained.record.unwrap().project, Some(current));
    }

    #[tokio::test]
    async fn confirmed_project_preserves_full_refresh_age_and_rejects_another_wave() {
        let (_directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let snapshot = task_planning_snapshot(&wave, &project, &task);
        store.put_pm_snapshot(snapshot.clone(), None).await.unwrap();
        let mut confirmed = snapshot.snapshot.projects[0].clone();
        confirmed.name = "Ordinary work".into();
        confirmed.slug = "ordinary-work".into();
        confirmed.workflow.clear();
        confirmed.krs.push(crate::pm::PmKr {
            text: "Preserve the authored proof".into(),
            holds: false,
        });
        confirmed.revision = Some("2026-10-05T12:00:01Z".into());
        store
            .put_pm_project(
                wave.id(),
                "linear",
                &snapshot.initiative,
                confirmed.clone(),
                10,
                None,
            )
            .await
            .unwrap();
        let accepted = store.pm_snapshot(wave.id()).await.unwrap().unwrap();
        assert_eq!(accepted.synced_at, snapshot.synced_at);
        assert_eq!(accepted.snapshot.projects, vec![confirmed.clone()]);
        let durable = store.get_project(&project.id).await.unwrap().unwrap();
        assert_eq!(durable.plan.pm_snapshot_synced_at, Some(10));
        assert!(durable
            .plan
            .prompt_context
            .starts_with("Project metric targets:"));
        assert!(!durable.plan.prompt_context.contains("flow:"));
        assert!(durable.plan.prompt_context.contains(&confirmed.krs[0].text));
        let other = Wave::new(WaveId::new(), "other".into(), wave.repo().into());
        store.create_wave(&other).await.unwrap();
        assert!(store
            .put_pm_project(
                other.id(),
                "linear",
                &snapshot.initiative,
                confirmed,
                20,
                None
            )
            .await
            .is_err());
        assert_eq!(
            store.get_project(&project.id).await.unwrap().unwrap(),
            durable
        );
        assert!(store
            .list_projects(Some(other.id()))
            .await
            .unwrap()
            .is_empty());
        store.put_pm_snapshot(snapshot, None).await.unwrap();
        assert_eq!(
            store.pm_snapshot(wave.id()).await.unwrap().unwrap(),
            accepted
        );
        assert_eq!(
            store.get_project(&project.id).await.unwrap().unwrap(),
            durable
        );
    }

    #[tokio::test]
    async fn interrupted_rotation_transfer_survives_a_late_planning_response() {
        let (_directory, store, wave) = planning_store().await;
        let predecessor = make_project(&wave);
        store.create_project(&predecessor).await.unwrap();
        select_project(&store, &predecessor);
        let mut successor = make_project(&wave);
        successor.plan.linear_id =
            Some(crate::planning::LinearProjectId::new("successor-project").unwrap());
        let task = make_task(&wave, &predecessor);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        store.create_project(&successor).await.unwrap();
        let mut response = task_planning_snapshot(&wave, &predecessor, &task);
        let mut next = response.snapshot.projects[0].clone();
        next.id = successor
            .plan
            .linear_id
            .as_ref()
            .unwrap()
            .as_str()
            .to_string();
        response.snapshot.projects.push(next);
        store.put_pm_snapshot(response.clone(), None).await.unwrap();
        // Confirmed transfer is accepted before rotation's final full refresh.
        let mut confirmed = response.snapshot.items[0].clone();
        confirmed.project_id = Some(successor.plan.linear_id.as_ref().unwrap().as_str().into());
        confirmed.revision = Some("2026-10-05T12:00:01Z".into());
        store
            .put_pm_task(
                wave.repo(),
                "linear",
                crate::store::PmTaskRecord {
                    item: confirmed,
                    project: Some(response.snapshot.projects[1].clone()),
                    observed_at: 2,
                },
                None,
                None,
            )
            .await
            .unwrap();
        let transferred = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(transferred.project_id, successor.id);
        // A delayed response cannot reverse the confirmed revision.
        store.put_pm_snapshot(response, None).await.unwrap();
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert_eq!(
            store.get_task(&task.id).await.unwrap().unwrap(),
            transferred
        );
    }

    #[tokio::test]
    async fn chapter_transfer_preserves_flow_pr_and_rejects_stale_parent_updates() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let predecessor = make_project(&wave);
        store.create_project(&predecessor).await.unwrap();
        select_project(&store, &predecessor);
        let task = make_task(&wave, &predecessor);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        // An operation step run in the checkout is started work.
        store.sqlite.test_flow(
            "code",
            &task.worktree.as_ref().unwrap().to_string_lossy(),
            &[("commit -m work", Some("succeeded"))],
            None,
        );
        store.sqlite.mark_task_started(&task.id).unwrap();
        assert!(store.chapter_task_evidence(&task.id).await.unwrap().begun);
        let flows = store.sqlite.task_flows(&task.id).unwrap();
        assert_eq!(flows.len(), 1);

        let mut successor = make_project(&wave);
        successor.plan.linear_id = Some(LinearProjectId::new("next-chapter").unwrap());
        store.create_project(&successor).await.unwrap();
        let response = task_planning_snapshot(&wave, &successor, &task);
        store.put_pm_snapshot(response, None).await.unwrap();
        let moved = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(moved.project_id, successor.id);
        assert_eq!(moved.worktree, task.worktree);
        assert_eq!(moved.plan.linear_id, task.plan.linear_id);
        assert_eq!(store.sqlite.task_flows(&task.id).unwrap(), flows);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::Failed {
                    error: "review needed".into(),
                    resumable: true,
                },
            )
            .await
            .unwrap();
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, TaskEventKind::Failed { .. })));
    }

    #[tokio::test]
    async fn steers_are_one_ordered_work_input_stream() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("registry.db");
        let store =
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database_path.clone()))
                .await
                .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let target = ChildRef::Task(task.id.clone());
        let work = store.work_for_child(&target).await.unwrap();
        let first = store
            .append_steer(&work, Author::User, "inspect the failing test")
            .await
            .unwrap();
        let second = store
            .append_steer(&work, Author::User, "preserve the public behavior")
            .await
            .unwrap();

        let steers = store.task_steers(&task.id).await.unwrap();
        assert_eq!(
            steers.iter().map(|steer| steer.id).collect::<Vec<_>>(),
            [first.id, second.id]
        );
        assert_eq!(
            steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["inspect the failing test", "preserve the public behavior"]
        );

        // The cross-Work `lf activity` timeline reads the same comments through
        // `steers_since`, attributed to their Work and ordered by time. Stamp the
        // two events so the `since` filter has something to bite on.
        let conn = rusqlite::Connection::open(&database_path).unwrap();
        conn.execute(
            "UPDATE task_events SET created_at=1700000001 WHERE id=?1",
            [first.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_events SET created_at=1700000021 WHERE id=?1",
            [second.id],
        )
        .unwrap();
        let timeline = store.steers_since(0).await.unwrap();
        assert_eq!(
            timeline
                .iter()
                .map(|comment| comment.steer.text.as_str())
                .collect::<Vec<_>>(),
            ["inspect the failing test", "preserve the public behavior"]
        );
        assert!(timeline.iter().all(|comment| comment.work == work));
        assert_eq!(
            store
                .steers_since(1_700_000_010)
                .await
                .unwrap()
                .into_iter()
                .map(|comment| comment.steer.id)
                .collect::<Vec<_>>(),
            [second.id]
        );
    }

    #[tokio::test]
    async fn ci_incident_reports_find_human_help_in_task_comments() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let observed_at = task.created_at - time::Duration::SECOND;
        let settled_at = task.created_at + time::Duration::SECOND;
        store
            .observe_ci_incident(&CiIncident {
                identity: "human-help-proof".to_string(),
                landing_id: None,
                task_id: Some(task.id.clone()),
                pr_id: None,
                repo: "loopflow".to_string(),
                pr_number: 123,
                failed_head_sha: "deadbeef".to_string(),
                repaired_head_sha: None,
                failure_set: vec!["tests".to_string()],
                provider_completed_at: None,
                poll_observed_at: Some(observed_at),
                webhook_received_at: None,
                claimed_landing_generation: None,
                responded_at: None,
                green_at: Some(settled_at),
                merged_at: None,
                blocked_at: None,
                blocked_reason: None,
                created_at: observed_at,
                updated_at: settled_at,
            })
            .await
            .unwrap();
        store
            .append_steer(
                &WorkRef::Task(task.id.clone()),
                Author::User,
                "try the flaky test again",
            )
            .await
            .unwrap();

        let incidents = store
            .ci_incidents_since(observed_at - time::Duration::SECOND, None, None)
            .await
            .unwrap();

        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].human_assisted);
    }

    /// Records the text of every `send_current` it accepts; `steerable=false`
    /// stands in for a worker (or a between-turns gap) that can't take live
    /// input, so the caller must defer to the next boundary.
    #[derive(Default)]
    struct RecordingHarness {
        sent: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        interrupts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        steerable: bool,
    }

    #[async_trait::async_trait]
    impl crate::harness::Harness for RecordingHarness {
        async fn start(&mut self, _config: &crate::engine::AgentConfig) -> anyhow::Result<()> {
            Ok(())
        }
        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn send_current(&mut self, content: &str) -> crate::harness::SendCurrentOutcome {
            if self.steerable {
                self.sent.lock().unwrap().push(content.to_string());
                crate::harness::SendCurrentOutcome::Sent {
                    provider_turn_id: "turn".to_string(),
                }
            } else {
                crate::harness::SendCurrentOutcome::NotSteerable
            }
        }
        async fn interrupt(&mut self) -> anyhow::Result<()> {
            self.interrupts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        async fn stop(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    #[tokio::test]
    async fn live_steers_inject_new_comments_and_defer_when_not_steerable() {
        let directory = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let work = WorkRef::Task(task.id.clone());

        let sent = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut harness = RecordingHarness {
            sent: sent.clone(),
            steerable: true,
            ..Default::default()
        };

        let first = store
            .append_steer(&work, Author::User, "focus on the parser")
            .await
            .unwrap();
        let second = store
            .append_steer(&work, Author::User, "keep the API stable")
            .await
            .unwrap();
        let mut cursor = 0;
        crate::ops::child::inject_live_steers(&store, &task.id, &mut harness, &mut cursor).await;
        assert_eq!(
            *sent.lock().unwrap(),
            ["focus on the parser", "keep the API stable"]
        );
        assert_eq!(
            cursor, second.id,
            "the cursor advances past what was injected"
        );
        assert!(first.id < second.id);

        // A comment that arrives later injects only itself — the cursor gates it.
        let third = store
            .append_steer(&work, Author::User, "add a regression test")
            .await
            .unwrap();
        crate::ops::child::inject_live_steers(&store, &task.id, &mut harness, &mut cursor).await;
        assert_eq!(sent.lock().unwrap().len(), 3);
        assert_eq!(cursor, third.id);

        // A provider that can't take live input leaves the cursor where it is, so
        // the comment rides the next skill boundary's seed instead.
        store
            .append_steer(&work, Author::User, "later direction")
            .await
            .unwrap();
        let mut deaf = RecordingHarness {
            steerable: false,
            ..Default::default()
        };
        let before = cursor;
        crate::ops::child::inject_live_steers(&store, &task.id, &mut deaf, &mut cursor).await;
        assert_eq!(
            cursor, before,
            "NotSteerable defers the comment to the next boundary seed"
        );
    }

    #[tokio::test]
    async fn interrupt_request_ends_the_turn_once_per_request() {
        let directory = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap(),
        );
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let work = WorkRef::Task(task.id.clone());

        let interrupts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut harness = RecordingHarness {
            interrupts: interrupts.clone(),
            ..Default::default()
        };
        let count = || interrupts.load(std::sync::atomic::Ordering::SeqCst);

        // A run launches with the cursor at the newest interrupt: prior requests
        // are inert history, never re-fired.
        let mut cursor = store.latest_interrupt_id(&work).await.unwrap();
        crate::ops::child::observe_interrupt(&store, &work, &mut harness, &mut cursor).await;
        assert_eq!(count(), 0, "no interrupt requested yet");

        // A new request ends the current turn exactly once.
        store.append_interrupt(&work).await.unwrap();
        crate::ops::child::observe_interrupt(&store, &work, &mut harness, &mut cursor).await;
        assert_eq!(count(), 1);
        crate::ops::child::observe_interrupt(&store, &work, &mut harness, &mut cursor).await;
        assert_eq!(count(), 1, "the same request never fires twice");

        // A second request fires again.
        store.append_interrupt(&work).await.unwrap();
        crate::ops::child::observe_interrupt(&store, &work, &mut harness, &mut cursor).await;
        assert_eq!(count(), 2);
    }

    #[tokio::test]
    async fn task_deletion_identity_follows_fresh_ownership_without_removing_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let original = make_wave("/repo");
        let current = Wave::new(WaveId::new(), "successor".into(), "/repo".into());
        store.create_wave(&original).await.unwrap();
        store.create_wave(&current).await.unwrap();
        let project = make_project(&current);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&current, &project);
        for wave in [&original, &current] {
            store
                .retain_task_issue_identity(
                    wave.id(),
                    task.plan.linear_id.as_ref().unwrap().as_str(),
                    &task.plan.identifier,
                )
                .await
                .unwrap();
        }
        assert!(store
            .task_issue_identity(original.id(), &task.plan.identifier)
            .await
            .unwrap()
            .is_none());
        for selector in [
            task.plan.linear_id.as_ref().unwrap().as_str(),
            &task.plan.identifier,
        ] {
            assert_eq!(
                store
                    .task_issue_identity(current.id(), selector)
                    .await
                    .unwrap(),
                Some((
                    task.plan.linear_id.as_ref().unwrap().as_str().to_string(),
                    task.plan.identifier.clone()
                ))
            );
        }
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        assert_eq!(store.list_tasks(None).await.unwrap(), vec![task]);
        assert!(store
            .deleted_task_issues(current.id())
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn task_deletion_confirmation_serializes_with_registration() {
        for registration_first in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            ))
            .await
            .unwrap();
            let wave = make_wave("/repo");
            store.create_wave(&wave).await.unwrap();
            let project = make_project(&wave);
            store.create_project(&project).await.unwrap();
            select_project(&store, &project);
            let task = make_task(&wave, &project);
            let pr = make_task_pr(&task);
            if registration_first {
                store.create_task(&task, &pr, None).await.unwrap();
                store
                    .confirm_task_deletion(
                        wave.id(),
                        task.plan.linear_id.as_ref().unwrap().as_str(),
                        &task.plan.identifier,
                    )
                    .await
                    .unwrap();
                assert!(store
                    .deleted_task_issues(wave.id())
                    .await
                    .unwrap()
                    .contains(task.plan.linear_id.as_ref().unwrap().as_str()));
                assert_eq!(store.get_task(&task.id).await.unwrap(), Some(task));
                assert_eq!(store.active_task_pr(&pr.task_id).await.unwrap(), Some(pr));
            } else {
                store
                    .confirm_task_deletion(
                        wave.id(),
                        task.plan.linear_id.as_ref().unwrap().as_str(),
                        &task.plan.identifier,
                    )
                    .await
                    .unwrap();
                assert!(store.create_task(&task, &pr, None).await.is_err());
                assert!(store.get_task(&task.id).await.unwrap().is_none());
                assert_eq!(
                    store
                        .task_deletion(wave.id(), &task.plan.identifier)
                        .await
                        .unwrap(),
                    Some((
                        task.plan.linear_id.as_ref().unwrap().as_str().to_string(),
                        task.plan.identifier
                    ))
                );
            }
        }
    }

    #[tokio::test]
    async fn task_creation_records_placement_without_synthetic_direction() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("registry.db");
        let store =
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database_path.clone()))
                .await
                .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let home = crate::durable::MachineId::new();
        store
            .add_machine(&home, "ssh://fixture", "ssh://fixture", ".")
            .await
            .unwrap();
        store
            .place_work(&WorkRef::Project(project.id.clone()), &home)
            .await
            .unwrap();
        let mut task = make_task(&wave, &project);
        task.worktree = Some(directory.path().join("uncreated-child-worktree"));
        let pr = make_task_pr(&task);

        store
            .create_task_with_worktree(&task, &pr, None)
            .await
            .expect("generic Run identity is opaque provenance, not planning authority");
        let placement = store
            .placement(&WorkRef::Task(task.id.clone()))
            .await
            .unwrap();
        assert_eq!(placement.machine_id, home);
        assert_eq!(store.get_task(&task.id).await.unwrap(), Some(task.clone()));
        let durable_child_rows = |path: &std::path::Path| {
            rusqlite::Connection::open(path)
                .unwrap()
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM tasks WHERE id=?1),
                        (SELECT COUNT(*) FROM task_events
                         WHERE task_id=?1 AND json_extract(kind_json, '$.kind')='steer'),
                        (SELECT COUNT(*) FROM task_prs WHERE task_id=?1)",
                    [task.id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .unwrap()
        };
        assert_eq!(durable_child_rows(&database_path), (1, 0, 1));

        let child_steers = store.task_steers(&task.id).await.unwrap();
        assert!(child_steers.is_empty());
        assert_eq!(
            store
                .latest_task_event(&task.id)
                .await
                .unwrap()
                .expect("initial Task publication records placement")
                .kind,
            TaskEventKind::WorktreeInitializing {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                branch: pr.branch.clone(),
                path: task.worktree.as_ref().unwrap().display().to_string(),
                base_commit: pr.base_commit.clone(),
            }
        );
        assert_eq!(durable_child_rows(&database_path), (1, 0, 1));
        assert!(!task.worktree.as_ref().unwrap().exists());
    }

    #[tokio::test]
    async fn sibling_completion_is_observation_not_task_recovery_authority() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let target = make_task(&wave, &project);
        store
            .create_task(&target, &make_task_pr(&target), None)
            .await
            .unwrap();
        let target_work = WorkRef::Task(target.id.clone());
        let target_status = store.work_status(&target_work).await.unwrap();
        let target_prs = store.task_prs(&target.id).await.unwrap();

        let mut sibling = make_task(&wave, &project);
        sibling.plan.linear_id = Some(LinearIssueId::new("sibling-issue-uuid").unwrap());
        sibling.plan.identifier = "INF-124".to_string();
        sibling.worktree = Some(PathBuf::from("/repo.inf-124"));
        store
            .create_task(&sibling, &make_task_pr(&sibling), None)
            .await
            .unwrap();
        store
            .append_task_event(
                &sibling.id,
                &TaskEventKind::Completed {
                    summary: "dependency complete".to_string(),
                },
            )
            .await
            .unwrap();

        assert_eq!(
            store.work_status(&target_work).await.unwrap(),
            target_status
        );
        assert_eq!(store.task_prs(&target.id).await.unwrap(), target_prs);
        assert!(store.task_steers(&target.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn task_facts_update_without_rewriting_work_progression() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("registry.db");
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();
        let persisted = store.get_task(&task.id).await.unwrap().unwrap();
        let before_pr = store.active_task_pr(&task.id).await.unwrap();
        let mut plan = persisted.plan.clone();
        plan.title = "Edited planning title".into();
        plan.description = "Edited planning notes".into();
        plan.pm_snapshot_synced_at = plan.pm_snapshot_synced_at.map(|at| at + 1);
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        snapshot.synced_at = plan.pm_snapshot_synced_at.unwrap();
        snapshot.snapshot.items[0].name = plan.title.clone();
        snapshot.snapshot.items[0].description = plan.description.clone();
        store.put_pm_snapshot(snapshot, None).await.unwrap();
        let by_stable_id = store
            .get_task_by_issue(task.id.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_stable_id.plan, plan);
        let mut expected = persisted;
        expected.plan = plan;
        assert_eq!(
            by_stable_id, expected,
            "editing planning facts must preserve execution facts"
        );
        assert_eq!(store.active_task_pr(&task.id).await.unwrap(), before_pr);

        let conn = rusqlite::Connection::open(database).unwrap();
        let columns = |table: &str| {
            conn.prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        let task_columns = columns("tasks");
        assert!(!task_columns.contains(&"phase_epoch".to_string()));
        assert!(!task_columns.contains(&"lifecycle_phase".to_string()));
        assert!(!columns("task_controller_state").contains(&"lifecycle_phase".to_string()));
    }

    #[tokio::test]
    async fn task_requires_an_existing_project_in_its_wave() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        let task = make_task(&wave, &project);

        let missing = store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("requires Project"));

        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let other_wave = make_wave("/other-repo");
        store.create_wave(&other_wave).await.unwrap();
        let wrong_wave = make_task(&other_wave, &project);
        let mismatched = store
            .create_task(&wrong_wave, &make_task_pr(&wrong_wave), None)
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not belong"));
    }

    #[tokio::test]
    async fn project_definition_updates_without_rewriting_the_task() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task), None)
            .await
            .unwrap();

        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        let mut observed = snapshot.snapshot.projects.remove(0);
        observed.workflow = "incident".into();
        observed.status = crate::pm::ProjectStatus::Completed;
        store
            .put_pm_project(
                wave.id(),
                "linear",
                &snapshot.initiative,
                observed.clone(),
                10,
                None,
            )
            .await
            .unwrap();

        let stored_project = store.get_project(&project.id).await.unwrap().unwrap();
        let stored_task = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(stored_project.plan.workflow, observed.workflow);
        assert_eq!(stored_project.plan.status, observed.status);
        assert_eq!(stored_project.plan.pm_snapshot_synced_at, Some(10));
        assert!(store.pm_snapshot(wave.id()).await.unwrap().is_none());
        assert_eq!(stored_project.iteration, 0);
        assert_eq!(stored_task.plan, task.plan);
        assert_eq!(stored_task.project_id, stored_project.id);
    }

    #[tokio::test]
    async fn task_pr_persists_presentation_github_and_ci_observations() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();

        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            presentation: Some(PrPresentation {
                title: "Ship the proof".to_string(),
                body: "Reviewer context".to_string(),
                head_sha: "sha-abc".to_string(),
            }),
            github: Some(GithubPr {
                number: 902,
                url: "https://github.com/loopflow/loopflow/pull/902".to_string(),
                head_sha: Some("sha-abc".to_string()),
            }),
            merge: None,
        });
        pr.ci_observation = Some(crate::work::task::CiObservation {
            head_sha: "sha-abc".to_string(),
            state: crate::work::task::CiState::Failing,
            failing_checks: vec![crate::work::task::CiCheck {
                name: "build".to_string(),
                url: Some("https://ci/build".to_string()),
            }],
            observed_at: OffsetDateTime::now_utc(),
        });
        pr.github_observation = Some(crate::work::task::GithubObservation {
            checked_at: OffsetDateTime::now_utc(),
            result: crate::work::task::GithubObservationResult::Degraded {
                reason: "GitHub API rate limit exhausted".to_string(),
            },
        });
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        let read = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(read.head_sha(), Some("sha-abc"));
        assert_eq!(read.presentation().unwrap().title, "Ship the proof");
        let ci = read.fresh_ci().expect("reading matches the current head");
        assert_eq!(ci.state, crate::work::task::CiState::Failing);
        assert_eq!(ci.failing_checks[0].name, "build");
        assert_eq!(read.github_observation, pr.github_observation);
    }

    #[tokio::test]
    async fn task_pr_persists_linear_linkage() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();

        pr.linear_attachment_id = Some("att-1".to_string());
        pr.linear_comment_id = Some("comment-1".to_string());
        pr.linear_link_error = Some("linear is down".to_string());
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        let read = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(read.linear_attachment_id.as_deref(), Some("att-1"));
        assert_eq!(read.linear_comment_id.as_deref(), Some("comment-1"));
        assert_eq!(read.linear_link_error.as_deref(), Some("linear is down"));
        assert_eq!(
            store
                .get_task_by_branch(&pr.branch)
                .await
                .unwrap()
                .unwrap()
                .id,
            task.id
        );
    }

    #[tokio::test]
    async fn task_prs_are_ordered_and_rotation_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut first = make_task_pr(&task);
        store.create_task(&task, &first, None).await.unwrap();

        first.publication = Some(PrPublication {
            requested_at: first.updated_at,
            presentation: None,
            github: Some(GithubPr {
                number: 101,
                url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
                head_sha: None,
            }),
            merge: None,
        });
        first.merge_commit = Some("merge-101".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        let now = OffsetDateTime::now_utc();
        let second = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 2,
            slug: "released-proof".to_string(),
            branch: format!("jack/{}-released-proof", task.workspace_slug),
            base_commit: "main-after-101".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        store.settle_task_pr(&first, Some(&second)).await.unwrap();
        store.settle_task_pr(&first, Some(&second)).await.unwrap();

        assert_eq!(
            store
                .task_prs(&task.id)
                .await
                .unwrap()
                .iter()
                .map(|pr| pr.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            store.active_task_pr(&task.id).await.unwrap().unwrap().id,
            second.id
        );
        assert!(store
            .get_task_by_branch(&first.branch)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .get_task_by_branch(&second.branch)
                .await
                .unwrap()
                .unwrap()
                .id,
            task.id
        );

        let mut abandoned = second.clone();
        let abandoned_at = OffsetDateTime::now_utc();
        abandoned.abandoned_at = Some(abandoned_at);
        abandoned.updated_at = abandoned_at;
        let conflicting = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 3,
            slug: "conflict".to_string(),
            branch: first.branch.clone(),
            base_commit: "main-after-102".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        assert!(store
            .settle_task_pr(&abandoned, Some(&conflicting))
            .await
            .is_err());
        assert_eq!(
            store
                .active_task_pr(&task.id)
                .await
                .unwrap()
                .unwrap()
                .phase(),
            PrPhase::Working
        );
    }

    /// The settle equality includes `abandoned_at`, so a re-settle must carry
    /// the original abandonment time. GitHub-observation reconcile once
    /// re-stamped it with `now` on every `lf task status`, and the task
    /// wedged permanently on "already settled differently" (live: W2-283).
    #[tokio::test]
    async fn re_settling_an_abandoned_pr_is_idempotent_only_at_its_original_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();

        let first_abandonment = OffsetDateTime::now_utc();
        pr.abandoned_at = Some(first_abandonment);
        pr.updated_at = first_abandonment;
        store.settle_task_pr(&pr, None).await.unwrap();
        store
            .settle_task_pr(&pr, None)
            .await
            .expect("same settle is idempotent");

        let mut restamped = pr.clone();
        restamped.abandoned_at = Some(first_abandonment + time::Duration::seconds(30));
        assert!(
            store.settle_task_pr(&restamped, None).await.is_err(),
            "a drifted abandonment time is a different settle and must refuse"
        );
    }

    #[tokio::test]
    async fn separate_task_worktree_tracks_and_collapses_its_parent_pr() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let parent_task = make_task(&wave, &project);
        let mut parent = make_task_pr(&parent_task);
        store
            .create_task(&parent_task, &parent, None)
            .await
            .unwrap();

        // The parent is published but not merged — the child stacks on it.
        parent.publication = Some(PrPublication {
            requested_at: parent.updated_at,
            presentation: None,
            github: Some(GithubPr {
                number: 200,
                url: "https://github.com/loopflowstudio/loopflow/pull/200".to_string(),
                head_sha: Some("parent-tip".to_string()),
            }),
            merge: None,
        });
        store.update_task_pr(&parent).await.unwrap();

        let mut child = make_task(&wave, &project);
        child.plan.linear_id = Some(LinearIssueId::new("issue-child").unwrap());
        child.plan.identifier = "INF-124".to_string();
        child.worktree = Some(PathBuf::from("/repo.child-task"));
        let now = OffsetDateTime::now_utc();
        let child_pr = TaskPr {
            id: TaskPrId::new(),
            task_id: child.id.clone(),
            sequence: 1,
            slug: child.workspace_slug.clone(),
            branch: "jack/child-task".to_string(),
            base_commit: "parent-tip".to_string(),
            parent_pr_id: Some(parent.id.clone()),
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
        store.create_task(&child, &child_pr, None).await.unwrap();

        let active = store.active_task_pr(&child.id).await.unwrap().unwrap();
        assert_eq!(active.id, child_pr.id);
        assert_eq!(active.parent_pr_id, Some(parent.id.clone()));
        assert_eq!(
            store.get_task_pr(&parent.id).await.unwrap(),
            Some(parent.clone())
        );

        // A parent update moves the child's durable fork without changing its
        // ownership or parent link.
        store
            .sync_task_pr(
                &child_pr.id,
                "parent-tip-2",
                false,
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
        let synced = store.get_task_pr(&child_pr.id).await.unwrap().unwrap();
        assert_eq!(synced.base_commit, "parent-tip-2");
        assert_eq!(synced.parent_pr_id, Some(parent.id.clone()));

        // The parent merges; the child collapses onto main, dropping the link.
        parent.merge_commit = Some("merge-200".to_string());
        parent.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&parent).await.unwrap();
        store
            .sync_task_pr(
                &child_pr.id,
                "main-after-200",
                true,
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();

        let collapsed = store.active_task_pr(&child.id).await.unwrap().unwrap();
        assert_eq!(collapsed.id, child_pr.id);
        assert_eq!(collapsed.parent_pr_id, None);
        assert_eq!(collapsed.base_commit, "main-after-200");

        // The tracked branch, not the worktree path, identifies the Task.
        let by_branch = store
            .get_task_by_branch(&child_pr.branch)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_branch.id, child.id);
    }

    #[tokio::test]
    async fn pr_publication_round_trips_before_github_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();

        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            presentation: None,
            github: None,
            merge: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let publishing = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(publishing.phase(), PrPhase::Publishing);
        assert_eq!(publishing.publication, pr.publication);

        pr.publication.as_mut().unwrap().github = Some(GithubPr {
            number: 101,
            url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
            head_sha: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let open = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(open.phase(), PrPhase::Open);
        assert_eq!(open.publication, pr.publication);
    }

    #[tokio::test]
    async fn empty_pr_is_retired_with_cleanup_identity_when_task_completes() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();

        let mut refreshed_plan = task.plan.clone();
        refreshed_plan.title = "Latest provider title".into();
        let mut snapshot = task_planning_snapshot(&wave, &project, &task);
        snapshot.synced_at = refreshed_plan.pm_snapshot_synced_at.unwrap();
        snapshot.snapshot.items[0].name = refreshed_plan.title.clone();
        snapshot.snapshot.items[0].description = refreshed_plan.description.clone();
        store.put_pm_snapshot(snapshot, None).await.unwrap();
        store
            .complete_task(&task, Some(&pr), crate::store::sqlite::EndMove::Set, None)
            .await
            .unwrap();
        let retained = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(retained.plan, refreshed_plan);
        let stored = store.task_prs(&task.id).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, pr.id);
        assert_eq!(stored[0].branch, pr.branch);
        assert_eq!(stored[0].base_commit, pr.base_commit);
        assert_eq!(stored[0].phase(), PrPhase::Abandoned);
        assert!(store.active_task_pr(&task.id).await.unwrap().is_none());
    }

    async fn run_store_basic_suite(store: &super::Store) {
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(wave.id()).await.expect("get wave").is_some());

        let updated = Wave::new(
            wave.id().clone(),
            "renamed-without-relocation".to_string(),
            "/repo-updated".to_string(),
        );
        assert!(updated.promoted_at().is_none());
        store.update_wave(&updated).await.expect("update wave");
        let loaded = store
            .get_wave(wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(
            loaded.repo(),
            "/repo",
            "ordinary Wave updates cannot bypass relocation"
        );
        assert_eq!(loaded.name(), wave.name());

        store.delete_wave(wave.id()).await.expect("delete wave");
        assert!(store
            .get_wave(wave.id())
            .await
            .expect("get deleted wave")
            .is_none());
    }

    #[tokio::test]
    async fn sqlite_store_basic_suite() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");
        run_store_basic_suite(&store).await;
    }

    // Directory children share their parent's repository.
    #[tokio::test]
    async fn sqlite_wave_ancestry_and_children() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let parent = make_wave("/chord");
        store.create_wave(&parent).await.expect("create parent");

        let child_a = make_wave("/chord").with_parent(parent.id().clone());
        let child_b = make_wave("/chord").with_parent(parent.id().clone());
        store.create_wave(&child_a).await.expect("create child a");
        store.create_wave(&child_b).await.expect("create child b");

        // Wave exposes its parent relation after a round-trip.
        let reloaded = store
            .get_wave(child_a.id())
            .await
            .expect("get child")
            .expect("child exists");
        assert_eq!(reloaded.parent_wave_id(), Some(parent.id()));

        // Each child retains the shared directory parent.
        let children = store
            .list_child_waves(parent.id())
            .await
            .expect("list children");
        assert_eq!(children.len(), 2);
        let repos: Vec<&str> = children.iter().map(|w| w.repo()).collect();
        assert_eq!(repos, ["/chord", "/chord"]);

        // A leaf wave has no children.
        assert!(store
            .list_child_waves(child_a.id())
            .await
            .expect("leaf children")
            .is_empty());

        // The root wave itself has no parent.
        let root = store
            .get_wave(parent.id())
            .await
            .expect("get parent")
            .expect("parent exists");
        assert_eq!(root.parent_wave_id(), None);
    }

    #[tokio::test]
    async fn pm_snapshot_replacement_is_atomic_per_wave() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("store should open");
        let wave = Wave::new(WaveId::new(), "product".to_string(), "/repo".to_string());
        store.create_wave(&wave).await.expect("create Wave");
        let mut snapshot = PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: 1,
            snapshot: crate::pm::PmSnapshot {
                projects: vec![],
                items: vec![],
            },
        };
        store
            .put_pm_snapshot(snapshot.clone(), None)
            .await
            .expect("write snapshot");
        snapshot.synced_at = 2;
        store
            .put_pm_snapshot(snapshot.clone(), None)
            .await
            .expect("replace snapshot");

        assert_eq!(
            store.pm_snapshot(wave.id()).await.expect("read snapshot"),
            Some(snapshot)
        );
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn sqlite_health_check_succeeds() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        store.health_check().await.expect("sqlite health check");
    }

    #[tokio::test]
    async fn provider_token_round_trip() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Initially empty
        assert!(store
            .list_provider_tokens()
            .await
            .expect("list empty")
            .is_empty());
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get missing")
            .is_none());

        // Upsert a token
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_abc123".to_string(),
            refresh_token: Some("ghr_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        // Read it back
        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token should exist");
        assert_eq!(loaded.provider, "github");
        assert_eq!(loaded.access_token, "gho_abc123");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_refresh"));
        assert_eq!(loaded.oauth_client_id.as_deref(), Some("github-client"));
        assert_eq!(loaded.expires_at, Some(1700000000));
        assert_eq!(loaded.login.as_deref(), Some("octocat"));

        // Upsert overwrites
        let updated = super::ProviderToken {
            access_token: "gho_new456".to_string(),
            refresh_token: None,
            updated_at: 1699500000,
            ..token.clone()
        };
        store
            .upsert_provider_token(&updated)
            .await
            .expect("upsert update");
        let reloaded = store
            .get_provider_token("github")
            .await
            .expect("get updated")
            .expect("exists");
        assert_eq!(reloaded.access_token, "gho_new456");
        assert!(reloaded.refresh_token.is_none());

        assert_eq!(reloaded.credential_type, super::CredentialType::OAuth);

        // Add a second provider and list
        let claude_token = super::ProviderToken {
            provider: "claude".to_string(),
            access_token: "sk-ant-key".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&claude_token)
            .await
            .expect("upsert claude");
        let all = store.list_provider_tokens().await.expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].provider, "claude");
        assert_eq!(all[1].provider, "github");

        // Delete one
        store
            .delete_provider_token("github")
            .await
            .expect("delete github");
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get deleted")
            .is_none());
        assert_eq!(
            store
                .list_provider_tokens()
                .await
                .expect("list after delete")
                .len(),
            1
        );
    }

    fn provider_account(provider: &str, account_id: &str, utilization: u8) -> ProviderAccount {
        ProviderAccount {
            provider: provider.to_string(),
            account_id: ProviderAccountId::parse(account_id).unwrap(),
            home: Some(PathBuf::from(format!("/accounts/{provider}/{account_id}"))),
            login_email: Some(EmailAddress::parse(&format!("{account_id}@example.com")).unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: Some(utilization),
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[tokio::test]
    async fn lifecycle_updates_do_not_overwrite_runtime_health() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let mut stale_account = provider_account("claude", "primary", 0);
        store.upsert_provider_account(&stale_account).await.unwrap();
        store
            .record_provider_account_health(
                "claude",
                &stale_account.account_id,
                Some(100),
                Some(OffsetDateTime::now_utc().unix_timestamp() + 300),
                Some("rate-limited"),
            )
            .await
            .unwrap();

        stale_account.plan = Some("max".to_string());
        stale_account.routing_state = RoutingState::ExplicitOnly;
        store
            .update_provider_account_lifecycle(&stale_account)
            .await
            .unwrap();

        let account = store
            .get_provider_account("claude", &stale_account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.plan.as_deref(), Some("max"));
        assert_eq!(account.routing_state, RoutingState::ExplicitOnly);
        assert_eq!(account.utilization_percent, Some(100));
        assert_eq!(account.cooldown_reason.as_deref(), Some("rate-limited"));
    }

    #[tokio::test]
    async fn provider_login_email_is_unique_within_each_provider() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            dir.path().join("registry.db"),
        ))
        .await
        .unwrap();
        let primary = provider_account("claude", "primary", 0);
        let mut duplicate = provider_account("claude", "duplicate", 0);
        duplicate.login_email = primary.login_email.clone();

        store.upsert_provider_account(&primary).await.unwrap();
        assert!(store.upsert_provider_account(&duplicate).await.is_err());

        duplicate.provider = "codex".to_string();
        store.upsert_provider_account(&duplicate).await.unwrap();
    }

    #[tokio::test]
    async fn provider_tokens_are_encrypted_at_rest_in_sqlite() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path.clone());
        let store = super::open_store(&config).await.expect("store should open");
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_secret_access".to_string(),
            refresh_token: Some("ghr_secret_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, raw_refresh, encrypted): (String, Option<String>, bool) = conn
            .query_row(
                "SELECT access_token, refresh_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query provider token");

        assert_ne!(raw_access, "gho_secret_access");
        assert_ne!(
            raw_refresh.as_deref(),
            Some("ghr_secret_refresh"),
            "refresh token should be encrypted"
        );
        assert!(encrypted, "encrypted flag should be true");
    }

    #[tokio::test]
    async fn sqlite_open_migrates_existing_plaintext_provider_tokens() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite db");
            super::migrations::apply_sqlite(&conn).expect("apply migrations");
            // The open below expects this build's schema, drafts included.
            for draft in crate::build_info::migration_draft_manifest() {
                conn.execute_batch(draft.sql).expect("apply draft");
            }
            conn.execute(
                "INSERT INTO provider_tokens
                 (provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![
                    "github",
                    "gho_plaintext",
                    "ghr_plaintext",
                    1700000000_i64,
                    "octocat",
                    1699000000_i64,
                    "oauth",
                ],
            )
            .expect("insert plaintext token");
        }

        // This test exercises the production open path itself (plaintext token
        // migration only runs there), so it must NOT use the hermetic helper.
        let store = super::open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("open store");

        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token exists");
        assert_eq!(loaded.access_token, "gho_plaintext");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_plaintext"));

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, encrypted): (String, bool) = conn
            .query_row(
                "SELECT access_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query provider token");
        assert_ne!(raw_access, "gho_plaintext");
        assert!(encrypted);
    }
    #[tokio::test]
    async fn task_abandonment_refuses_launch_and_retains_flow_history() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("store.db"),
        ))
        .await
        .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        select_project(&store, &project);
        let task = make_task(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr, None).await.unwrap();
        store.sqlite.test_flow(
            "code",
            &task.worktree.as_ref().unwrap().to_string_lossy(),
            &[("implement", Some("failed"))],
            Some("failed"),
        );
        let work = WorkRef::Task(task.id.clone());
        store.sqlite.require_task_launch(&task.id).unwrap();
        store.begin_task_abandon(&task.id).await.unwrap();
        assert!(store.sqlite.require_task_launch(&task.id).is_err());

        store.abandon(&work, "canceled").await.unwrap();
        assert!(store.sqlite.require_task_launch(&task.id).is_err());
        assert_eq!(store.sqlite.task_flows(&task.id).unwrap().len(), 1);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), vec![pr]);
        assert!(store
            .get_task_by_issue(&task.plan.identifier)
            .await
            .unwrap()
            .is_some());
    }
}
