pub mod agent_process;
pub mod antigravity;
pub mod claude;
pub(crate) mod claude_trust;
#[cfg(unix)]
pub mod codex;
#[cfg(unix)]
pub mod codex_connection;
mod codex_rpc;
mod common;
#[cfg(test)]
mod conformance_tests;
pub mod context;
mod dispatch;
#[cfg(all(test, unix))]
mod dispatch_tests;
pub mod drive;
mod fact_writer;
#[cfg(all(test, unix))]
mod launch_matrix;
pub(crate) mod native_exit;
pub mod native_session;
pub mod opencode;
pub mod pi;
pub mod task_updates;

use anyhow::Result;
use async_trait::async_trait;
use loopflow_harness::transcript::{self, Source};
use loopflow_harness::{Harness, HarnessKind};
use loopflow_model::chat::types::ConversationEvent;
use loopflow_model::id::AgentSessionId;
use tokio::sync::mpsc;

use crate::agent::AgentConfig;
use crate::transcript::{Gap, Located, RetentionState};

pub(crate) fn configure_vendor_std_env(command: &mut std::process::Command) -> Result<()> {
    let context = crate::os_process::execution_context()?;
    set_vendor_std_env(command, &context.lf_bin, &context.lf_home)
}

pub(crate) fn configure_agent_env(command: &mut tokio::process::Command, config: &AgentConfig) {
    crate::agent::remove_caller_session_env(command.as_std_mut());
    command
        .envs(&config.env)
        .env_remove("LOOPFLOW_DIRECTIVE_FILE");
    if let Some(path) = &config.directive_relay {
        command.env("LOOPFLOW_DIRECTIVE_FILE", path);
    }
    if let Some(cwd) = &config.cwd {
        // OpenCode roots its session in `PWD`, not the process directory, so an
        // inherited `PWD` would put a Task's worker in the main checkout.
        command.current_dir(cwd).env("PWD", cwd);
    }
    crate::provider_auth::apply_provider_env_to_command(config.agent.harness, command.as_std_mut());
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
        .env("LF_HOME", control_home);
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

#[cfg(test)]
mod environment_tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{configure_agent_env, set_vendor_std_env};

    #[test]
    fn conversation_tools_observe_sanitized_overrides_and_removals() {
        let mut config = crate::agent::AgentConfig::default();
        for key in ["LOOPFLOW_DIRECTIVE_FILE", "LF_BIN", "LF_HOME"] {
            config.env.insert(key.into(), "stale-fixture".into());
        }
        config
            .env
            .insert("LF_AGENT_CALLER".into(), "current-fixture".into());
        let mut provider = tokio::process::Command::new("vendor");
        configure_agent_env(&mut provider, &config);
        set_vendor_std_env(
            provider.as_std_mut(),
            Path::new("/control/lf"),
            Path::new("/private"),
        )
        .unwrap();
        // Provider account environment is not conversation tool authority.
        provider.env("PROVIDER_ACCOUNT_FIXTURE", "not-for-tools");
        let tools = super::conversation_environment(provider.as_std(), &config);
        let script = "test -z \"${LOOPFLOW_DIRECTIVE_FILE+x}${PROVIDER_ACCOUNT_FIXTURE+x}\" && test \"$LF_AGENT_CALLER\" = current-fixture && test \"$LF_HOME\" = /private && test \"$LF_BIN\" = /control/lf";
        assert!(std::process::Command::new("/bin/sh")
            .env_clear()
            .envs(tools)
            .args(["-c", script])
            .status()
            .unwrap()
            .success());
    }

    #[tokio::test]
    async fn provider_child_starts_in_its_checkout_with_a_matching_pwd() {
        let checkout = tempfile::tempdir().unwrap();
        let mut command = tokio::process::Command::new("/usr/bin/printenv");
        command.arg("PWD").env("PWD", "/main/checkout");
        let config = crate::agent::AgentConfig {
            cwd: Some(checkout.path().to_path_buf()),
            ..Default::default()
        };
        configure_agent_env(&mut command, &config);
        let output = command.output().await.unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            checkout.path().to_str().unwrap()
        );
        assert_eq!(command.as_std().get_current_dir(), Some(checkout.path()));
    }

    #[test]
    fn agent_receives_only_fresh_capture_context() {
        let mut command = tokio::process::Command::new("vendor");
        command
            .env(crate::session_record::CAPTURE_KEY_ENV, "run_stale")
            .env("LF_RUN_DIR", "/stale/run");
        let mut config = crate::agent::AgentConfig::default();
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

/// How a harness answers vendor approval/permission requests.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    /// Approve every request the vendor asks about.
    AutoApprove,
    /// Bootstrap a conversation without answering for the native client.
    NativeClient,
}

