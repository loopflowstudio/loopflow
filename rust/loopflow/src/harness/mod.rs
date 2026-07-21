pub mod claude;
mod claude_mapping;
pub mod codex;
mod codex_mapping;
mod common;
#[cfg(test)]
mod conformance_tests;
mod lf_tag;
pub mod opencode;
mod opencode_mapping;
pub mod opencode_runtime;

pub(crate) use claude_mapping::rate_limit_signal as claude_rate_limit_signal;
pub(crate) use codex_mapping::rate_limit_signal as codex_rate_limit_signal;
/// Name a codex rate-limit window by duration — shared with the subscription
/// poller so stream and poll observations land on the same window key.
pub(crate) use codex_mapping::window_name as codex_window_name;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::chat::types::ConversationEvent;
use crate::engine::agent::{AgentAuthority, AgentConfig};

pub(crate) fn configure_vendor_std_env(command: &mut std::process::Command) -> Result<()> {
    let (control_bin, control_home, control_db) = vendor_control_context()?;
    set_vendor_std_env(command, &control_bin, &control_home, &control_db);
    Ok(())
}

pub(crate) fn configure_vendor_tokio_env(command: &mut tokio::process::Command) -> Result<()> {
    let (control_bin, control_home, control_db) = vendor_control_context()?;
    command
        .env(crate::store::CONTROL_BIN_ENV, control_bin)
        .env(crate::store::CONTROL_HOME_ENV, control_home)
        .env(crate::store::CONTROL_DB_PATH_ENV, control_db)
        .env_remove("LF_BIN")
        .env_remove("LF_HOME")
        .env_remove("LF_DB_PATH");
    Ok(())
}

pub(crate) fn configure_agent_authority(
    command: &mut tokio::process::Command,
    authority: AgentAuthority,
) {
    if authority != AgentAuthority::Detached {
        return;
    }
    command
        .env_remove(crate::durable::RUN_LEASE_ENV)
        .env_remove(crate::durable::AGENT_INVOCATION_ENV);
}

fn vendor_control_context() -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)>
{
    let context = crate::engine::process::pinned_execution_context()?;
    Ok((context.lf_bin, context.lf_home, context.db_path))
}

fn set_vendor_std_env(
    command: &mut std::process::Command,
    control_bin: &std::path::Path,
    control_home: &std::path::Path,
    control_db: &std::path::Path,
) {
    command
        .env(crate::store::CONTROL_BIN_ENV, control_bin)
        .env(crate::store::CONTROL_HOME_ENV, control_home)
        .env(crate::store::CONTROL_DB_PATH_ENV, control_db)
        .env_remove("LF_BIN")
        .env_remove("LF_HOME")
        .env_remove("LF_DB_PATH");
}

#[derive(Debug, Clone)]
pub struct RawProviderEvent {
    pub stream: &'static str,
    pub line: String,
}

