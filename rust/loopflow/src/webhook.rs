//! Verified Linear webhook ingestion into Task control.
//!
//! Linear pushes each issue/comment change to a Loopflow receiver. This module
//! is the receiver's brain: verify the signature, parse the event, and map it
//! onto the durable Task input spine — a title/description edit or a human
//! comment becomes an ordered Steer. Exactly-once lives in the store
//! ([`crate::ops::linear_observe`] + the `task_linear_*` tables); this module
//! never bypasses them, so a redelivered or out-of-order webhook is a no-op.
//!
//! The HTTP glue ([`router`]/[`serve`]) is intentionally thin — all judgement is
//! in [`verify_signature`], [`parse_event`], and [`ingest_event`], which are
//! pure of the socket and unit-tested.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use time::OffsetDateTime;

use crate::ops::linear_observe::{linear_follow_up_text, reconcile_linear_observation};
use crate::pm::IssueObservation;
use crate::store::{Store, StoreError};

type HmacSha256 = Hmac<Sha256>;

/// Linear signs each delivery with the hex HMAC-SHA256 of the raw body in this
/// header, keyed by the webhook signing secret.
pub const SIGNATURE_HEADER: &str = "linear-signature";

/// How far the body's `webhookTimestamp` may sit from now before the delivery is
/// rejected as a replay. Linear recommends one minute.
pub const REPLAY_TOLERANCE_MS: i64 = 60_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WebhookError {
    #[error("signature is not valid hex")]
    MalformedSignature,
    #[error("signature does not match the body")]
    BadSignature,
    #[error("payload is not valid JSON: {0}")]
    Malformed(String),
}

/// Verify the hex HMAC-SHA256 signature over the raw request body. The compare is
/// constant-time (`Mac::verify_slice`), so a mismatch leaks no timing signal.
pub fn verify_signature(
    secret: &[u8],
    raw_body: &[u8],
    signature_hex: &str,
) -> Result<(), WebhookError> {
    let expected =
        hex::decode(signature_hex.trim()).map_err(|_| WebhookError::MalformedSignature)?;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(raw_body);
    mac.verify_slice(&expected)
        .map_err(|_| WebhookError::BadSignature)
}

/// True when a delivery's `webhookTimestamp` (ms) is close enough to now to not
/// be a replay of an old, captured request.
pub fn within_replay_window(webhook_timestamp_ms: i64, now: OffsetDateTime) -> bool {
    let now_ms = now.unix_timestamp_nanos() / 1_000_000;
    (i128::from(webhook_timestamp_ms) - now_ms).unsigned_abs() <= REPLAY_TOLERANCE_MS as u128
}

/// The only Linear changes that carry Task direction. Everything else — status,
/// assignment, labels, removals — decodes to `Ignored`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookEvent {
    /// A title and/or description edit. `revision` is the issue `updatedAt`.
    IssueEdit {
        issue_id: String,
        title: String,
        description: String,
        revision: String,
        actor_id: Option<String>,
    },
    /// A new comment. `author_id` distinguishes a human from Loopflow's own
    /// writeback and from an integration/bot with no backing user.
    Comment {
        issue_id: String,
        comment_id: String,
        body: String,
        author_id: Option<String>,
    },
    Ignored,
}

#[derive(Deserialize)]
struct RawWebhook {
    #[serde(default)]
    action: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    data: serde_json::Value,
    #[serde(rename = "updatedFrom", default)]
    updated_from: serde_json::Value,
    #[serde(default)]
    actor: serde_json::Value,
    #[serde(rename = "webhookTimestamp", default)]
    webhook_timestamp: i64,
}

fn nested_str(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(key)?;
    }
    cursor.as_str().map(str::to_string)
}

