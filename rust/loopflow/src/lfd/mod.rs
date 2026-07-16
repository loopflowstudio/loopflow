//! `lfd` — the machine-level Home daemon: durable webhook ingress + liveness.
//!
//! `lfd` is the one process that must always be running on a Home machine: it
//! receives external HTTP that cannot be SSH/`lf` (webhook deliveries) and
//! serves liveness probes. It is *not* an API — reads become `lf` queries;
//! hands become `lf` directly.
//!
//! The ingress path is a durable delivery inbox: each signed Linear delivery is
//! persisted to `provider_deliveries` *before* it is acknowledged, deduplicated
//! by `(delivery_id, provider)`, and routed to the owning Task Session through
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
//!   │ /linear/webhook  → verify → inbox → ingest     │
//!   └────────────────────────────────────────────────┘
//! ```

pub mod service;

use std::ffi::OsStr;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::store::provider_deliveries::{DeliveryCompletion, DeliveryEventKind, DeliveryStatus};
use crate::store::Store;
use crate::webhook::{self, WebhookEvent, WebhookOutcome, SIGNATURE_HEADER};

/// Body limit on webhook routes. Linear deliveries are small; a hard cap keeps
/// a malformed or hostile request from buffering unbounded bytes.
const WEBHOOK_BODY_LIMIT: usize = 256 * 1024;

/// Everything the `lfd` receiver needs, shared across requests.
#[derive(Clone)]
pub struct LfdState {
    /// Root directory containing `wave/*/` endpoint files, scanned per
    /// `/status` request (never cached).
    repo_root: PathBuf,
    /// The durable store — always open; the delivery inbox lives here.
    store: Arc<Store>,
    /// Linear webhook config. When absent, `/linear/webhook` returns 503.
    linear: Option<LinearConfig>,
}

/// Linear webhook verification + ingestion config, sourced from env.
#[derive(Clone)]
pub struct LinearConfig {
    pub secret: Arc<Vec<u8>>,
    pub viewer_id: Arc<String>,
}

/// Build the `lfd` router.
pub fn router(state: LfdState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route(
            "/linear/webhook",
            post(webhook_handler).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
        )
        .with_state(state)
}

// -- Handlers ----------------------------------------------------------------

#[derive(Serialize)]
struct HealthBody {
    status: &'static str,
}

async fn health_handler() -> Json<HealthBody> {
    Json(HealthBody { status: "ok" })
}

#[derive(Serialize)]
struct StatusBody {
    waves: usize,
    deliveries: i64,
}

async fn status_handler(State(state): State<LfdState>) -> Json<StatusBody> {
    let waves = scan_wave_endpoints(&state.repo_root).len();
    let deliveries = state.store.delivery_count().await.unwrap_or(0);
    Json(StatusBody { waves, deliveries })
}

/// Receive a signed Linear delivery, persist it to the durable inbox, and route
/// it to the owning Task Session. Idempotent across retries and restarts: a
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

/// Map a processing outcome onto the delivery row's terminal status and the
/// target kind it routed to. `Ignored` and `NoTarget` carry no target; a
/// self-authored or applied edit/comment resolved a Task Session.
fn map_outcome(outcome: &WebhookOutcome) -> (DeliveryStatus, Option<&'static str>) {
    match outcome {
        WebhookOutcome::Ignored => (DeliveryStatus::Ignored, None),
        WebhookOutcome::NoTarget => (DeliveryStatus::NoTarget, None),
        WebhookOutcome::SelfAuthored => (DeliveryStatus::Processed, Some("task_session")),
        WebhookOutcome::Edit { .. } => (DeliveryStatus::Processed, Some("task_session")),
        WebhookOutcome::Comment { .. } => (DeliveryStatus::Processed, Some("task_session")),
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
    directive_applied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered: Option<bool>,
}

impl<'a> From<&'a WebhookOutcome> for OutcomeSummary<'a> {
    fn from(outcome: &'a WebhookOutcome) -> Self {
        match outcome {
            WebhookOutcome::Ignored => Self {
                outcome: "ignored",
                directive_applied: None,
                delivered: None,
            },
            WebhookOutcome::NoTarget => Self {
                outcome: "no_target",
                directive_applied: None,
                delivered: None,
            },
            WebhookOutcome::SelfAuthored => Self {
                outcome: "self_authored",
                directive_applied: None,
                delivered: None,
            },
            WebhookOutcome::Edit { directive_applied } => Self {
                outcome: "edit",
                directive_applied: Some(*directive_applied),
                delivered: None,
            },
            WebhookOutcome::Comment { delivered } => Self {
                outcome: "comment",
                directive_applied: None,
                delivered: Some(*delivered),
            },
        }
    }
}