#[cfg(test)]
mod environment_tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{configure_agent_authority, set_vendor_std_env};
    use crate::engine::agent::AgentAuthority;

    #[test]
    fn vendor_receives_control_context_but_not_ordinary_store_context() {
        let mut command = std::process::Command::new("vendor");
        command
            .env("LF_BIN", "/ambient/lf")
            .env("LF_HOME", "/production")
            .env("LF_DB_PATH", "/production/loopflow.db")
            .env("LF_CONTROL_HOME", "/old-control");

        set_vendor_std_env(
            &mut command,
            Path::new("/control/lf"),
            Path::new("/custom"),
            Path::new("/custom/loopflow.db"),
        );

        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_string_lossy().to_string(), value.map(OsString::from)))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(environment["LF_HOME"], None);
        assert_eq!(environment["LF_DB_PATH"], None);
        assert_eq!(environment["LF_BIN"], None);
        assert_eq!(
            environment["LF_CONTROL_BIN"],
            Some(OsString::from("/control/lf"))
        );
        assert_eq!(
            environment["LF_CONTROL_HOME"],
            Some(OsString::from("/custom"))
        );
        assert_eq!(
            environment["LF_CONTROL_DB_PATH"],
            Some(OsString::from("/custom/loopflow.db"))
        );
    }

    #[test]
    fn detached_agent_receives_no_work_authority() {
        let mut command = tokio::process::Command::new("vendor");
        command
            .env(crate::durable::RUN_CONTEXT_ENV, "agent")
            .env(crate::durable::RUN_LEASE_ENV, "secret-lease")
            .env(crate::durable::AGENT_INVOCATION_ENV, "invocation_core");

        configure_agent_authority(&mut command, AgentAuthority::Detached);

        let environment = command
            .as_std()
            .get_envs()
            .map(|(key, value)| (key.to_string_lossy().to_string(), value.map(OsString::from)))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            environment[crate::durable::RUN_CONTEXT_ENV],
            Some(OsString::from("agent"))
        );
        assert_eq!(environment[crate::durable::RUN_LEASE_ENV], None);
        assert_eq!(environment[crate::durable::AGENT_INVOCATION_ENV], None);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("turn already in progress")]
    TurnAlreadyInProgress,
}

pub fn is_turn_in_progress(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<HarnessError>(),
        Some(HarnessError::TurnAlreadyInProgress)
    )
}

/// A stream error that means the harness session itself is dead (not just the
/// turn). Claude has no such code by construction: it runs one subprocess per
/// turn, so a crash fails the turn (`TurnCompleted { Failed }`) and the next
/// turn spawns fresh via `--resume`.
pub fn is_terminal_harness_error(code: &str) -> bool {
    matches!(code, "codex_disconnected" | "opencode_disconnected")
}

/// A failure whose root cause is a disconnected or truncated stream — the class
/// that should route to a backup provider rather than blindly retrying the same
/// one. Covers both the harness's own event-stream drop (`opencode_disconnected`,
/// session-terminal) and the upstream truncation surfaces (`opencode_hollow_body`,
/// `opencode_decode_gap`, turn-terminal). The runner records the code from the
/// trailing `Error` event; the supervisor reads it to decide retry vs. handoff.
pub(crate) fn is_disconnect_class_failure(code: &str) -> bool {
    matches!(
        code,
        opencode::OPENCODE_DISCONNECTED_CODE
            | opencode_mapping::HOLLOW_BODY_CODE
            | opencode_mapping::DECODE_GAP_CODE
    )
}

/// Drain events trailing a `TurnCompleted { Failed }` to extract an actionable
/// error code. The opencode mapping emits usage before completion, then an
/// `Error { code }` for hollow-body, decode-gap, and disconnect failures. The
/// harness sends them synchronously, so the error is already in the buffer when
/// the runner processes the completion.
/// Returns the best failure reason: the error code/message if found, else the
/// generic fallback.
pub(crate) fn drain_turn_failure_reason(
    event_rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    fallback: &str,
) -> String {
    match event_rx.try_recv() {
        Ok(ConversationEvent::Error { code, message, .. }) => format!("{code}: {message}"),
        Ok(other) => {
            tracing::debug!(
                event = ?other,
                "unexpected event trailing a Failed turn; keeping fallback reason"
            );
            fallback.to_string()
        }
        Err(_) => fallback.to_string(),
    }
}

/// What the runner should do after a body failure, based on whether it's a
/// disconnect-class failure and whether a backup agent is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryDecision {
    /// Not a disconnect-class failure — normal failure handling.
    Normal,
    /// Hand the next generation to the backup agent. The caller finishes the
    /// current process, calls `handoff_{task,project}_body`, and returns Ok so
    /// the supervisor relaunches with the new agent.
    HandoffToBackup { agent: String, provider: String },
    /// The turn was replay-safe (no durable side effects) — allow the
    /// supervisor to respawn the same agent for one bounded retry.
    AllowRetry,
    /// Not replay-safe and no backup configured — stop with a non-convergence
    /// record. Never silently re-run a side-effecting body.
    Stop,
}