impl RawWebhook {
    fn classify(&self) -> WebhookEvent {
        match (self.kind.as_str(), self.action.as_str()) {
            ("Issue", "update") => {
                // `updatedFrom` names exactly the fields that changed, so a
                // metadata-only edit (status, assignee, labels) carries neither
                // title nor description here and is ignored.
                let content_changed = self.updated_from.get("title").is_some()
                    || self.updated_from.get("description").is_some();
                match (content_changed, nested_str(&self.data, &["id"])) {
                    (true, Some(issue_id)) => WebhookEvent::IssueEdit {
                        issue_id,
                        title: nested_str(&self.data, &["title"]).unwrap_or_default(),
                        description: nested_str(&self.data, &["description"]).unwrap_or_default(),
                        revision: nested_str(&self.data, &["updatedAt"]).unwrap_or_default(),
                        actor_id: nested_str(&self.actor, &["id"]),
                    },
                    _ => WebhookEvent::Ignored,
                }
            }
            ("Comment", "create") => {
                match (
                    nested_str(&self.data, &["id"]),
                    nested_str(&self.data, &["issue", "id"]),
                ) {
                    (Some(comment_id), Some(issue_id)) => WebhookEvent::Comment {
                        issue_id,
                        comment_id,
                        body: nested_str(&self.data, &["body"]).unwrap_or_default(),
                        author_id: nested_str(&self.data, &["user", "id"])
                            .or_else(|| nested_str(&self.actor, &["id"])),
                    },
                    _ => WebhookEvent::Ignored,
                }
            }
            _ => WebhookEvent::Ignored,
        }
    }
}

/// Parse a raw body into a typed event plus its `webhookTimestamp` (ms).
pub fn parse_event(raw_body: &[u8]) -> Result<(WebhookEvent, i64), WebhookError> {
    let raw: RawWebhook = serde_json::from_slice(raw_body)
        .map_err(|error| WebhookError::Malformed(error.to_string()))?;
    Ok((raw.classify(), raw.webhook_timestamp))
}

/// What one verified webhook did — enough for the receiver's log and response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookOutcome {
    /// Not an event that carries direction (metadata edit, removal, unknown).
    Ignored,
    /// A real event on an issue that has no Task — nothing to steer.
    NoTarget,
    /// Loopflow's own edit/comment, skipped to avoid a feedback loop.
    SelfAuthored,
    /// A title/description edit; `steer_applied` is false for a duplicate.
    Edit { steer_applied: bool },
    /// A human comment; `delivered` is false for a duplicate delivery.
    Comment { delivered: bool },
}

fn is_human(author_id: Option<&str>, viewer_id: &str) -> bool {
    author_id.is_some_and(|id| id != viewer_id)
}

/// Map one verified, parsed event onto the durable Task control substrate.
/// Resolves the target Task by Linear issue id; a missing Task or a
/// self-authored change writes nothing.
pub async fn ingest_event(
    store: &Store,
    event: WebhookEvent,
    viewer_id: &str,
    now: OffsetDateTime,
) -> Result<WebhookOutcome, StoreError> {
    let issue_id = match &event {
        WebhookEvent::IssueEdit { issue_id, .. } | WebhookEvent::Comment { issue_id, .. } => {
            issue_id.clone()
        }
        WebhookEvent::Ignored => return Ok(WebhookOutcome::Ignored),
    };
    let Some(task) = store.get_task_by_issue(&issue_id).await? else {
        return Ok(WebhookOutcome::NoTarget);
    };
    match event {
        WebhookEvent::IssueEdit {
            title,
            description,
            revision,
            actor_id,
            ..
        } => {
            if actor_id.as_deref() == Some(viewer_id) {
                return Ok(WebhookOutcome::SelfAuthored);
            }
            let observation = IssueObservation {
                revision,
                title,
                description,
                comments: vec![],
            };
            let outcome =
                reconcile_linear_observation(store, &task, observation, viewer_id, now).await?;
            Ok(WebhookOutcome::Edit {
                steer_applied: outcome.content_steer_applied,
            })
        }
        WebhookEvent::Comment {
            comment_id,
            body,
            author_id,
            ..
        } => {
            if !is_human(author_id.as_deref(), viewer_id) {
                return Ok(WebhookOutcome::SelfAuthored);
            }
            let text = linear_follow_up_text(&body);
            let created = store
                .apply_linear_comment(&task.id, comment_id, text, now)
                .await?;
            Ok(WebhookOutcome::Comment {
                delivered: created.is_some(),
            })
        }
        WebhookEvent::Ignored => Ok(WebhookOutcome::Ignored),
    }
}

/// Everything the receiver route needs, shared across requests.
#[derive(Clone)]
pub struct WebhookState {
    pub store: Arc<Store>,
    pub secret: Arc<Vec<u8>>,
    pub viewer_id: Arc<String>,
}

/// The receiver router: one signed `POST /linear/webhook`.
pub fn router(state: WebhookState) -> Router {
    Router::new()
        .route("/linear/webhook", post(handle))
        .with_state(state)
}