#[async_trait]
pub trait Driver: Send + Sync {
    /// The invocation owner receives replacement attachment snapshots when a
    /// provider must respawn within the same lf invocation.
    fn set_runtime(&mut self, _runtime: Option<crate::process::LfProcessRuntime>) {}
    async fn start(&mut self) -> Result<()>;
    /// Start the next provider Turn from durable seed input.
    async fn send_input(&mut self, content: &str) -> Result<()>;
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
    fn agent_session(&self) -> Option<AgentSessionId>;
    /// The owned provider child, for read-only activity sampling. This does not
    /// grant process-group signal authority.
    fn pid(&self) -> Option<u32> {
        None
    }
    /// Independently isolated provider process group, when the harness owns
    /// one. Providers that remain in the runner's process group return None;
    /// the Run retains the runner group recorded at activation.
    fn process_group_id(&self) -> Option<u32> {
        None
    }
}

/// Whether someone is in the conversation a launch opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Mode {
    Interactive,
    #[default]
    Headless,
}

impl Mode {
    /// What the prompt says about who is in the conversation, so a skill
    /// knows where a real dependency should route.
    pub fn instructions(self) -> &'static str {
        match self {
            Self::Interactive => loopflow_definitions::builtins::MODE_INTERACTIVE,
            Self::Headless => loopflow_definitions::builtins::MODE_HEADLESS,
        }
    }
}

/// Launch kind, independently of Task attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchKind {
    Chat,
    Batch,
    WorkSession,
    Worker,
}

impl LaunchKind {
    pub fn mode(self) -> Mode {
        match self {
            Self::Chat | Self::WorkSession => Mode::Interactive,
            Self::Batch | Self::Worker => Mode::Headless,
        }
    }

    /// Whether the launch reports turns to the Machine server.
    pub fn is_managed(self) -> bool {
        matches!(self, Self::WorkSession | Self::Worker)
    }
}

/// The store, Session and attachment this launch records under.
#[derive(Debug, Clone)]
pub struct Recording {
    pub store: loopflow_storage::sqlite::SqliteStore,
    pub session: String,
    pub attachment: crate::process::SessionAttachment,
}

impl Recording {
    pub fn assigned_thread(&self) -> Result<AgentSessionId> {
        let session = self
            .store
            .session(&self.session)?
            .ok_or_else(|| anyhow::anyhow!("admitted Session is missing"))?;
        let key = session
            .artifact_key
            .ok_or_else(|| anyhow::anyhow!("admitted Session has no capture"))?;
        let raw = key.strip_prefix("run_").unwrap_or(&key);
        Ok(uuid::Uuid::parse_str(raw)?.to_string().into())
    }

    pub fn from_runtime(runtime: &crate::process::LfProcessRuntime) -> Result<Self> {
        runtime
            .recording()
            .ok_or_else(|| anyhow::anyhow!("invocation has not claimed its Session attachment"))
    }
}

/// Native CLI for a provider; API-only services have no corresponding harness.
pub fn from_provider(provider: crate::provider_auth::Provider) -> Option<HarnessKind> {
    match provider {
        crate::provider_auth::Provider::Claude => Some(HarnessKind::Claude),
        crate::provider_auth::Provider::Codex => Some(HarnessKind::Codex),
        _ => None,
    }
}

