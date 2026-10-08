mod attention;
pub mod claude;
mod claude_history;
mod claude_mapping;
pub mod codex;
#[cfg(unix)]
pub mod codex_connection;
mod codex_history;
mod codex_mapping;
mod common;
#[cfg(test)]
mod conformance_tests;
mod dispatch;
#[cfg(all(test, unix))]
mod dispatch_tests;
mod lf_tag;
pub(crate) mod native_titles;
pub mod opencode;
pub(crate) mod opencode_history;
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
use crate::engine::agent::AgentConfig;

pub(crate) fn configure_vendor_std_env(command: &mut std::process::Command) -> Result<()> {
    let context = crate::engine::process::execution_context()?;
    set_vendor_std_env(command, &context.lf_bin, &context.lf_home)
}

pub(crate) fn configure_agent_env(command: &mut tokio::process::Command, config: &AgentConfig) {
    for name in crate::engine::agent::EXECUTION_IDENTITY_ENV {
        command.env_remove(name);
    }
    command
        .envs(&config.env)
        .env_remove(crate::engine::process::DISCORD_TOKEN_ENV)
        .env_remove("LOOPFLOW_DIRECTIVE_FILE");
    if let Some(path) = &config.directive_relay {
        command.env("LOOPFLOW_DIRECTIVE_FILE", path);
    }
    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    crate::provider_auth::apply_provider_env_to_command(&program, command.as_std_mut());
}

pub(crate) fn conversation_environment(
    command: &std::process::Command,
    config: &AgentConfig,
) -> std::collections::BTreeMap<String, String> {
    command
        .get_envs()
        .filter_map(|(key, value)| {
            let key = key.to_string_lossy();
            let intended = config.env.contains_key(key.as_ref())
                || matches!(
                    key.as_ref(),
                    "PATH" | "LF_BIN" | "LF_HOME" | "LOOPFLOW_DIRECTIVE_FILE"
                );
            intended
                .then_some(value)
                .flatten()
                .map(|value| (key.into_owned(), value.to_string_lossy().into_owned()))
        })
        .collect()
}

fn set_vendor_std_env(
    command: &mut std::process::Command,
    control_bin: &std::path::Path,
    control_home: &std::path::Path,
) -> Result<()> {
    command
        .env("LF_BIN", control_bin)
        .env("LF_HOME", control_home)
        .env_remove(crate::engine::process::DISCORD_TOKEN_ENV);
    let mut paths = vec![control_bin
        .parent()
        .expect("absolute lf has a parent")
        .to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    command.env("PATH", std::env::join_paths(paths)?);
    Ok(())
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

    use super::{configure_agent_env, set_vendor_std_env};
    use crate::engine::agent::AgentConfig;

    #[test]
    fn conversation_tools_observe_sanitized_overrides_and_removals() {
        let mut config = AgentConfig::default();
        for key in [
            "LF_DISCORD_TOKEN",
            "LOOPFLOW_DIRECTIVE_FILE",
            "LF_BIN",
            "LF_HOME",
        ] {
            config.env.insert(key.into(), "stale-fixture".into());
        }
        config
            .env
            .insert("LF_AGENT_CALLER".into(), "current-fixture".into());
        let mut engine = tokio::process::Command::new("vendor");
        configure_agent_env(&mut engine, &config);
        set_vendor_std_env(
            engine.as_std_mut(),
            Path::new("/control/lf"),
            Path::new("/private"),
        )
        .unwrap();
        // Provider account environment is not conversation tool authority.
        engine.env("PROVIDER_ACCOUNT_FIXTURE", "not-for-tools");
        let tools = super::conversation_environment(engine.as_std(), &config);
        let script = "test -z \"${LF_DISCORD_TOKEN+x}${LOOPFLOW_DIRECTIVE_FILE+x}${PROVIDER_ACCOUNT_FIXTURE+x}\" && test \"$LF_AGENT_CALLER\" = current-fixture && test \"$LF_HOME\" = /private && test \"$LF_BIN\" = /control/lf";
        assert!(std::process::Command::new("/bin/sh")
            .env_clear()
            .envs(tools)
            .args(["-c", script])
            .status()
            .unwrap()
            .success());
    }

    #[tokio::test]
    async fn provider_child_cannot_read_the_bridge_token() {
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args(["-c", "test -z \"${LF_DISCORD_TOKEN+x}\""]);
        command.env(crate::engine::process::DISCORD_TOKEN_ENV, "fixture-token");
        let mut config = AgentConfig::default();
        config.env.insert(
            crate::engine::process::DISCORD_TOKEN_ENV.into(),
            "fixture-override".into(),
        );
        configure_agent_env(&mut command, &config);
        assert!(command.status().await.unwrap().success());
    }

    #[test]
    fn agent_receives_only_fresh_capture_context() {
        let mut command = tokio::process::Command::new("vendor");
        command
            .env(crate::session_record::CAPTURE_KEY_ENV, "run_stale")
            .env("LF_RUN_DIR", "/stale/run");
        let mut config = crate::engine::agent::AgentConfig::default();
        config.env.insert(
            crate::session_record::CAPTURE_KEY_ENV.to_string(),
            "run_fresh".to_string(),
        );

        configure_agent_env(&mut command, &config);

        let environment = command
            .as_std()
            .get_envs()
            .map(|(key, value)| (key.to_string_lossy().to_string(), value.map(OsString::from)))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            environment[crate::session_record::CAPTURE_KEY_ENV],
            Some(OsString::from("run_fresh"))
        );
        assert_eq!(environment["LF_RUN_DIR"], None);
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
    /// The owned provider child, for read-only activity sampling. This does not
    /// grant process-group signal authority.
    fn process_id(&self) -> Option<u32> {
        None
    }
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
    fn hollow_and_decode_gap_are_turn_terminal_not_session_terminal() {
        // The hollow-body and decode-gap codes fail the turn, not the session:
        // the harness process may still be alive (the upstream stream
        // truncated, not our /event stream). They must NOT register as
        // session-terminal — that would prevent a same-session retry or
        // handoff that reuses the opencode server.
        assert!(!is_terminal_harness_error("opencode_hollow_body"));
        assert!(!is_terminal_harness_error("opencode_decode_gap"));
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