/// Classify a body failure and decide the recovery path.
///
/// `failure_reason` is the string from `drain_turn_failure_reason` (format:
/// `"code: message"`). `backup_agent` is the wave's configured backup from
/// GOAL.md frontmatter. `turn_had_durable_side_effect` is true if the failed
/// turn completed a Command or File tool item (making a same-agent retry
/// unsafe — the side effect would double-apply).
pub(crate) fn classify_disconnect_recovery(
    failure_reason: &str,
    current_agent: &str,
    turn_had_durable_side_effect: bool,
    backup_agent: Option<&str>,
) -> RecoveryDecision {
    let code = failure_reason.split(':').next().unwrap_or("").trim();

    if !is_disconnect_class_failure(code) {
        return RecoveryDecision::Normal;
    }

    if let Some(backup) = backup_agent {
        let backup = backup.trim();
        if !backup.is_empty() && current_agent != backup {
            let provider = backup.split_once(':').map(|(p, _)| p).unwrap_or(backup);
            let provider = canonical_harness(provider).unwrap_or(provider);
            return RecoveryDecision::HandoffToBackup {
                agent: backup.to_string(),
                provider: provider.to_string(),
            };
        }
    }

    if !turn_had_durable_side_effect {
        RecoveryDecision::AllowRetry
    } else {
        RecoveryDecision::Stop
    }
}

/// What happened when the controller tried to deliver input to the exact
/// provider Turn active at the time of the call.
///
/// This is deliberately an outcome rather than a provider capability. Codex,
/// for example, accepts steering only for some Turn kinds, and a Turn can end
/// between observation and delivery. This receipt never proves incorporation;
/// authored input still belongs in a later boundary's durable seed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendCurrentOutcome {
    Sent {
        provider_turn_id: String,
    },
    NotSteerable,
    Failed {
        error: String,
    },
    Unknown {
        provider_turn_id: Option<String>,
        error: String,
    },
}

/// How a harness answers vendor approval/permission requests.
///
/// `AutoApprove` is the only variant until Decisions land; the enum exists so
/// approval behavior is an explicit construction-time policy instead of a
/// constant buried in each transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    /// Approve every request the vendor asks about.
    AutoApprove,
}

#[async_trait]
pub trait Harness: Send + Sync {
    async fn start(&mut self, config: &AgentConfig) -> Result<()>;
    /// Start the next provider Turn from durable seed input.
    async fn send_input(&mut self, content: &str) -> Result<()>;
    /// Try to deliver input to the exact Turn currently active.
    ///
    /// Drivers without same-Turn input keep the default. A rejection or race
    /// is not an error in the Work protocol; the controller seeds a later
    /// boundary instead.
    async fn send_current(&mut self, _content: &str) -> SendCurrentOutcome {
        SendCurrentOutcome::NotSteerable
    }
    /// Cancel the in-flight turn but keep the session alive for the next
    /// turn. The interrupted turn surfaces as a
    /// `TurnCompleted { status: Interrupted }` terminal event. No-op when no
    /// turn is in flight.
    async fn interrupt(&mut self) -> Result<()>;
    /// Full teardown: cancel any in-flight turn and end the vendor session.
    async fn stop(&mut self) -> Result<()>;
    /// Vendor session/thread id, once the vendor has announced it. Codex and
    /// opencode announce it by the time `start` returns; claude announces it
    /// on the first turn's stream. Callers persist this before driving turns.
    fn provider_session_id(&self) -> Option<String>;
    /// Independently isolated provider process group, when the harness owns
    /// one. Providers that remain in the runner's process group return None;
    /// the Run retains the runner group recorded at activation.
    fn process_group_id(&self) -> Option<u32> {
        None
    }
    /// Tee provider-native frames already visible to the adapter. The sender
    /// is optional because conformance tests and callers below the production
    /// launch gate do not own a trace capture.
    fn set_raw_provider_sender(
        &mut self,
        _raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    ) {
    }
    /// Seed a previously persisted vendor session id so the next turn resumes
    /// it. Drivers that take resume state at `start` instead ignore this.
    fn set_provider_session_id(&mut self, _provider_session_id: Option<String>) {}
    /// Pin this Invocation to the exact managed account already recorded in its
    /// durable route. Accountless providers keep the default no-op.
    fn set_provider_account_id(&mut self, _account_id: Option<crate::store::ProviderAccountId>) {}
    /// The managed account selected before the first provider Turn begins.
    fn provider_account_id(&self) -> Option<crate::store::ProviderAccountId> {
        None
    }
}

