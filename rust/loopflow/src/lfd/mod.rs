//! `lfd` — the machine-level Home daemon: Wave startup, webhook ingress, and liveness.
//!
//! `lfd` is the one process that must always be running on a Home machine: it
//! hosts local Wave agents, receives external HTTP that cannot be SSH/`lf`
//! (webhook deliveries), and serves liveness probes. It is *not* a
//! remote control API — reads become `lf` queries; hands become `lf` directly.
//!
//! The ingress path is a durable delivery inbox: each signed Linear delivery is
//! persisted to `provider_deliveries` *before* it is acknowledged, deduplicated
//! by `(delivery_id, provider)`, and routed to the owning Task through
//! the existing domain ops (`webhook::ingest_event`). The inbox deduplicates
//! *deliveries*; the domain tables deduplicate *events*. Both gates are needed
//! — a redelivered webhook is dropped at the inbox; an out-of-order or
//! crash-mid-flight delivery re-processes at the inbox but is a no-op at the
//! domain.
//!
//! ```text
//!   lfd serve --addr 127.0.0.1:8080
//!   ┌────────────────────────────────────────────────┐
//!   │ /health          → liveness probe              │
//!   │ /status          → wave count + delivery count │
//!   │ /waves/start     → local capability → start    │
//!   │ /waves/stop      → local capability → stop     │
//!   │ /landings/claim  → claim watched PR generation │
//!   │ /linear/webhook  → verify → inbox → ingest     │
//!   │ /github/webhook  → verify → prune worktree     │
//!   └────────────────────────────────────────────────┘
//! ```

pub mod service;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use fs2::FileExt;
use hmac::{Hmac, Mac};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::durable::HomeId;
use crate::engine::config::load_config_or_default;
use crate::engine::worktrees::{
    main_repo_root, prune_abandoned_prompt_logs, prune_branch_worktree, prune_terminal_worktree,
    prune_worktrees, TargetedPruneOutcome, WorktreePrunePolicy, WorktreePruneReason,
};
use crate::harness::opencode_runtime::reap_orphaned_opencode_servers_at;
use crate::id::WaveId;
use crate::pr_landing::{
    LandingClaim, LandingPlacement, PrLanding, PrLandingId, SUPERVISOR_STALE_AFTER,
};
use crate::repository::RepoId;
use crate::store::provider_deliveries::{DeliveryCompletion, DeliveryEventKind, DeliveryStatus};
use crate::store::Store;
use crate::wave_host::WaveHost;
use crate::wave_host::WaveStartOutcome;
use crate::webhook::{self, WebhookEvent, WebhookOutcome, SIGNATURE_HEADER};

/// Body limit on webhook routes. Linear deliveries are small; a hard cap keeps
/// a malformed or hostile request from buffering unbounded bytes.
const WEBHOOK_BODY_LIMIT: usize = 256 * 1024;
const GITHUB_SIGNATURE_HEADER: &str = "x-hub-signature-256";
const GITHUB_EVENT_HEADER: &str = "x-github-event";
const ABANDONED_LOG_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const LANDING_CLAIM_TIMEOUT: Duration = Duration::from_secs(2);
const LANDING_RECOVERY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct StartupSignal {
    pub attempt_id: String,
    pub receipt_path: PathBuf,
    pub socket_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum StartupState {
    Live { endpoint: String, home_id: HomeId },
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupReceipt {
    pub attempt_id: String,
    #[serde(flatten)]
    pub state: StartupState,
}

#[derive(Debug)]
struct StartupSocket {
    path: PathBuf,
    directory: PathBuf,
}

impl Drop for StartupSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

impl StartupSignal {
    pub async fn report(&self, state: StartupState) -> anyhow::Result<()> {
        let receipt = StartupReceipt {
            attempt_id: self.attempt_id.clone(),
            state,
        };
        write_startup_receipt(&self.receipt_path, &receipt)?;
        tokio::net::UnixStream::connect(&self.socket_path).await?;
        Ok(())
    }
}

/// Everything the `lfd` receiver needs, shared across requests.
#[derive(Clone)]
pub struct LfdState {
    /// Primary repository for webhook and maintenance configuration.
    repo_root: PathBuf,
    /// The durable store — always open; the delivery inbox lives here.
    store: Arc<Store>,
    /// Linear webhook config. When absent, `/linear/webhook` returns 503.
    linear: Option<LinearConfig>,
    /// GitHub webhook config. When absent, `/github/webhook` returns 503.
    github: Option<GithubConfig>,
    /// The Home-local keeper for Wave listener tasks.
    wave_host: WaveHost,
    /// Local capability required by the Wave control routes.
    control_token: Arc<String>,
}

/// Linear webhook verification + ingestion config, sourced from env.
#[derive(Clone)]
pub struct LinearConfig {
    pub secret: Arc<Vec<u8>>,
    pub viewer_id: Arc<String>,
}

#[derive(Clone)]
struct GithubConfig {
    secret: Arc<Vec<u8>>,
    webhook_url: Option<Arc<String>>,
}

async fn build_state(
    repo_root: PathBuf,
    store: Arc<Store>,
    linear: Option<LinearConfig>,
    github: Option<GithubConfig>,
    discord_token: Option<SecretString>,
) -> anyhow::Result<LfdState> {
    let local = store.local_home().await?;
    Ok(LfdState {
        repo_root,
        wave_host: WaveHost::new(local.id, store.clone(), discord_token),
        control_token: Arc::new(uuid::Uuid::new_v4().to_string()),
        store,
        linear,
        github,
    })
}

/// Build the `lfd` router.
pub fn router(state: LfdState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route("/waves/start", post(start_waves_handler))
        .route("/waves/reconcile", post(reconcile_waves_handler))
        .route("/waves/stop", post(stop_wave_handler))
        .route("/landings/claim", post(claim_landing_handler))
        .route(
            "/linear/webhook",
            post(webhook_handler).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
        )
        .route(
            "/github/webhook",
            post(github_webhook_handler).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
        )
        .with_state(state)
}

// -- Handlers ----------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct HealthBody {
    status: String,
    home_id: HomeId,
    store: String,
    runtime_generation: Option<u64>,
    build_version: Option<String>,
    source_revision: Option<String>,
    migration_frontier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LivenessHealthBody {
    status: String,
    home_id: HomeId,
}

async fn health_handler(State(state): State<LfdState>) -> Json<HealthBody> {
    Json(HealthBody {
        status: "ok".to_string(),
        home_id: state.wave_host.home_id().clone(),
        store: crate::store::database_path_from_env()
            .expect("running lfd already opened its selected store")
            .display()
            .to_string(),
        runtime_generation: Some(crate::lf::commands::install::current_runtime_generation()),
        build_version: Some(crate::build_info::BUILD_VERSION.to_string()),
        source_revision: Some(crate::build_info::source_revision().to_string()),
        migration_frontier: Some(crate::store::migrations::latest_known_version()),
    })
}

#[derive(Serialize)]
struct StatusBody {
    waves: usize,
    deliveries: i64,
}

async fn status_handler(State(state): State<LfdState>) -> Json<StatusBody> {
    let waves = state.wave_host.active_count().await;
    let deliveries = state.store.delivery_count().await.unwrap_or(0);
    Json(StatusBody { waves, deliveries })
}

#[derive(Debug, Serialize, Deserialize)]
struct StartWavesRequest {
    wave_ids: Vec<WaveId>,
}

async fn start_waves_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    Json(request): Json<StartWavesRequest>,
) -> Result<Json<Vec<WaveStartOutcome>>, (StatusCode, String)> {
    authorize_wave_control(&state, &headers)?;
    Ok(Json(state.wave_host.start_waves(request.wave_ids).await))
}

async fn reconcile_waves_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    Json(request): Json<StartWavesRequest>,
) -> Result<Json<Vec<WaveStartOutcome>>, (StatusCode, String)> {
    authorize_wave_control(&state, &headers)?;
    Ok(Json(
        state.wave_host.reconcile_waves(request.wave_ids).await,
    ))
}

#[derive(Debug, Serialize, Deserialize)]
struct StopWaveRequest {
    wave_id: WaveId,
}

async fn stop_wave_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    Json(request): Json<StopWaveRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_wave_control(&state, &headers)?;
    state
        .wave_host
        .stop_wave(&request.wave_id)
        .await
        .map(|requested| {
            if requested {
                StatusCode::ACCEPTED
            } else {
                StatusCode::NO_CONTENT
            }
        })
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

