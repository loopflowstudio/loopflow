//! Recovery belongs to one failed native turn. Session history owns dispatch;
//! reconnecting observers and Desktop requests cannot resend uncertain input.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::id::AgentSessionId;
use crate::store::ProviderAccountId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryState {
    WaitingCapacity,
    WaitingNative,
    Uncertain,
    Recovering,
    Continued,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionRecovery {
    pub agent_process_id: crate::id::AgentProcessId,
    pub thread: AgentSessionId,
    pub failed_turn: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    /// The login that received the rejected turn, retained after handoff.
    #[serde(deserialize_with = "Option::deserialize")]
    pub account: Option<ProviderAccountId>,
    pub state: RecoveryState,
    pub reason: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub retry_at: Option<i64>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub request: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub retry_turn: Option<String>,
}

impl SessionRecovery {
    /// Wait observations may change; dispatch and its correlated result cannot
    /// be undone by a worker that read the conversation before dispatch.
    pub(crate) fn observe(&mut self, phase: &str, mut observation: Self) {
        if phase == "dispatch" && self.request.is_none() && observation.request.is_some() {
            *self = observation;
            return;
        }
        if self.state == RecoveryState::Continued {
            return;
        }
        if let Some(request) = &self.request {
            if observation.request.as_ref() != Some(request)
                || !matches!(phase, "reply" | "unconfirmed" | "settled")
                || matches!(
                    observation.state,
                    RecoveryState::WaitingCapacity | RecoveryState::WaitingNative
                )
            {
                return;
            }
            match (&self.retry_turn, &observation.retry_turn) {
                (Some(turn), Some(other)) if turn != other => return,
                (Some(turn), None) => observation.retry_turn = Some(turn.clone()),
                _ => {}
            }
            if observation.state == RecoveryState::Continued && observation.retry_turn.is_none() {
                return;
            }
        } else if observation.request.is_some() || self.state == RecoveryState::Uncertain {
            return;
        }
        observation.model = observation.model.or_else(|| self.model.clone());
        observation.account = self.account.clone().or(observation.account);
        *self = observation;
    }

    pub(crate) fn can_retry(&self) -> bool {
        self.request.is_none()
            && matches!(
                self.state,
                RecoveryState::WaitingCapacity | RecoveryState::WaitingNative
            )
    }

    pub(crate) fn key(&self) -> String {
        format!("recovery:{}:{}", self.thread, self.failed_turn)
    }
}

/// Only complete native history can establish a rejection before work. Partial
/// turn notifications, absent items and unrecognized items retain uncertainty.
pub(crate) fn codex_rejected_turn(turn: &Value) -> bool {
    if turn["status"] != "failed"
        || turn["itemsView"] != "full"
        || turn["error"]["codexErrorInfo"] != "usageLimitExceeded"
    {
        return false;
    }
    let Some(items) = turn["items"].as_array() else {
        return false;
    };
    let [input] = items.as_slice() else {
        return false;
    };
    input["type"] == "userMessage"
        && input["content"]
            .as_array()
            .is_some_and(|content| !content.is_empty())
}

/// Follow Claude's native parent chain back to the rejected prompt. A synthetic
/// error alone says nothing about earlier tools; every intervening row matters.
pub(crate) fn claude_rejected_model(rows: &[Value], prompt: &str) -> Option<String> {
    let start = rows
        .iter()
        .rposition(|row| row["type"] == "user" && row["isSidechain"] == false)?;
    if rows[start]["promptId"] != prompt {
        return None;
    }
    let mut errors = rows[start + 1..]
        .iter()
        .filter(|row| row["type"] == "assistant");
    let error = errors.next()?;
    if errors.next().is_some()
        || error["isApiErrorMessage"] != true
        || error["failedResponseHadNoToolCall"] != true
        || error["error"] != "rate_limit"
        || error["isSidechain"] != false
    {
        return None;
    }
    let text = error
        .pointer("/message/content")?
        .as_array()?
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    crate::agent::subscription_limit_text(&text)?;
    let mut parent = error["parentUuid"].as_str()?;
    let mut visited = std::collections::HashSet::new();
    while parent != rows[start]["uuid"].as_str()? {
        if !visited.insert(parent) {
            return None;
        }
        let row = rows[start + 1..].iter().find(|row| row["uuid"] == parent)?;
        if row["type"] != "attachment" || row["isSidechain"] != false {
            return None;
        }
        parent = row["parentUuid"].as_str()?;
    }
    let mut requests = rows[start + 1..]
        .iter()
        .filter(|row| row["type"] == "api-request");
    let request = requests.next()?;
    if requests.next().is_some() {
        return None;
    }
    request
        .pointer("/params/model")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{claude_rejected_model, codex_rejected_turn, RecoveryState, SessionRecovery};
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use serde_json::{json, Value};

    #[test]
    fn only_complete_history_establishes_rejection_before_work() {
        let input = json!([{"type":"text","text":"do the work"}]);
        let turn = json!({"id":"failed", "status":"failed", "itemsView":"full",
            "error":{"codexErrorInfo":"usageLimitExceeded"},
            "items":[{"type":"userMessage","content":input}]});
        assert!(codex_rejected_turn(&turn));
        for kind in [
            "commandExecution",
            "fileChange",
            "reasoning",
            "agentMessage",
            "unknown",
        ] {
            let mut worked = turn.clone();
            worked["items"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":kind}));
            assert!(!codex_rejected_turn(&worked), "{kind}");
        }
        for view in [json!("notLoaded"), Value::Null] {
            let mut partial = turn.clone();
            partial["itemsView"] = view;
            assert!(!codex_rejected_turn(&partial));
        }
        let mut empty = turn.clone();
        empty["items"] = json!([]);
        assert!(!codex_rejected_turn(&empty));
        let mut interrupted = turn;
        interrupted["status"] = json!("interrupted");
        assert!(!codex_rejected_turn(&interrupted));
    }

    fn rejected_claude() -> Vec<Value> {
        vec![
            json!({"type":"user","isSidechain":false,"uuid":"user","promptId":"prompt"}),
            json!({"type":"attachment","isSidechain":false,"uuid":"context","parentUuid":"user"}),
            json!({"type":"api-request","params":{"model":"claude-sonnet-5-5"}}),
            json!({"type":"assistant","isSidechain":false,"uuid":"error","parentUuid":"context",
                "isApiErrorMessage":true,"failedResponseHadNoToolCall":true,"error":"rate_limit",
                "message":{"content":[{"type":"text","text":"You've hit your limit"}]}}),
        ]
    }

    #[test]
    fn claude_requires_the_entire_rejected_prompt_chain_and_its_actual_model() {
        let rows = rejected_claude();
        assert_eq!(
            claude_rejected_model(&rows, "prompt").as_deref(),
            Some("claude-sonnet-5-5")
        );
        let mut missing = rows.clone();
        missing.remove(1);
        assert!(claude_rejected_model(&missing, "prompt").is_none());
        let mut worked = rows.clone();
        worked.insert(
            1,
            json!({"type":"assistant","message":{"content":[{"type":"tool_use"}]}}),
        );
        assert!(claude_rejected_model(&worked, "prompt").is_none());
        let mut newer = rows.clone();
        newer.push(json!({"type":"user","isSidechain":false,"uuid":"new","promptId":"new"}));
        assert!(claude_rejected_model(&newer, "prompt").is_none());
        let mut no_model = rows;
        no_model[2]["params"] = json!({});
        assert!(claude_rejected_model(&no_model, "prompt").is_none());
    }

    #[test]
    fn lost_reply_reconnect_and_bulk_retry_cannot_dispatch_the_same_turn_twice() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("reader", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("reader", None, &process, true)
            .unwrap();
        store
            .record_session_connection("reader", &attachment, "fixture", &"thread".into())
            .unwrap();
        let mut recovery = SessionRecovery {
            agent_process_id: attachment.agent_process_id.clone(),
            thread: "thread".into(),
            failed_turn: "failed".into(),
            model: Some("model".into()),
            account: None,
            state: RecoveryState::WaitingCapacity,
            reason: "waiting".into(),
            retry_at: None,
            request: None,
            retry_turn: None,
        };
        store
            .record_session_recovery("reader", &recovery, "rejected")
            .unwrap();
        store.request_session_recovery("reader").unwrap();
        let requested = store.recovery_request_revision("reader").unwrap();
        assert!(requested > 0);
        recovery.request = Some("attempt".into());
        recovery.state = RecoveryState::Recovering;
        assert!(store
            .with_session_attachment("reader", &attachment, || store
                .record_session_recovery("reader", &recovery, "dispatch"))
            .unwrap());
        let mut late = recovery.clone();
        late.request = None;
        late.state = RecoveryState::WaitingCapacity;
        late.reason = "No capacity".into();
        late.retry_at = Some(1234);
        store
            .record_session_recovery("reader", &late, "wait:late")
            .unwrap();
        // Reopening the store models a disconnected reader, with no in-memory
        // worker state available to reconstruct dispatch.
        let reader = SqliteStore::new(&home.path().join("db")).unwrap();
        assert_eq!(
            reader.session_recovery("reader").unwrap(),
            Some(recovery.clone())
        );
        recovery.request = Some("reconnected-attempt".into());
        assert!(!store
            .record_session_recovery("reader", &recovery, "dispatch")
            .unwrap());
        store.request_session_recovery("reader").unwrap();
        assert_eq!(
            store.recovery_request_revision("reader").unwrap(),
            requested
        );
        assert_eq!(
            store
                .session_recovery("reader")
                .unwrap()
                .unwrap()
                .request
                .as_deref(),
            Some("attempt")
        );
        recovery.request = Some("attempt".into());
        recovery.state = RecoveryState::Uncertain;
        recovery.reason = "Reply lost".into();
        store
            .record_session_recovery("reader", &recovery, "unconfirmed")
            .unwrap();
        store
            .record_session_recovery("reader", &late, "wait:later")
            .unwrap();
        assert_eq!(
            reader.session_recovery("reader").unwrap(),
            Some(recovery.clone())
        );
        recovery.retry_turn = Some("native-retry".into());
        recovery.state = RecoveryState::Recovering;
        store
            .record_session_recovery("reader", &recovery, "reply")
            .unwrap();
        recovery.state = RecoveryState::Continued;
        recovery.reason = "The native retry completed".into();
        store
            .record_session_recovery("reader", &recovery, "settled")
            .unwrap();
        store
            .record_session_recovery("reader", &late, "wait:after-completion")
            .unwrap();
        assert_eq!(
            reader.session_recovery("reader").unwrap(),
            Some(recovery.clone())
        );

        // A new rejection owns its own attempt. An old worker finishing later
        // must not make it disappear or prevent a capacity recheck.
        late.failed_turn = "second-failure".into();
        store
            .record_session_recovery("reader", &late, "rejected")
            .unwrap();
        store
            .record_session_recovery("reader", &recovery, "wait:old-worker")
            .unwrap();
        assert_eq!(
            reader.session_recovery("reader").unwrap(),
            Some(late.clone())
        );
        store.request_session_recovery("reader").unwrap();
        assert!(store.recovery_request_revision("reader").unwrap() > requested);
        let newer = store
            .claim_session_attachment("reader", Some(&attachment), &process, true)
            .unwrap();
        assert!(store
            .with_session_attachment("reader", &attachment, || store
                .record_session_recovery("reader", &recovery, "stale"))
            .is_err());
        store
            .record_session_connection("reader", &newer, "fixture", &"thread".into())
            .unwrap();
        assert_eq!(store.session_recovery("reader").unwrap(), Some(late));
        store
            .record_session_connection("reader", &newer, "fixture", &"new-thread".into())
            .unwrap();
        assert!(
            store.session_recovery("reader").unwrap().is_none(),
            "a new conversation never inherits an old recovery"
        );
    }
}
