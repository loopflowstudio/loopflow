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
use crate::id::WaveId;
use crate::repository::RepoId;
use crate::store::provider_deliveries::{DeliveryCompletion, DeliveryEventKind, DeliveryStatus};
use crate::store::Store;
use crate::wave_host::WaveHost;
use crate::webhook::{self, WebhookEvent, WebhookOutcome, SIGNATURE_HEADER};

/// Body limit on webhook routes. Linear deliveries are small; a hard cap keeps
/// a malformed or hostile request from buffering unbounded bytes.
const WEBHOOK_BODY_LIMIT: usize = 256 * 1024;
const GITHUB_SIGNATURE_HEADER: &str = "x-hub-signature-256";
const GITHUB_EVENT_HEADER: &str = "x-github-event";
const ABANDONED_LOG_AGE: Duration = Duration::from_secs(24 * 60 * 60);

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
    /// The machine-local host for Wave listener tasks.
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
        .route("/waves/stop", post(stop_wave_handler))
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
}

async fn health_handler(State(state): State<LfdState>) -> Json<HealthBody> {
    Json(HealthBody {
        status: "ok".to_string(),
        home_id: state.wave_host.home_id().clone(),
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
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_wave_control(&state, &headers)?;
    state
        .wave_host
        .start_waves(request.wave_ids)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
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
    let wave_host = state.wave_host.clone();
    let reconciliation = tokio::spawn({
        let wave_host = wave_host.clone();
        async move { wave_host.reconcile_forever().await }
    });
    let autoprune = load_config_or_default(Some(&repo_root)).autoprune;
    if autoprune.enabled {
        let maintenance_state = state.clone();
        let interval = Duration::from_secs(autoprune.poll_interval_seconds.max(60));
        tokio::spawn(async move { maintenance_loop(maintenance_state, interval).await });
    }
    let subscription_state = state.clone();
    tokio::spawn(async move { ensure_github_subscriptions(&subscription_state).await });
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let client = LfdClientEndpoint {
        endpoint: local_client_endpoint(bound),
        token: state.control_token.as_ref().clone(),
    };
    write_endpoint(wave_host.home_id(), &client)?;
    tracing::info!(addr = %bound, home_id = %wave_host.home_id(), "lfd serving");
    let result = axum::serve(listener, router(state).into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await;
    reconciliation.abort();
    let _ = reconciliation.await;
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
    if live_endpoint(home_id).await.is_some() {
        return Ok(());
    }
    let argv = vec![
        crate::engine::process::resolve_lfd_binary()
            .to_string_lossy()
            .to_string(),
        "serve".to_string(),
        "--addr".to_string(),
        "127.0.0.1:0".to_string(),
        "--repo".to_string(),
        repo.display().to_string(),
    ];
    let launch = crate::engine::process::start_lf_session(
        &format!(
            "lfd-{}",
            crate::engine::process::tmux_session_slug(home_id.as_str())
        ),
        repo,
        &argv,
    )
    .await;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if live_endpoint(home_id).await.is_some() {
            return Ok(());
        }
    }
    match launch {
        Ok(()) => Err(anyhow::anyhow!(
            "lfd started for Home {home_id} but did not publish a live endpoint"
        )),
        Err(error) => Err(anyhow::anyhow!(
            "failed to start lfd for Home {home_id}: {error}"
        )),
    }
}

pub(crate) async fn start_waves(home_id: &HomeId, wave_ids: Vec<WaveId>) -> anyhow::Result<()> {
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
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "lfd refused Wave start with HTTP {status}: {}",
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
    let client = read_endpoint(home_id)?;
    let health = reqwest::Client::new()
        .get(format!("http://{}/health", client.endpoint))
        .timeout(Duration::from_millis(500))
        .send()
        .await
        .ok()?
        .json::<HealthBody>()
        .await
        .ok()?;
    (health.home_id == *home_id && health.status == "ok").then_some(client)
}

fn endpoint_path(home_id: &HomeId) -> PathBuf {
    endpoint_dir().join(format!("{}.endpoint", home_id.as_str()))
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

fn read_endpoint(home_id: &HomeId) -> Option<LfdClientEndpoint> {
    let bytes = std::fs::read(endpoint_path(home_id)).ok()?;
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
    use crate::store::StorageConfig;
    use crate::webhook::WebhookEvent;
    use std::ffi::OsString;
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
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let message = response.text().await.unwrap();
        assert!(message.contains(first.as_str()));
        assert!(message.contains(second.as_str()));
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
