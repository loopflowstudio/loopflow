//! Event types for WebSocket streaming.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;
use crate::lfd::provider_auth::Provider;
use crate::lfd::types::agent::AgentStatus;
use crate::lfd::types::ActivationSource;

/// Event payload variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    // Connection
    Connected {
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    Ping,

    // Provider auth
    #[serde(rename = "auth.flow_started")]
    AuthFlowStarted {
        provider: Provider,
        verification_uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        verification_uri_complete: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.connected")]
    AuthConnected {
        provider: Provider,
        #[serde(skip_serializing_if = "Option::is_none")]
        login: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.failed")]
    AuthFailed {
        provider: Provider,
        error: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.disconnected")]
    AuthDisconnected {
        provider: Provider,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.token_refreshed")]
    AuthTokenRefreshed {
        provider: Provider,
        #[serde(skip_serializing_if = "Option::is_none")]
        login: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.refresh_failed")]
    AuthRefreshFailed {
        provider: Provider,
        reason: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    #[serde(rename = "auth.refresh_required")]
    AuthRefreshRequired {
        provider: Provider,
        reason: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Wave lifecycle
    WaveCreated {
        wave_id: LfdId,
        name: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveUpdated {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveDeleted {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveStarted {
        wave_id: LfdId,
        wave_run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveStopped {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveWaiting {
        wave_id: LfdId,
        wave_run_id: LfdId,
        step: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<LfdId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        initial_user_message: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    CiFailure {
        wave_id: LfdId,
        wave_run_id: LfdId,
        pr_number: u32,
        branch: String,
        commit_sha: String,
        check_name: String,
        logs_url: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    ActivationQueued {
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    ActivationCoalesced {
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    ActivationDropped {
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Worktree
    WorktreeUpdated {
        worktree: String,
        repo: String,
        branch: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Agent
    AgentStarted {
        agent_id: LfdId,
        step: String,
        worktree: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    AgentEnded {
        agent_id: LfdId,
        status: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Output
    OutputLine {
        wave_id: LfdId,
        agent_id: LfdId,
        text: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
}

impl Event {
    pub fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    pub fn wave_created(wave_id: LfdId, name: String) -> Self {
        Self::WaveCreated {
            wave_id,
            name,
            timestamp: Self::now(),
        }
    }

    pub fn auth_flow_started(
        provider: Provider,
        verification_uri: String,
        verification_uri_complete: Option<String>,
    ) -> Self {
        Self::AuthFlowStarted {
            provider,
            verification_uri,
            verification_uri_complete,
            timestamp: Self::now(),
        }
    }

    pub fn auth_connected(provider: Provider, login: Option<String>) -> Self {
        Self::AuthConnected {
            provider,
            login,
            timestamp: Self::now(),
        }
    }

    pub fn auth_failed(provider: Provider, error: String) -> Self {
        Self::AuthFailed {
            provider,
            error,
            timestamp: Self::now(),
        }
    }

    pub fn auth_disconnected(provider: Provider) -> Self {
        Self::AuthDisconnected {
            provider,
            timestamp: Self::now(),
        }
    }

    pub fn auth_token_refreshed(provider: Provider, login: Option<String>) -> Self {
        Self::AuthTokenRefreshed {
            provider,
            login,
            timestamp: Self::now(),
        }
    }

    pub fn auth_refresh_failed(provider: Provider, reason: String) -> Self {
        Self::AuthRefreshFailed {
            provider,
            reason,
            timestamp: Self::now(),
        }
    }

    pub fn auth_refresh_required(provider: Provider, reason: String) -> Self {
        Self::AuthRefreshRequired {
            provider,
            reason,
            timestamp: Self::now(),
        }
    }

    pub fn wave_updated(wave_id: LfdId) -> Self {
        Self::WaveUpdated {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_deleted(wave_id: LfdId) -> Self {
        Self::WaveDeleted {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_started(wave_id: LfdId, wave_run_id: LfdId) -> Self {
        Self::WaveStarted {
            wave_id,
            wave_run_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_stopped(wave_id: LfdId) -> Self {
        Self::WaveStopped {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn worktree_updated(worktree: String, repo: String, branch: Option<String>) -> Self {
        Self::WorktreeUpdated {
            worktree,
            repo,
            branch,
            timestamp: Self::now(),
        }
    }

    pub fn wave_waiting(
        wave_id: LfdId,
        wave_run_id: LfdId,
        step: String,
        session_id: Option<LfdId>,
        initial_user_message: Option<String>,
    ) -> Self {
        Self::WaveWaiting {
            wave_id,
            wave_run_id,
            step,
            session_id,
            initial_user_message,
            timestamp: Self::now(),
        }
    }

    pub fn ci_failure(
        wave_id: LfdId,
        wave_run_id: LfdId,
        pr_number: u32,
        branch: String,
        commit_sha: String,
        check_name: String,
        logs_url: String,
    ) -> Self {
        Self::CiFailure {
            wave_id,
            wave_run_id,
            pr_number,
            branch,
            commit_sha,
            check_name,
            logs_url,
            timestamp: Self::now(),
        }
    }

    pub fn activation_queued(
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
    ) -> Self {
        Self::ActivationQueued {
            wave_id,
            stimulus_id,
            source,
            reason,
            queue_depth,
            timestamp: Self::now(),
        }
    }

    pub fn activation_coalesced(
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
    ) -> Self {
        Self::ActivationCoalesced {
            wave_id,
            stimulus_id,
            source,
            reason,
            queue_depth,
            timestamp: Self::now(),
        }
    }

    pub fn activation_dropped(
        wave_id: LfdId,
        stimulus_id: Option<LfdId>,
        source: ActivationSource,
        reason: String,
        queue_depth: u32,
    ) -> Self {
        Self::ActivationDropped {
            wave_id,
            stimulus_id,
            source,
            reason,
            queue_depth,
            timestamp: Self::now(),
        }
    }

    pub fn agent_started(agent_id: LfdId, step: String, worktree: String) -> Self {
        Self::AgentStarted {
            agent_id,
            step,
            worktree,
            timestamp: Self::now(),
        }
    }

    pub fn agent_ended(agent_id: LfdId, status: AgentStatus) -> Self {
        Self::AgentEnded {
            agent_id,
            status: status.as_str().to_string(),
            timestamp: Self::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_id(s: &str) -> LfdId {
        LfdId::from_raw(s)
    }

    #[test]
    fn wave_waiting_serializes_correctly() {
        let event = Event::wave_waiting(
            test_id("wave-1"),
            test_id("run-1"),
            "implement".to_string(),
            Some(test_id("session-1")),
            Some("Start with user prompt".to_string()),
        );
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "wave_waiting");
        assert_eq!(json["wave_id"], "wave-1");
        assert_eq!(json["wave_run_id"], "run-1");
        assert_eq!(json["step"], "implement");
        assert_eq!(json["session_id"], "session-1");
        assert_eq!(json["initial_user_message"], "Start with user prompt");
        assert!(json["timestamp"].is_string());
    }

    #[test]
    fn wave_waiting_omits_session_id_when_absent() {
        let event = Event::wave_waiting(
            test_id("wave-1"),
            test_id("run-1"),
            "implement".to_string(),
            None,
            None,
        );
        let json = serde_json::to_value(&event).unwrap();
        assert!(json.get("session_id").is_none());
        assert!(json.get("initial_user_message").is_none());
    }

    #[test]
    fn agent_started_serializes_correctly() {
        let event = Event::agent_started(
            test_id("agent-1"),
            "review".to_string(),
            "/tmp/wt".to_string(),
        );
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "agent_started");
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["step"], "review");
        assert_eq!(json["worktree"], "/tmp/wt");
    }

    #[test]
    fn agent_ended_serializes_correctly() {
        let event = Event::agent_ended(test_id("agent-1"), AgentStatus::Completed);
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "agent_ended");
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn event_roundtrips_through_json() {
        let id = || LfdId::new();
        let events = vec![
            Event::wave_waiting(id(), id(), "step".to_string(), None, None),
            Event::agent_started(id(), "s".to_string(), "/wt".to_string()),
            Event::agent_ended(id(), AgentStatus::Failed),
            Event::auth_connected(Provider::GitHub, Some("jackdanger".to_string())),
            Event::auth_token_refreshed(Provider::GitHub, Some("jackdanger".to_string())),
            Event::auth_refresh_failed(Provider::GitHub, "refresh failed".to_string()),
            Event::auth_refresh_required(Provider::Claude, "user must re-authenticate".to_string()),
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: Event = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn auth_events_use_dotted_type_names() {
        let event = Event::auth_connected(Provider::GitHub, Some("jackdanger".to_string()));
        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["type"], "auth.connected");
        assert_eq!(json["provider"], "github");
        assert_eq!(json["login"], "jackdanger");

        let refreshed = Event::auth_token_refreshed(Provider::Claude, None);
        let refreshed_json = serde_json::to_value(&refreshed).expect("serialize refreshed");
        assert_eq!(refreshed_json["type"], "auth.token_refreshed");
        assert_eq!(refreshed_json["provider"], "claude");
        assert!(refreshed_json.get("login").is_none());

        let failed = Event::auth_refresh_failed(Provider::Codex, "timed out".to_string());
        let failed_json = serde_json::to_value(&failed).expect("serialize failure");
        assert_eq!(failed_json["type"], "auth.refresh_failed");
        assert_eq!(failed_json["provider"], "codex");
        assert_eq!(failed_json["reason"], "timed out");

        let required =
            Event::auth_refresh_required(Provider::Claude, "user must re-authenticate".into());
        let required_json = serde_json::to_value(&required).expect("serialize required");
        assert_eq!(required_json["type"], "auth.refresh_required");
        assert_eq!(required_json["provider"], "claude");
        assert_eq!(required_json["reason"], "user must re-authenticate");
    }

    #[test]
    fn wave_event_can_be_enriched_with_extra_field() {
        let event = Event::wave_updated(test_id("wave-1"));
        let mut base = serde_json::to_value(&event).unwrap();

        // Simulate enrichment: add a "wave" object to the serialized event
        let wave_data = serde_json::json!({ "id": "wave-1", "name": "test-wave" });
        if let serde_json::Value::Object(ref mut map) = base {
            map.insert("wave".to_string(), wave_data);
        }

        let json_str = serde_json::to_string(&base).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Original event fields preserved
        assert_eq!(parsed["type"], "wave_updated");
        assert_eq!(parsed["wave_id"], "wave-1");
        assert!(parsed["timestamp"].is_string());

        // Enriched wave field present
        assert_eq!(parsed["wave"]["id"], "wave-1");
        assert_eq!(parsed["wave"]["name"], "test-wave");
    }

    #[test]
    fn non_wave_events_have_no_wave_id_for_enrichment() {
        // Agent events shouldn't be enriched — verify they don't match wave_id extraction
        let event = Event::agent_started(
            test_id("agent-1"),
            "review".to_string(),
            "/tmp/wt".to_string(),
        );
        let json = serde_json::to_value(&event).unwrap();
        assert!(json.get("wave_id").is_none());
    }

    #[test]
    fn activation_event_serializes_source_and_depth() {
        let event = Event::activation_queued(
            test_id("wave-1"),
            Some(test_id("stimulus-1")),
            ActivationSource::Push,
            "refs/heads/main advanced abc..def".to_string(),
            2,
        );
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "activation_queued");
        assert_eq!(json["source"], "push");
        assert_eq!(json["queue_depth"], 2);
    }
}