fn authorize_wave_control(
    state: &LfdState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    let supplied = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if supplied == Some(state.control_token.as_str()) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            "Wave control requires the local lfd capability".to_string(),
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaimLandingRequest {
    landing_id: PrLandingId,
    generation: u64,
}

async fn claim_landing_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    Json(request): Json<ClaimLandingRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_wave_control(&state, &headers)?;
    let now = OffsetDateTime::now_utc();
    let claim = LandingClaim {
        placement: LandingPlacement::Home {
            home_id: state.wave_host.home_id().clone(),
        },
        process_id: std::process::id(),
        heartbeat_at: now,
    };
    let claimed = state
        .store
        .claim_pr_landing(
            &request.landing_id,
            request.generation,
            &claim,
            now - SUPERVISOR_STALE_AFTER,
        )
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let Some(claimed) = claimed else {
        return Ok(StatusCode::CONFLICT);
    };
    spawn_claimed_landing(state.store.clone(), claimed);
    Ok(StatusCode::ACCEPTED)
}

fn spawn_claimed_landing(store: Arc<Store>, landing: PrLanding) {
    tokio::spawn(async move {
        let driver = crate::ops::pr_landing::github_landing_driver();
        if let Err(error) = crate::ops::supervise_pr_landing(
            store,
            landing.clone(),
            driver,
            Duration::from_secs(30),
        )
        .await
        {
            tracing::warn!(landing = %landing.id, %error, "watched PR landing stopped");
        }
    });
}

async fn claim_recoverable_pr_landings(state: &LfdState, now: OffsetDateTime) -> Vec<PrLanding> {
    let recoverable = match state
        .store
        .recoverable_pr_landings(now - SUPERVISOR_STALE_AFTER)
        .await
    {
        Ok(landings) => landings,
        Err(error) => {
            tracing::warn!(%error, "could not scan watched PR landings");
            return Vec::new();
        }
    };
    let mut claimed = Vec::new();
    for landing in recoverable {
        let claim = LandingClaim {
            placement: LandingPlacement::Home {
                home_id: state.wave_host.home_id().clone(),
            },
            process_id: std::process::id(),
            heartbeat_at: now,
        };
        match state
            .store
            .claim_pr_landing(
                &landing.id,
                landing.generation,
                &claim,
                now - SUPERVISOR_STALE_AFTER,
            )
            .await
        {
            Ok(Some(landing)) => claimed.push(landing),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(landing = %landing.id, %error, "could not recover watched PR landing")
            }
        }
    }
    claimed
}

async fn recover_pr_landings(state: &LfdState) {
    for landing in claim_recoverable_pr_landings(state, OffsetDateTime::now_utc()).await {
        spawn_claimed_landing(state.store.clone(), landing);
    }
}

async fn recover_pr_landings_forever(state: LfdState) {
    loop {
        recover_pr_landings(&state).await;
        tokio::time::sleep(LANDING_RECOVERY_INTERVAL).await;
    }
}