/// How Loopflow launches and drives `kind`; what the client writes is
/// `kind.harness()`.
pub fn launcher(kind: HarnessKind) -> &'static dyn Launcher {
    match kind {
        HarnessKind::Claude => &loopflow_harness::Claude,
        HarnessKind::Codex => &loopflow_harness::Codex,
        HarnessKind::OpenCode => &loopflow_harness::OpenCode,
        HarnessKind::Pi => &loopflow_harness::Pi,
        HarnessKind::Antigravity => &loopflow_harness::Antigravity,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadName {
    Assigned,
    Reported,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadNaming {
    pub bare: ThreadName,
    pub managed: ThreadName,
}

impl ThreadNaming {
    pub fn for_kind(self, kind: LaunchKind) -> ThreadName {
        if kind.is_managed() {
            self.managed
        } else {
            self.bare
        }
    }
}

/// Where `lf self sync-skills` puts skills for a harness to list as its own:
/// one directory per scope, not every directory the harness reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledSkills {
    /// Relative to the home directory.
    pub global: &'static str,
    /// Relative to the repository root.
    pub repository: &'static str,
    /// A `skills.json` under the home directory that must list `global`
    /// (as an absolute path) before the harness reads it.
    pub global_registry: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub thread: ThreadNaming,
    pub initial_prompt_on_resume: bool,
    pub native_skills: Option<crate::skills::catalog::SkillDialect>,
    pub installed_skills: InstalledSkills,
    pub turn_limit: bool,
    pub output_schema: bool,
    pub respawns_after_crash: bool,
    pub install_hint: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub enum Start<'a> {
    New,
    Resume { thread: &'a AgentSessionId },
}

impl<'a> Start<'a> {
    pub fn from_thread(thread: Option<&'a AgentSessionId>) -> Self {
        thread.map_or(Self::New, |thread| Self::Resume { thread })
    }

    pub fn thread(self) -> Option<AgentSessionId> {
        match self {
            Self::New => None,
            Self::Resume { thread } => Some(thread.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    pub config: &'a AgentConfig,
    pub harness: &'static dyn Launcher,
    pub kind: LaunchKind,
    pub recording: &'a Recording,
    pub start: Start<'a>,
    pub context_file: Option<&'a std::path::Path>,
}

impl Launch<'_> {
    pub fn thread(&self) -> Result<Option<AgentSessionId>> {
        let naming = self.harness.capabilities().thread;
        match self.start.thread() {
            None if naming.for_kind(self.kind) == ThreadName::Assigned => self
                .recording
                .assigned_thread()
                .map(|thread| Some(self.harness.thread_name(thread))),
            thread => Ok(thread),
        }
    }

    pub(crate) fn terminal_input(&self) -> Result<(Vec<String>, Option<String>)> {
        anyhow::ensure!(
            self.config.max_turns.is_none() && self.config.output_schema.is_none(),
            "turn limits and response schemas require a headless Run (-b)"
        );
        match self.start {
            Start::New => {
                let (flags, prompt) = match &self.config.skill_invocation {
                    Some(skill) => skill.terminal_input(self.harness, self.config)?,
                    None => (Vec::new(), self.config.task_prompt.clone()),
                };
                Ok((flags, Some(prompt)))
            }
            Start::Resume { .. } => Ok((
                Vec::new(),
                (!self.config.task_prompt.is_empty()).then(|| self.config.task_prompt.clone()),
            )),
        }
    }
}

/// A provider hook's callback, read by the harness whose provider sent it.
#[derive(Debug, Clone, PartialEq)]
pub struct HookCallback {
    /// Where in the conversation the hook ran.
    pub moment: crate::context_block::ContextMoment,
    /// The callback in the shape Claude sends: `session_id`,
    /// `hook_event_name` and, at a start, `source`.
    pub payload: serde_json::Value,
    /// A start that is also the prompt opening a turn, for a provider with
    /// no hook of its own at a prompt.
    pub prompt: bool,
}

/// Launching and driving a harness, and joining what it wrote to Loopflow's
/// store and the process table.
pub trait Launcher: Harness {
    fn capabilities(&self) -> Capabilities;
    /// What an error this harness's decoder reported says went wrong, for the
    /// failures the provider leaves to lf: a spent quota, a revoked login or
    /// an unserved model moves the run to the next agent in its order, and a
    /// transient failure is retried. `None` for an error that names no cause,
    /// and for a failure the provider retries itself.
    fn classify_failure(&self, code: &str, message: &str) -> Option<crate::agent::AgentFailure>;
    /// What a hook callback from this harness's provider means: the moment
    /// it ran at and the conversation it names. `declared` is the moment its
    /// hook declared; `thread_file` is where the launch keeps the
    /// conversation the person is in (`context_block::reported_thread_path`),
    /// and an implementation whose provider names the conversation only here
    /// writes it there whenever a person's input reaches another one. `None`
    /// when the callback is not a conversation a person's input reached: lf answers
    /// nothing, records no boundary and claims no steers. The default reads
    /// a callback that has Claude's shape and ran at the moment its hook
    /// declared; one with an `agent_id` is a subagent's.
    fn hook_callback(
        &self,
        payload: serde_json::Value,
        declared: crate::context_block::ContextMoment,
        _thread_file: Option<&std::path::Path>,
    ) -> Option<HookCallback> {
        payload
            .get("agent_id")
            .is_none_or(serde_json::Value::is_null)
            .then_some(HookCallback {
                moment: declared,
                payload,
                prompt: false,
            })
    }
    /// The reply to the hook named `event` that puts `text` in front of the
    /// model; Claude's shape by default.
    fn hook_reply(&self, event: &str, text: &str) -> serde_json::Value {
        serde_json::json!({"hookSpecificOutput": {
            "hookEventName": event, "additionalContext": text,
        }})
    }
    fn open(&self, launch: &Launch<'_>) -> Result<native_session::NativeClient>;
    fn driver(
        &self,
        launch: &Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        approval: ApprovalPolicy,
    ) -> Box<dyn Driver>;

    /// The native name of the conversation lf assigned `assigned` to.
    fn thread_name(&self, assigned: AgentSessionId) -> AgentSessionId {
        assigned
    }

    fn validate_agent_process_close(
        &self,
        _store: &loopflow_storage::sqlite::SqliteStore,
        _session: &str,
        _process: &loopflow_model::id::AgentProcessId,
    ) -> crate::store::StoreResult<()> {
        Ok(())
    }
    fn conversation_owners(
        &self,
        _home: &std::path::Path,
        _processes: &std::collections::HashMap<u32, &crate::process::OsProcess>,
    ) -> std::collections::HashMap<u32, String> {
        std::collections::HashMap::new()
    }
    /// The provider whose home keeps this harness's transcripts and accounts.
    fn provider(&self) -> Option<crate::provider_auth::Provider> {
        None
    }
    /// Whether this harness's CLI is on `PATH`.
    fn installed(&self) -> bool {
        crate::os_process::which_on_path(std::path::Path::new(self.kind().as_str())).is_some()
    }
    /// Whether a running process is this client's terminal program.
    fn recognizes(&self, process: &crate::process::OsProcess) -> bool {
        process.command == self.kind().as_str()
    }
    fn locate_transcript(
        &self,
        store: &loopflow_storage::sqlite::SqliteStore,
        id: &AgentSessionId,
        cwd: &std::path::Path,
    ) -> Result<Located> {
        let Some(provider) = self.provider() else {
            return Ok(Located::Gone(Gap::FileMissing {
                home: self.transcript_home(),
            }));
        };
        let Some(home) = crate::transcript::conversation_home(store, provider, id)? else {
            return Ok(Located::Gone(Gap::AccountHomeRemoved));
        };
        Ok(
            match transcript::find(self.kind(), &home, id.as_str(), cwd) {
                Some(path) => Located::Found {
                    source: Source::File(path),
                    home: Some(home),
                },
                None => Located::Gone(Gap::FileMissing { home }),
            },
        )
    }
    fn transcript_retention(&self, _thread: &AgentSessionId) -> (RetentionState, Option<i64>) {
        (RetentionState::Unknown, None)
    }
    fn recent_transcripts(
        &self,
        home: &std::path::Path,
        cwd: &std::path::Path,
        since: i64,
        owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, Source, i64)> {
        crate::transcript::raw::unowned(
            transcript::scan(self.kind(), home)
                .into_iter()
                .filter(|(_, indexed)| {
                    indexed.modified >= since
                        && transcript::recorded_cwd(&indexed.path).as_deref() == Some(cwd)
                })
                .map(|(id, indexed)| (id, Source::File(indexed.path), indexed.modified))
                .collect(),
            owned,
        )
    }
    fn terminal_error(&self, code: &str) -> bool {
        !self.capabilities().respawns_after_crash
            && code == format!("{}_disconnected", self.kind().as_str())
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn launch_for_test<'a>(
    kind: HarnessKind,
    config: &'a AgentConfig,
    recording: &'a Recording,
) -> Launch<'a> {
    Launch {
        config,
        recording,
        harness: launcher(kind),
        kind: LaunchKind::Batch,
        start: Start::from_thread(config.resume_token.as_ref()),
        context_file: None,
    }
}

/// Admit a fixture launch: a private ledger with one Session attached to one
/// recorded lf invocation. Keep the guard for the life of the harness.
#[cfg(any(test, feature = "test-support"))]
pub fn admit_for_test(home: &std::path::Path) -> Recording {
    let database = home.join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&database).unwrap();
    let process = loopflow_model::id::LfProcessId::new();
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'00000000-0000-0000-0000-000000000001',1)",
            [process.as_str()],
        )
        .unwrap();
    store.test_session("fixture", &crate::session_record::new_artifact_key());
    let attachment =
        crate::process::attachment::claim_session_attachment(&store, "fixture", None, &process)
            .unwrap();
    Recording {
        store,
        session: "fixture".into(),
        attachment,
    }
}

#[cfg(all(test, unix))]
mod pi_tests;
