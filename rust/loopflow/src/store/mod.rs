//! Daemonless local persistence shared by `lf`, Waves, Projects, and Tasks.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::chat::types::TurnUsage;
use crate::id::WaveId;
use crate::profile::{
    AccessProfile, AccountAccessProfile, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::wave::{Wave, WaveLocator};
mod children;
pub(crate) mod ci_incidents;
mod durable;
pub(crate) use durable::{AskCommentWrite, TaskWriterState};
pub mod migrations;
pub mod provider_deliveries;
pub mod rows;
pub mod sqlite;
mod token_crypto;

/// One row of the machine-grain run ledger (`run_events`): a lifecycle event
/// for a run, flow, or skill, written directly by `lf` into the local store.
///
/// Lineage only. Usage lives in `turn_usage_samples`, the grain the provider
/// actually measures; readers join through the Turn rather than reading tokens
/// from here.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEventRow {
    pub run_id: String,
    pub process_id: String,
    pub parent_process_id: Option<String>,
    pub seq: i64,
    pub ts: i64,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub wave: Option<String>,
    pub node: String,
    pub event: String,
    pub command: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub step_index: Option<i64>,
    pub error: Option<String>,
}

/// One provider-measured Turn's latest cumulative usage, joined to the
/// invocation that names where it ran.
///
/// Token fields stay `Option`: a provider can report one measurement and omit
/// another, and omission is not zero. Turns with no usage report at all do not
/// materialize in this internal additive view. Public consumers use
/// `UsageSnapshot` instead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AttributedTurnUsage {
    pub turn_id: String,
    pub invocation_id: String,
    pub exec_id: String,
    pub repo: String,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub provider: String,
    /// When the provider finished measuring. Falls back to the start for a turn
    /// still running, so a live turn still lands in a time bucket.
    pub at: i64,
    pub usage: TurnUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct TaskFirstProgressEvidenceRow {
    pub worktree: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub first_material_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct TaskPrPerformanceEvidenceRow {
    pub worktree: String,
    pub requested_at: i64,
    pub merged_at: Option<i64>,
    pub merge_tracking_complete: bool,
    pub repair_tracking_complete: bool,
    pub merge_observation_complete: bool,
    pub avoidable_rebase_agent: bool,
    pub manual_git_repair: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct PerformanceEvidenceSnapshot {
    pub authority_started_at: i64,
    pub task_runs: Vec<TaskFirstProgressEvidenceRow>,
    pub task_prs: Vec<TaskPrPerformanceEvidenceRow>,
}

/// One provider-reported cumulative observation for a Turn.
///
/// Checkpoints are cumulative so a lost observation cannot corrupt later
/// totals. `output_tokens` is the provider's inclusive billed output total;
/// `reasoning_tokens` is only its optional breakdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnUsageSample {
    pub turn_id: String,
    pub observed_at: i64,
    pub final_receipt: bool,
    pub usage: TurnUsage,
}

/// One wave's locally readable PM projection. Linear owns the payload; sync
/// replaces this row atomically so readers never observe a partial refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmSnapshotRow {
    pub wave_id: WaveId,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub payload: String,
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
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("cannot reserve {target} while Run {run_id} is {state:?}")]
    RunFenced {
        target: String,
        run_id: crate::durable::RunId,
        state: crate::durable::RunState,
    },
    #[error(
        "Home upgrade {upgrade_id} fences Run reservation for runtime generation {runtime_generation:?}"
    )]
    HomeUpgradeFenced {
        upgrade_id: String,
        runtime_generation: Option<u64>,
    },
    #[error("{target} generation {generation} no longer holds its write lease")]
    LeaseRevoked { target: String, generation: u32 },
    #[error("stale Basis: expected {expected}, current {current}")]
    StaleBasis { expected: String, current: String },
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

pub const CONTROL_BIN_ENV: &str = "LF_CONTROL_BIN";
pub const CONTROL_HOME_ENV: &str = "LF_CONTROL_HOME";
pub const CONTROL_DB_PATH_ENV: &str = "LF_CONTROL_DB_PATH";

fn machine_home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn production_database_path() -> PathBuf {
    machine_home_dir().join(".lf/loopflow.db")
}

/// Resolve the live Home evidence store for read-only operator surfaces.
///
/// Development builds normally isolate writes under `.lf-dev/worktrees`.
/// Observability is different: it must describe the Home that launched the
/// process, and opening it through `open_run_ledger_read_only` cannot migrate
/// or otherwise mutate its schema. Explicit control authority wins, followed
/// by an ordinary override, then the installed Home.
pub(crate) fn authority_home_dir() -> PathBuf {
    std::env::var_os(CONTROL_HOME_ENV)
        .or_else(|| std::env::var_os("LF_HOME"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| machine_home_dir().join(".lf"))
}

pub(crate) fn observability_home_dir() -> PathBuf {
    authority_home_dir()
}

pub(crate) fn observability_database_path() -> Result<PathBuf, std::io::Error> {
    let home = observability_home_dir();
    let candidate = std::env::var_os(CONTROL_DB_PATH_ENV)
        .or_else(|| std::env::var_os("LF_DB_PATH"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("loopflow.db"));
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "observability database path must not escape its Home",
        ));
    }
    Ok(home.join(candidate))
}

pub(crate) fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    sqlite::read_nonterminal_task_worktrees(path)
}

pub(crate) fn writer_invocation_is_authoritative(
    path: &Path,
    invocation_id: &str,
) -> StoreResult<bool> {
    sqlite::writer_invocation_is_authoritative(path, invocation_id)
}

fn default_lf_home_dir() -> PathBuf {
    default_lf_home_dir_for(
        &machine_home_dir(),
        crate::build_info::provenance(),
        &crate::build_info::source_identity(),
    )
}

fn default_lf_home_dir_for(
    home: &Path,
    provenance: crate::build_info::BuildProvenance,
    source_identity: &str,
) -> PathBuf {
    if provenance.is_release() {
        home.join(".lf")
    } else {
        home.join(".lf-dev/worktrees").join(source_identity)
    }
}

pub(crate) fn lf_home_dir() -> PathBuf {
    select_store_env_value(
        crate::build_info::provenance(),
        std::env::var_os(CONTROL_HOME_ENV),
        std::env::var_os("LF_HOME"),
    )
    .map(PathBuf::from)
    .unwrap_or_else(default_lf_home_dir)
}

pub fn default_db_path() -> PathBuf {
    lf_home_dir().join("loopflow.db")
}

pub fn database_path_from_env() -> Result<PathBuf, std::io::Error> {
    resolve_database_path(
        select_store_env_value(
            crate::build_info::provenance(),
            std::env::var_os(CONTROL_DB_PATH_ENV),
            std::env::var_os("LF_DB_PATH"),
        ),
        lf_home_dir(),
    )
}