/// Receive a signed Linear delivery, persist it to the durable inbox, and route
/// it to the owning Task. Idempotent across retries and restarts: a
/// duplicate delivery in a terminal state is dropped; one left `pending` by a
/// crash is re-processed (the domain gate makes re-processing a no-op).
async fn webhook_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(ref linear) = state.linear else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let Some(signature) = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok()) else {
        return StatusCode::UNAUTHORIZED;
    };
    if webhook::verify_signature(&linear.secret, &body, signature).is_err() {
        return StatusCode::UNAUTHORIZED;
    }
    let (event, webhook_timestamp) = match webhook::parse_event(&body) {
        Ok(parsed) => parsed,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let now = OffsetDateTime::now_utc();
    if !webhook::within_replay_window(webhook_timestamp, now) {
        return StatusCode::UNAUTHORIZED;
    }

    let delivery_id = derive_delivery_id(&event, webhook_timestamp, &body);
    let event_kind = delivery_event_kind(&event);
    let received_at = (now.unix_timestamp_nanos() / 1_000_000) as i64;

    let record = match state
        .store
        .record_delivery(
            delivery_id.clone(),
            "linear".to_string(),
            event_kind
                .map(DeliveryEventKind::as_str)
                .map(str::to_string),
            received_at,
        )
        .await
    {
        Ok(record) => record,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };
    // A duplicate in a terminal state was already handled — acknowledge so
    // Linear stops retrying. A duplicate left `pending` is a crashed prior
    // attempt: re-process (the domain gate is idempotent).
    if !record.inserted && record.existing_status != Some(DeliveryStatus::Pending) {
        return StatusCode::OK;
    }

    let outcome = match webhook::ingest_event(&state.store, event, &linear.viewer_id, now).await {
        Ok(outcome) => outcome,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    let (status, target_kind) = map_outcome(&outcome);
    let outcome_json = serde_json::to_string(&OutcomeSummary::from(&outcome)).ok();
    let processed_at = (now.unix_timestamp_nanos() / 1_000_000) as i64;
    let completion = DeliveryCompletion {
        delivery_id,
        provider: "linear".to_string(),
        status,
        target_kind: target_kind.map(str::to_string),
        target_id: None,
        outcome: outcome_json,
        processed_at,
    };
    if state.store.complete_delivery(completion).await.is_err() {
        // The directive was applied (or classified) but the inbox row could not
        // be stamped. Leave it `pending` and 500 so Linear retries; the domain
        // gate makes the retry a no-op before the stamp succeeds.
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
struct GithubRepositoryPayload {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct GithubHeadPayload {
    r#ref: String,
}

#[derive(Debug, Deserialize)]
struct GithubPullRequestPayload {
    merged: bool,
    head: GithubHeadPayload,
}

#[derive(Debug, Deserialize)]
struct GithubPullRequestEvent {
    action: String,
    repository: GithubRepositoryPayload,
    pull_request: GithubPullRequestPayload,
}

#[derive(Debug, Deserialize)]
struct GithubDeleteEvent {
    r#ref: String,
    ref_type: String,
    repository: GithubRepositoryPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubPruneEvent {
    repo: String,
    branch: String,
    reason: WorktreePruneReason,
}

fn parse_github_prune_event(event: &str, body: &[u8]) -> Result<Option<GithubPruneEvent>, ()> {
    match event {
        "pull_request" => {
            let payload: GithubPullRequestEvent = serde_json::from_slice(body).map_err(|_| ())?;
            if payload.action != "closed" || !payload.pull_request.merged {
                return Ok(None);
            }
            Ok(Some(GithubPruneEvent {
                repo: payload.repository.full_name,
                branch: payload.pull_request.head.r#ref,
                reason: WorktreePruneReason::Merged,
            }))
        }
        "delete" => {
            let payload: GithubDeleteEvent = serde_json::from_slice(body).map_err(|_| ())?;
            if payload.ref_type != "branch" {
                return Ok(None);
            }
            Ok(Some(GithubPruneEvent {
                repo: payload.repository.full_name,
                branch: payload.r#ref,
                reason: WorktreePruneReason::RemoteGone,
            }))
        }
        _ => Ok(None),
    }
}

fn verify_github_signature(secret: &[u8], body: &[u8], signature: &str) -> bool {
    let Some(signature) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

async fn github_webhook_handler(
    State(state): State<LfdState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(ref github) = state.github else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let Some(signature) = headers
        .get(GITHUB_SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::UNAUTHORIZED;
    };
    if !verify_github_signature(&github.secret, &body, signature) {
        return StatusCode::UNAUTHORIZED;
    }
    let Some(event_name) = headers
        .get(GITHUB_EVENT_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::BAD_REQUEST;
    };
    let event = match parse_github_prune_event(event_name, &body) {
        Ok(Some(event)) => event,
        Ok(None) => return StatusCode::OK,
        Err(()) => return StatusCode::BAD_REQUEST,
    };
    tokio::spawn(async move {
        if let Err(error) = prune_github_event(&state, event).await {
            tracing::warn!(error = %error, "github worktree prune failed");
        }
    });
    StatusCode::ACCEPTED
}

/// Map a processing outcome onto the delivery row's terminal status and the
/// target kind it routed to. `Ignored` and `NoTarget` carry no target; a
/// self-authored or applied edit/comment resolved a Task.
fn map_outcome(outcome: &WebhookOutcome) -> (DeliveryStatus, Option<&'static str>) {
    match outcome {
        WebhookOutcome::Ignored => (DeliveryStatus::Ignored, None),
        WebhookOutcome::NoTarget => (DeliveryStatus::NoTarget, None),
        WebhookOutcome::SelfAuthored => (DeliveryStatus::Processed, Some("task")),
        WebhookOutcome::Edit { .. } => (DeliveryStatus::Processed, Some("task")),
        WebhookOutcome::Comment { .. } => (DeliveryStatus::Processed, Some("task")),
    }
}

/// The typed event kind a delivery carried, for the `event_kind` column.
fn delivery_event_kind(event: &WebhookEvent) -> Option<DeliveryEventKind> {
    match event {
        WebhookEvent::IssueEdit { .. } => Some(DeliveryEventKind::IssueEdit),
        WebhookEvent::Comment { .. } => Some(DeliveryEventKind::Comment),
        WebhookEvent::Ignored => Some(DeliveryEventKind::Ignored),
    }
}

/// Derive the provider delivery id — the inbox's dedup key — from the parsed
/// event. `IssueEdit` keys on the issue + its monotonic `updatedAt` revision
/// (aligned with the domain-level guard); `Comment` on the globally unique
/// comment id; `Ignored` on the webhook timestamp plus a body digest, since an
/// ignored delivery carries no domain identity.
fn derive_delivery_id(event: &WebhookEvent, webhook_timestamp: i64, body: &[u8]) -> String {
    match event {
        WebhookEvent::IssueEdit {
            issue_id, revision, ..
        } => format!("linear:issue:{issue_id}:{revision}"),
        WebhookEvent::Comment { comment_id, .. } => format!("linear:comment:{comment_id}"),
        WebhookEvent::Ignored => {
            let digest = Sha256::digest(body);
            format!(
                "linear:ignored:{webhook_timestamp}:{}",
                hex::encode(&digest[..4])
            )
        }
    }
}

#[derive(Serialize)]
struct OutcomeSummary<'a> {
    outcome: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    steer_applied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered: Option<bool>,
}

impl<'a> From<&'a WebhookOutcome> for OutcomeSummary<'a> {
    fn from(outcome: &'a WebhookOutcome) -> Self {
        match outcome {
            WebhookOutcome::Ignored => Self {
                outcome: "ignored",
                steer_applied: None,
                delivered: None,
            },
            WebhookOutcome::NoTarget => Self {
                outcome: "no_target",
                steer_applied: None,
                delivered: None,
            },
            WebhookOutcome::SelfAuthored => Self {
                outcome: "self_authored",
                steer_applied: None,
                delivered: None,
            },
            WebhookOutcome::Edit { steer_applied } => Self {
                outcome: "edit",
                steer_applied: Some(*steer_applied),
                delivered: None,
            },
            WebhookOutcome::Comment { delivered } => Self {
                outcome: "comment",
                steer_applied: None,
                delivered: Some(*delivered),
            },
        }
    }
}

async fn managed_repo_roots(state: &LfdState) -> Vec<PathBuf> {
    let mut candidates = vec![state.repo_root.clone()];
    if let Ok(tasks) = state.store.list_tasks(None).await {
        candidates.extend(
            tasks
                .into_iter()
                .map(|task| task.worktree)
                .filter(|path| path.exists()),
        );
    }
    let mut roots = HashSet::new();
    for candidate in candidates {
        if let Ok(root) = main_repo_root(&candidate) {
            roots.insert(root);
        }
    }
    let mut roots = roots.into_iter().collect::<Vec<_>>();
    roots.sort();
    roots
}

async fn protected_worktree_paths(state: &LfdState) -> anyhow::Result<HashSet<PathBuf>> {
    let mut protected = crate::lf::commands::top::running_workspace_paths();
    for task in state.store.list_tasks(None).await? {
        let work = state
            .store
            .work_for_child(&crate::child::ChildRef::Task(task.id.clone()))
            .await?;
        let status = state.store.work_status(&work).await?;
        if !matches!(
            status,
            crate::durable::WorkStatus::Done | crate::durable::WorkStatus::Abandoned
        ) {
            protected.insert(task.worktree);
        }
    }
    let production = crate::store::production_database_path();
    if production.exists() {
        protected.extend(crate::store::read_nonterminal_task_worktrees(&production)?);
    }
    Ok(protected)
}

async fn prune_github_event(state: &LfdState, event: GithubPruneEvent) -> anyhow::Result<()> {
    let protected_paths = protected_worktree_paths(state).await?;
    let roots = managed_repo_roots(state)
        .await
        .into_iter()
        .filter(|root| RepoId::discover(root).is_ok_and(|repo| repo.as_str() == event.repo))
        .collect::<Vec<_>>();
    for root in roots {
        let current_path = state.repo_root.clone();
        let branch = event.branch.clone();
        let reason = event.reason;
        let protected_paths = protected_paths.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            prune_branch_worktree(&root, &current_path, &branch, reason, &protected_paths)
        })
        .await??;
        match outcome {
            TargetedPruneOutcome::Removed(target) => tracing::info!(
                path = %target.path.display(),
                branch = target.branch.as_deref().unwrap_or("detached"),
                reason = target.reason.as_str(),
                "github event pruned worktree"
            ),
            TargetedPruneOutcome::RetainedDirty(path) => tracing::warn!(
                path = %path.display(),
                "github event retained dirty worktree"
            ),
            TargetedPruneOutcome::Protected | TargetedPruneOutcome::NotFound => {}
        }
    }
    Ok(())
}

async fn maintenance_sweep(state: &LfdState) {
    let lf_home = crate::store::lf_home_dir();
    match tokio::task::spawn_blocking(move || reap_orphaned_opencode_servers_at(&lf_home)).await {
        Ok(report) => {
            if report.reaped > 0 {
                tracing::info!(
                    reaped = report.reaped,
                    "OpenCode orphan maintenance complete"
                );
            }
            if report.errors > 0 {
                tracing::warn!(
                    errors = report.errors,
                    "OpenCode orphan maintenance incomplete"
                );
            }
        }
        Err(error) => tracing::warn!(%error, "OpenCode orphan maintenance task failed"),
    }

    let protected_paths = match protected_worktree_paths(state).await {
        Ok(paths) => paths,
        Err(error) => {
            tracing::warn!(
                %error,
                "cannot verify Task ownership; skipping automatic worktree pruning"
            );
            return;
        }
    };
    for root in managed_repo_roots(state).await {
        let current_path = state.repo_root.clone();
        let root_for_log = root.clone();
        let protected_paths = protected_paths.clone();
        match tokio::task::spawn_blocking(move || {
            prune_worktrees(
                &root,
                &current_path,
                &protected_paths,
                WorktreePrunePolicy::automatic(),
                false,
            )
        })
        .await
        {
            Ok(Ok(report)) => {
                if !report.removed.is_empty() || !report.retained_dirty.is_empty() {
                    tracing::info!(
                        repo = %root_for_log.display(),
                        removed = report.removed.len(),
                        retained_dirty = report.retained_dirty.len(),
                        "automatic worktree prune complete"
                    );
                }
                for failure in report.failed {
                    tracing::warn!(
                        path = %failure.target.path.display(),
                        error = %failure.error,
                        "automatic worktree prune failed"
                    );
                }
            }
            Ok(Err(error)) => tracing::warn!(
                repo = %root_for_log.display(),
                error = %error,
                "automatic worktree scan failed"
            ),
            Err(error) => tracing::warn!(
                repo = %root_for_log.display(),
                error = %error,
                "automatic worktree scan task failed"
            ),
        }
    }

    let now = OffsetDateTime::now_utc();
    if let Ok(tasks) = state.store.list_tasks(None).await {
        let mut active = HashSet::new();
        let mut terminal = HashSet::new();
        for task in tasks {
            let Ok(work) = state
                .store
                .work_for_child(&crate::child::ChildRef::Task(task.id.clone()))
                .await
            else {
                continue;
            };
            let Ok(status) = state.store.work_status(&work).await else {
                continue;
            };
            if matches!(
                status,
                crate::durable::WorkStatus::Done | crate::durable::WorkStatus::Abandoned
            ) {
                if task.updated_at <= now - time::Duration::hours(1) && task.worktree.exists() {
                    terminal.insert(task.worktree);
                }
            } else {
                active.insert(task.worktree);
            }
        }
        terminal.retain(|path| !active.contains(path));
        for path in terminal {
            let Ok(root) = main_repo_root(&path) else {
                continue;
            };
            let current_path = state.repo_root.clone();
            let protected_paths = protected_paths.clone();
            match tokio::task::spawn_blocking(move || {
                prune_terminal_worktree(&root, &current_path, &path, &protected_paths)
            })
            .await
            {
                Ok(Ok(TargetedPruneOutcome::Removed(target))) => tracing::info!(
                    path = %target.path.display(),
                    "pruned terminal Task worktree"
                ),
                Ok(Ok(TargetedPruneOutcome::RetainedDirty(path))) => tracing::warn!(
                    path = %path.display(),
                    "retained dirty terminal Task worktree"
                ),
                Ok(Ok(TargetedPruneOutcome::Protected | TargetedPruneOutcome::NotFound)) => {}
                Ok(Err(error)) => tracing::warn!(error = %error, "terminal worktree prune failed"),
                Err(error) => tracing::warn!(error = %error, "terminal worktree prune task failed"),
            }
        }
    }

    let lf_home = crate::store::lf_home_dir();
    match tokio::task::spawn_blocking(move || {
        prune_abandoned_prompt_logs(&lf_home, ABANDONED_LOG_AGE)
    })
    .await
    {
        Ok(Ok(removed)) if !removed.is_empty() => {
            tracing::info!(removed = removed.len(), "pruned abandoned prompt logs")
        }
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(error = %error, "abandoned prompt log prune failed"),
        Err(error) => tracing::warn!(error = %error, "prompt log prune task failed"),
    }
}

async fn maintenance_loop(state: LfdState, interval: Duration) {
    loop {
        maintenance_sweep(&state).await;
        tokio::time::sleep(interval).await;
    }
}

fn config_value(repo: &Path, name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    let output = Command::new("doppler")
        .current_dir(repo)
        .args(["secrets", "get", name, "--plain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim_end_matches(['\r', '\n']);
    (!value.is_empty()).then(|| value.to_string())
}

fn github_config(repo: &Path) -> Option<GithubConfig> {
    let secret = config_value(repo, "LF_GITHUB_WEBHOOK_SECRET")?;
    if secret.is_empty() {
        return None;
    }
    let webhook_url = config_value(repo, "LF_GITHUB_WEBHOOK_URL").map(Arc::new);
    Some(GithubConfig {
        secret: Arc::new(secret.into_bytes()),
        webhook_url,
    })
}

fn ensure_github_subscription(repo: &Path, github: &GithubConfig) -> anyhow::Result<()> {
    let Some(url) = github.webhook_url.as_deref() else {
        return Ok(());
    };
    let repo_id = RepoId::discover(repo)?;
    let hooks_endpoint = format!("repos/{}/hooks", repo_id.as_str());
    let hooks = Command::new("gh")
        .current_dir(repo)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["api", &hooks_endpoint])
        .output()?;
    if !hooks.status.success() {
        anyhow::bail!("GitHub hook lookup failed for {repo_id}");
    }
    let hooks: serde_json::Value = serde_json::from_slice(&hooks.stdout)?;
    let existing_id = hooks.as_array().and_then(|hooks| {
        hooks.iter().find_map(|hook| {
            (hook.pointer("/config/url").and_then(|value| value.as_str()) == Some(url.as_str()))
                .then(|| hook.get("id").and_then(|value| value.as_u64()))
                .flatten()
        })
    });
    let (method, endpoint) = match existing_id {
        Some(id) => ("PATCH", format!("{hooks_endpoint}/{id}")),
        None => ("POST", hooks_endpoint),
    };
    let secret = std::str::from_utf8(&github.secret)?;
    let body = serde_json::to_vec(&serde_json::json!({
        "name": "web",
        "active": true,
        "events": ["pull_request", "delete"],
        "config": {
            "url": url.as_str(),
            "content_type": "json",
            "secret": secret,
            "insecure_ssl": "0"
        }
    }))?;
    let mut child = Command::new("gh")
        .current_dir(repo)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["api", "--method", method, &endpoint, "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("GitHub hook request stdin unavailable"))?
        .write_all(&body)?;
    if !child.wait()?.success() {
        anyhow::bail!("GitHub hook registration failed for {repo_id}");
    }
    Ok(())
}

async fn ensure_github_subscriptions(state: &LfdState) {
    let Some(github) = state.github.clone() else {
        return;
    };
    if github.webhook_url.is_none() {
        return;
    }
    for root in managed_repo_roots(state).await {
        let root_for_log = root.clone();
        let github = github.clone();
        match tokio::task::spawn_blocking(move || ensure_github_subscription(&root, &github)).await
        {
            Ok(Ok(())) => {
                tracing::info!(repo = %root_for_log.display(), "github worktree webhook subscribed")
            }
            Ok(Err(error)) => {
                tracing::warn!(repo = %root_for_log.display(), error = %error, "github webhook subscription failed")
            }
            Err(error) => {
                tracing::warn!(repo = %root_for_log.display(), error = %error, "github webhook subscription task failed")
            }
        }
    }
}

// -- Serve -------------------------------------------------------------------

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HomeRunReconciliation {
    pub present: usize,
    pub absent: usize,
    pub unprovable: usize,
    pub asks_requeued: usize,
    pub errors: usize,
}

pub(crate) async fn reconcile_home_runs_once(
    store: &Store,
    home_id: &HomeId,
) -> HomeRunReconciliation {
    use crate::durable::ContainmentObservation;

    let mut summary = HomeRunReconciliation::default();
    match store.local_home().await {
        Ok(local) if local.id == *home_id => {}
        Ok(local) => {
            summary.errors += 1;
            tracing::warn!(%home_id, local_home_id = %local.id, "refusing non-local Run reconciliation");
            return summary;
        }
        Err(error) => {
            summary.errors += 1;
            tracing::warn!(%error, %home_id, "could not verify local Home for Run reconciliation");
            return summary;
        }
    }
    match store.repair_stranded_asks().await {
        Ok(repaired) => summary.asks_requeued = repaired,
        Err(error) => {
            summary.errors += 1;
            tracing::warn!(%error, %home_id, "could not repair stranded Ask claims");
        }
    }
    let runs = match store.nonterminal_runs_for_home(home_id).await {
        Ok(runs) => runs,
        Err(error) => {
            summary.errors += 1;
            tracing::warn!(%error, %home_id, "could not list Home Runs for reconciliation");
            return summary;
        }
    };
    let observations = futures_util::future::join_all(runs.into_iter().map(|run| async move {
        let observation = match run.containment.as_ref() {
            Some(containment) => crate::engine::process::containment_observation(containment).await,
            None => ContainmentObservation::Unprovable,
        };
        (run, observation)
    }))
    .await;
    for (run, observation) in observations {
        match store.recover_run(&run.id, observation).await {
            Ok(_) => match observation {
                ContainmentObservation::Present => summary.present += 1,
                ContainmentObservation::Absent => summary.absent += 1,
                ContainmentObservation::Unprovable => summary.unprovable += 1,
            },
            Err(error) => {
                summary.errors += 1;
                tracing::warn!(
                    %error,
                    run_id = %run.id,
                    work_kind = run.work.kind(),
                    work_id = run.work.id(),
                    "Home Run reconciliation failed"
                );
            }
        }
    }
    summary
}

async fn reconcile_home_runs_forever(store: Arc<Store>, home_id: HomeId) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    interval.tick().await;
    loop {
        interval.tick().await;
        reconcile_home_runs_once(&store, &home_id).await;
    }
}

async fn reconcile_home_runs_before_startup_live(
    store: &Store,
    home_id: &HomeId,
    startup: Option<&StartupSignal>,
    client: &LfdClientEndpoint,
) -> HomeRunReconciliation {
    let summary = reconcile_home_runs_once(store, home_id).await;
    tracing::info!(
        present = summary.present,
        absent = summary.absent,
        unprovable = summary.unprovable,
        asks_requeued = summary.asks_requeued,
        errors = summary.errors,
        "lfd startup Run reconciliation completed"
    );
    if let Some(startup) = startup {
        if let Err(error) = startup
            .report(StartupState::Live {
                endpoint: client.endpoint.clone(),
                home_id: home_id.clone(),
            })
            .await
        {
            tracing::warn!(%error, "could not publish lfd startup receipt");
        }
    }
    summary
}

/// Bind and serve `lfd` until the process ends. The store is always open (the
/// inbox lives there); Linear and GitHub config are optional — absent webhook
/// credentials leave their corresponding route at 503.
///
/// A non-loopback bind is refused unless `LF_LFD_ALLOW_NON_LOOPBACK=1`
/// explicitly permits exposure. Operators must gate the network boundary
/// independently.
pub async fn serve(
    repo_root: PathBuf,
    addr: SocketAddr,
    store: Arc<Store>,
    linear: Option<LinearConfig>,
    startup: Option<StartupSignal>,
) -> anyhow::Result<()> {
    ensure_bind_allowed(
        addr,
        std::env::var_os("LF_LFD_ALLOW_NON_LOOPBACK").as_deref(),
    )?;
    if !addr.ip().is_loopback() {
        tracing::warn!(
            %addr,
            "lfd bound off loopback with LF_LFD_ALLOW_NON_LOOPBACK=1; \
             gate this listener at the network boundary"
        );
    }
    let discord_token =
        config_value(&repo_root, crate::wave::discord::TOKEN_ENV).map(SecretString::new);
    let state = build_state(
        repo_root.clone(),
        store,
        linear,
        github_config(&repo_root),
        discord_token,
    )
    .await?;
    let _home_lock = lock_home(state.wave_host.home_id())?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let client = LfdClientEndpoint {
        endpoint: local_client_endpoint(bound),
        token: state.control_token.as_ref().clone(),
    };
    write_endpoint(state.wave_host.home_id(), &client)?;
    reconcile_home_runs_before_startup_live(
        &state.store,
        state.wave_host.home_id(),
        startup.as_ref(),
        &client,
    )
    .await;
    let wave_host = state.wave_host.clone();
    let reconciliation = tokio::spawn({
        let wave_host = wave_host.clone();
        async move { wave_host.reconcile_forever().await }
    });
    let run_reconciliation = tokio::spawn(reconcile_home_runs_forever(
        state.store.clone(),
        wave_host.home_id().clone(),
    ));
    let autoprune = load_config_or_default(Some(&repo_root)).autoprune;
    if autoprune.enabled {
        let maintenance_state = state.clone();
        let interval = Duration::from_secs(autoprune.poll_interval_seconds.max(60));
        tokio::spawn(async move { maintenance_loop(maintenance_state, interval).await });
    }
    let subscription_state = state.clone();
    tokio::spawn(async move { ensure_github_subscriptions(&subscription_state).await });
    let landing_state = state.clone();
    tokio::spawn(recover_pr_landings_forever(landing_state));
    tracing::info!(addr = %bound, home_id = %wave_host.home_id(), "lfd serving");
    let result = axum::serve(listener, router(state).into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await;
    reconciliation.abort();
    let _ = reconciliation.await;
    run_reconciliation.abort();
    let _ = run_reconciliation.await;
    wave_host.shutdown().await;
    remove_endpoint(wave_host.home_id(), &client);
    result.map_err(anyhow::Error::from)
}

fn local_client_endpoint(bound: SocketAddr) -> String {
    match bound.ip() {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() => {
            SocketAddr::from(([127, 0, 0, 1], bound.port())).to_string()
        }
        std::net::IpAddr::V6(ip) if ip.is_unspecified() => {
            SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, bound.port())).to_string()
        }
        _ => bound.to_string(),
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut hangup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
        _ = hangup.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

pub(crate) async fn ensure(home_id: &HomeId, repo: &Path) -> anyhow::Result<()> {
    ensure_with_selection(home_id, repo, None, None).await
}

pub(crate) async fn ensure_for_switch(
    home_id: &HomeId,
    repo: &Path,
    switch_id: &str,
) -> anyhow::Result<()> {
    let selection = install_switch_selection(switch_id)?;
    ensure_with_selection(home_id, repo, Some(&selection), Some(switch_id)).await
}

pub(crate) async fn ensure_install_selection(
    home_id: &HomeId,
    repo: &Path,
    selection: &crate::machine_install::InstallSelection,
) -> anyhow::Result<()> {
    ensure_with_selection(home_id, repo, Some(selection), None).await
}

fn install_switch_selection(
    switch_id: &str,
) -> anyhow::Result<crate::machine_install::InstallSelection> {
    match crate::machine_install::read_state(&crate::machine_install::root()?)? {
        crate::machine_install::MachineInstallState::Switching(receipt)
            if receipt.id == switch_id
                && receipt.phase == crate::machine_install::SwitchPhase::Reconciling
                && receipt.target_store_advanced =>
        {
            Ok(receipt.target.clone())
        }
        crate::machine_install::MachineInstallState::Switching(receipt) => {
            anyhow::bail!(
                "install switch {} is not reconciling as {switch_id}",
                receipt.id
            )
        }
        _ => anyhow::bail!("install switch {switch_id} is no longer active"),
    }
}

async fn ensure_with_selection(
    home_id: &HomeId,
    repo: &Path,
    selection: Option<&crate::machine_install::InstallSelection>,
    switch_id: Option<&str>,
) -> anyhow::Result<()> {
    let lf_home = selection
        .and_then(|selection| selection.store.parent().map(Path::to_path_buf))
        .unwrap_or_else(crate::store::lf_home_dir);
    let endpoints = lf_home.join("lfd");
    if live_endpoint_at(home_id, &endpoints).await.is_some() {
        return Ok(());
    }
    let _launch_lock = lock_start_at(home_id, &endpoints).await?;
    if live_endpoint_at(home_id, &endpoints).await.is_some() {
        return Ok(());
    }
    let lfd = match selection {
        Some(selection) => {
            let daemon = selection
                .artifact_set
                .artifact(&crate::machine_install::ArtifactRole::Daemon)
                .ok_or_else(|| anyhow::anyhow!("install selection has no daemon"))?;
            daemon.verify()?;
            daemon.path.clone()
        }
        None => crate::engine::process::resolve_lfd_binary_checked()?,
    };
    let attempt_id = uuid::Uuid::new_v4().simple().to_string();
    let startup_dir = endpoints.join("startup");
    std::fs::create_dir_all(&startup_dir)?;
    let socket_id = &attempt_id[..12];
    let socket_dir = std::env::temp_dir().join(format!("lfd-{socket_id}"));
    std::fs::create_dir(&socket_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&socket_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let socket_path = socket_dir.join("startup.sock");
    let _socket_cleanup = StartupSocket {
        path: socket_path.clone(),
        directory: socket_dir,
    };
    let receipt_path = startup_dir.join(format!("{attempt_id}.json"));
    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    let mut argv = vec![
        lfd.to_string_lossy().to_string(),
        "serve".to_string(),
        "--addr".to_string(),
        "127.0.0.1:0".to_string(),
        "--repo".to_string(),
        repo.display().to_string(),
        "--startup-attempt".to_string(),
        attempt_id.clone(),
        "--startup-receipt".to_string(),
        receipt_path.display().to_string(),
        "--startup-socket".to_string(),
        socket_path.display().to_string(),
    ];
    if let Some(switch_id) = switch_id {
        argv.extend(["--install-switch".to_string(), switch_id.to_string()]);
    }
    let session = format!(
        "lfd-{}",
        crate::engine::process::tmux_session_slug(home_id.as_str())
    );
    let launch = match selection {
        Some(selection) => {
            crate::engine::process::start_home_session_for_install_selection(
                &session, repo, &argv, selection, switch_id,
            )
            .await
        }
        _ => crate::engine::process::start_home_session(&session, repo, &argv).await,
    };
    if let Err(error) = launch {
        return Err(anyhow::anyhow!(
            "failed to start lfd for Home {home_id}: {error}"
        ));
    }
    let receipt = tokio::time::timeout(
        STARTUP_TIMEOUT,
        read_startup_signal(listener, &receipt_path, &attempt_id),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "lfd did not publish a live or failed startup receipt for Home {home_id} within {STARTUP_TIMEOUT:?}; inspect {}",
            lf_home.join("logs/lfd.log").display()
        )
    })??;
    match receipt.state {
        StartupState::Live {
            home_id: reported, ..
        } if reported == *home_id => live_endpoint_at(home_id, &endpoints)
            .await
            .map(|_| ())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "lfd reported live for Home {home_id}, but its endpoint is not answering"
                )
            }),
        StartupState::Live {
            home_id: reported, ..
        } => Err(anyhow::anyhow!(
            "lfd startup reached Home {reported}, not requested Home {home_id}"
        )),
        StartupState::Failed { reason } => {
            if live_endpoint_at(home_id, &endpoints).await.is_some() {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "failed to start lfd for Home {home_id}: {reason}"
                ))
            }
        }
    }
}

