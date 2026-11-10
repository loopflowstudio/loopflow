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
    pub model: Option<String>,
    /// The login that received the rejected turn, retained after handoff.
    pub account: Option<ProviderAccountId>,
    pub state: RecoveryState,
    pub reason: String,
    pub retry_at: Option<i64>,
    pub request: Option<String>,
    pub retry_turn: Option<String>,
}

impl SessionRecovery {
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
pub(crate) fn codex_rejected_input(turn: &Value) -> Option<&Value> {
    if turn["status"] != "failed"
        || turn["itemsView"] != "full"
        || turn.pointer("/error/codexErrorInfo")?.as_str()? != "usageLimitExceeded"
    {
        return None;
    }
    let items = turn["items"].as_array()?;
    if items.len() != 1 || items[0]["type"] != "userMessage" {
        return None;
    }
    let content = &items[0]["content"];
    (!content.as_array()?.is_empty()).then_some(content)
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
    let errors: Vec<_> = rows[start + 1..]
        .iter()
        .filter(|row| row["type"] == "assistant")
        .collect();
    if errors.len() != 1 {
        return None;
    }
    let error = errors[0];
    if error["isApiErrorMessage"] != true
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
    let requests: Vec<_> = rows[start + 1..]
        .iter()
        .filter(|row| row["type"] == "api-request")
        .collect();
    if requests.len() != 1 {
        return None;
    }
    requests[0]
        .pointer("/params/model")?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{claude_rejected_model, codex_rejected_input, RecoveryState, SessionRecovery};
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use serde_json::{json, Value};

    #[test]
    fn only_a_complete_rejected_turn_can_be_resent() {
        let input = json!([{"type":"text","text":"do the work"}]);
        let turn = json!({"id":"failed", "status":"failed", "itemsView":"full",
            "error":{"codexErrorInfo":"usageLimitExceeded"},
            "items":[{"type":"userMessage","content":input}]});
        assert_eq!(codex_rejected_input(&turn), Some(&input));
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
            assert!(codex_rejected_input(&worked).is_none(), "{kind}");
        }
        for view in [json!("notLoaded"), Value::Null] {
            let mut partial = turn.clone();
            partial["itemsView"] = view;
            assert!(codex_rejected_input(&partial).is_none());
        }
        let mut empty = turn.clone();
        empty["items"] = json!([]);
        assert!(codex_rejected_input(&empty).is_none());
        let mut interrupted = turn;
        interrupted["status"] = json!("interrupted");
        assert!(codex_rejected_input(&interrupted).is_none());
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
        let newer = store
            .claim_session_attachment("reader", Some(&attachment), &process, false)
            .unwrap();
        assert!(store
            .with_session_attachment("reader", &attachment, || store
                .record_session_recovery("reader", &recovery, "stale"))
            .is_err());
        store
            .record_session_connection("reader", &newer, "fixture", &"new-thread".into())
            .unwrap();
        assert!(
            store.session_recovery("reader").unwrap().is_none(),
            "a new conversation never inherits an old recovery"
        );
    }
}