/// Resolve the current Home lf's home directory, ignoring `LF_CONTROL_HOME`.
///
/// A relaunch must target the current Home, not the historical control home a
/// legacy body carries in `LF_CONTROL_HOME`. Only `LF_HOME` (or the built-in
/// default) is honored here — the control-plane selection is deliberately not.
pub(crate) fn current_home_lf_home_dir() -> PathBuf {
    std::env::var_os("LF_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_lf_home_dir)
}

/// Resolve the current Home lf's database path, ignoring `LF_CONTROL_DB_PATH`.
///
/// The companion to [`current_home_lf_home_dir`]: a relaunch resolves the
/// current store, never the launching body's pinned control database.
pub(crate) fn current_home_database_path() -> Result<PathBuf, std::io::Error> {
    resolve_database_path(std::env::var_os("LF_DB_PATH"), current_home_lf_home_dir())
}

fn resolve_database_path(
    candidate_env: Option<OsString>,
    home_dir: PathBuf,
) -> Result<PathBuf, std::io::Error> {
    let candidate = candidate_env
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join("loopflow.db"));
    let path = if candidate.is_absolute() {
        candidate
    } else {
        if candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "LF_DB_PATH must not escape LF_HOME",
            ));
        }
        home_dir.join(candidate)
    };
    guard_development_database(&path, crate::build_info::provenance(), &machine_home_dir())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn select_store_env_value(
    provenance: crate::build_info::BuildProvenance,
    control: Option<OsString>,
    ordinary: Option<OsString>,
) -> Option<OsString> {
    if provenance.is_release() {
        control.or(ordinary)
    } else {
        ordinary
    }
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
/// binary. Only `lf install promote` — under the exclusive promotion lock and
/// the drained live-body fence — opens the store as `Authorized`.
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
/// is always the caller's to initialize and advance — that is the isolated dev
/// database. The shared release store is exclusive to the promotion boundary: a
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

fn same_database_file(left: &Path, right: &Path) -> Result<bool, std::io::Error> {
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

fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf, std::io::Error> {
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

#[derive(Debug)]
pub struct Store {
    sqlite: sqlite::SqliteStore,
}

async fn run_sqlite<T, F>(store: &sqlite::SqliteStore, func: F) -> StoreResult<T>
where
    T: Send + 'static,
    F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || func(store))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}

impl Store {
    #[cfg(test)]
    pub(crate) fn from_sqlite_for_test(sqlite: sqlite::SqliteStore) -> Self {
        Self { sqlite }
    }

    pub async fn put_pm_snapshot(&self, snapshot: PmSnapshotRow) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| store.put_pm_snapshot(&snapshot)).await
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

    pub async fn set_account_access_profiles(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        profile_ids: &[ProfileId],
    ) -> StoreResult<()> {
        let account_id = account_id.clone();
        let profile_ids = profile_ids.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.set_account_access_profiles(provider, &account_id, &profile_ids)
        })
        .await
    }

    pub async fn list_account_access_profiles(
        &self,
        provider: Option<Provider>,
        account_id: Option<&ProviderAccountId>,
    ) -> StoreResult<Vec<AccountAccessProfile>> {
        let account_id = account_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_account_access_profiles(provider, account_id.as_ref())
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
    ) -> StoreResult<()> {
        let provider_session_id = provider_session_id.to_string();
        let account_id = account_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pin_provider_session_route(provider, &provider_session_id, &account_id)
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

/// Open the machine's shared registry store only if one already exists.
/// `None` means this machine has no registry yet; callers that instrument best-effort (lf
/// self-registration, the wave server) treat that as "not instrumented" and
/// stay silent rather than conjuring an empty db.
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
    use super::sqlite::SqliteStore;
    use super::{
        default_lf_home_dir_for, guard_development_database, may_apply_migrations, open_store,
        read_nonterminal_task_worktrees, select_store_env_value, CredentialState, PmSnapshotRow,
        ProviderAccount, ProviderAccountId, RoutingState, RunEventRow, StorageConfig,
        TaskPrMergeEvidenceOutcome,
    };
    use crate::build_info::{BuildProvenance, MigrationAuthority};
    use crate::child::ChildRef;
    use crate::durable::{
        AdvanceReceipt, AgentInvocation, AuthenticatedRequest, Author, Basis, BoundaryState,
        Containment, ContainmentObservation, ControlCtx, EpochState, InvocationRoute, RunAdvance,
        RunLease, RunTrigger, StopCause, Turn, WorkRef, WorkStatus,
    };
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::profile::EmailAddress;
    use crate::project::{Project, ProjectId};
    use crate::task::{
        AfterMerge, GithubObservation, GithubObservationResult, GithubPr, PmWritebackState,
        PrMergeMode, PrMergeRequest, PrPhase, PrPresentation, PrPublication, Task, TaskEventKind,
        TaskId, TaskPr, TaskPrId, TaskPrRepairKind,
    };
    use crate::wave::Wave;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn build_provenance_selects_separate_default_store_universes() {
        let home = PathBuf::from("/home/operator");
        assert_eq!(
            default_lf_home_dir_for(&home, BuildProvenance::Release, "branch-a"),
            home.join(".lf")
        );
        assert_eq!(
            default_lf_home_dir_for(&home, BuildProvenance::Development, "branch-a"),
            home.join(".lf-dev/worktrees/branch-a")
        );
        assert_ne!(
            default_lf_home_dir_for(&home, BuildProvenance::Development, "branch-a"),
            default_lf_home_dir_for(&home, BuildProvenance::Development, "branch-b")
        );
    }

    #[test]
    fn reads_nonterminal_task_ownership_without_opening_the_store_for_writes() {
        let temp = tempfile::tempdir().expect("create temp directory");
        let path = temp.path().join("registry.db");
        let connection = rusqlite::Connection::open(&path).expect("open fixture database");
        connection
            .execute_batch(
                "CREATE TABLE tasks (id TEXT PRIMARY KEY, worktree TEXT NOT NULL);
                 CREATE TABLE epochs (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    number INTEGER NOT NULL,
                    state TEXT NOT NULL
                 );
                 INSERT INTO tasks VALUES ('running', '/repo.running');
                 INSERT INTO tasks VALUES ('waiting', '/repo.waiting');
                 INSERT INTO tasks VALUES ('completed', '/repo.completed');
                 INSERT INTO tasks VALUES ('abandoned', '/repo.abandoned');
                 INSERT INTO epochs VALUES ('e1', 'running', 1, 'open');
                 INSERT INTO epochs VALUES ('e2', 'waiting', 1, 'open');
                 INSERT INTO epochs VALUES ('e3', 'completed', 1, 'done');
                 INSERT INTO epochs VALUES ('e4', 'abandoned', 1, 'abandoned');",
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
    fn release_prefers_control_store_while_development_ignores_it() {
        let control = Some("/control".into());
        let ordinary = Some("/ordinary".into());
        assert_eq!(
            select_store_env_value(BuildProvenance::Release, control.clone(), ordinary.clone()),
            control
        );
        assert_eq!(
            select_store_env_value(BuildProvenance::Development, control, ordinary.clone()),
            ordinary
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

        // A private/isolated store is always the caller's to initialize and
        // advance, regardless of authority or boundary.
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
                id: LinearIssueId::new("issue-uuid").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Add hello world".to_string(),
                description: "Ship one command".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: PathBuf::from("/repo.inf-123"),
            workspace_slug: format!("task-{}", &id.as_str()[3..11]),
            lifecycle: crate::task::TaskLifecyclePlan::defaults(),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Loop,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
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

    fn make_project(wave: &Wave) -> Project {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("project-uuid").unwrap(),
                slug: "developer-efficiency".to_string(),
                name: "Developer Efficiency".to_string(),
                prompt_context: "Definition:\nKeep local work fast.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-project".to_string()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn start_project_turn(
        store: &super::Store,
        work: &WorkRef,
        basis: Basis,
        containment: &str,
    ) -> (RunLease, AgentInvocation, Turn) {
        let (_run, lease) = store
            .reserve_run(
                work,
                RunTrigger::Input {
                    basis: basis.clone(),
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: containment.to_string(),
                    },
                    cwd: PathBuf::from("/repo"),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        let crate::durable::AdvanceReceipt::Turn(turn) = store
            .advance_run(
                &lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Turn receipt")
        };
        (lease, invocation, turn)
    }

    #[tokio::test]
    async fn performance_authority_restart_preserves_missing_zero_and_incident() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let mut legacy_task = make_task(&wave, &project);
        legacy_task.plan.id = LinearIssueId::new("issue-legacy").unwrap();
        legacy_task.plan.identifier = "INF-200".to_string();
        legacy_task.worktree = PathBuf::from("/repo.legacy");
        legacy_task.workspace_slug = "legacy-landing".to_string();
        let legacy_pr = make_task_pr(&legacy_task);
        store.create_task(&legacy_task, &legacy_pr).await.unwrap();
        if store
            .sqlite
            .performance_evidence_since(0)
            .unwrap()
            .is_none()
        {
            store
                .sqlite
                .apply_migration_for_test("task_performance_authority")
                .unwrap();
        }

        let mut clean_task = make_task(&wave, &project);
        clean_task.plan.id = LinearIssueId::new("issue-clean").unwrap();
        clean_task.plan.identifier = "INF-201".to_string();
        clean_task.worktree = PathBuf::from("/repo.clean");
        clean_task.workspace_slug = "clean-landing".to_string();
        let clean_pr = make_task_pr(&clean_task);
        store.create_task(&clean_task, &clean_pr).await.unwrap();

        let mut repaired_task = make_task(&wave, &project);
        repaired_task.plan.id = LinearIssueId::new("issue-repaired").unwrap();
        repaired_task.plan.identifier = "INF-202".to_string();
        repaired_task.worktree = PathBuf::from("/repo.repaired");
        repaired_task.workspace_slug = "repaired-landing".to_string();
        let repaired_pr = make_task_pr(&repaired_task);
        store
            .create_task(&repaired_task, &repaired_pr)
            .await
            .unwrap();

        let mut missing_merge_task = make_task(&wave, &project);
        missing_merge_task.plan.id = LinearIssueId::new("issue-missing-merge").unwrap();
        missing_merge_task.plan.identifier = "INF-203".to_string();
        missing_merge_task.worktree = PathBuf::from("/repo.missing-merge");
        missing_merge_task.workspace_slug = "missing-merge-authority".to_string();
        let missing_merge_pr = make_task_pr(&missing_merge_task);
        store
            .create_task(&missing_merge_task, &missing_merge_pr)
            .await
            .unwrap();

        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "UPDATE task_prs SET repair_tracking_complete=0 WHERE id=?1",
                [legacy_pr.id.as_str()],
            )
            .unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT repair_tracking_complete FROM task_prs WHERE id=?1",
                    [legacy_pr.id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT merge_tracking_complete FROM task_prs WHERE id=?1",
                    [legacy_pr.id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1,
            "an active cutover PR has complete future merge instrumentation"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT repair_tracking_complete FROM task_prs WHERE id=?1",
                    [clean_pr.id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        drop(connection);

        let clean_work = store
            .work_for_child(&ChildRef::Task(clean_task.id.clone()))
            .await
            .unwrap();
        let (_, clean_lease) = store
            .reserve_run(&clean_work, RunTrigger::User)
            .await
            .unwrap();
        let AdvanceReceipt::Run(clean_run) = store
            .advance_run(
                &clean_lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "clean-progress".to_string(),
                    },
                    cwd: clean_task.worktree.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected a started Run")
        };
        let first = clean_run.started_at.unwrap();
        assert_eq!(
            store
                .record_first_material_at(&clean_lease, first)
                .await
                .unwrap(),
            first
        );
        assert_eq!(
            store
                .record_first_material_at(&clean_lease, first + time::Duration::seconds(5))
                .await
                .unwrap(),
            first,
            "the first accepted material receipt is immutable"
        );
        store
            .stop_run(
                &clean_lease,
                StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();

        let repaired_work = store
            .work_for_child(&ChildRef::Task(repaired_task.id.clone()))
            .await
            .unwrap();
        let (_, missing_lease) = store
            .reserve_run(&repaired_work, RunTrigger::User)
            .await
            .unwrap();
        store
            .advance_run(
                &missing_lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "missing-progress".to_string(),
                    },
                    cwd: repaired_task.worktree.clone(),
                },
            )
            .await
            .unwrap();
        store
            .stop_run(
                &missing_lease,
                StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();

        let merge_time = OffsetDateTime::now_utc() + time::Duration::seconds(30);
        let prepare_pr = |mut pr: TaskPr, number: u32| {
            let requested_at = merge_time - time::Duration::seconds(10);
            let head = format!("head-{number}");
            pr.publication = Some(PrPublication {
                requested_at: requested_at - time::Duration::seconds(5),
                presentation: Some(PrPresentation {
                    title: "Ship it".to_string(),
                    body: "Reviewer context".to_string(),
                    head_sha: head.clone(),
                }),
                github: Some(GithubPr {
                    number,
                    url: format!("https://github.com/loopflow/loopflow/pull/{number}"),
                    head_sha: Some(head.clone()),
                }),
                merge: Some(PrMergeRequest {
                    mode: PrMergeMode::Auto,
                    requested_at,
                    head_sha: head,
                    after_merge: AfterMerge::CompleteTask,
                    next_slug: None,
                }),
            });
            pr.github_observation = Some(GithubObservation {
                checked_at: merge_time,
                result: GithubObservationResult::Fresh,
            });
            pr.updated_at = requested_at;
            pr
        };

        let mut clean_pr = prepare_pr(clean_pr, 201);
        store.update_task_pr(&clean_pr).await.unwrap();
        clean_pr.merge_commit = Some("merge-clean".to_string());
        clean_pr.updated_at = merge_time;
        assert_eq!(
            store
                .settle_task_pr_merged(&clean_pr, Some(merge_time))
                .await
                .unwrap(),
            TaskPrMergeEvidenceOutcome::Accepted
        );
        assert!(
            store
                .record_task_pr_repair_incident(
                    &clean_pr.id,
                    TaskPrRepairKind::ManualGitRepair,
                    merge_time + time::Duration::seconds(1),
                )
                .await
                .is_err(),
            "a completed zero cannot become a later repair incident"
        );

        let mut repaired_pr = prepare_pr(repaired_pr, 202);
        store.update_task_pr(&repaired_pr).await.unwrap();
        assert!(store
            .record_task_pr_repair_incident(
                &repaired_pr.id,
                TaskPrRepairKind::AvoidableRebaseAgent,
                merge_time - time::Duration::seconds(2),
            )
            .await
            .unwrap());
        assert!(!store
            .record_task_pr_repair_incident(
                &repaired_pr.id,
                TaskPrRepairKind::AvoidableRebaseAgent,
                merge_time - time::Duration::seconds(1),
            )
            .await
            .unwrap());
        repaired_pr.merge_commit = Some("merge-repaired".to_string());
        repaired_pr.updated_at = merge_time;
        store
            .settle_task_pr_merged(&repaired_pr, Some(merge_time))
            .await
            .unwrap();
        repaired_pr = store.get_task_pr(&repaired_pr.id).await.unwrap().unwrap();
        assert_eq!(
            store
                .settle_task_pr_merged(&repaired_pr, Some(merge_time + time::Duration::seconds(5)),)
                .await
                .unwrap(),
            TaskPrMergeEvidenceOutcome::Conflict {
                accepted_at: merge_time.unix_timestamp(),
            },
            "a conflicting replay preserves the first merge instant"
        );

        let mut missing_merge_pr = prepare_pr(missing_merge_pr, 203);
        missing_merge_pr.github_observation = Some(GithubObservation {
            checked_at: merge_time,
            result: GithubObservationResult::Partial {
                reason: "GitHub merged_at missing".to_string(),
            },
        });
        store.update_task_pr(&missing_merge_pr).await.unwrap();
        missing_merge_pr.merge_commit = Some("merge-without-time".to_string());
        missing_merge_pr.updated_at = merge_time;
        assert_eq!(
            store
                .settle_task_pr_merged(&missing_merge_pr, None)
                .await
                .unwrap(),
            TaskPrMergeEvidenceOutcome::Missing,
            "correctness settles even when timing authority is absent"
        );

        drop(store);
        let reader = SqliteStore::open_run_ledger_read_only(&database).unwrap();
        let evidence = reader
            .performance_evidence_since(0)
            .unwrap()
            .expect("authority schema survives restart");

        assert_eq!(evidence.task_runs.len(), 2);
        assert_eq!(
            evidence
                .task_runs
                .iter()
                .filter(|run| run.first_material_at.is_some())
                .count(),
            1
        );
        assert_eq!(evidence.task_prs.len(), 3);
        assert_eq!(
            evidence
                .task_prs
                .iter()
                .filter(|pr| pr.merge_observation_complete)
                .count(),
            1,
            "the conflicting merge receipt remains durable partial evidence"
        );
        assert!(evidence
            .task_prs
            .iter()
            .all(|pr| pr.merge_tracking_complete));
        assert!(evidence
            .task_prs
            .iter()
            .all(|pr| pr.repair_tracking_complete));
        assert_eq!(
            evidence
                .task_prs
                .iter()
                .find(|pr| pr.worktree == repaired_task.worktree.to_string_lossy())
                .unwrap()
                .merged_at,
            Some(merge_time.unix_timestamp())
        );
        let missing_merge = evidence
            .task_prs
            .iter()
            .find(|pr| pr.worktree == missing_merge_task.worktree.to_string_lossy())
            .unwrap();
        assert_eq!(missing_merge.merged_at, None);
        assert!(!missing_merge.merge_observation_complete);
        assert_eq!(
            evidence
                .task_prs
                .iter()
                .filter(|pr| pr.avoidable_rebase_agent)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn task_run_reservation_refuses_remote_placement() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let remote = store
            .observe_home(&crate::durable::HomeId::new(), "ssh://operator@remote-home")
            .await
            .unwrap();
        store.place_work(&work, &remote.id).await.unwrap();

        let error = store
            .reserve_task_process(&task, WorkStatus::Ready)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("cannot reserve task"));
        assert!(store.current_run(&work).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn steers_are_one_ordered_basis_checked_input_stream() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let target = ChildRef::Task(task.id.clone());
        let work = store.work_for_child(&target).await.unwrap();
        let initial = store.current_epoch(&work).await.unwrap().current_basis;

        let first = store
            .append_steer(
                &work,
                Author::User,
                "inspect the failing test",
                Some(&initial),
            )
            .await
            .unwrap();
        let second = store
            .append_steer(
                &work,
                Author::User,
                "preserve the public behavior",
                Some(&first.steer.basis),
            )
            .await
            .unwrap();
        let stale = store
            .append_steer(&work, Author::User, "stale write", Some(&initial))
            .await
            .expect_err("an old Basis cannot append direction");
        assert!(matches!(stale, super::StoreError::StaleBasis { .. }));

        let seed = store.boundary_seed(&work).await.unwrap();
        assert_eq!(seed.basis, second.steer.basis);
        assert_eq!(
            seed.steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["inspect the failing test", "preserve the public behavior"]
        );
    }

    #[tokio::test]
    async fn only_the_active_parent_run_can_steer_child_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let child_lease = store
            .reserve_project_process(&project, WorkStatus::Ready)
            .await
            .unwrap()
            .unwrap();
        store
            .activate_project_process(&project, &child_lease)
            .await
            .unwrap();
        let parent_lease = store
            .resolve_run_lease(child_lease.run_token.clone())
            .await
            .unwrap();
        let task_work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();

        let error = store
            .steer(
                &ControlCtx::Run(&parent_lease),
                &task_work,
                "unproven parent Turn",
                None,
            )
            .await
            .expect_err("child control requires a durable parent Turn Basis");
        assert!(matches!(error, super::StoreError::InvalidAuthority(_)));
        let invocation = store
            .open_invocation_for_run(&parent_lease.run_id)
            .await
            .unwrap()
            .unwrap();
        store
            .advance_run(
                &parent_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id,
                },
            )
            .await
            .unwrap();

        let receipt = store
            .steer(
                &ControlCtx::Run(&parent_lease),
                &task_work,
                "inspect the child result",
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            receipt.steer.author,
            Author::Run(parent_lease.run_id.clone())
        );

        store
            .stop_run(
                &parent_lease,
                StopCause::Requested,
                ContainmentObservation::Absent,
            )
            .await
            .unwrap();
        let error = store
            .steer(
                &ControlCtx::Run(&parent_lease),
                &task_work,
                "stale parent",
                None,
            )
            .await
            .expect_err("a stopped parent Run cannot steer");
        assert!(matches!(error, super::StoreError::InvalidAuthority(_)));
        assert!(matches!(
            store.resolve_run_lease(child_lease.run_token.clone()).await,
            Err(super::StoreError::InvalidAuthority(_))
        ));
        assert!(crate::durable::RunLeaseToken::parse("run_not-a-capability").is_err());
    }

    #[tokio::test]
    async fn newer_parent_steer_fences_stale_child_run_reservation() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let project_work = WorkRef::Project(project.id.clone());
        let task_work = WorkRef::Task(task.id.clone());

        let selected = store
            .append_steer(&project_work, Author::User, "proceed with the Task", None)
            .await
            .unwrap();
        let (_run, project_lease) = store
            .reserve_run(
                &project_work,
                RunTrigger::Input {
                    basis: selected.steer.basis.clone(),
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &project_lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "project-race".to_string(),
                    },
                    cwd: PathBuf::from("/repo"),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &project_lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        let crate::durable::AdvanceReceipt::Turn(stale_turn) = store
            .advance_run(
                &project_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Turn receipt")
        };
        assert_eq!(stale_turn.basis, selected.steer.basis);

        let hold = store
            .append_steer(
                &project_work,
                Author::User,
                "hold this Task",
                Some(&selected.steer.basis),
            )
            .await
            .unwrap();
        let task_basis = store.current_epoch(&task_work).await.unwrap().current_basis;
        let error = store
            .reserve_child_run(
                &project_lease,
                &task_work,
                RunTrigger::Input { basis: task_basis },
            )
            .await
            .expect_err("revision N cannot launch after hold N+1");

        assert!(matches!(error, super::StoreError::StaleBasis { .. }));
        assert!(store.current_run(&task_work).await.unwrap().is_none());
        let seed = store.boundary_seed(&project_work).await.unwrap();
        assert_eq!(seed.basis, hold.steer.basis);
        assert_eq!(
            seed.steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["proceed with the Task", "hold this Task"]
        );

        store
            .advance_run(
                &project_lease,
                RunAdvance::TurnEnded {
                    turn_id: stale_turn.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(next_turn) = store
            .advance_run(
                &project_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected next Turn receipt")
        };
        assert_eq!(next_turn.basis, hold.steer.basis);
        assert!(store.current_run(&task_work).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn newer_parent_steer_rolls_back_initial_child_creation_before_files_exist() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("registry.db");
        let store = open_store(&StorageConfig::sqlite(database_path.clone()))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let project_work = WorkRef::Project(project.id.clone());
        let selected = store
            .append_steer(&project_work, Author::User, "run the child", None)
            .await
            .unwrap();
        let (project_lease, invocation, stale_turn) = start_project_turn(
            &store,
            &project_work,
            selected.steer.basis.clone(),
            "project-create-race",
        )
        .await;
        let hold = store
            .append_steer(
                &project_work,
                Author::User,
                "hold every dependent child",
                Some(&selected.steer.basis),
            )
            .await
            .unwrap();
        let mut task = make_task(&wave, &project);
        task.worktree = directory.path().join("uncreated-child-worktree");
        let pr = make_task_pr(&task);

        let error = store
            .create_task_run(
                &ControlCtx::Run(&project_lease),
                &task,
                &pr,
                "a sibling completed; begin file-writing work",
            )
            .await
            .expect_err("revision N cannot create a child after hold N+1");

        assert!(matches!(error, super::StoreError::StaleBasis { .. }));
        assert!(store.get_task(&task.id).await.unwrap().is_none());
        assert!(store.task_prs(&task.id).await.unwrap().is_empty());
        assert!(!task.worktree.exists());
        let durable_child_rows = |path: &std::path::Path| {
            rusqlite::Connection::open(path)
                .unwrap()
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM tasks WHERE id=?1),
                        (SELECT COUNT(*) FROM epochs WHERE task_id=?1),
                        (SELECT COUNT(*) FROM steers
                         WHERE epoch_id IN (SELECT id FROM epochs WHERE task_id=?1)),
                        (SELECT COUNT(*) FROM task_prs WHERE task_id=?1),
                        (SELECT COUNT(*) FROM runs
                         WHERE epoch_id IN (SELECT id FROM epochs WHERE task_id=?1))",
                    [task.id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .unwrap()
        };
        assert_eq!(durable_child_rows(&database_path), (0, 0, 0, 0, 0));
        let seed = store.boundary_seed(&project_work).await.unwrap();
        assert_eq!(seed.basis, hold.steer.basis);
        assert_eq!(
            seed.steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["run the child", "hold every dependent child"]
        );

        store
            .advance_run(
                &project_lease,
                RunAdvance::TurnEnded {
                    turn_id: stale_turn.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(current_turn) = store
            .advance_run(
                &project_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected current Turn receipt")
        };
        assert_eq!(current_turn.basis, hold.steer.basis);

        let (child_run, _child_lease) = store
            .create_task_run(
                &ControlCtx::Run(&project_lease),
                &task,
                &pr,
                "create a different child under current direction",
            )
            .await
            .expect("the current parent Turn can create its immediate child");

        let task_work = WorkRef::Task(task.id.clone());
        let child_seed = store.boundary_seed(&task_work).await.unwrap();
        assert_eq!(
            child_seed.steers[0].author,
            Author::Run(project_lease.run_id.clone())
        );
        assert_eq!(
            store.current_run(&task_work).await.unwrap().unwrap().id,
            child_run.id
        );
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
                path: task.worktree.display().to_string(),
                base_commit: pr.base_commit.clone(),
            }
        );
        assert_eq!(durable_child_rows(&database_path), (1, 1, 1, 1, 1));
        assert!(!task.worktree.exists());
    }

    #[tokio::test]
    async fn historical_project_route_cannot_authorize_automated_task_settlement() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let historical_project = make_project(&wave);
        store.create_project(&historical_project).await.unwrap();
        let task = make_task(&wave, &historical_project);
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();
        let task_work = WorkRef::Task(task.id.clone());
        let task_direction = store
            .append_steer(
                &task_work,
                Author::User,
                "historical successor direction",
                None,
            )
            .await
            .unwrap();
        let (_task_run, task_lease) = store
            .reserve_run(
                &task_work,
                RunTrigger::Input {
                    basis: task_direction.steer.basis.clone(),
                },
            )
            .await
            .unwrap();

        store
            .validate_task_run_route(&task, &task_lease, historical_project.plan.id.as_str())
            .await
            .expect("the launch Project is initially the current PM route");

        let mut current_project = make_project(&wave);
        current_project.plan.id = LinearProjectId::new("current-project-uuid").unwrap();
        current_project.plan.slug = "performance-efficiency".to_string();
        current_project.plan.name = "Performance and Efficiency".to_string();
        store.create_project(&current_project).await.unwrap();
        let current_project_work = WorkRef::Project(current_project.id.clone());
        let hold = store
            .append_steer(
                &current_project_work,
                Author::User,
                "hold the historically routed Task",
                None,
            )
            .await
            .unwrap();
        let route_error = store
            .validate_task_run_route(&task, &task_lease, current_project.plan.id.as_str())
            .await
            .expect_err("historical Project identity cannot cross a current PM move");
        assert!(matches!(
            route_error,
            super::StoreError::InvalidAuthority(_)
        ));
        assert!(route_error.to_string().contains("current PM routing names"));
        assert_eq!(store.task_prs(&task.id).await.unwrap(), [pr]);
        assert_eq!(
            store.current_epoch(&task_work).await.unwrap().state,
            EpochState::Open
        );
        assert_eq!(
            store.boundary_seed(&task_work).await.unwrap().basis,
            task_direction.steer.basis
        );
        assert_eq!(
            store.current_run(&task_work).await.unwrap().unwrap().id,
            task_lease.run_id
        );
        assert_eq!(
            store
                .boundary_seed(&current_project_work)
                .await
                .unwrap()
                .basis,
            hold.steer.basis
        );
    }

    #[tokio::test]
    async fn abandoned_task_recovery_requires_user_after_parent_freshness() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("registry.db");
        let store = open_store(&StorageConfig::sqlite(database_path.clone()))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let mut task = make_task(&wave, &project);
        task.worktree = directory.path().join("preserved-task-worktree");
        let pr = make_task_pr(&task);
        let request = AuthenticatedRequest::cli();
        store.create_task(&task, &pr).await.unwrap();
        let task_work = WorkRef::Task(task.id.clone());
        store
            .append_steer(&task_work, Author::User, "initial Task direction", None)
            .await
            .unwrap();
        let task_basis = store.current_epoch(&task_work).await.unwrap().current_basis;
        let (historical_run, task_lease) = store
            .reserve_run(
                &task_work,
                RunTrigger::Input {
                    basis: task_basis.clone(),
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &task_lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "abandoned-task".to_string(),
                    },
                    cwd: task.worktree.clone(),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(task_invocation) = store
            .advance_run(
                &task_lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Task Invocation receipt")
        };
        let abandoned = store
            .abandon(&task_work, "hold this Task", &task_basis)
            .await
            .unwrap()
            .epoch;
        assert_eq!(abandoned.state, EpochState::Abandoned);
        let historical_prs = store.task_prs(&task.id).await.unwrap();
        let historical_task_run = store.run_by_id(&historical_run.id).await.unwrap();
        let historical_invocations = store.invocations_for_run(&historical_run.id).await.unwrap();
        assert_eq!(historical_invocations.len(), 1);
        assert_eq!(historical_invocations[0].id, task_invocation.id);

        let project_work = WorkRef::Project(project.id.clone());
        let selected = store
            .append_steer(&project_work, Author::User, "dependency pending", None)
            .await
            .unwrap();
        let (project_lease, invocation, stale_turn) = start_project_turn(
            &store,
            &project_work,
            selected.steer.basis.clone(),
            "project-recovery-race",
        )
        .await;
        let hold = store
            .append_steer(
                &project_work,
                Author::User,
                "do not recover this Task",
                Some(&selected.steer.basis),
            )
            .await
            .unwrap();
        let mut recovered = task.clone();
        recovered.phase_epoch += 1;
        recovered.updated_at = time::OffsetDateTime::now_utc();

        let stale = store
            .reopen_task(
                &ControlCtx::Run(&project_lease),
                &recovered,
                None,
                "a sibling completed; recover the Task",
            )
            .await
            .expect_err("stale Project direction cannot reopen terminal Task Work");
        assert!(matches!(stale, super::StoreError::StaleBasis { .. }));
        assert!(matches!(
            store.current_epoch(&task_work).await,
            Err(super::StoreError::NotFound)
        ));
        assert_eq!(store.task_prs(&task.id).await.unwrap(), historical_prs);
        assert_eq!(
            store.run_by_id(&historical_run.id).await.unwrap(),
            historical_task_run
        );
        assert_eq!(
            store.invocations_for_run(&historical_run.id).await.unwrap(),
            historical_invocations
        );
        assert!(!task.worktree.exists());

        store
            .advance_run(
                &project_lease,
                RunAdvance::TurnEnded {
                    turn_id: stale_turn.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(current_turn) = store
            .advance_run(
                &project_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected current Turn receipt")
        };
        assert_eq!(current_turn.basis, hold.steer.basis);
        let dependency_only = store
            .reopen_task(
                &ControlCtx::Run(&project_lease),
                &recovered,
                None,
                "the dependency is complete",
            )
            .await
            .expect_err("current Project direction is not User recovery authority");
        assert!(matches!(
            dependency_only,
            super::StoreError::InvalidAuthority(_)
        ));
        assert!(dependency_only
            .to_string()
            .contains("explicit User recovery is required"));
        assert!(matches!(
            store.current_epoch(&task_work).await,
            Err(super::StoreError::NotFound)
        ));
        assert_eq!(store.task_prs(&task.id).await.unwrap(), historical_prs);
        assert!(!task.worktree.exists());

        store
            .reopen_task(
                &ControlCtx::User(&request),
                &recovered,
                None,
                "explicit User recovery",
            )
            .await
            .expect("User direction opens exactly one successor Epoch");

        let successor = store.current_epoch(&task_work).await.unwrap();
        assert_eq!(successor.number, abandoned.number + 1);
        assert_eq!(successor.state, EpochState::Open);
        assert_eq!(store.task_prs(&task.id).await.unwrap(), historical_prs);
        assert_eq!(
            store.run_by_id(&historical_run.id).await.unwrap(),
            historical_task_run
        );
        assert_eq!(
            store.invocations_for_run(&historical_run.id).await.unwrap(),
            historical_invocations
        );
        assert!(store.current_run(&task_work).await.unwrap().is_none());
        let successor_seed = store.boundary_seed(&task_work).await.unwrap();
        assert_eq!(successor_seed.steers.len(), 1);
        assert_eq!(successor_seed.steers[0].author, Author::User);
        assert_eq!(successor_seed.steers[0].text, "explicit User recovery");
        let steers = SqliteStore::new(&database_path)
            .unwrap()
            .list_steers_since(0)
            .unwrap();
        assert!(steers
            .iter()
            .any(|steer| steer.text == "initial Task direction"));
        assert!(steers
            .iter()
            .any(|steer| steer.text == "explicit User recovery"));
        assert!(!task.worktree.exists());
    }

    #[tokio::test]
    async fn sibling_completion_is_observation_not_task_recovery_authority() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let target = make_task(&wave, &project);
        store
            .create_task(&target, &make_task_pr(&target))
            .await
            .unwrap();
        let target_work = WorkRef::Task(target.id.clone());
        let target_epoch = store.current_epoch(&target_work).await.unwrap();
        let target_prs = store.task_prs(&target.id).await.unwrap();

        let mut sibling = make_task(&wave, &project);
        sibling.plan.id = LinearIssueId::new("sibling-issue-uuid").unwrap();
        sibling.plan.identifier = "INF-124".to_string();
        sibling.worktree = PathBuf::from("/repo.inf-124");
        store
            .create_task(&sibling, &make_task_pr(&sibling))
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
        let observations = store
            .pending_project_observations(&project.id)
            .await
            .unwrap();
        assert_eq!(observations.len(), 1);
        assert!(store
            .consume_task_observation_for_project(&project.id, &observations[0])
            .await
            .unwrap());

        assert_eq!(
            store.current_epoch(&target_work).await.unwrap(),
            target_epoch
        );
        assert_eq!(store.task_prs(&target.id).await.unwrap(), target_prs);
        assert!(store.current_run(&target_work).await.unwrap().is_none());
        assert!(store
            .boundary_seed(&target_work)
            .await
            .unwrap()
            .steers
            .is_empty());
    }

    #[tokio::test]
    async fn task_lifecycle_plan_and_position_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let mut task = make_task(&wave, &project);
        task.lifecycle =
            crate::task::TaskLifecyclePlan::standard("task-design", "code", "ship-demo");
        task.phase_cursor = 2;
        task.phase_iteration = 4;
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let persisted = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted.lifecycle.loop_.flow, "code");
        assert_eq!(persisted.lifecycle.first.flow, "task-design");
        assert_eq!(persisted.lifecycle.finally.flow, "ship-demo");
        assert_eq!(persisted.phase_cursor, 2);
        assert_eq!(persisted.phase_iteration, 4);

        task.phase_cursor = 3;
        task.phase_iteration = 5;
        store.update_task(&task).await.unwrap();
        let resumed = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!((resumed.phase_cursor, resumed.phase_iteration), (2, 4));
    }

    #[tokio::test]
    async fn task_phase_epoch_allows_resets_and_rejects_stale_positions() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let mut task = make_task(&wave, &project);
        task.lifecycle_phase = crate::task::TaskLifecyclePhase::First;
        task.phase_cursor = 1;
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let lease = store
            .reserve_task_process(&task, WorkStatus::Ready)
            .await
            .unwrap()
            .unwrap();
        store.activate_task_process(&task, &lease).await.unwrap();
        let mut stale = task.clone();

        task.enter_loop().unwrap();
        store.update_task_for_lease(&task, &lease).await.unwrap();
        let iterating = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(
            (
                iterating.lifecycle_phase,
                iterating.phase_epoch,
                iterating.phase_cursor
            ),
            (crate::task::TaskLifecyclePhase::Loop, 2, 0)
        );

        stale.phase_cursor = 9;
        stale.phase_iteration = 9;
        store.update_task_for_lease(&stale, &lease).await.unwrap();
        let after_stale = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(
            (
                after_stale.lifecycle_phase,
                after_stale.phase_epoch,
                after_stale.phase_cursor,
                after_stale.phase_iteration
            ),
            (crate::task::TaskLifecyclePhase::Loop, 2, 0, 0)
        );

        task.enter_finally(crate::task::TaskGateProposal {
            done: true,
            reason: "implementation complete".to_string(),
        })
        .unwrap();
        store.update_task_for_lease(&task, &lease).await.unwrap();
        let gating = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(
            gating.lifecycle_phase,
            crate::task::TaskLifecyclePhase::Finally
        );
        assert_eq!(gating.phase_epoch, 3);
        assert_eq!(gating.gate_cycle, 1);
        assert_eq!(
            gating.gate_proposal.unwrap().reason,
            "implementation complete"
        );
    }

    #[tokio::test]
    async fn task_requires_an_existing_project_in_its_wave() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        let task = make_task(&wave, &project);

        let missing = store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("requires Project"));

        store.create_project(&project).await.unwrap();
        let other_wave = make_wave("/other-repo");
        store.create_wave(&other_wave).await.unwrap();
        let wrong_wave = make_task(&other_wave, &project);
        let mismatched = store
            .create_task(&wrong_wave, &make_task_pr(&wrong_wave))
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not belong"));
    }

    #[tokio::test]
    async fn project_definition_updates_without_rewriting_the_task() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();

        project.plan.prompt_context = "Definition:\nCurrent proof".to_string();
        project.plan.pm_snapshot_synced_at += 1;
        store.update_project(&project).await.unwrap();

        let stored_project = store.get_project(&project.id).await.unwrap().unwrap();
        let stored_task = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(stored_project.plan, project.plan);
        assert_eq!(stored_task.plan, task.plan);
        assert_eq!(stored_task.project_id, stored_project.id);
    }

    #[tokio::test]
    async fn task_issue_identifier_rebinds_only_without_a_writing_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();

        let waiting = make_task(&wave, &project);
        store
            .create_task(&waiting, &make_task_pr(&waiting))
            .await
            .unwrap();
        assert!(store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());
        assert!(store.get_task_by_issue("INF-123").await.unwrap().is_none());
        assert_eq!(
            store
                .get_task_by_issue("PRD-8")
                .await
                .unwrap()
                .unwrap()
                .plan
                .identifier,
            "PRD-8"
        );
        assert!(!store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());

        let mut running = make_task(&wave, &project);
        running.plan.id = LinearIssueId::new("issue-running").unwrap();
        running.plan.identifier = "W2-9".to_string();
        running.worktree = PathBuf::from("/repo.running");
        store
            .create_task(&running, &make_task_pr(&running))
            .await
            .unwrap();
        store
            .reserve_task_process(&running, WorkStatus::Ready)
            .await
            .unwrap()
            .expect("active Run reserves");
        assert!(store
            .rebind_task_issue_identifier("issue-running", "W2-9", "PRD-9")
            .await
            .unwrap_err()
            .to_string()
            .contains("active Run"));
    }

    #[tokio::test]
    async fn task_pr_persists_presentation_github_and_ci_observations() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();

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
        pr.ci_observation = Some(crate::task::CiObservation {
            head_sha: "sha-abc".to_string(),
            state: crate::task::CiState::Failing,
            failing_checks: vec![crate::task::CiCheck {
                name: "build".to_string(),
                url: Some("https://ci/build".to_string()),
            }],
            observed_at: OffsetDateTime::now_utc(),
        });
        pr.github_observation = Some(crate::task::GithubObservation {
            checked_at: OffsetDateTime::now_utc(),
            result: crate::task::GithubObservationResult::Degraded {
                reason: "GitHub API rate limit exhausted".to_string(),
            },
        });
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        let read = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(read.head_sha(), Some("sha-abc"));
        assert_eq!(read.presentation().unwrap().title, "Ship the proof");
        let ci = read.fresh_ci().expect("reading matches the current head");
        assert_eq!(ci.state, crate::task::CiState::Failing);
        assert_eq!(ci.failing_checks[0].name, "build");
        assert_eq!(read.github_observation, pr.github_observation);
    }

    #[tokio::test]
    async fn task_pr_persists_linear_linkage() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();

        pr.linear_attachment_id = Some("att-1".to_string());
        pr.linear_comment_id = Some("comment-1".to_string());
        pr.linear_link_error = Some("linear is down".to_string());
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        let read = store.active_task_pr(&task.id).await.unwrap().unwrap();
        assert_eq!(read.linear_attachment_id.as_deref(), Some("att-1"));
        assert_eq!(read.linear_comment_id.as_deref(), Some("comment-1"));
        assert_eq!(read.linear_link_error.as_deref(), Some("linear is down"));
    }

    #[tokio::test]
    async fn task_prs_are_ordered_and_rotation_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        let mut first = make_task_pr(&task);
        store.create_task(&task, &first).await.unwrap();

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
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();

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
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let parent_task = make_task(&wave, &project);
        let mut parent = make_task_pr(&parent_task);
        store.create_task(&parent_task, &parent).await.unwrap();

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
        child.plan.id = LinearIssueId::new("issue-child").unwrap();
        child.plan.identifier = "INF-124".to_string();
        child.worktree = PathBuf::from("/repo.child-task");
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
        store.create_task(&child, &child_pr).await.unwrap();

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
            .rebase_task_pr(
                &child_pr.id,
                "parent-tip-2",
                false,
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
        let rebased = store.get_task_pr(&child_pr.id).await.unwrap().unwrap();
        assert_eq!(rebased.base_commit, "parent-tip-2");
        assert_eq!(rebased.parent_pr_id, Some(parent.id.clone()));

        // The parent merges; the child collapses onto main, dropping the link.
        parent.merge_commit = Some("merge-200".to_string());
        parent.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&parent).await.unwrap();
        store
            .rebase_task_pr(
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

        // The worktree lookup the rebase path relies on resolves the task.
        let by_worktree = store
            .get_task_by_worktree(&child.worktree.display().to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_worktree.id, child.id);
    }

    #[tokio::test]
    async fn pr_publication_round_trips_before_github_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        let mut pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();

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
    async fn empty_pr_is_skipped_when_task_completes() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let mut task = make_task(&wave, &project);
        task.enter_finally(crate::task::TaskGateProposal {
            done: true,
            reason: "empty Task is complete".to_string(),
        })
        .unwrap();
        let pr = make_task_pr(&task);
        store.create_task(&task, &pr).await.unwrap();

        store.complete_task(&task, Some(&pr)).await.unwrap();
        assert!(store.get_task(&task.id).await.unwrap().is_some());
        let stored = store.task_prs(&task.id).await.unwrap();
        assert!(stored.is_empty());
    }

    #[tokio::test]
    async fn concurrent_task_launchers_reserve_exactly_one_write_lease() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();
        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let launch = |store: Arc<super::Store>, barrier: Arc<tokio::sync::Barrier>| {
            let candidate = task.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                store
                    .reserve_task_process(&candidate, WorkStatus::Ready)
                    .await
                    .unwrap()
            })
        };
        let first = launch(Arc::clone(&store), Arc::clone(&barrier));
        let second = launch(Arc::clone(&store), Arc::clone(&barrier));
        barrier.wait().await;
        let (first, second) = tokio::join!(first, second);
        let leases = [first.unwrap(), second.unwrap()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        assert_eq!(leases.len(), 1);
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        assert!(store.current_run(&work).await.unwrap().is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn task_and_project_reservations_defer_during_promotion() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("registry.db");
        let store = Arc::new(
            super::open_store(&StorageConfig::sqlite(database.clone()))
                .await
                .unwrap(),
        );
        let connection = rusqlite::Connection::open(&database).unwrap();
        let enablement_is_materialized = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('work_placements') WHERE name='enabled'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !enablement_is_materialized {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "work_enablement",
                ))
                .unwrap();
        }
        drop(connection);
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave);
        store.create_project(&project).await.unwrap();

        let task = make_task(&wave, &project);
        store
            .create_task(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let promotion = crate::promotion_lock::acquire_exclusive().unwrap();
        let task_error = store
            .reserve_task_process(&task, WorkStatus::Ready)
            .await
            .unwrap_err();
        let project_error = store
            .reserve_project_process(&project, WorkStatus::Ready)
            .await
            .unwrap_err();
        for error in [task_error, project_error] {
            assert!(
                error.to_string().contains("promotion lock"),
                "reservation returned the wrong promotion deferral: {error}"
            );
        }

        drop(promotion);
        let task_lease = store
            .reserve_task_process(&task, WorkStatus::Ready)
            .await
            .unwrap();
        let project_lease = store
            .reserve_project_process(&project, WorkStatus::Ready)
            .await
            .unwrap();
        assert!(task_lease.is_some());
        assert!(project_lease.is_some());
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

    // A chord is a wave whose children point back at it via `parent_wave_id`.
    // `list_child_waves` returns those children (ordered, repos stitched); a
    // leaf wave returns none. This is the ancestry the WaveAgentTree needs.
    #[tokio::test]
    async fn sqlite_wave_ancestry_and_children() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let parent = make_wave("/chord");
        store.create_wave(&parent).await.expect("create parent");

        let child_a = make_wave("/repo-a").with_parent(parent.id().clone());
        let child_b = make_wave("/repo-b").with_parent(parent.id().clone());
        store.create_wave(&child_a).await.expect("create child a");
        store.create_wave(&child_b).await.expect("create child b");

        // Wave exposes its parent relation after a round-trip.
        let reloaded = store
            .get_wave(child_a.id())
            .await
            .expect("get child")
            .expect("child exists");
        assert_eq!(reloaded.parent_wave_id(), Some(parent.id()));

        // Chord contents = children where parent_wave_id = id, one per repo.
        let children = store
            .list_child_waves(parent.id())
            .await
            .expect("list children");
        assert_eq!(children.len(), 2);
        let repos: Vec<&str> = children.iter().map(|w| w.repo()).collect();
        assert!(repos.contains(&"/repo-a"));
        assert!(repos.contains(&"/repo-b"));

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

    /// A run_events row with no usage attached.
    fn event_row(run_id: &str, seq: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq,
            ts: seq,
            repo: Some("/repo".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: None,
            flow: None,
            skill: None,
            step_index: None,
            error: None,
        }
    }

    #[tokio::test]
    async fn pm_snapshot_replacement_is_atomic_per_wave() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let store = open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("store should open");
        let wave = Wave::new(WaveId::new(), "product".to_string(), "/repo".to_string());
        store.create_wave(&wave).await.expect("create Wave");
        let mut snapshot = PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: 1,
            payload: "{\"version\":1}".to_string(),
        };
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("write snapshot");
        snapshot.synced_at = 2;
        snapshot.payload = "{\"version\":2}".to_string();
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("replace snapshot");

        assert_eq!(
            store.pm_snapshot(wave.id()).await.expect("read snapshot"),
            Some(snapshot)
        );
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn a_closed_vocabulary_rejects_an_unknown_node() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");
        let row = event_row("bad-node", 0, "task", "started");
        let error = store
            .insert_run_event(&row)
            .expect_err("unknown node must violate the ledger contract");
        assert!(error.to_string().contains("CHECK constraint failed"));
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
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
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
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
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
}