pub(crate) async fn start_waves(
    home_id: &HomeId,
    wave_ids: Vec<WaveId>,
) -> anyhow::Result<Vec<WaveStartOutcome>> {
    let client = live_endpoint(home_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("lfd is not running for Home {home_id}"))?;
    let response = reqwest::Client::new()
        .post(format!("http://{}/waves/start", client.endpoint))
        .bearer_auth(&client.token)
        .json(&StartWavesRequest { wave_ids })
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        response
            .json::<Vec<WaveStartOutcome>>()
            .await
            .map_err(anyhow::Error::from)
    } else {
        Err(anyhow::anyhow!(
            "lfd refused Wave start with HTTP {status}: {}",
            response.text().await.unwrap_or_default()
        ))
    }
}

pub(crate) async fn claim_pr_landing(
    home_id: &HomeId,
    landing_id: &PrLandingId,
    generation: u64,
) -> anyhow::Result<bool> {
    let Some(client) = live_endpoint(home_id).await else {
        return Ok(false);
    };
    let response = reqwest::Client::new()
        .post(format!("http://{}/landings/claim", client.endpoint))
        .bearer_auth(&client.token)
        .json(&ClaimLandingRequest {
            landing_id: landing_id.clone(),
            generation,
        })
        .timeout(LANDING_CLAIM_TIMEOUT)
        .send()
        .await?;
    match response.status() {
        StatusCode::ACCEPTED => Ok(true),
        StatusCode::CONFLICT | StatusCode::NOT_FOUND => Ok(false),
        status => Err(anyhow::anyhow!(
            "lfd refused landing claim with HTTP {status}: {}",
            response.text().await.unwrap_or_default()
        )),
    }
}