/// Constructor fn: `(harness_kind, approval, event_tx) -> harness`.
pub type CreateHarnessFn =
    fn(&str, ApprovalPolicy, mpsc::UnboundedSender<ConversationEvent>) -> Result<Box<dyn Harness>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Codex,
    Claude,
    OpenCode,
}

impl HarnessKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }

    fn create(
        self,
        approval: ApprovalPolicy,
        event_tx: mpsc::UnboundedSender<ConversationEvent>,
    ) -> Box<dyn Harness> {
        match self {
            Self::Codex => Box::new(codex::CodexHarness::new(event_tx, approval)),
            // Claude approvals ride the per-turn CLI flags built from
            // AgentConfig, not a runtime channel; no policy to thread.
            Self::Claude => Box::new(claude::ClaudeHarness::new(event_tx)),
            Self::OpenCode => Box::new(opencode::OpenCodeHarness::new(event_tx, approval)),
        }
    }
}

pub fn canonical_harness(name: &str) -> Option<&'static str> {
    HarnessKind::parse(name).map(HarnessKind::as_str)
}

/// How a body builds its provider. `default_create_harness` is the only
/// production implementation; holding it as a value rather than calling it
/// directly is what lets a body's construction be substituted.
pub type CreateHarness = Box<
    dyn Fn(
            &str,
            ApprovalPolicy,
            mpsc::UnboundedSender<ConversationEvent>,
        ) -> Result<Box<dyn Harness>>
        + Send,
>;