async fn handle(State(state): State<WebhookState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Some(signature) = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok()) else {
        return StatusCode::UNAUTHORIZED;
    };
    if verify_signature(&state.secret, &body, signature).is_err() {
        return StatusCode::UNAUTHORIZED;
    }
    let (event, webhook_timestamp) = match parse_event(&body) {
        Ok(parsed) => parsed,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let now = OffsetDateTime::now_utc();
    if !within_replay_window(webhook_timestamp, now) {
        return StatusCode::UNAUTHORIZED;
    }
    match ingest_event(&state.store, event, &state.viewer_id, now).await {
        // A local store error is ours, not Linear's — 500 asks it to retry.
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        Ok(_) => StatusCode::OK,
    }
}

/// Bind the receiver to `addr` and serve until the process ends. Callers own
/// the secret (from Doppler) and the Loopflow viewer id.
pub async fn serve(
    store: Arc<Store>,
    secret: Vec<u8>,
    viewer_id: String,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let state = WebhookState {
        store,
        secret: Arc::new(secret),
        viewer_id: Arc::new(viewer_id),
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state).into_make_service()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_event, verify_signature, within_replay_window, WebhookEvent};
    use hmac::{Hmac, Mac};
    use serde_json::json;
    use sha2::Sha256;
    use time::OffsetDateTime;

    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(body);
        hex::encode(mac.finalize().into_bytes())
    }

    #[test]
    fn a_correct_signature_verifies_and_a_tampered_body_does_not() {
        let secret = b"whsec_example";
        let body = br#"{"action":"update","type":"Issue"}"#;
        let signature = sign(secret, body);
        assert!(verify_signature(secret, body, &signature).is_ok());
        assert!(verify_signature(secret, b"tampered", &signature).is_err());
        assert!(verify_signature(b"wrong-secret", body, &signature).is_err());
        assert!(verify_signature(secret, body, "not-hex-zz").is_err());
    }

    #[test]
    fn replay_window_rejects_a_stale_timestamp() {
        let now = OffsetDateTime::from_unix_timestamp(1_784_160_000).unwrap();
        let now_ms = now.unix_timestamp() * 1000;
        assert!(within_replay_window(now_ms, now));
        assert!(within_replay_window(now_ms - 30_000, now));
        assert!(!within_replay_window(now_ms - 120_000, now));
    }

    #[test]
    fn an_issue_content_edit_parses_but_a_metadata_edit_is_ignored() {
        let edit = json!({
            "action": "update",
            "type": "Issue",
            "data": { "id": "issue-1", "title": "New", "description": "Body", "updatedAt": "2026-07-15T01:00:00.000Z" },
            "updatedFrom": { "title": "Old" },
            "actor": { "id": "user-human" },
            "webhookTimestamp": 1_784_160_000_000i64
        });
        let (event, ts) = parse_event(edit.to_string().as_bytes()).unwrap();
        assert_eq!(ts, 1_784_160_000_000);
        assert_eq!(
            event,
            WebhookEvent::IssueEdit {
                issue_id: "issue-1".into(),
                title: "New".into(),
                description: "Body".into(),
                revision: "2026-07-15T01:00:00.000Z".into(),
                actor_id: Some("user-human".into()),
            }
        );

        let metadata = json!({
            "action": "update",
            "type": "Issue",
            "data": { "id": "issue-1", "title": "New" },
            "updatedFrom": { "stateId": "s-1" },
            "webhookTimestamp": 1i64
        });
        let (event, _) = parse_event(metadata.to_string().as_bytes()).unwrap();
        assert_eq!(event, WebhookEvent::Ignored);
    }

    #[test]
    fn a_comment_create_parses_with_its_author() {
        let comment = json!({
            "action": "create",
            "type": "Comment",
            "data": { "id": "c-1", "body": "please prioritize", "issue": { "id": "issue-1" }, "user": { "id": "user-human" } },
            "webhookTimestamp": 2i64
        });
        let (event, _) = parse_event(comment.to_string().as_bytes()).unwrap();
        assert_eq!(
            event,
            WebhookEvent::Comment {
                issue_id: "issue-1".into(),
                comment_id: "c-1".into(),
                body: "please prioritize".into(),
                author_id: Some("user-human".into()),
            }
        );
    }
}