// -- Endpoint discovery ------------------------------------------------------

/// Scan `wave/*/` for `.wave-endpoint` files. `/status` reports the count; the
/// scan runs per request and is never cached.
fn scan_wave_endpoints(repo_root: &Path) -> Vec<String> {
    let wave_dir = repo_root.join("wave");
    let Ok(entries) = std::fs::read_dir(&wave_dir) else {
        return Vec::new();
    };
    let mut endpoints = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let endpoint_file = entry.path().join(crate::wave::server::ENDPOINT_FILE);
        if let Ok(addr) = std::fs::read_to_string(&endpoint_file) {
            let addr = addr.trim();
            if !addr.is_empty() {
                endpoints.push(addr.to_string());
            }
        }
    }
    endpoints
}

// -- Serve -------------------------------------------------------------------

/// Bind and serve `lfd` until the process ends. The store is always open (the
/// inbox lives there); Linear config is optional — when absent the daemon runs
/// but `/linear/webhook` returns 503.
///
/// A non-loopback bind is refused unless `LF_LFD_AUTH_TOKEN` is set, matching
/// the Home-only posture: the daemon is meant to receive external HTTP on a
/// machine that also runs `lf`, not to be exposed.
pub async fn serve(
    repo_root: PathBuf,
    addr: SocketAddr,
    store: Arc<Store>,
    linear: Option<LinearConfig>,
) -> anyhow::Result<()> {
    ensure_loopback_or_token(addr, std::env::var_os("LF_LFD_AUTH_TOKEN").as_deref())?;
    if !addr.ip().is_loopback() {
        tracing::warn!(
            %addr,
            "lfd bound off loopback with LF_LFD_AUTH_TOKEN set; \
             ensure the token gates access at the network boundary"
        );
    }
    let state = LfdState {
        repo_root,
        store,
        linear,
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "lfd serving");
    axum::serve(listener, router(state).into_make_service()).await?;
    Ok(())
}

/// Refuse a non-loopback bind unless `LF_LFD_AUTH_TOKEN` is present. The daemon
/// is meant to receive external HTTP on a Home machine, not to be exposed; the
/// token is the opt-in for an operator who has gated the network boundary.
fn ensure_loopback_or_token(addr: SocketAddr, auth_token: Option<&OsStr>) -> anyhow::Result<()> {
    if addr.ip().is_loopback() || auth_token.is_some() {
        return Ok(());
    }
    anyhow::bail!(
        "refusing non-loopback bind {addr} without LF_LFD_AUTH_TOKEN; \
         bind 127.0.0.1 or set LF_LFD_AUTH_TOKEN to opt in"
    )
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StorageConfig;
    use crate::webhook::WebhookEvent;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::ffi::OsString;
    use std::path::Path;

    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(body);
        hex::encode(mac.finalize().into_bytes())
    }

    fn make_state(repo: &Path, store: Arc<Store>, linear: Option<LinearConfig>) -> LfdState {
        LfdState {
            repo_root: repo.to_path_buf(),
            store,
            linear,
        }
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
        let state = make_state(repo.path(), open_store(repo.path()).await, None);
        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let resp = reqwest::get(format!("http://{addr}/health")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn status_reports_zero_waves_and_deliveries_on_a_fresh_store() {
        let repo = tempfile::tempdir().unwrap();
        let state = make_state(repo.path(), open_store(repo.path()).await, None);
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
        let state = make_state(repo.path(), open_store(repo.path()).await, None);
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
        let state = make_state(repo.path(), open_store(repo.path()).await, Some(linear));
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
            (DeliveryStatus::Processed, Some("task_session"))
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::Edit {
                directive_applied: false
            }),
            (DeliveryStatus::Processed, Some("task_session"))
        );
        assert_eq!(
            map_outcome(&WebhookOutcome::Comment { delivered: true }),
            (DeliveryStatus::Processed, Some("task_session"))
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
    fn bind_guard_allows_loopback_without_token_and_refuses_off_loopback() {
        let loopback: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(ensure_loopback_or_token(loopback, None).is_ok());
        let off: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        assert!(ensure_loopback_or_token(off, None).is_err());
        assert!(ensure_loopback_or_token(off, Some(&OsString::from("tok"))).is_ok());
    }
}
