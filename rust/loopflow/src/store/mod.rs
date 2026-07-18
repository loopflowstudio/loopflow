//! Daemonless local persistence shared by `lf`, Waves, Projects, and Tasks.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::id::WaveId;
use crate::profile::{
    AccessProfile, AccountAccessProfile, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::wave::Wave;
mod child_sessions;
pub(crate) mod ci_incidents;
mod durable;
pub(crate) use durable::TaskWriterState;
pub mod migrations;
pub mod provider_deliveries;
pub mod rows;
pub mod sqlite;
mod token_crypto;

/// One row of the machine-grain run ledger (`run_events`): a lifecycle event
/// for a run, flow, or skill, written directly by `lf` into the local store.
///
/// Lineage only. Spend lives on `agent_turns`, the grain the provider actually
/// measures; readers join `run_events -> agent_launches -> agent_turns` rather
/// than reading tokens from here.
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

/// One provider-measured Turn's spend, joined to the launch that names where it
/// was spent. This is the only additive usage grain: `lf usage`, `lf top`, and
/// the trace tree all sum these rows, and every total is a grouping of them.
///
/// Token fields stay `Option`: a provider can report one measurement and omit
/// another, and omission is not zero. Turns with no usage report at all do not
/// materialize in this additive view. `lf usage --json` emits this shape, so
/// every field is required or explicitly optional — no wire defaults.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnSpendRow {
    pub turn_id: String,
    pub launch_id: String,
    pub trace_id: String,
    pub exec_id: String,
    pub repo: String,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    /// When the provider finished measuring. Falls back to the start for a turn
    /// still running, so a live turn still lands in a time bucket.
    pub at: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
}

/// One frame on the agent bus (`bus_messages`). `byline` is testimony — what
/// the publishing client said it was — and `channel` is evidence: where the
/// row actually arrived. Nothing derives identity server-side, so a forged
/// byline shows up as a mismatch between the two rather than being prevented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusMessage {
    /// Monotonic id; a subscriber's cursor is an id it has already read.
    pub id: i64,
    pub channel: String,
    pub byline: String,
    pub text: String,
    /// Unix seconds. The sweeper's window is measured against this.
    pub at: i64,
}