pub fn default_create_harness(
    name: &str,
    approval: ApprovalPolicy,
    event_tx: mpsc::UnboundedSender<ConversationEvent>,
) -> Result<Box<dyn Harness>> {
    if let Some(kind) = HarnessKind::parse(name) {
        return Ok(kind.create(approval, event_tx));
    }
    anyhow::bail!(
        "unsupported session harness: {}",
        name.trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_harness_is_case_insensitive_and_trimmed() {
        assert_eq!(canonical_harness(" claUDe "), Some("claude"));
        assert_eq!(canonical_harness(" CODEX"), Some("codex"));
        assert_eq!(canonical_harness("OpenCode"), Some("opencode"));
        assert_eq!(canonical_harness("lfharness"), None);
    }

    #[test]
    fn default_create_harness_rejects_unknown() {
        let (tx, _rx) = mpsc::unbounded_channel();
        match default_create_harness("lfharness", ApprovalPolicy::AutoApprove, tx) {
            Ok(_) => panic!("should reject unknown harness"),
            Err(err) => assert!(err.to_string().contains("unsupported session harness")),
        }
    }

    #[test]
    fn terminal_harness_error_recognizes_disconnects_only() {
        assert!(is_terminal_harness_error("opencode_disconnected"));
        assert!(is_terminal_harness_error("codex_disconnected"));
        assert!(!is_terminal_harness_error("opencode_error"));
        // Claude has no session-terminal code: per-turn subprocess.
        assert!(!is_terminal_harness_error("claude_harness_crashed"));
    }

    #[test]
    fn disconnect_class_failure_covers_all_three_opencode_codes() {
        assert!(is_disconnect_class_failure("opencode_disconnected"));
        assert!(is_disconnect_class_failure("opencode_hollow_body"));
        assert!(is_disconnect_class_failure("opencode_decode_gap"));
        // A generic opencode error is not a disconnect-class failure.
        assert!(!is_disconnect_class_failure("opencode_error"));
        assert!(!is_disconnect_class_failure("codex_disconnected"));
    }

    #[test]
    fn hollow_and_decode_gap_are_turn_terminal_not_session_terminal() {
        // The hollow-body and decode-gap codes fail the turn, not the session:
        // the harness process may still be alive (the upstream stream
        // truncated, not our /event stream). They must NOT register as
        // session-terminal — that would prevent a same-session retry or
        // handoff that reuses the opencode server.
        assert!(!is_terminal_harness_error("opencode_hollow_body"));
        assert!(!is_terminal_harness_error("opencode_decode_gap"));
    }

    #[test]
    fn recovery_classifier_routes_to_backup_when_configured() {
        let decision = classify_disconnect_recovery(
            "opencode_disconnected: stream died",
            "opencode:glm-5.2",
            false,
            Some("claude:opus"),
        );
        assert_eq!(
            decision,
            RecoveryDecision::HandoffToBackup {
                agent: "claude:opus".to_string(),
                provider: "claude".to_string(),
            }
        );
    }

    #[test]
    fn recovery_classifier_routes_to_backup_even_with_durable_side_effect() {
        // A durable side effect makes retry unsafe, but the backup is still the
        // preferred path — it re-reads current state rather than replaying.
        let decision = classify_disconnect_recovery(
            "opencode_hollow_body: hollow turn",
            "opencode:glm-5.2",
            true,
            Some("claude:opus"),
        );
        assert!(matches!(decision, RecoveryDecision::HandoffToBackup { .. }));
    }

    #[test]
    fn recovery_classifier_allows_retry_when_replay_safe_and_no_backup() {
        let decision = classify_disconnect_recovery(
            "opencode_disconnected: stream died",
            "opencode:glm-5.2",
            false,
            None,
        );
        assert_eq!(decision, RecoveryDecision::AllowRetry);
    }

    #[test]
    fn recovery_classifier_stops_when_not_replay_safe_and_no_backup() {
        let decision = classify_disconnect_recovery(
            "opencode_hollow_body: hollow turn",
            "opencode:glm-5.2",
            true,
            None,
        );
        assert_eq!(decision, RecoveryDecision::Stop);
    }

    #[test]
    fn recovery_classifier_skips_backup_already_in_use() {
        // If the session is already running the backup agent, don't hand off
        // again (prevents ping-pong). Fall through to replay-safety check.
        let decision = classify_disconnect_recovery(
            "opencode_disconnected: stream died",
            "claude:opus",
            false,
            Some("claude:opus"),
        );
        assert_eq!(decision, RecoveryDecision::AllowRetry);
    }

    #[test]
    fn recovery_classifier_returns_normal_for_non_disconnect_failures() {
        let decision = classify_disconnect_recovery(
            "provider turn failed",
            "opencode:glm-5.2",
            false,
            Some("claude:opus"),
        );
        assert_eq!(decision, RecoveryDecision::Normal);
    }

    #[tokio::test]
    async fn current_send_is_decided_from_the_active_turn() {
        let (tx, _rx) = mpsc::unbounded_channel();
        for name in ["codex", "claude", "opencode"] {
            let mut harness = default_create_harness(name, ApprovalPolicy::AutoApprove, tx.clone())
                .expect("known harness");
            assert_eq!(
                harness.send_current("direction").await,
                SendCurrentOutcome::NotSteerable,
                "an inactive {name} harness has no exact Turn to steer"
            );
        }
    }
}