pub(crate) async fn reconcile_waves(
    home_id: &HomeId,
    wave_ids: Vec<WaveId>,
) -> anyhow::Result<Vec<WaveStartOutcome>> {
    let client = live_endpoint(home_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("lfd is not running for Home {home_id}"))?;
    let response = reqwest::Client::new()
        .post(format!("http://{}/waves/reconcile", client.endpoint))
        .bearer_auth(&client.token)
        .json(&StartWavesRequest { wave_ids })
        .send()
        .await?;
    let status = response.status();
    if status.is_success() {
        response
            .json::<Vec<WaveStartOutcome>>()
            .await
            .map_err(anyhow::Error::from)
    } else {
        Err(anyhow::anyhow!(
            "lfd refused Wave reconciliation with HTTP {status}: {}",
            response.text().await.unwrap_or_default()
        ))
    }
}

pub(crate) async fn stop_wave(home_id: &HomeId, wave_id: &WaveId) -> anyhow::Result<Option<bool>> {
    let Some(client) = live_endpoint(home_id).await else {
        return Ok(None);
    };
    let response = reqwest::Client::new()
        .post(format!("http://{}/waves/stop", client.endpoint))
        .bearer_auth(&client.token)
        .json(&StopWaveRequest {
            wave_id: wave_id.clone(),
        })
        .send()
        .await?;
    match response.status() {
        StatusCode::ACCEPTED => Ok(Some(true)),
        StatusCode::NO_CONTENT => Ok(Some(false)),
        status => Err(anyhow::anyhow!(
            "lfd refused Wave stop with HTTP {status}: {}",
            response.text().await.unwrap_or_default()
        )),
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LfdClientEndpoint {
    endpoint: String,
    token: String,
}

async fn live_endpoint(home_id: &HomeId) -> Option<LfdClientEndpoint> {
    live_endpoint_at(home_id, &endpoint_dir()).await
}

async fn live_endpoint_at(home_id: &HomeId, endpoints: &Path) -> Option<LfdClientEndpoint> {
    let client = read_endpoint_at(home_id, endpoints)?;
    let health = reqwest::Client::new()
        .get(format!("http://{}/health", client.endpoint))
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .ok()?
        .json::<LivenessHealthBody>()
        .await
        .ok()?;
    (health.home_id == *home_id && health.status == "ok").then_some(client)
}

pub(crate) async fn home_is_live(home_id: &HomeId) -> bool {
    live_endpoint(home_id).await.is_some()
}

pub(crate) async fn home_is_live_at(home_id: &HomeId, lf_home: &Path) -> bool {
    live_endpoint_at(home_id, &lf_home.join("lfd"))
        .await
        .is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HomeHealthIdentity {
    pub store: PathBuf,
    pub runtime_generation: u64,
    pub build_version: String,
    pub source_revision: String,
    pub migration_frontier: String,
}

pub(crate) async fn home_health_identity(home_id: &HomeId) -> Option<HomeHealthIdentity> {
    home_health_identity_at(home_id, &crate::store::lf_home_dir()).await
}

pub(crate) async fn home_health_identity_at(
    home_id: &HomeId,
    lf_home: &Path,
) -> Option<HomeHealthIdentity> {
    let client = read_endpoint_at(home_id, &lf_home.join("lfd"))?;
    let health = reqwest::Client::new()
        .get(format!("http://{}/health", client.endpoint))
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .ok()?
        .json::<HealthBody>()
        .await
        .ok()?;
    if health.home_id != *home_id || health.status != "ok" {
        return None;
    }
    Some(HomeHealthIdentity {
        store: PathBuf::from(health.store),
        runtime_generation: health.runtime_generation?,
        build_version: health.build_version?,
        source_revision: health.source_revision?,
        migration_frontier: health.migration_frontier?,
    })
}

fn endpoint_path(home_id: &HomeId) -> PathBuf {
    endpoint_path_at(home_id, &endpoint_dir())
}

fn endpoint_path_at(home_id: &HomeId, endpoints: &Path) -> PathBuf {
    endpoints.join(format!("{}.endpoint", home_id.as_str()))
}

fn endpoint_dir() -> PathBuf {
    crate::store::lf_home_dir().join("lfd")
}

fn lock_home(home_id: &HomeId) -> anyhow::Result<File> {
    let directory = endpoint_dir();
    std::fs::create_dir_all(&directory)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(format!("{}.lock", home_id.as_str())))?;
    file.try_lock_exclusive().map_err(|error| {
        anyhow::anyhow!("another lfd process already owns Home {home_id}: {error}")
    })?;
    Ok(file)
}

async fn lock_start_at(home_id: &HomeId, endpoints: &Path) -> anyhow::Result<File> {
    let path = endpoints.join(format!("{}.start.lock", home_id.as_str()));
    tokio::task::spawn_blocking(move || {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        file.lock_exclusive()?;
        Ok::<_, anyhow::Error>(file)
    })
    .await
    .map_err(|error| anyhow::anyhow!("lfd launch lock task failed: {error}"))?
}

async fn read_startup_signal(
    listener: tokio::net::UnixListener,
    receipt_path: &Path,
    attempt_id: &str,
) -> anyhow::Result<StartupReceipt> {
    listener.accept().await?;
    let bytes = std::fs::read(receipt_path)?;
    let receipt: StartupReceipt = serde_json::from_slice(&bytes)?;
    if receipt.attempt_id != attempt_id {
        anyhow::bail!(
            "lfd startup receipt belongs to attempt {}, not {attempt_id}",
            receipt.attempt_id
        );
    }
    Ok(receipt)
}

fn write_startup_receipt(path: &Path, receipt: &StartupReceipt) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("startup receipt path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(receipt)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn read_endpoint(home_id: &HomeId) -> Option<LfdClientEndpoint> {
    read_endpoint_at(home_id, &endpoint_dir())
}

fn read_endpoint_at(home_id: &HomeId, endpoints: &Path) -> Option<LfdClientEndpoint> {
    let bytes = std::fs::read(endpoint_path_at(home_id, endpoints)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_endpoint(home_id: &HomeId, client: &LfdClientEndpoint) -> anyhow::Result<()> {
    let path = endpoint_path(home_id);
    let parent = path
        .parent()
        .expect("an lfd endpoint always has a parent directory");
    std::fs::create_dir_all(parent)?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(&serde_json::to_vec(client)?)?;
    Ok(())
}

fn remove_endpoint(home_id: &HomeId, client: &LfdClientEndpoint) {
    let path = endpoint_path(home_id);
    let owned = read_endpoint(home_id).as_ref() == Some(client);
    if owned {
        let _ = std::fs::remove_file(path);
    }
}

/// Refuse a non-loopback bind unless the operator explicitly allows it.
fn ensure_bind_allowed(addr: SocketAddr, allow_non_loopback: Option<&OsStr>) -> anyhow::Result<()> {
    if addr.ip().is_loopback() || allow_non_loopback == Some(OsStr::new("1")) {
        return Ok(());
    }
    anyhow::bail!(
        "refusing non-loopback bind {addr}; bind 127.0.0.1 or set \
         LF_LFD_ALLOW_NON_LOOPBACK=1"
    )
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::{
        AdvanceReceipt, Containment, InvocationRoute, RunAdvance, RunTrigger, WorkRef, WorkStatus,
    };
    use crate::store::sqlite::SqliteStore;
    use crate::store::StorageConfig;
    use crate::wave::Wave;
    use crate::webhook::WebhookEvent;
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    use std::path::Path;

    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(body);
        hex::encode(mac.finalize().into_bytes())
    }

    async fn make_state(repo: &Path, store: Arc<Store>, linear: Option<LinearConfig>) -> LfdState {
        build_state(repo.to_path_buf(), store, linear, None, None)
            .await
            .unwrap()
    }

    async fn open_store(dir: &Path) -> Arc<Store> {
        Arc::new(
            crate::store::open_store(&StorageConfig::sqlite(dir.join("registry.db")))
                .await
                .unwrap(),
        )
    }

    fn status_truth_store(dir: &Path) -> (Arc<Store>, HomeId) {
        let path = dir.join("status-truth.db");
        let sqlite = SqliteStore::new(&path).unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        let status_truth_is_materialized = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type='table' AND name='run_liveness'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !status_truth_is_materialized {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "status_truth",
                ))
                .unwrap();
        }
        drop(connection);
        let home_id = sqlite.local_home().unwrap().id;
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO home_runtime_generations (
                    home_id, generation, build_version, source_revision,
                    migration_frontier, activated_at
                 ) VALUES (?1, 1, 'test', 'test', 'status_truth', ?2)",
                rusqlite::params![home_id.as_str(), OffsetDateTime::now_utc().unix_timestamp()],
            )
            .unwrap();
        (Arc::new(Store::from_sqlite_for_test(sqlite)), home_id)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reconcile_home_runs_startup_cycle_settles_gone_invocation() {
        let directory = tempfile::tempdir().unwrap();
        let (store, home_id) = status_truth_store(directory.path());
        let wave = Wave::new(
            WaveId::new(),
            "startup-reconcile".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let work = WorkRef::Wave(wave.id().clone());
        let (_, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        let mut process = std::process::Command::new("/usr/bin/true");
        process.process_group(0);
        let mut process = process.spawn().unwrap();
        let process_group = i64::from(process.id());
        assert!(process.wait().unwrap().success());
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::ProcessGroup { id: process_group },
                    cwd: directory.path().to_path_buf(),
                },
            )
            .await
            .unwrap();
        let invocation = store
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
            .unwrap();
        let AdvanceReceipt::Invocation(invocation) = invocation else {
            panic!("expected Invocation receipt")
        };
        let turn = store
            .advance_run(
                &lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let AdvanceReceipt::Turn(turn) = turn else {
            panic!("expected Turn receipt")
        };

        let socket_path = directory.path().join("startup.sock");
        let receipt_path = directory.path().join("startup.json");
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let signal = StartupSignal {
            attempt_id: "attempt-status-truth".to_string(),
            receipt_path: receipt_path.clone(),
            socket_path,
        };
        let client = LfdClientEndpoint {
            endpoint: "127.0.0.1:4567".to_string(),
            token: "test-token".to_string(),
        };
        let reconciliation_store = store.clone();
        let reconciliation_home = home_id.clone();
        let reconciliation_signal = signal.clone();
        let reconciliation_client = client.clone();
        let reconciliation = tokio::spawn(async move {
            reconcile_home_runs_before_startup_live(
                &reconciliation_store,
                &reconciliation_home,
                Some(&reconciliation_signal),
                &reconciliation_client,
            )
            .await
        });
        let receipt = read_startup_signal(listener, &receipt_path, &signal.attempt_id)
            .await
            .unwrap();

        assert_eq!(
            receipt,
            StartupReceipt {
                attempt_id: signal.attempt_id,
                state: StartupState::Live {
                    endpoint: client.endpoint,
                    home_id: home_id.clone(),
                },
            }
        );
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        let current = crate::child::observe_current_work(
            &store,
            &work,
            &WorkStatus::Ready,
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
        assert_eq!(current.state, crate::child::CurrentWorkState::Ready);
        assert_eq!(current.reason, "ready");
        assert!(current.liveness.is_none());
        assert!(store.invocation_surfaces(true).await.unwrap().is_empty());
        let summary = reconciliation.await.unwrap();

        assert_eq!(summary.absent, 1);
        assert_eq!(summary.errors, 0);
        let surface = store
            .invocation_surface(&invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            surface.current.state,
            crate::durable::InvocationObservationState::History
        );
        assert_eq!(
            surface.handback,
            Some(crate::durable::BoundaryState::Unknown)
        );
        let turn_status: String =
            rusqlite::Connection::open(directory.path().join("status-truth.db"))
                .unwrap()
                .query_row(
                    "SELECT status FROM agent_turns WHERE id=?1",
                    [turn.id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
        assert_eq!(turn_status, "partial");
    }

    async fn open_landing_store(dir: &Path) -> Arc<Store> {
        let path = dir.join("registry.db");
        drop(open_store(dir).await);
        let connection = rusqlite::Connection::open(&path).unwrap();
        let migrated = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='pr_landings')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !migrated {
            connection
                .execute_batch(&crate::store::migrations::migration_sql_for_test(
                    "pr_landings",
                ))
                .unwrap();
        }
        Arc::new(
            crate::store::open_store(&StorageConfig::sqlite(path))
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let repo = tempfile::tempdir().unwrap();
        let state = make_state(repo.path(), open_store(repo.path()).await, None).await;
        let home_id = state.wave_host.home_id().clone();
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let resp = reqwest::get(format!("http://{addr}/health")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
        assert_eq!(body["home_id"], home_id.as_str());
        assert!(body["store"].as_str().is_some());
        assert!(body["runtime_generation"].is_number());
        assert_eq!(body["build_version"], crate::build_info::BUILD_VERSION);
        assert_eq!(
            body["source_revision"],
            crate::build_info::source_revision()
        );
        assert_eq!(
            body["migration_frontier"],
            crate::store::migrations::latest_known_version()
        );
    }

    #[test]
    fn liveness_probe_accepts_health_from_a_pre_store_identity_daemon() {
        let home_id = HomeId::new();
        let body = serde_json::json!({
            "status": "ok",
            "home_id": home_id.as_str(),
            "runtime_generation": 1,
            "build_version": "0.12.8",
            "source_revision": "published",
            "migration_frontier": "0.12.8.001_release"
        });

        let health: LivenessHealthBody = serde_json::from_value(body).unwrap();

        assert_eq!(health.status, "ok");
        assert_eq!(health.home_id, home_id);
    }

    #[tokio::test]
    async fn home_recovery_never_steals_a_live_landing_and_fences_a_stale_one() {
        let repo = tempfile::tempdir().unwrap();
        let store = open_landing_store(repo.path()).await;
        let state = make_state(repo.path(), store.clone(), None).await;
        let now = OffsetDateTime::now_utc();
        let candidate = crate::pr_landing::PrLanding::new(
            crate::pr_landing::NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: repo.path().to_path_buf(),
                branch: "jack/watched-landing".to_string(),
                task_id: None,
                requested_head_sha: "head-a".to_string(),
                after_merge: None,
                next_slug: None,
            },
            now,
        )
        .unwrap();
        let landing = store.start_or_join_pr_landing(&candidate).await.unwrap();
        let local = store
            .claim_pr_landing(
                &landing.id,
                landing.generation,
                &LandingClaim {
                    placement: LandingPlacement::Local,
                    process_id: 41,
                    heartbeat_at: now,
                },
                now - SUPERVISOR_STALE_AFTER,
            )
            .await
            .unwrap()
            .unwrap();

        assert!(
            claim_recoverable_pr_landings(&state, now + time::Duration::minutes(1))
                .await
                .is_empty()
        );

        let recovered =
            claim_recoverable_pr_landings(&state, now + time::Duration::minutes(3)).await;
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].generation, local.generation + 1);
        assert_eq!(
            recovered[0]
                .supervisor
                .as_ref()
                .map(|owner| &owner.placement),
            Some(&LandingPlacement::Home {
                home_id: state.wave_host.home_id().clone(),
            })
        );
    }

    #[tokio::test]
    async fn wave_start_attempts_every_requested_wave() {
        let repo = tempfile::tempdir().unwrap();
        let state = make_state(repo.path(), open_store(repo.path()).await, None).await;
        let control_token = state.control_token.as_ref().clone();
        let first = WaveId::new();
        let second = WaveId::new();
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let unauthorized = reqwest::Client::new()
            .post(format!("http://{addr}/waves/start"))
            .json(&StartWavesRequest {
                wave_ids: vec![first.clone(), second.clone()],
            })
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/waves/start"))
            .bearer_auth(control_token)
            .json(&StartWavesRequest {
                wave_ids: vec![first.clone(), second.clone()],
            })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let outcomes = response.json::<Vec<WaveStartOutcome>>().await.unwrap();
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].wave_id, first);
        assert!(matches!(
            outcomes[0].state,
            crate::wave_host::WaveStartState::Failed { .. }
        ));
        assert_eq!(outcomes[1].wave_id, second);
        assert!(matches!(
            outcomes[1].state,
            crate::wave_host::WaveStartState::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn status_reports_zero_waves_and_deliveries_on_a_fresh_store() {
        let repo = tempfile::tempdir().unwrap();
        let state = make_state(repo.path(), open_store(repo.path()).await, None).await;
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let body: serde_json::Value = reqwest::get(format!("http://{addr}/status"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["waves"], 0);
        assert_eq!(body["deliveries"], 0);
    }

    #[tokio::test]
    async fn webhook_returns_503_when_linear_config_absent() {
        let repo = tempfile::tempdir().unwrap();
        let state = make_state(repo.path(), open_store(repo.path()).await, None).await;
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/linear/webhook"))
            .header(SIGNATURE_HEADER, "deadbeef")
            .body("test")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn webhook_rejects_unsigned_delivery_with_401() {
        let repo = tempfile::tempdir().unwrap();
        let linear = LinearConfig {
            secret: Arc::new(b"whsec_test".to_vec()),
            viewer_id: Arc::new("viewer-1".to_string()),
        };
        let state = make_state(repo.path(), open_store(repo.path()).await, Some(linear)).await;
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let resp = reqwest::Client::new()
            .post(format!("http://{addr}/linear/webhook"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn inbox_dedups_an_ignored_delivery_across_retries() {
        let repo = tempfile::tempdir().unwrap();
        let store = open_store(repo.path()).await;
        // An ignored event: an Issue update with no content change.
        let body = br#"{"action":"update","type":"Issue","data":{"id":"i-1","title":"T"},"updatedFrom":{"stateId":"s-1"},"webhookTimestamp":1700000000000}"#;
        // Stretch the replay window by patching the timestamp into the parsed
        // event rather than the live clock: derive the id directly and exercise
        // the store dedup, which is the gate under test.
        let (event, ts) = webhook::parse_event(body).unwrap();
        assert_eq!(event, WebhookEvent::Ignored);
        let delivery_id = derive_delivery_id(&event, ts, body);

        let first = store
            .record_delivery(
                delivery_id.clone(),
                "linear".to_string(),
                Some("ignored".to_string()),
                ts,
            )
            .await
            .unwrap();
        assert!(first.inserted);

        let second = store
            .record_delivery(
                delivery_id.clone(),
                "linear".to_string(),
                Some("ignored".to_string()),
                ts,
            )
            .await
            .unwrap();
        assert!(!second.inserted);
        assert_eq!(second.existing_status, Some(DeliveryStatus::Pending));

        // Stamp it ignored, then a third arrival is a true duplicate.
        store
            .complete_delivery(DeliveryCompletion {
                delivery_id: delivery_id.clone(),
                provider: "linear".to_string(),
                status: DeliveryStatus::Ignored,
                target_kind: None,
                target_id: None,
                outcome: Some(r#"{"outcome":"ignored"}"#.to_string()),
                processed_at: ts,
            })
            .await
            .unwrap();
        let third = store
            .record_delivery(
                delivery_id,
                "linear".to_string(),
                Some("ignored".to_string()),
                ts,
            )
            .await
            .unwrap();
        assert!(!third.inserted);
        assert_eq!(third.existing_status, Some(DeliveryStatus::Ignored));

        assert_eq!(store.delivery_count().await.unwrap(), 1);
    }

    #[test]
    fn delivery_id_is_unique_per_issue_revision_and_comment() {
        let edit_a = WebhookEvent::IssueEdit {
            issue_id: "i-1".into(),
            title: "T".into(),
            description: "D".into(),
            revision: "r-1".into(),
            actor_id: None,
        };
        let edit_b = WebhookEvent::IssueEdit {
            issue_id: "i-1".into(),
            title: "T2".into(),
            description: "D".into(),
            revision: "r-2".into(),
            actor_id: None,
        };
        assert_ne!(
            derive_delivery_id(&edit_a, 0, &[]),
            derive_delivery_id(&edit_b, 0, &[])
        );
        let comment = WebhookEvent::Comment {
            issue_id: "i-1".into(),
            comment_id: "c-1".into(),
            body: "hi".into(),
            author_id: None,
        };
        assert_eq!(derive_delivery_id(&comment, 0, &[]), "linear:comment:c-1");
    }

    #[test]
    fn ignored_delivery_id_is_stable_for_the_same_body() {
        let id_a = derive_delivery_id(&WebhookEvent::Ignored, 100, b"body-x");
        let id_b = derive_delivery_id(&WebhookEvent::Ignored, 100, b"body-x");
        let id_c = derive_delivery_id(&WebhookEvent::Ignored, 100, b"body-y");
        assert_eq!(id_a, id_b);
        assert_ne!(id_a, id_c);
    }

    #[test]
    fn map_outcome_routes_every_terminal_case() {
        assert_eq!(
            map_outcome(&WebhookOutcome::Ignored),
            (DeliveryStatus::Ignored, None)
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::NoTarget),
            (DeliveryStatus::NoTarget, None)
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::SelfAuthored),
            (DeliveryStatus::Processed, Some("task"))
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::Edit {
                steer_applied: false
            }),
            (DeliveryStatus::Processed, Some("task"))
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::Comment { delivered: true }),
            (DeliveryStatus::Processed, Some("task"))
        );
    }

    #[test]
    fn a_signed_ignored_delivery_round_trips_through_parse_and_dedup_key() {
        let secret = b"whsec_test";
        let body = br#"{"action":"update","type":"Issue","data":{"id":"i-1"},"updatedFrom":{"stateId":"s"},"webhookTimestamp":1}"#;
        let sig = sign(secret, body);
        assert!(webhook::verify_signature(secret, body, &sig).is_ok());
        let (event, ts) = webhook::parse_event(body).unwrap();
        assert_eq!(event, WebhookEvent::Ignored);
        let id = derive_delivery_id(&event, ts, body);
        assert!(id.starts_with("linear:ignored:1:"));
    }

    #[test]
    fn github_signature_requires_the_sha256_prefix_and_matching_body() {
        let secret = b"github-secret";
        let body = br#"{"action":"closed"}"#;
        let signature = format!("sha256={}", sign(secret, body));

        assert!(verify_github_signature(secret, body, &signature));
        assert!(!verify_github_signature(secret, b"changed", &signature));
        assert!(!verify_github_signature(secret, body, &sign(secret, body)));
    }

    #[test]
    fn github_events_select_merged_prs_and_deleted_branches() {
        let merged = br#"{
            "action":"closed",
            "repository":{"full_name":"acme/widgets"},
            "pull_request":{"merged":true,"head":{"ref":"user/landed"}}
        }"#;
        assert_eq!(
            parse_github_prune_event("pull_request", merged).unwrap(),
            Some(GithubPruneEvent {
                repo: "acme/widgets".to_string(),
                branch: "user/landed".to_string(),
                reason: WorktreePruneReason::Merged,
            })
        );

        let deleted = br#"{
            "ref":"user/abandoned",
            "ref_type":"branch",
            "repository":{"full_name":"acme/widgets"}
        }"#;
        assert_eq!(
            parse_github_prune_event("delete", deleted).unwrap(),
            Some(GithubPruneEvent {
                repo: "acme/widgets".to_string(),
                branch: "user/abandoned".to_string(),
                reason: WorktreePruneReason::RemoteGone,
            })
        );
    }

    #[test]
    fn github_events_ignore_unmerged_closures_and_tag_deletions() {
        let closed = br#"{
            "action":"closed",
            "repository":{"full_name":"acme/widgets"},
            "pull_request":{"merged":false,"head":{"ref":"user/open"}}
        }"#;
        assert_eq!(
            parse_github_prune_event("pull_request", closed).unwrap(),
            None
        );

        let tag = br#"{
            "ref":"v1.0.0",
            "ref_type":"tag",
            "repository":{"full_name":"acme/widgets"}
        }"#;
        assert_eq!(parse_github_prune_event("delete", tag).unwrap(), None);
    }

    #[tokio::test]
    async fn signed_github_merge_is_accepted_for_background_cleanup() {
        let repo = tempfile::tempdir().unwrap();
        let secret = b"github-secret";
        let body = br#"{
            "action":"closed",
            "repository":{"full_name":"acme/widgets"},
            "pull_request":{"merged":true,"head":{"ref":"user/landed"}}
        }"#;
        let mut state = make_state(repo.path(), open_store(repo.path()).await, None).await;
        state.github = Some(GithubConfig {
            secret: Arc::new(secret.to_vec()),
            webhook_url: None,
        });
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/github/webhook"))
            .header(GITHUB_EVENT_HEADER, "pull_request")
            .header(
                GITHUB_SIGNATURE_HEADER,
                format!("sha256={}", sign(secret, body)),
            )
            .body(body.as_slice().to_vec())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[test]
    fn bind_guard_requires_an_explicit_non_loopback_value() {
        let loopback: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(ensure_bind_allowed(loopback, None).is_ok());
        let off: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        assert!(ensure_bind_allowed(off, None).is_err());
        assert!(ensure_bind_allowed(off, Some(&OsString::new())).is_err());
        assert!(ensure_bind_allowed(off, Some(&OsString::from("true"))).is_err());
        assert!(ensure_bind_allowed(off, Some(&OsString::from("1"))).is_ok());
    }
}