/// One wave's locally readable PM projection. Linear owns the payload; sync
/// replaces this row atomically so readers never observe a partial refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmSnapshotRow {
    pub repo: String,
    pub wave: String,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub payload: String,
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
    #[error("{target} generation {generation} no longer holds its write lease")]
    LeaseRevoked { target: String, generation: u32 },
    #[error("stale Basis: expected {expected}, current {current}")]
    StaleBasis { expected: String, current: String },
    #[error("invalid control authority: {0}")]
    InvalidAuthority(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

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

pub(crate) fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    sqlite::read_nonterminal_task_worktrees(path)
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
    pub async fn put_pm_snapshot(&self, snapshot: PmSnapshotRow) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| store.put_pm_snapshot(&snapshot)).await
    }

    pub async fn pm_snapshot(
        &self,
        repo: String,
        wave: String,
    ) -> StoreResult<Option<PmSnapshotRow>> {
        run_sqlite(&self.sqlite, move |store| store.pm_snapshot(&repo, &wave)).await
    }

    // The agent bus: publish is an INSERT, subscribe is a forward poll from an
    // id cursor. No server is in the path; the tables live in the baseline schema.

    /// Publish one frame, stamped now. Returns its id.
    pub async fn publish_bus(
        &self,
        channel: String,
        byline: String,
        text: String,
    ) -> StoreResult<i64> {
        let at = time::OffsetDateTime::now_utc().unix_timestamp();
        run_sqlite(&self.sqlite, move |store| {
            store.publish_bus(&channel, &byline, &text, at)
        })
        .await
    }

    /// Drop every frame published before `cutoff` (unix seconds). Production
    /// never sweeps by hand — publishing and every read carry the broom (see
    /// [`Self::swept_read`]); this aims it at a chosen cutoff so a test can
    /// age the bus out without waiting the window.
    #[cfg(test)]
    pub async fn sweep_bus(&self, cutoff: i64) -> StoreResult<usize> {
        run_sqlite(&self.sqlite, move |store| store.sweep_bus(cutoff)).await
    }

    /// Sweep the window, then read. The retention window is enforced by
    /// whoever looks, not only by whoever publishes next, so a lone report on
    /// a bus that then went quiet expires on schedule instead of waiting for a
    /// writer that may never come. Every bus read rides this broom.
    async fn swept_read<T, F>(&self, read: F) -> StoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
    {
        let cutoff = time::OffsetDateTime::now_utc().unix_timestamp() - sqlite::BUS_WINDOW_SECS;
        run_sqlite(&self.sqlite, move |store| {
            store.sweep_bus(cutoff)?;
            read(store)
        })
        .await
    }

    /// Every surviving frame after `cursor`, oldest first.
    pub async fn read_bus_after(&self, cursor: i64) -> StoreResult<Vec<BusMessage>> {
        self.swept_read(move |store| store.read_bus_after(cursor))
            .await
    }

    /// The high-water mark — where a fresh subscriber tunes in.
    pub async fn bus_head(&self) -> StoreResult<i64> {
        self.swept_read(|store| store.bus_head()).await
    }

    /// The oldest readable id. A durable cursor below `floor - 1` is a gap:
    /// frames this subscriber will never see.
    pub async fn bus_floor(&self) -> StoreResult<Option<i64>> {
        self.swept_read(|store| store.bus_floor()).await
    }

    pub async fn bus_cursor(&self, subscriber: String) -> StoreResult<Option<i64>> {
        run_sqlite(&self.sqlite, move |store| store.bus_cursor(&subscriber)).await
    }

    pub async fn set_bus_cursor(&self, subscriber: String, cursor: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.set_bus_cursor(&subscriber, cursor)
        })
        .await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let repo = repo.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| store.list_waves(repo.as_deref())).await
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

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        run_sqlite(&self.sqlite, move |store| store.get_wave_by_name(&name)).await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.create_wave(&wave)).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.update_wave(&wave)).await
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    };
    use crate::build_info::{BuildProvenance, MigrationAuthority};
    use crate::child_session::{
        BinaryProvenance, ChildBodyOutcome, ChildLeaseState, ChildProcessGeneration, ChildRef,
        ObservationRecipient,
    };
    use crate::durable::{
        AttentionRoute, AuthenticatedRequest, Author, Containment, ContainmentObservation,
        ControlCtx, FlowPosition, RunAdvance, SendState, StopCause,
    };
    use crate::id::WaveId;
    use crate::profile::EmailAddress;
    use crate::project_session::{
        ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::task::{
        AfterMerge, CiIncident, GithubPr, PmWritebackState, PrPhase, PrPublication, TaskEventKind,
        TaskPr, TaskPrId, TaskSession, TaskSessionId, TaskSessionStatus,
    };
    use crate::trace::{AgentLaunchRow, AgentTurnRow};
    use crate::wave::Wave;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;
    use time::{Duration, OffsetDateTime};

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
                "CREATE TABLE task_sessions (status TEXT NOT NULL, worktree TEXT NOT NULL);
                 INSERT INTO task_sessions VALUES ('running', '/repo.running');
                 INSERT INTO task_sessions VALUES ('waiting', '/repo.waiting');
                 INSERT INTO task_sessions VALUES ('completed', '/repo.completed');
                 INSERT INTO task_sessions VALUES ('abandoned', '/repo.abandoned');",
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

    fn make_task_session(wave: &Wave, project: &ProjectSession) -> TaskSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        let id = TaskSessionId::new();
        TaskSession {
            id: id.clone(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-uuid").unwrap(),
                    identifier: "INF-123".to_string(),
                    title: "Add hello world".to_string(),
                    description: "Ship one command".to_string(),
                },
                project: project.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id.clone(),
            status: TaskSessionStatus::Created,
            status_reason: "task session reserved".to_string(),
            status_at: now,
            worktree: PathBuf::from("/repo.inf-123"),
            workspace_slug: format!("task-{}", &id.as_str()[3..11]),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        }
    }

    fn make_task_pr(session: &TaskSession) -> TaskPr {
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: format!("jack/{}", session.workspace_slug),
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
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }

    fn make_project_session(wave: &Wave) -> ProjectSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-uuid").unwrap(),
                    slug: "developer-efficiency".to_string(),
                    name: "Developer Efficiency".to_string(),
                    prompt_context: "Definition:\nKeep local work fast.".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            status: ProjectSessionStatus::Running,
            status_reason: "project turn active".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-project".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                process_group_id: None,
                tmux_name: "lf-project-test".to_string(),
                agent: "codex".to_string(),
                provider: "codex".to_string(),
                provider_session_id: Some("thread-project".to_string()),
                started_at: now,
                state: crate::child_session::ChildLeaseState::Active,
                outcome: None,
                provenance: None,
            }),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn trace_launch(id: &str) -> AgentLaunchRow {
        AgentLaunchRow {
            id: id.to_string(),
            run_id: format!("run-{id}"),
            process_id: format!("process-{id}"),
            started_at: 1,
            ended_at: None,
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            wave: None,
            flow: None,
            skill: None,
            project: None,
            task: None,
            provider: "codex".to_string(),
            model: None,
            surface: "headless".to_string(),
            capture_status: "capturing".to_string(),
            incomplete_reason: None,
            outcome: "running".to_string(),
            artifact_dir: format!("trace/{id}"),
            conversation_path: format!("trace/{id}/conversation.jsonl"),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 0,
            conversation_bytes: 0,
            control: None,
        }
    }

    fn trace_turn(
        id: &str,
        launch_id: &str,
        ordinal: i64,
        status: &str,
        basis: crate::durable::Basis,
    ) -> AgentTurnRow {
        AgentTurnRow {
            id: id.to_string(),
            launch_id: launch_id.to_string(),
            ordinal,
            provider_turn_id: None,
            started_at: ordinal,
            ended_at: (status != "running").then_some(ordinal + 1),
            status: status.to_string(),
            input_op: "initial".to_string(),
            context_coverage: "unknown".to_string(),
            tokenizer: "unknown".to_string(),
            system_prompt_path: None,
            task_prompt_path: format!("trace/{id}/prompt.md"),
            system_tokens: 0,
            task_tokens: 0,
            supplied_context_tokens: 0,
            provider_input_tokens: None,
            provider_total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            provider_output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: 0,
            first_event_seq: None,
            last_event_seq: None,
            root_output: None,
            basis: Some(basis),
        }
    }

    #[tokio::test]
    async fn live_legacy_children_register_their_product_launch_before_control() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let project_work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        let project_run = store.current_run(&project_work).await.unwrap().unwrap();
        let project_launch = store
            .sqlite
            .control_launch_for_run(&project_run.id)
            .unwrap()
            .expect("live Project body has a product Launch");
        assert_eq!(project_launch.state, crate::durable::LaunchState::Live);
        assert_eq!(
            project_launch.containment,
            Containment::Tmux {
                name: "lf-project-test".to_string()
            }
        );

        let mut task = make_task_session(&wave, &project);
        task.status = TaskSessionStatus::Running;
        task.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-test".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-task".to_string()),
            started_at: task.created_at,
            state: ChildLeaseState::Active,
            outcome: None,
            provenance: None,
        });
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let task_work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let task_run = store.current_run(&task_work).await.unwrap().unwrap();
        let task_launch = store
            .sqlite
            .control_launch_for_run(&task_run.id)
            .unwrap()
            .expect("live Task body has a product Launch");
        assert_eq!(task_launch.state, crate::durable::LaunchState::Live);
        assert_eq!(
            task_launch.containment,
            Containment::Tmux {
                name: "lf-task-test".to_string()
            }
        );

        let request = AuthenticatedRequest::cli();
        let receipt = store
            .interrupt(&ControlCtx::User(&request), &task_work, &task_run.id)
            .await
            .unwrap();
        assert_eq!(receipt.launch_id, task_launch.id);
        assert_eq!(receipt.turn_id, None);
    }

    #[tokio::test]
    async fn legacy_body_lifecycle_keeps_its_run_launch_honest() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        task.begin_generation("lf-task-lifecycle".to_string());
        let reservation = store
            .reserve_task_process(&task, TaskSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let run = store.current_run(&work).await.unwrap().unwrap();
        let launch = store
            .sqlite
            .control_launch_for_run(&run.id)
            .unwrap()
            .expect("reservation registered a starting Launch");
        assert_eq!(launch.state, crate::durable::LaunchState::Starting);

        task.latest_process.as_mut().unwrap().state = ChildLeaseState::Active;
        task.set_status(TaskSessionStatus::Running, "active");
        store
            .activate_task_process(&task, &reservation)
            .await
            .unwrap();
        let active = store
            .launch_surface(&launch.id)
            .await
            .unwrap()
            .expect("active Launch remains addressable");
        assert_eq!(active.launch.state, crate::durable::LaunchState::Live);

        let revoked = store
            .revoke_task_process(
                &task.id,
                &ChildBodyOutcome::Lost {
                    reason: "provider exited".to_string(),
                },
            )
            .await
            .unwrap();
        let stopping = store
            .launch_surface(&launch.id)
            .await
            .unwrap()
            .expect("stopping Launch remains observable");
        assert_eq!(stopping.launch.state, crate::durable::LaunchState::Stopping);

        store
            .finish_revoked_task_process(&task.id, revoked.generation)
            .await
            .unwrap();
        let ended = store
            .launch_surface(&launch.id)
            .await
            .unwrap()
            .expect("ended Launch remains historical evidence");
        assert_eq!(ended.launch.state, crate::durable::LaunchState::Ended);
        assert_eq!(ended.handback, Some(crate::durable::BoundaryState::Unknown));
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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
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
        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        project.begin_generation("parent-run".to_string());
        let child_lease = store
            .reserve_project_process(&project, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        project.latest_process.as_mut().unwrap().state = ChildLeaseState::Active;
        project.set_status(ProjectSessionStatus::Running, "active");
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

        task.begin_generation("child-run".to_string());
        let task_child_lease = store
            .reserve_task_process(&task, TaskSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        task.latest_process.as_mut().unwrap().state = ChildLeaseState::Active;
        task.set_status(TaskSessionStatus::Running, "active");
        store
            .activate_task_process(&task, &task_child_lease)
            .await
            .unwrap();
        let task_run_lease = store
            .resolve_run_lease(task_child_lease.run_token.clone())
            .await
            .unwrap();
        let launch = store
            .sqlite
            .control_launch_for_run(&task_run_lease.run_id)
            .unwrap()
            .expect("Task reservation registered its product Launch");
        assert_eq!(launch.state, crate::durable::LaunchState::Live);
        assert_eq!(
            launch.containment,
            Containment::Tmux {
                name: "child-run".to_string()
            }
        );
        let task_basis = store.current_epoch(&task_work).await.unwrap().current_basis;
        store
            .set_flow_position(
                &task_run_lease,
                FlowPosition {
                    work: task_work.clone(),
                    epoch_id: task_basis.epoch_id,
                    flow: "task".to_string(),
                    step: "review".to_string(),
                    step_index: 1,
                    iteration: 0,
                    interactive: true,
                    updated_at: OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();
        store
            .route_review(
                &task_run_lease,
                &launch.id,
                AttentionRoute::Parent(parent_lease.work.clone()),
            )
            .await
            .unwrap();
        assert!(store.review(&task_work).await.unwrap().is_some());

        let turn = store
            .advance_run(
                &task_run_lease,
                RunAdvance::TurnStarting {
                    launch_id: launch.id.clone(),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Turn(turn) = turn else {
            panic!("expected child Turn")
        };
        let mut turn_row = store
            .sqlite
            .agent_turn(turn.id.as_str())
            .unwrap()
            .expect("child Turn is stored");
        turn_row.root_output = Some("The retry still reuses the failed head.".to_string());
        store.sqlite.finish_agent_turn_capture(&turn_row).unwrap();
        let attention = store.child_attention(&parent_lease.work).await.unwrap();
        assert_eq!(attention.len(), 1);
        assert_eq!(
            attention[0].latest_output.as_deref(),
            Some("The retry still reuses the failed head.")
        );
        let control_seed = attention[0].render();
        assert!(control_seed.contains("The retry still reuses the failed head."));
        assert!(control_seed.contains("lf work steer task"));
        assert!(control_seed.contains("lf work close task"));

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
        let parked = store
            .review(&task_work)
            .await
            .unwrap()
            .expect("steering does not close Review attention");
        assert!(parked.attention_at.is_none());
        assert!(store
            .child_attention(&parent_lease.work)
            .await
            .unwrap()
            .is_empty());
        store
            .route_review(
                &task_run_lease,
                &launch.id,
                AttentionRoute::Parent(parent_lease.work.clone()),
            )
            .await
            .unwrap();
        assert!(
            store
                .review(&task_work)
                .await
                .unwrap()
                .expect("re-entering the same flow keeps the route")
                .attention_at
                .is_none(),
            "only a later terminal child Turn may re-arm attention"
        );

        let parent_basis = store
            .current_epoch(&parent_lease.work)
            .await
            .unwrap()
            .current_basis;
        turn_row.status = "completed".to_string();
        turn_row.ended_at = Some(OffsetDateTime::now_utc().unix_timestamp());
        store.sqlite.finish_agent_turn_capture(&turn_row).unwrap();
        let rearmed = store
            .review(&task_work)
            .await
            .unwrap()
            .expect("the child's next reply keeps the Review open");
        assert!(rearmed.attention_at.is_some());
        assert_eq!(
            store
                .child_attention(&parent_lease.work)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .current_epoch(&parent_lease.work)
                .await
                .unwrap()
                .current_basis
                .revision,
            parent_basis.revision + 1
        );

        store.sqlite.finish_agent_turn_capture(&turn_row).unwrap();
        assert_eq!(
            store
                .current_epoch(&parent_lease.work)
                .await
                .unwrap()
                .current_basis
                .revision,
            parent_basis.revision + 1,
            "one child Turn allocates one parent evidence revision"
        );

        let answered = store
            .steer(
                &ControlCtx::Run(&parent_lease),
                &task_work,
                "the failed head must be observed fresh",
                None,
            )
            .await
            .unwrap();
        assert!(store
            .child_attention(&parent_lease.work)
            .await
            .unwrap()
            .is_empty());
        let review = store
            .review(&task_work)
            .await
            .unwrap()
            .expect("answering a child parks but does not close its Review");
        assert!(review.attention_at.is_none());
        assert_eq!(review.basis, answered.steer.basis);
        store
            .close_review(&ControlCtx::Run(&parent_lease), &task_work, &review.basis)
            .await
            .unwrap();
        assert!(store.review(&task_work).await.unwrap().is_none());
        assert!(store
            .child_attention(&parent_lease.work)
            .await
            .unwrap()
            .is_empty());

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
    async fn live_send_never_advances_fixed_turn_basis_or_completion() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let basis_zero = store.current_epoch(&work).await.unwrap().current_basis;
        let launch = trace_launch("fixed-basis");
        let completed = trace_turn(
            "turn-completed",
            &launch.id,
            1,
            "completed",
            basis_zero.clone(),
        );
        store
            .sqlite
            .insert_trace_capture(&launch, &completed, &[], &[])
            .unwrap();
        let running = trace_turn("turn-running", &launch.id, 2, "running", basis_zero.clone());
        store
            .sqlite
            .insert_agent_turn_capture(&running, &[], &[])
            .unwrap();
        store
            .validate_completion_basis(&work, &basis_zero)
            .await
            .unwrap();

        let receipt = store
            .append_steer(&work, Author::User, "change course", Some(&basis_zero))
            .await
            .unwrap();
        let live = store
            .begin_live_send(&receipt.steer.id, &running.id)
            .await
            .unwrap()
            .expect("the active Turn can receive the Steer");
        store
            .finish_send(
                &live.id,
                SendState::Unknown,
                None,
                Some("provider receipt lost"),
            )
            .await
            .unwrap();

        let stored_turn = store
            .sqlite
            .agent_turn(&running.id)
            .unwrap()
            .expect("stored Turn");
        assert_eq!(stored_turn.basis, Some(basis_zero.clone()));
        assert!(store
            .validate_completion_basis(&work, &basis_zero)
            .await
            .is_err());
        assert_eq!(store.boundary_seed(&work).await.unwrap().steers.len(), 1);

        let applied = trace_turn(
            "turn-applied",
            &launch.id,
            3,
            "completed",
            receipt.steer.basis.clone(),
        );
        store
            .sqlite
            .insert_agent_turn_capture(&applied, &[], &[])
            .unwrap();
        store
            .validate_completion_basis(&work, &receipt.steer.basis)
            .await
            .unwrap();
        assert!(store.boundary_seed(&work).await.unwrap().steers.is_empty());
    }

    #[tokio::test]
    async fn ci_incident_preserves_recovery_milestones_after_current_pr_state_moves_on() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let task = make_task_session(&wave, &project);
        let pr = make_task_pr(&task);
        store.create_task_session(&task, &pr).await.unwrap();

        let observed_at = task.created_at - Duration::seconds(5);
        let incident = CiIncident {
            identity: "github:ci:owner/repo:42:bad-head:digest".to_string(),
            task_session_id: task.id.clone(),
            pr_id: pr.id.clone(),
            repo: "owner/repo".to_string(),
            pr_number: 42,
            failed_head_sha: "bad-head".to_string(),
            repaired_head_sha: None,
            failure_set: vec!["test".to_string()],
            provider_completed_at: None,
            poll_observed_at: Some(observed_at),
            webhook_received_at: None,
            claimed_run_id: None,
            responded_at: None,
            green_at: None,
            merged_at: None,
            blocked_at: None,
            blocked_reason: None,
            created_at: observed_at,
            updated_at: observed_at,
        };
        store.observe_ci_incident(&incident).await.unwrap();
        store.observe_ci_incident(&incident).await.unwrap();

        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        store
            .append_steer(&work, Author::User, "inspect the failed checks", None)
            .await
            .unwrap();

        let mut running = task.clone();
        running.set_status(TaskSessionStatus::Waiting, "CI incident is runnable");
        store.update_task_session(&running).await.unwrap();
        running.begin_generation("ci-incident-run".to_string());
        let lease = store
            .reserve_task_process(&running, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        running
            .latest_process
            .as_mut()
            .expect("reserved process")
            .state = ChildLeaseState::Active;
        running.set_status(TaskSessionStatus::Running, "repairing CI");
        store.activate_task_process(&running, &lease).await.unwrap();
        let run = store.current_run(&work).await.unwrap().unwrap();
        assert!(store
            .claim_ci_incident(
                &incident.identity,
                &run.id,
                observed_at + Duration::seconds(10),
            )
            .await
            .unwrap());
        store
            .mark_ci_incidents_blocked(
                &pr.id,
                observed_at + Duration::seconds(20),
                "waiting for credentials",
            )
            .await
            .unwrap();
        store
            .mark_ci_incidents_green(&pr.id, observed_at + Duration::seconds(30))
            .await
            .unwrap();
        store
            .mark_ci_incidents_merged(&pr.id, observed_at + Duration::seconds(40))
            .await
            .unwrap();

        let rows = store
            .ci_incidents_since(observed_at, None, Some("owner/repo"))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.incident.poll_observed_at, Some(observed_at));
        assert_eq!(row.incident.claimed_run_id.as_ref(), Some(&run.id));
        assert_eq!(
            row.incident.responded_at,
            Some(observed_at + Duration::seconds(10))
        );
        assert_eq!(
            row.incident.green_at,
            Some(observed_at + Duration::seconds(30))
        );
        assert_eq!(
            row.incident.merged_at,
            Some(observed_at + Duration::seconds(40))
        );
        assert_eq!(
            row.incident.blocked_at,
            Some(observed_at + Duration::seconds(20))
        );
        assert_eq!(
            row.incident.blocked_reason.as_deref(),
            Some("waiting for credentials")
        );
        assert!(row.human_assisted);
    }

    #[tokio::test]
    async fn task_lifecycle_plan_and_position_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task_session(&wave, &project);
        task.lifecycle = crate::task::TaskLifecyclePlan::standard("code");
        task.phase_cursor = 2;
        task.phase_iteration = 4;
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let persisted = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted.lifecycle.iterate.flow, "code");
        assert_eq!(
            persisted.lifecycle.iterate.interaction_policy,
            crate::engine::InteractionPolicy::Defer
        );
        assert_eq!(persisted.lifecycle.kickoff.flow, "task-kickoff");
        assert_eq!(persisted.lifecycle.gate.flow, "task-gate");
        assert_eq!(persisted.phase_cursor, 2);
        assert_eq!(persisted.phase_iteration, 4);

        task.phase_cursor = 3;
        task.phase_iteration = 5;
        store.update_task_session(&task).await.unwrap();
        let resumed = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!((resumed.phase_cursor, resumed.phase_iteration), (2, 4));
    }

    #[tokio::test]
    async fn task_session_provenance_round_trips_through_insert() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut task = make_task_session(&wave, &project);
        task.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-prov".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Reserved,
            outcome: None,
            provenance: Some(BinaryProvenance {
                version: "0.12.0".to_string(),
                provenance: "release".to_string(),
                source_identity: "release".to_string(),
            }),
        });
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let persisted = store.get_task_session(&task.id).await.unwrap().unwrap();
        let provenance = persisted
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived insert");
        assert_eq!(provenance.version, "0.12.0");
        assert_eq!(provenance.provenance, "release");
        assert_eq!(provenance.source_identity, "release");
    }

    #[tokio::test]
    async fn task_session_provenance_survives_reserve_and_activate() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        // Create without a generation, then begin one and record provenance the
        // way the launcher does. The reserve write must persist provenance.
        let mut task = make_task_session(&wave, &project);
        task.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("lf-task-prov".to_string());
        if let Some(process) = task.latest_process.as_mut() {
            process.provenance = Some(BinaryProvenance {
                version: "0.12.1".to_string(),
                provenance: "development".to_string(),
                source_identity: "loopflow-deadbeef".to_string(),
            });
        }
        let lease = store
            .reserve_task_process(&task, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        let reserved = store.get_task_session(&task.id).await.unwrap().unwrap();
        let provenance = reserved
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived reserve");
        assert_eq!(provenance.version, "0.12.1");
        assert_eq!(provenance.source_identity, "loopflow-deadbeef");

        // Activation re-writes the generation through the lease update; the
        // immutable provenance must survive unchanged.
        if let Some(process) = &mut task.latest_process {
            process.state = ChildLeaseState::Active;
        }
        task.set_status(TaskSessionStatus::Running, "active");
        store.activate_task_process(&task, &lease).await.unwrap();
        let active = store.get_task_session(&task.id).await.unwrap().unwrap();
        let provenance = active
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived activate");
        assert_eq!(provenance.version, "0.12.1");
        assert_eq!(provenance.provenance, "development");
        assert_eq!(provenance.source_identity, "loopflow-deadbeef");
    }

    /// The cross-version invariant: provenance describes the binary that boots a
    /// generation (B), never the launcher (A). Launcher A reserves a generation
    /// carrying A's provenance; the booting binary then stamps its own identity
    /// via `mark_booted` at activation. The persisted audit row must record B.
    #[tokio::test]
    async fn generation_provenance_is_the_booting_binary_not_the_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        // Launcher A reserves a generation stamped with A's (wrong) provenance.
        let launcher_a = BinaryProvenance {
            version: "A-0.0.0".to_string(),
            provenance: "release".to_string(),
            source_identity: "launcher-A".to_string(),
        };
        let mut task = make_task_session(&wave, &project);
        task.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("lf-task-ab".to_string());
        if let Some(process) = task.latest_process.as_mut() {
            process.provenance = Some(launcher_a.clone());
        }
        let lease = store
            .reserve_task_process(&task, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();

        // Binary B boots the generation and stamps its own identity, exactly as
        // the child runner does. This overwrites A's provenance with B's.
        if let Some(process) = task.latest_process.as_mut() {
            process.mark_booted();
        }
        task.set_status(TaskSessionStatus::Running, "active");
        store.activate_task_process(&task, &lease).await.unwrap();

        let active = store.get_task_session(&task.id).await.unwrap().unwrap();
        let recorded = active
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("the booted generation records provenance");
        let booting_b = BinaryProvenance::current();
        assert_eq!(
            recorded, &booting_b,
            "provenance must describe what ran (B)"
        );
        assert_ne!(
            recorded, &launcher_a,
            "provenance must not be the launcher's (A)"
        );
    }
    #[tokio::test]
    async fn project_session_provenance_round_trips_through_insert() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut project = make_project_session(&wave);
        project.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-project-prov".to_string(),
            agent: "claude".to_string(),
            provider: "claude".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Reserved,
            outcome: None,
            provenance: Some(BinaryProvenance {
                version: "0.12.0".to_string(),
                provenance: "release".to_string(),
                source_identity: "release".to_string(),
            }),
        });
        store.create_project_session(&project).await.unwrap();
        let persisted = store
            .get_project_session(&project.id)
            .await
            .unwrap()
            .unwrap();
        let provenance = persisted
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived insert");
        assert_eq!(provenance.version, "0.12.0");
        assert_eq!(provenance.provenance, "release");
    }

    #[tokio::test]
    async fn project_session_provenance_survives_reserve_and_activate() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.status_reason = "ready".to_string();
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        project.begin_generation("lf-project-prov".to_string());
        if let Some(process) = project.latest_process.as_mut() {
            process.provenance = Some(BinaryProvenance {
                version: "0.12.1".to_string(),
                provenance: "development".to_string(),
                source_identity: "loopflow-cafe".to_string(),
            });
        }
        let lease = store
            .reserve_project_process(&project, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        let reserved = store
            .get_project_session(&project.id)
            .await
            .unwrap()
            .unwrap();
        let provenance = reserved
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived reserve");
        assert_eq!(provenance.version, "0.12.1");
        assert_eq!(provenance.source_identity, "loopflow-cafe");

        if let Some(process) = &mut project.latest_process {
            process.state = ChildLeaseState::Active;
        }
        project.set_status(ProjectSessionStatus::Running, "active");
        store
            .activate_project_process(&project, &lease)
            .await
            .unwrap();
        let active = store
            .get_project_session(&project.id)
            .await
            .unwrap()
            .unwrap();
        let provenance = active
            .latest_process
            .as_ref()
            .and_then(|process| process.provenance.as_ref())
            .expect("provenance survived activate");
        assert_eq!(provenance.version, "0.12.1");
        assert_eq!(provenance.source_identity, "loopflow-cafe");
    }

    #[tokio::test]
    async fn session_generation_without_provenance_round_trips_as_none() {
        // A generation recorded before this field existed deserializes with
        // provenance `None`, so old sessions remain readable without a backfill.
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task_session(&wave, &project);
        task.latest_process = Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "lf-task-legacy".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            state: ChildLeaseState::Reserved,
            outcome: None,
            provenance: None,
        });
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let persisted = store.get_task_session(&task.id).await.unwrap().unwrap();
        let process = persisted
            .latest_process
            .as_ref()
            .expect("generation survived");
        assert_eq!(process.generation, 1);
        assert_eq!(process.tmux_name, "lf-task-legacy");
        assert!(process.provenance.is_none());
    }

    #[tokio::test]
    async fn task_phase_epoch_allows_resets_and_rejects_stale_positions() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut task = make_task_session(&wave, &project);
        task.lifecycle_phase = crate::task::TaskLifecyclePhase::Kickoff;
        task.phase_cursor = 1;
        task.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("task-lifecycle".to_string());
        let lease = store
            .reserve_task_process(&task, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut task.latest_process {
            process.state = ChildLeaseState::Active;
        }
        task.set_status(TaskSessionStatus::Running, "active");
        store.activate_task_process(&task, &lease).await.unwrap();
        let mut stale = task.clone();

        task.enter_iterate().unwrap();
        store
            .update_task_session_for_lease(&task, &lease)
            .await
            .unwrap();
        let iterating = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(
            (
                iterating.lifecycle_phase,
                iterating.phase_epoch,
                iterating.phase_cursor
            ),
            (crate::task::TaskLifecyclePhase::Iterate, 2, 0)
        );

        stale.phase_cursor = 9;
        stale.phase_iteration = 9;
        store
            .update_task_session_for_lease(&stale, &lease)
            .await
            .unwrap();
        let after_stale = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(
            (
                after_stale.lifecycle_phase,
                after_stale.phase_epoch,
                after_stale.phase_cursor,
                after_stale.phase_iteration
            ),
            (crate::task::TaskLifecyclePhase::Iterate, 2, 0, 0)
        );

        let mut completed = task.clone();
        completed.set_status(TaskSessionStatus::Completed, "implementation complete");
        store.update_task_session(&completed).await.unwrap();
        task.status = TaskSessionStatus::Completed;
        task.status_reason = completed.status_reason;
        task.enter_gate(crate::task::TaskGateProposal {
            status: TaskSessionStatus::Completed,
            reason: "implementation complete".to_string(),
        })
        .unwrap();
        task.set_status(TaskSessionStatus::Running, "gate active");
        store
            .update_task_session_for_lease(&task, &lease)
            .await
            .unwrap();
        let gating = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(
            gating.lifecycle_phase,
            crate::task::TaskLifecyclePhase::Gate
        );
        assert_eq!(gating.phase_epoch, 3);
        assert_eq!(gating.gate_cycle, 1);
        assert_eq!(
            gating.gate_proposal.unwrap().reason,
            "implementation complete"
        );
    }
    #[tokio::test]
    async fn terminal_project_history_keeps_one_current_successor() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut predecessor = make_project_session(&wave);
        predecessor.set_status(ProjectSessionStatus::Abandoned, "legacy Session ended");
        predecessor.latest_process = None;
        predecessor.provider_session_id = None;
        store.create_project_session(&predecessor).await.unwrap();

        let mut successor = make_project_session(&wave);
        successor.status = ProjectSessionStatus::Created;
        successor.status_reason = format!("successor to {}", predecessor.id);
        successor.latest_process = None;
        successor.provider_session_id = None;
        successor.created_at += time::Duration::SECOND;
        successor.updated_at = successor.created_at;
        store.create_project_session(&successor).await.unwrap();

        assert_eq!(
            store
                .get_project_session_by_project("developer-efficiency")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );
        assert_eq!(
            store
                .get_project_session_by_project(predecessor.id.as_str())
                .await
                .unwrap()
                .unwrap()
                .id,
            predecessor.id
        );
        assert_eq!(
            store
                .list_project_sessions(Some(wave.id()))
                .await
                .unwrap()
                .len(),
            2
        );

        let mut parallel = successor.clone();
        parallel.id = ProjectSessionId::new();
        assert!(store.create_project_session(&parallel).await.is_err());
    }

    // W2-243: route existing Task Sessions to the successor Project Session.
    // The historical project_session_id is provenance; the live successor is the
    // routing target. These three tests prove the five Done-when criteria.

    #[tokio::test]
    async fn resolve_task_project_route_targets_live_successor_and_fails_dead_chains() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        // A live Project Session routes to itself — no successor needed.
        let mut predecessor = make_project_session(&wave);
        predecessor.status = ProjectSessionStatus::Created;
        predecessor.status_reason = "available for routing".to_string();
        predecessor.latest_process = None;
        predecessor.provider_session_id = None;
        store.create_project_session(&predecessor).await.unwrap();
        let task = make_task_session(&wave, &predecessor);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        let route = crate::ops::project::resolve_task_project_route(&store, &task)
            .await
            .unwrap();
        assert!(!route.succeeded);
        assert_eq!(route.historical, predecessor.id);
        assert_eq!(route.current, predecessor.id);

        // Abandon the predecessor and create a successor for the same Linear
        // project. The Task still records the predecessor as provenance; routing
        // follows the chain to the successor.
        let mut abandoned = predecessor.clone();
        abandoned.set_status(ProjectSessionStatus::Abandoned, "replaced append-only");
        store.update_project_session(&abandoned).await.unwrap();
        let mut successor = make_project_session(&wave);
        successor.status = ProjectSessionStatus::Created;
        successor.status_reason = format!("successor to {}", predecessor.id);
        successor.latest_process = None;
        successor.provider_session_id = None;
        successor.created_at += time::Duration::SECOND;
        successor.updated_at = successor.created_at;
        store.create_project_session(&successor).await.unwrap();
        let route = crate::ops::project::resolve_task_project_route(&store, &task)
            .await
            .unwrap();
        assert!(route.succeeded);
        assert_eq!(route.historical, predecessor.id);
        assert_eq!(route.current, successor.id);
        assert!(!route.current_status.is_terminal());

        // Broken chain: the successor is terminal too, and no further successor
        // exists. Routing fails actionably, naming the dead session and project.
        let mut dead_successor = successor.clone();
        dead_successor.set_status(ProjectSessionStatus::Abandoned, "no successor");
        store.update_project_session(&dead_successor).await.unwrap();
        let error = crate::ops::project::resolve_task_project_route(&store, &task)
            .await
            .expect_err("dead chain must fail actionably");
        let message = error.to_string();
        assert!(message.contains("no live successor"), "{message}");
        assert!(message.contains(&predecessor.id.to_string()), "{message}");
    }

    #[tokio::test]
    async fn successor_consumes_observations_addressed_to_predecessor() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut predecessor = make_project_session(&wave);
        predecessor.status = ProjectSessionStatus::Created;
        predecessor.status_reason = "available for observations".to_string();
        predecessor.latest_process = None;
        predecessor.provider_session_id = None;
        store.create_project_session(&predecessor).await.unwrap();
        let task = make_task_session(&wave, &predecessor);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        // Enqueue a project-observable event while the predecessor is live. The
        // observation is addressed to the historical predecessor (provenance).
        // A lifecycle boundary is project-observable, so it enqueues exactly
        // one Project observation without changing the historical addressee.
        store
            .append_task_event(
                &task.id,
                &TaskEventKind::StatusChanged {
                    from: TaskSessionStatus::Waiting,
                    to: TaskSessionStatus::Running,
                    reason: "child needs attention".to_string(),
                },
            )
            .await
            .unwrap();
        let predecessor_queue = store
            .pending_observations(&ObservationRecipient::Project {
                session_id: predecessor.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(predecessor_queue.len(), 1);
        assert_eq!(
            predecessor_queue[0].recipient,
            ObservationRecipient::Project {
                session_id: predecessor.id.clone()
            }
        );

        // Abandon the predecessor and create a successor for the same project.
        let mut abandoned = predecessor.clone();
        abandoned.set_status(ProjectSessionStatus::Abandoned, "replaced append-only");
        store.update_project_session(&abandoned).await.unwrap();
        let mut successor = make_project_session(&wave);
        successor.status = ProjectSessionStatus::Created;
        successor.status_reason = "successor".to_string();
        successor.latest_process = None;
        successor.created_at += time::Duration::SECOND;
        successor.updated_at = successor.created_at;
        store.create_project_session(&successor).await.unwrap();

        // The chain query routes the predecessor-addressed observation to the
        // successor without rewriting the outbox recipient.
        let chain = store
            .pending_project_observations_for_chain(predecessor.launch.project.id.as_str())
            .await
            .unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(
            chain[0].recipient,
            ObservationRecipient::Project {
                session_id: predecessor.id.clone()
            }
        );

        // The successor consumes the observation under its own write lease.
        successor.begin_generation("successor-consume".to_string());
        let successor_lease = store
            .reserve_project_process(&successor, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut successor.latest_process {
            process.state = ChildLeaseState::Active;
        }
        successor.set_status(ProjectSessionStatus::Running, "consuming chain");
        store
            .activate_project_process(&successor, &successor_lease)
            .await
            .unwrap();
        let inserted = store
            .consume_task_observation_for_project_for_lease(
                &successor.id,
                &chain[0],
                &successor_lease,
            )
            .await
            .unwrap();
        assert!(
            inserted,
            "successor must consume the predecessor's observation"
        );

        // The observation is delivered; the predecessor's own-id queue drains.
        assert!(store
            .pending_observations(&ObservationRecipient::Project {
                session_id: predecessor.id.clone()
            })
            .await
            .unwrap()
            .is_empty());
        // The successor recorded TaskObserved under its own id — only the
        // successor wakes to the Task's event.
        let events = store.project_events_after(&successor.id, 0).await.unwrap();
        assert!(events.iter().any(|event| matches!(
            event.kind,
            ProjectEventKind::TaskObserved { ref task_session_id, .. }
            if task_session_id == &task.id
        )));
    }
    #[tokio::test]
    async fn task_session_requires_its_matching_project_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        let task = make_task_session(&wave, &project);

        let missing = store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("requires Project Session"));

        store.create_project_session(&project).await.unwrap();
        let mut wrong_project = make_task_session(&wave, &project);
        wrong_project.launch.project.id = LinearProjectId::new("another-project").unwrap();
        let mismatched = store
            .create_task_session(&wrong_project, &make_task_pr(&wrong_project))
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not own Task"));

        let other_wave = make_wave("/other-repo");
        store.create_wave(&other_wave).await.unwrap();
        let wrong_wave = make_task_session(&other_wave, &project);
        let mismatched = store
            .create_task_session(&wrong_wave, &make_task_pr(&wrong_wave))
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not own Task"));
    }

    #[tokio::test]
    async fn task_issue_identifier_rebinds_only_without_a_writing_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut waiting = make_task_session(&wave, &project);
        waiting.set_status(TaskSessionStatus::Waiting, "PR is open");
        store
            .create_task_session(&waiting, &make_task_pr(&waiting))
            .await
            .unwrap();
        assert!(store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());
        assert!(store
            .get_task_session_by_issue("INF-123")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .get_task_session_by_issue("PRD-8")
                .await
                .unwrap()
                .unwrap()
                .launch
                .issue
                .identifier,
            "PRD-8"
        );
        assert!(!store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());

        let mut running = make_task_session(&wave, &project);
        running.launch.issue.id = LinearIssueId::new("issue-running").unwrap();
        running.launch.issue.identifier = "W2-9".to_string();
        running.worktree = PathBuf::from("/repo.running");
        running.begin_generation("task-body".to_string());
        store
            .create_task_session(&running, &make_task_pr(&running))
            .await
            .unwrap();
        assert!(store
            .rebind_task_issue_identifier("issue-running", "W2-9", "PRD-9")
            .await
            .unwrap_err()
            .to_string()
            .contains("active Run"));
    }

    #[tokio::test]
    async fn terminal_task_history_keeps_one_current_successor() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        // A completed predecessor and a live successor share issue id,
        // identifier, and worktree. The schema permits it (terminal history),
        // and resolution selects the live successor.
        let mut predecessor = make_task_session(&wave, &project);
        predecessor.set_status(TaskSessionStatus::Completed, "PR merged");
        predecessor.created_at = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        predecessor.updated_at = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        store
            .create_task_session(&predecessor, &make_task_pr(&predecessor))
            .await
            .unwrap();

        let mut successor = make_task_session(&wave, &project);
        successor.status_reason = "successor to the completed attempt".to_string();
        successor.created_at = OffsetDateTime::from_unix_timestamp(2_000).unwrap();
        successor.updated_at = OffsetDateTime::from_unix_timestamp(2_000).unwrap();
        store
            .create_task_session(&successor, &make_task_pr(&successor))
            .await
            .unwrap();

        // Terminal predecessor never wins an operational lookup while a live
        // successor exists, by every key.
        assert_eq!(
            store
                .get_task_session_by_issue("INF-123")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );
        assert_eq!(
            store
                .get_task_session_by_issue("issue-uuid")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );
        assert_eq!(
            store
                .get_task_session_by_worktree("/repo.inf-123")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );

        // A second live successor for the same key is rejected: one current.
        let mut parallel = make_task_session(&wave, &project);
        parallel.created_at = OffsetDateTime::from_unix_timestamp(3_000).unwrap();
        parallel.updated_at = OffsetDateTime::from_unix_timestamp(3_000).unwrap();
        assert!(store
            .create_task_session(&parallel, &make_task_pr(&parallel))
            .await
            .is_err());

        // When the live successor completes, resolution falls back to the most
        // recent terminal predecessor so status reads still resolve a Task.
        let mut terminal = successor.clone();
        terminal.set_status(TaskSessionStatus::Completed, "second PR merged");
        terminal.updated_at = OffsetDateTime::from_unix_timestamp(4_000).unwrap();
        store.update_task_session(&terminal).await.unwrap();
        let resolved = store
            .get_task_session_by_issue("INF-123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resolved.id, terminal.id);
        assert_eq!(resolved.status, TaskSessionStatus::Completed);
    }

    #[tokio::test]
    async fn abandoned_task_recovery_atomically_adopts_its_pr_and_direction() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut predecessor = make_task_session(&wave, &project);
        let pr = make_task_pr(&predecessor);
        let initial = "preserve the existing work and PR";
        store
            .create_task_session_with_steer(
                &predecessor,
                &pr,
                crate::durable::Author::User,
                initial,
            )
            .await
            .unwrap();
        predecessor.set_status(TaskSessionStatus::Abandoned, "stopped explicitly");
        store.update_task_session(&predecessor).await.unwrap();

        let mut successor = predecessor.clone();
        successor.id = TaskSessionId::new();
        successor.status = TaskSessionStatus::Waiting;
        successor.status_reason = "recovered; resume to continue".to_string();
        successor.status_at = OffsetDateTime::now_utc();
        successor.latest_process = None;
        successor.created_at = OffsetDateTime::now_utc();
        successor.updated_at = successor.created_at;
        let created = store
            .recover_task_session_successor(
                &predecessor,
                &successor,
                crate::durable::Author::User,
                initial,
            )
            .await
            .unwrap();
        assert!(created.created);
        assert_eq!(created.session.id, successor.id);
        assert_eq!(
            store
                .get_task_session_by_issue("INF-123")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );
        assert!(store.task_prs(&predecessor.id).await.unwrap().is_empty());
        let adopted = store.task_prs(&successor.id).await.unwrap();
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].id, pr.id);
        let successor_work = store
            .work_for_child(&ChildRef::Task(successor.id.clone()))
            .await
            .unwrap();
        assert!(store
            .boundary_seed(&successor_work)
            .await
            .unwrap()
            .render()
            .contains(initial));
        assert_eq!(
            store
                .task_session_chain_neighbors(&successor.id)
                .await
                .unwrap(),
            (Some(predecessor.id.to_string()), None)
        );

        let repeated = store
            .recover_task_session_successor(
                &predecessor,
                &successor,
                crate::durable::Author::User,
                initial,
            )
            .await
            .unwrap();
        assert!(!repeated.created);
        assert_eq!(repeated.session.id, successor.id);
    }

    #[tokio::test]
    async fn task_session_resolution_fails_actionably_on_multiple_live_successors() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        // Two live sessions collide only across the issue_id OR identifier
        // columns: the partial unique index blocks two lives on the same column,
        // but a lookup keyed on `issue_id = ? OR issue_identifier = ?` can match
        // one row by id and a different row by identifier. That cross-match is
        // actionable ambiguity, never a silent pick.
        let mut a = make_task_session(&wave, &project);
        a.launch.issue.id = LinearIssueId::new("issue-a").unwrap();
        a.launch.issue.identifier = "INF-200".to_string();
        a.worktree = PathBuf::from("/repo.a");
        store
            .create_task_session(&a, &make_task_pr(&a))
            .await
            .unwrap();

        let mut b = make_task_session(&wave, &project);
        b.launch.issue.id = LinearIssueId::new("INF-200").unwrap();
        b.launch.issue.identifier = "INF-201".to_string();
        b.worktree = PathBuf::from("/repo.b");
        store
            .create_task_session(&b, &make_task_pr(&b))
            .await
            .unwrap();

        // `INF-200` matches `a` by identifier and `b` by issue id: two live.
        let error = store
            .get_task_session_by_issue("INF-200")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("2 live Task Sessions"), "{error}");
        assert!(error.contains("INF-200"), "{error}");
        assert!(error.contains("INF-201"), "{error}");
    }

    #[tokio::test]
    async fn rebind_task_issue_identifier_targets_the_current_successor() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut predecessor = make_task_session(&wave, &project);
        predecessor.set_status(TaskSessionStatus::Completed, "PR merged");
        predecessor.created_at = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        predecessor.updated_at = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        store
            .create_task_session(&predecessor, &make_task_pr(&predecessor))
            .await
            .unwrap();

        let mut successor = make_task_session(&wave, &project);
        successor.set_status(TaskSessionStatus::Waiting, "awaiting review");
        successor.created_at = OffsetDateTime::from_unix_timestamp(2_000).unwrap();
        successor.updated_at = OffsetDateTime::from_unix_timestamp(2_000).unwrap();
        store
            .create_task_session(&successor, &make_task_pr(&successor))
            .await
            .unwrap();

        // Rebind updates the live successor only; the terminal predecessor
        // keeps its historical identifier as history.
        assert!(store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());

        let rebound = store
            .get_task_session_by_issue("PRD-8")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rebound.id, successor.id);
        assert_eq!(rebound.launch.issue.identifier, "PRD-8");

        // The completed predecessor is still reachable by its own id and keeps
        // the original identifier — rebind did not rewrite history.
        let predecessor_row = store
            .get_task_session(&predecessor.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(predecessor_row.launch.issue.identifier, "INF-123");
        assert_eq!(predecessor_row.status, TaskSessionStatus::Completed);

        // Idempotent: rebinding the current successor to its own identifier is a
        // no-op, not an error.
        assert!(!store
            .rebind_task_issue_identifier("issue-uuid", "INF-123", "PRD-8")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn task_pr_persists_github_and_ci_observations() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 902,
                url: "https://github.com/loopflow/loopflow/pull/902".to_string(),
                head_sha: Some("sha-abc".to_string()),
            }),
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

        let read = store.active_task_pr(&session.id).await.unwrap().unwrap();
        assert_eq!(read.head_sha(), Some("sha-abc"));
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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        pr.linear_attachment_id = Some("att-1".to_string());
        pr.linear_comment_id = Some("comment-1".to_string());
        pr.linear_link_error = Some("linear is down".to_string());
        pr.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&pr).await.unwrap();

        let read = store.active_task_pr(&session.id).await.unwrap().unwrap();
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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut first = make_task_pr(&session);
        store.create_task_session(&session, &first).await.unwrap();

        first.publication = Some(PrPublication {
            requested_at: first.updated_at,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 101,
                url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
                head_sha: None,
            }),
        });
        first.merge_commit = Some("merge-101".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        let now = OffsetDateTime::now_utc();
        let second = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 2,
            slug: "released-proof".to_string(),
            branch: format!("jack/{}-released-proof", session.workspace_slug),
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
                .task_prs(&session.id)
                .await
                .unwrap()
                .iter()
                .map(|pr| pr.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            store.active_task_pr(&session.id).await.unwrap().unwrap().id,
            second.id
        );

        let mut abandoned = second.clone();
        let abandoned_at = OffsetDateTime::now_utc();
        abandoned.abandoned_at = Some(abandoned_at);
        abandoned.updated_at = abandoned_at;
        let conflicting = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
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
                .active_task_pr(&session.id)
                .await
                .unwrap()
                .unwrap()
                .phase(),
            PrPhase::Working
        );
    }

    /// The settle equality includes `abandoned_at`, so a re-settle must carry
    /// the original abandonment time. GitHub-observation reconcile once
    /// re-stamped it with `now` on every `lf task status`, and the session
    /// wedged permanently on "already settled differently" (live: W2-283).
    #[tokio::test]
    async fn re_settling_an_abandoned_pr_is_idempotent_only_at_its_original_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let parent_session = make_task_session(&wave, &project);
        let mut parent = make_task_pr(&parent_session);
        store
            .create_task_session(&parent_session, &parent)
            .await
            .unwrap();

        // The parent is published but not merged — the child stacks on it.
        parent.publication = Some(PrPublication {
            requested_at: parent.updated_at,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 200,
                url: "https://github.com/loopflowstudio/loopflow/pull/200".to_string(),
                head_sha: Some("parent-tip".to_string()),
            }),
        });
        store.update_task_pr(&parent).await.unwrap();

        let mut child_session = make_task_session(&wave, &project);
        child_session.launch.issue.id = LinearIssueId::new("issue-child").unwrap();
        child_session.launch.issue.identifier = "INF-124".to_string();
        child_session.worktree = PathBuf::from("/repo.child-task");
        let now = OffsetDateTime::now_utc();
        let child = TaskPr {
            id: TaskPrId::new(),
            task_session_id: child_session.id.clone(),
            sequence: 1,
            slug: child_session.workspace_slug.clone(),
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
        store
            .create_task_session(&child_session, &child)
            .await
            .unwrap();

        let active = store
            .active_task_pr(&child_session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.id, child.id);
        assert_eq!(active.parent_pr_id, Some(parent.id.clone()));
        assert_eq!(
            store.get_task_pr(&parent.id).await.unwrap(),
            Some(parent.clone())
        );

        // A parent update moves the child's durable fork without changing its
        // ownership or parent link.
        store
            .rebase_task_pr(&child.id, "parent-tip-2", false, OffsetDateTime::now_utc())
            .await
            .unwrap();
        let rebased = store.get_task_pr(&child.id).await.unwrap().unwrap();
        assert_eq!(rebased.base_commit, "parent-tip-2");
        assert_eq!(rebased.parent_pr_id, Some(parent.id.clone()));

        // The parent merges; the child collapses onto main, dropping the link.
        parent.merge_commit = Some("merge-200".to_string());
        parent.updated_at = OffsetDateTime::now_utc();
        store.update_task_pr(&parent).await.unwrap();
        store
            .rebase_task_pr(&child.id, "main-after-200", true, OffsetDateTime::now_utc())
            .await
            .unwrap();

        let collapsed = store
            .active_task_pr(&child_session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(collapsed.id, child.id);
        assert_eq!(collapsed.parent_pr_id, None);
        assert_eq!(collapsed.base_commit, "main-after-200");

        // The worktree lookup the rebase path relies on resolves the session.
        let by_worktree = store
            .get_task_session_by_worktree(&child_session.worktree.display().to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_worktree.id, child_session.id);
    }

    #[tokio::test]
    async fn pr_publication_round_trips_before_github_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
            github: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let publishing = store.active_task_pr(&session.id).await.unwrap().unwrap();
        assert_eq!(publishing.phase(), PrPhase::Publishing);
        assert_eq!(publishing.publication, pr.publication);

        pr.publication.as_mut().unwrap().github = Some(GithubPr {
            number: 101,
            url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
            head_sha: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let open = store.active_task_pr(&session.id).await.unwrap().unwrap();
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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut session = make_task_session(&wave, &project);
        let pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        session.set_status(TaskSessionStatus::Completed, "investigation recorded");
        store
            .complete_task_session(&session, Some(&pr))
            .await
            .unwrap();

        assert_eq!(
            store
                .get_task_session(&session.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskSessionStatus::Completed
        );
        let stored = store.task_prs(&session.id).await.unwrap();
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
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut session = make_task_session(&wave, &project);
        session.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();

        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let launch = |store: Arc<super::Store>, barrier: Arc<tokio::sync::Barrier>| {
            let mut candidate = session.clone();
            candidate.begin_generation("task-race".to_string());
            tokio::spawn(async move {
                barrier.wait().await;
                store
                    .reserve_task_process(&candidate, TaskSessionStatus::Waiting)
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
        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(persisted.status, TaskSessionStatus::Starting);
        let process = persisted.latest_process.unwrap();
        assert_eq!(process.generation, 1);
        assert_eq!(process.state, ChildLeaseState::Reserved);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn task_and_project_reservations_wait_for_promotion() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.status_reason = "ready".to_string();
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        project.begin_generation("project-reservation".to_string());

        let mut task = make_task_session(&wave, &project);
        task.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("task-reservation".to_string());

        let promotion = crate::promotion_lock::acquire_exclusive().unwrap();
        let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();

        let task_store = Arc::clone(&store);
        let task_started = started_tx.clone();
        let mut task_reservation = tokio::spawn(async move {
            task_started.send(()).unwrap();
            task_store
                .reserve_task_process(&task, TaskSessionStatus::Waiting)
                .await
        });

        let project_store = Arc::clone(&store);
        let mut project_reservation = tokio::spawn(async move {
            started_tx.send(()).unwrap();
            project_store
                .reserve_project_process(&project, ProjectSessionStatus::Created)
                .await
        });

        started_rx.recv().await.unwrap();
        started_rx.recv().await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut task_reservation)
                .await
                .is_err(),
            "Task reservation crossed the exclusive promotion fence"
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                &mut project_reservation
            )
            .await
            .is_err(),
            "Project reservation crossed the exclusive promotion fence"
        );

        drop(promotion);
        let task_lease = tokio::time::timeout(std::time::Duration::from_secs(2), task_reservation)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let project_lease =
            tokio::time::timeout(std::time::Duration::from_secs(2), project_reservation)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        assert!(task_lease.is_some());
        assert!(project_lease.is_some());
    }

    #[tokio::test]
    async fn task_process_resume_reserves_each_session_generation_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut session = make_task_session(&wave, &project);
        session.set_status(TaskSessionStatus::Waiting, "waiting for review");
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();

        let mut first_resume = session.clone();
        assert_eq!(first_resume.begin_generation("task-one".to_string()), 1);
        let first_lease = store
            .reserve_task_process(&first_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("first generation reserves a write lease");

        let mut racing_resume = session.clone();
        racing_resume.begin_generation("task-one".to_string());
        assert!(store
            .reserve_task_process(&racing_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .is_none());

        if let Some(process) = &mut first_resume.latest_process {
            process.state = ChildLeaseState::Active;
        }
        first_resume.set_status(TaskSessionStatus::Running, "active");
        store
            .activate_task_process(&first_resume, &first_lease)
            .await
            .unwrap();
        if let Some(process) = &mut first_resume.latest_process {
            process.state = ChildLeaseState::Finished;
            process.outcome = Some(ChildBodyOutcome::Interrupted {
                reason: "test boundary".to_string(),
            });
        }
        first_resume.set_status(TaskSessionStatus::Waiting, "ready again");
        store
            .finish_task_process(&first_resume, &first_lease)
            .await
            .unwrap();

        let mut stale_resume = session.clone();
        stale_resume.begin_generation("stale-generation-one".to_string());
        assert!(store
            .reserve_task_process(&stale_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .is_none());
        let mut current_resume = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(current_resume.begin_generation("task-next".to_string()), 2);
        store
            .reserve_task_process(&current_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("only the current receipt may advance the generation");

        let mut second = make_task_session(&wave, &project);
        second.launch.issue.id = LinearIssueId::new("issue-two").unwrap();
        second.launch.issue.identifier = "INF-124".to_string();
        second.worktree = PathBuf::from("/repo.inf-124");
        second.set_status(TaskSessionStatus::Waiting, "waiting for work");
        store
            .create_task_session(&second, &make_task_pr(&second))
            .await
            .unwrap();
        second.begin_generation("task-two".to_string());
        let second_lease = store
            .reserve_task_process(&second, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .expect("other Session reserves its own write lease");
        assert_ne!(first_lease.token, second_lease.token);

        let loaded = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, TaskSessionStatus::Starting);
        assert_eq!(loaded.latest_process.unwrap().generation, 2);
    }

    #[tokio::test]
    async fn revoked_task_lease_rejects_stale_writes_and_bars_its_successor_until_reaped() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut session = make_task_session(&wave, &project);
        session.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();
        session.begin_generation("task-lease".to_string());
        let lease = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut session.latest_process {
            process.state = ChildLeaseState::Active;
        }
        session.set_status(TaskSessionStatus::Running, "active");
        store.activate_task_process(&session, &lease).await.unwrap();

        let revoked = store
            .revoke_task_process(
                &session.id,
                &ChildBodyOutcome::Superseded {
                    reason: "test replacement".to_string(),
                },
            )
            .await
            .unwrap();
        session.status_reason = "stale writer".to_string();
        assert!(matches!(
            store.update_task_session_for_lease(&session, &lease).await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
        assert!(matches!(
            store
                .append_task_event_for_lease(
                    &session.id,
                    &lease,
                    &TaskEventKind::Progress {
                        summary: "stale progress".to_string(),
                    },
                )
                .await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
        let mut pr = store.active_task_pr(&session.id).await.unwrap().unwrap();
        pr.updated_at = time::OffsetDateTime::now_utc();
        assert!(matches!(
            store.update_task_pr_for_lease(&pr, &lease).await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
        if let Some(process) = &mut session.latest_process {
            process.state = ChildLeaseState::Finished;
            process.outcome = Some(ChildBodyOutcome::Completed);
        }
        assert!(matches!(
            store.finish_task_process(&session, &lease).await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
        let mut completed = session.clone();
        completed.set_status(TaskSessionStatus::Completed, "stale completion");
        assert!(matches!(
            store
                .complete_task_session_for_lease(&completed, None, &lease)
                .await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));

        let mut waiting = store.get_task_session(&session.id).await.unwrap().unwrap();
        waiting.set_status(TaskSessionStatus::Waiting, "replacement requested");
        store.update_task_session(&waiting).await.unwrap();
        let mut successor = waiting.clone();
        assert_eq!(successor.begin_generation("task-successor".to_string()), 2);
        assert!(store
            .reserve_task_process(&successor, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .is_none());

        store
            .finish_revoked_task_process(&session.id, revoked.generation)
            .await
            .unwrap();
        let mut successor = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(successor.begin_generation("task-successor".to_string()), 2);
        assert!(store
            .reserve_task_process(&successor, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn project_and_task_sessions_share_stale_write_fencing() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.status_reason = "reserved".to_string();
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        project.begin_generation("project-lease".to_string());
        let lease = store
            .reserve_project_process(&project, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut project.latest_process {
            process.state = ChildLeaseState::Active;
        }
        project.set_status(ProjectSessionStatus::Running, "active");
        store
            .activate_project_process(&project, &lease)
            .await
            .unwrap();
        store
            .revoke_project_process(
                &project.id,
                &ChildBodyOutcome::Superseded {
                    reason: "test replacement".to_string(),
                },
            )
            .await
            .unwrap();

        project.status_reason = "stale Project writer".to_string();
        assert!(matches!(
            store
                .update_project_session_for_lease(&project, &lease)
                .await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
    }

    #[tokio::test]
    async fn terminal_intent_cannot_be_reverted_by_the_current_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut project = make_project_session(&wave);
        project.status = ProjectSessionStatus::Created;
        project.status_reason = "ready".to_string();
        project.latest_process = None;
        store.create_project_session(&project).await.unwrap();
        project.begin_generation("project-body".to_string());
        let project_lease = store
            .reserve_project_process(&project, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut project.latest_process {
            process.state = ChildLeaseState::Active;
        }
        project.set_status(ProjectSessionStatus::Running, "active");
        store
            .activate_project_process(&project, &project_lease)
            .await
            .unwrap();
        let mut terminal_project = project.clone();
        terminal_project.set_status(ProjectSessionStatus::Abandoned, "operator stopped intent");
        store
            .update_project_session(&terminal_project)
            .await
            .unwrap();
        assert!(matches!(
            store
                .update_project_session_for_lease(&project, &project_lease)
                .await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));

        let mut task = make_task_session(&wave, &project);
        task.set_status(TaskSessionStatus::Waiting, "ready");
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        task.begin_generation("task-body".to_string());
        let task_lease = store
            .reserve_task_process(&task, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut task.latest_process {
            process.state = ChildLeaseState::Active;
        }
        task.set_status(TaskSessionStatus::Running, "active");
        store
            .activate_task_process(&task, &task_lease)
            .await
            .unwrap();
        let mut terminal_task = task.clone();
        terminal_task.set_status(TaskSessionStatus::Completed, "work delivered");
        store.update_task_session(&terminal_task).await.unwrap();
        assert!(matches!(
            store
                .update_task_session_for_lease(&task, &task_lease)
                .await,
            Err(super::StoreError::LeaseRevoked { generation: 1, .. })
        ));
        assert_eq!(
            store
                .get_task_session(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskSessionStatus::Completed
        );
    }

    async fn run_store_basic_suite(store: &super::Store) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(wave.id()).await.expect("get wave").is_some());

        wave.repo = "/repo-updated".to_string();
        store.update_wave(&wave).await.expect("update wave");
        let loaded = store
            .get_wave(wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(loaded.repo(), "/repo-updated");

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
        let mut snapshot = PmSnapshotRow {
            repo: "/repo".to_string(),
            wave: "product".to_string(),
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
            store
                .pm_snapshot("/repo".to_string(), "product".to_string())
                .await
                .expect("read snapshot"),
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
