//! Antigravity, whose CLI is `agy`. A headless turn is one
//! `agy --input-format stream-json --output-format stream-json` process that
//! reads one message and exits; `agy` names the conversation in its first
//! line, and the next turn continues it with `--conversation`.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentConfig, AgentFailure};
use crate::context_block::ContextMoment;
use crate::harness::antigravity_decode::Turn;
use crate::harness::fact::Decoded;
use crate::harness::fact_writer::FactWriter;
use crate::harness::{Driver as StreamDriver, HarnessError, HookCallback, LaunchKind, Recording};
use loopflow_model::chat::types::ConversationEvent;
use loopflow_model::id::AgentSessionId;

#[derive(Debug, Default)]
struct State {
    /// The conversation, once lf resumed it or `agy` named it.
    thread: Option<AgentSessionId>,
    turn: Option<Turn>,
    /// Writes what the current process's turn establishes.
    writer: Option<FactWriter>,
    /// lf stopped the current turn's process.
    interrupted: bool,
}

impl State {
    /// Record what a turn established and hand its display events on. A
    /// record lf could not write is a gap, never a lost conversation event.
    fn publish(
        &mut self,
        decoded: Vec<Decoded>,
        events: &mpsc::UnboundedSender<ConversationEvent>,
    ) {
        if let Some(writer) = self.writer.as_mut() {
            if let Err(error) = writer.record(&decoded) {
                tracing::warn!(%error, "failed to record Antigravity activity");
            }
        }
        for decoded in decoded {
            if let Decoded::Display(event) = decoded {
                let _ = events.send(event);
            }
        }
    }
}

#[derive(Debug)]
pub struct Driver {
    events: mpsc::UnboundedSender<ConversationEvent>,
    state: Arc<Mutex<State>>,
    config: AgentConfig,
    owner: Recording,
    runtime: Option<crate::process::LfProcessRuntime>,
    kind: LaunchKind,
    first_turn: bool,
    /// The current turn's process, until its exit is recorded.
    pid: Option<u32>,
    reader: Option<JoinHandle<()>>,
}

impl Driver {
    pub fn new(
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
    ) -> Self {
        Self {
            events,
            state: Arc::new(Mutex::new(State {
                thread: launch.start.thread(),
                ..State::default()
            })),
            config: launch.config.clone(),
            owner: launch.recording.clone(),
            runtime: None,
            kind: launch.kind,
            first_turn: true,
            pid: None,
            reader: None,
        }
    }

    /// Record the last process's exit and stop whatever of it is left.
    async fn settle_process(&mut self) -> Result<()> {
        if self.pid.take().is_some() {
            super::agent_process::stop(&self.owner.clone())?;
        }
        if let Some(reader) = self.reader.take() {
            // The process is gone, so its output ends and the reader settles the turn.
            let _ = reader.await;
        }
        Ok(())
    }

    /// The run for one turn, and the file its stderr goes to.
    fn command(
        &self,
        config: &AgentConfig,
        thread: Option<&AgentSessionId>,
    ) -> Result<(Command, std::fs::File)> {
        let mut command = Command::new(super::Harness::program(&Antigravity));
        command.args([
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
        ]);
        // Nobody is there to answer a permission prompt.
        command.arg("--dangerously-skip-permissions");
        command.args(options(config, thread, self.kind)?);
        super::configure_agent_env(&mut command, config);
        if let Some(context) = &config.conversation_context {
            context.configure_hooks(command.as_std_mut())?;
        }
        super::configure_vendor_std_env(command.as_std_mut())?;
        let log = match super::common::diagnostics(config, "antigravity")? {
            Some(log) => log,
            None => tempfile::tempfile()?,
        };
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log.try_clone()?)
            // The shared stop owns cleanup; dropping a stale handle must not signal.
            .kill_on_drop(false);
        Ok((command, log))
    }

    fn spawn_reader(
        &self,
        mut child: Child,
        stdout: ChildStdout,
        mut diagnostics: std::fs::File,
        // Where this run's stderr begins in a file earlier runs appended to.
        diagnostics_start: u64,
    ) -> JoinHandle<()> {
        let state = self.state.clone();
        let events = self.events.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    tracing::debug!(line = %line, "Antigravity printed a line that is not JSON");
                    continue;
                };
                let mut state = state.lock().expect("Antigravity state lock poisoned");
                if let Some(turn) = state.turn.as_mut() {
                    let decoded = turn.event(&value);
                    if let Some(thread) = turn.thread().cloned() {
                        state.thread = Some(thread);
                    }
                    state.publish(decoded, &events);
                }
            }
            let _ = child.wait().await;
            let mut state = state.lock().expect("Antigravity state lock poisoned");
            let Some(mut turn) = state.turn.take() else {
                return;
            };
            // Stopped by this driver, or with the lf process a signal is ending.
            let interrupted = std::mem::take(&mut state.interrupted) || crate::agent::interrupted();
            use std::io::{Read, Seek, SeekFrom};
            let mut stderr = String::new();
            let _ = diagnostics
                .seek(SeekFrom::Start(diagnostics_start))
                .and_then(|_| diagnostics.read_to_string(&mut stderr));
            let decoded = turn.finish(interrupted, &stderr);
            state.publish(decoded, &events);
        })
    }
}

/// The options a headless turn and a terminal share.
fn options(
    config: &AgentConfig,
    thread: Option<&AgentSessionId>,
    kind: LaunchKind,
) -> Result<Vec<String>> {
    let mut args = Vec::new();
    if let Some(model) = &config.agent.model {
        args.extend(["--model".into(), model.clone()]);
    }
    if let Some(thread) = thread {
        args.extend(["--conversation".into(), thread.to_string()]);
    }
    if let Some(folder) = launch_folder(config, kind)? {
        args.extend(["--add-dir".into(), folder.display().to_string()]);
    }
    Ok(args)
}

/// A folder of this launch's own that `--add-dir` hands to `agy`, which reads
/// `.agents/AGENTS.md` there as instructions and `.agents/hooks.json` as
/// hooks. Nothing is written to `~/.gemini` or to the checkout's own
/// `.agents`. It sits under the launch's directory because Antigravity runs
/// commands in the first workspace folder by path order, and a folder under
/// the checkout always sorts after it. `None` when the launch has neither
/// instructions nor a conversation context.
fn launch_folder(config: &AgentConfig, kind: LaunchKind) -> Result<Option<PathBuf>> {
    let instructions = crate::agent::system_prompt_with_structured_replies(config);
    let instructions = Some(instructions).filter(|text| !text.trim().is_empty());
    let context = config.conversation_context.as_ref();
    if instructions.is_none() && context.is_none() {
        return Ok(None);
    }
    let cwd = match &config.cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir()?,
    };
    let prompts = cwd.join(".lf/prompts");
    std::fs::create_dir_all(&prompts)?;
    crate::git::exclude(&cwd, ".lf/prompts/")?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let folder = tempfile::Builder::new()
        .prefix(&format!("{stamp}-"))
        .suffix("-antigravity")
        .tempdir_in(&prompts)?
        .keep();
    let agents = folder.join(".agents");
    std::fs::create_dir(&agents)?;
    if let Some(instructions) = instructions {
        std::fs::write(agents.join("AGENTS.md"), instructions)?;
    }
    if let Some(context) = context {
        let script = folder.join("hook.sh");
        std::fs::write(&script, hook_script(context)?)?;
        let hooks = hooks(&script, kind);
        std::fs::write(agents.join("hooks.json"), serde_json::to_vec(&hooks)?)?;
    }
    Ok(Some(folder))
}

/// The hooks lf declares. `PreInvocation` runs before every model call, so
/// it is the start hook, the prompt hook and the after-a-tool hook; a managed
/// launch also reports its turn ends. Nothing else is declared: a
/// `PreToolUse` handler that answers `{}` denies every tool.
fn hooks(script: &Path, kind: LaunchKind) -> Value {
    let run = |moment: &str| {
        serde_json::json!([{"type": "command", "command": format!(
            "/bin/sh {} {moment}",
            crate::os_process::shell_escape(&script.to_string_lossy()),
        )}])
    };
    let mut hooks = serde_json::json!({});
    if kind.is_managed() {
        // A managed prompt is a boundary the server records.
        hooks["PreInvocation"] = run("tool");
        hooks["Stop"] = run("stop");
    } else {
        hooks["PreInvocation"] = run("steer");
    }
    serde_json::json!({"loopflow": hooks})
}

/// The script every declared hook runs, with the moment it declares. It
/// decides one thing, whether starting lf is worth it: a model call inside a
/// turn (`invocationNum` 1 and up) starts lf only when a steer may be
/// waiting. lf reads the payload. A payload the pattern does not recognize
/// starts lf, and a quote inside the user's text is escaped in the payload,
/// so that text cannot match it. The script always exits 0 with a reply:
/// `{}` when lf printed nothing or could not start.
fn hook_script(context: &crate::context_block::ContextDelivery) -> Result<String> {
    Ok(format!(
        r#"payload=$(cat)
quiet() ( {gate} exit 1 )
case "$1:$payload" in
stop:*) ;;
*'"invocationNum":'[1-9]*) quiet && {{ echo '{{}}'; exit 0; }} ;;
esac
reply=$(printf '%s' "$payload" | {command} "$1" --harness antigravity)
if [ -n "$reply" ]; then printf '%s\n' "$reply"; else echo '{{}}'; fi
exit 0
"#,
        gate = context.steer_gate(),
        command = context.hook_command()?,
    ))
}

/// Whether the conversation `id` is the one the launch owns, kept in
/// `thread_file`, and whether this callback is the first to say so. The
/// launch's own conversation calls first; a subagent runs the same hooks
/// later under its own id.
fn claim_thread(thread_file: &Path, id: &str) -> Option<bool> {
    match std::fs::read_to_string(thread_file) {
        Ok(owner) if !owner.trim().is_empty() => (owner.trim() == id).then_some(false),
        _ => {
            if let Err(error) = std::fs::write(thread_file, id) {
                tracing::warn!(%error, "cannot keep the Antigravity conversation this launch owns");
            }
            Some(true)
        }
    }
}

#[async_trait]
impl StreamDriver for Driver {
    fn set_runtime(&mut self, runtime: Option<crate::process::LfProcessRuntime>) {
        self.runtime = runtime;
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }

    fn process_group_id(&self) -> Option<u32> {
        // Admission makes this child the leader of its own group.
        self.pid.filter(|pid| *pid > 1)
    }

    async fn start(&mut self) -> Result<()> {
        // `agy` names a new conversation in its first line.
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let Some(message) = build_turn_content(content, &self.config, self.first_turn) else {
            return Ok(());
        };
        let thread = {
            let state = self.state.lock().expect("Antigravity state lock poisoned");
            if state.turn.is_some() {
                return Err(HarnessError::TurnAlreadyInProgress.into());
            }
            state.thread.clone()
        };
        self.settle_process().await?;
        {
            // Each run is a new AgentProcess of the same Session.
            let Recording {
                store,
                session,
                attachment,
            } = &mut self.owner;
            *attachment = match &self.runtime {
                Some(runtime) => runtime.prepare_agent_process(session, attachment)?,
                None => crate::process::attachment::prepare_session_agent_process(
                    store, session, attachment,
                )?,
            };
            self.config.env.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(
                    &attachment.caller(session.clone().into(), crate::journal::current_trace_id()),
                )?,
            );
        }
        let owner = self.owner.clone();
        let (command, diagnostics) = self.command(&self.config, thread.as_ref())?;
        let diagnostics_start = diagnostics.metadata()?.len();
        let mut child = super::agent_process::spawn(command, None, &owner, self.kind)?;
        self.pid = child.id();
        let mut stdin = child.stdin.take().expect("Antigravity stdin was piped");
        let stdout = child.stdout.take().expect("Antigravity stdout was piped");

        {
            let Recording {
                store,
                session,
                attachment,
            } = owner.clone();
            let mut state = self.state.lock().expect("Antigravity state lock poisoned");
            state.interrupted = false;
            state.writer = Some(FactWriter::new(
                store,
                session,
                Some(attachment),
                Default::default(),
            ));
            state.turn = Some(Turn::new(thread));
        }
        self.reader = Some(self.spawn_reader(child, stdout, diagnostics, diagnostics_start));

        // `agy` runs one turn per line of input and exits at end of input.
        let line = format!(
            "{}\n",
            serde_json::json!({"event": "user", "message": {"content": message}})
        );
        let Recording {
            store,
            session,
            attachment,
        } = owner;
        let sent = tokio::task::spawn_blocking(move || {
            store.with_session_dispatch(&session, &attachment, || {
                super::dispatch::write_fenced(&mut stdin, line.as_bytes())
            })
        })
        .await?;
        if let Err(error) = sent {
            self.settle_process().await?;
            return Err(error.into());
        }
        self.first_turn = false;
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // A run has no in-band cancel: stopping its process ends the turn, and
        // the next input continues the conversation by its `--conversation`.
        {
            let mut state = self.state.lock().expect("Antigravity state lock poisoned");
            if state.turn.is_none() {
                return Ok(());
            }
            state.interrupted = true;
        }
        self.settle_process().await
    }

    async fn stop(&mut self) -> Result<()> {
        self.state
            .lock()
            .expect("Antigravity state lock poisoned")
            .interrupted = true;
        self.settle_process().await
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.state
            .lock()
            .expect("Antigravity state lock poisoned")
            .thread
            .clone()
    }
}

fn build_turn_content(content: &str, config: &AgentConfig, first_turn: bool) -> Option<String> {
    let task = if first_turn {
        config.task_prompt.trim()
    } else {
        ""
    };
    let parts: Vec<&str> = [task, content.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

#[derive(Debug)]
pub struct Antigravity;

impl super::Harness for Antigravity {
    fn kind(&self) -> super::HarnessKind {
        super::HarnessKind::Antigravity
    }
    fn program(&self) -> &'static str {
        "agy"
    }
    fn capabilities(&self) -> super::Capabilities {
        super::Capabilities {
            thread: super::ThreadNaming {
                bare: super::ThreadName::Reported,
                managed: super::ThreadName::Reported,
            },
            initial_prompt_on_resume: false,
            native_skills: None,
            turn_limit: false,
            output_schema: false,
            respawns_after_crash: false,
            install_hint:
                "Install it with `brew install --cask antigravity-cli`; the command is `agy`.",
        }
    }
    fn classify_failure(&self, code: &str, message: &str) -> Option<AgentFailure> {
        let words = message.to_ascii_lowercase();
        let says = |phrases: &[&str]| phrases.iter().any(|phrase| words.contains(phrase));
        // "authentication failed or timed out" is a signed-out `agy`; the
        // quota phrases are the binary's own strings, not a refusal seen live.
        if says(&["authentication required", "authentication failed"]) {
            Some(AgentFailure::CredentialInvalidated)
        } else if says(&[
            "exhausted your quota",
            "quota reached",
            "quota_exhausted",
            "resource_exhausted",
            "usage limit reached",
            "out of credits",
        ]) {
            Some(AgentFailure::UsageLimit)
        } else if says(&["is not recognized as a known model"]) {
            Some(AgentFailure::ModelUnavailable)
        } else {
            // `agy` is taken to retry a transient failure itself.
            crate::agent::routing_failure(code, message)
        }
    }
    fn hook_callback(
        &self,
        payload: Value,
        declared: ContextMoment,
        thread_file: Option<&Path>,
    ) -> Option<HookCallback> {
        let conversation = payload["conversationId"]
            .as_str()
            .filter(|id| !id.is_empty())?;
        let first = match thread_file {
            Some(file) => claim_thread(file, conversation)?,
            None => false,
        };
        let stop = payload["hookEventName"] == "Stop" || declared == ContextMoment::Stop;
        // Each `agy` process counts its model calls from 0, and call 0 of a
        // later turn in the same process is that turn's prompt.
        let (moment, event) = if stop {
            (ContextMoment::Stop, "Stop")
        } else if first {
            (ContextMoment::Start, "SessionStart")
        } else if payload["invocationNum"] == 0 {
            (declared, "UserPromptSubmit")
        } else {
            (ContextMoment::Steer, "PostToolUse")
        };
        let resumed = payload["initialNumSteps"].as_i64().is_some_and(|n| n > 1);
        Some(HookCallback {
            moment,
            payload: serde_json::json!({
                "session_id": conversation,
                "hook_event_name": event,
                "source": if resumed { "resume" } else { "startup" },
            }),
            prompt: moment == ContextMoment::Start,
        })
    }
    fn hook_reply(&self, _event: &str, text: &str) -> Value {
        serde_json::json!({"injectSteps": [{"systemMessage": {"systemMessage": text}}]})
    }
    fn sign_in_advice(&self) -> &'static str {
        "run `agy` and sign in"
    }
    fn recognizes(&self, process: &crate::process::OsProcess) -> bool {
        // `agy` execs a process named `antigravity`.
        matches!(process.command.as_str(), "agy" | "antigravity")
    }
    fn recognizes_driver(&self, command: &str) -> bool {
        (super::command_program(command, "agy") || super::command_program(command, "antigravity"))
            && command
                .split_whitespace()
                .any(|arg| arg == "--output-format")
    }
    fn transcript_home(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".gemini/antigravity-cli")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<crate::transcript::Turn> {
        crate::transcript::antigravity::read
    }
    fn recent_transcripts(
        &self,
        _home: &std::path::Path,
        _cwd: &std::path::Path,
        _since: i64,
        _owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, crate::transcript::Source, i64)> {
        // A transcript row carries no directory, so none can be matched to a checkout.
        Vec::new()
    }
    fn driver(
        &self,
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        _approval: super::ApprovalPolicy,
    ) -> Box<dyn super::Driver> {
        Box::new(Driver::new(launch, events))
    }
    fn open(&self, launch: &super::Launch<'_>) -> Result<super::native_session::NativeClient> {
        launch.terminal_input()?;
        bail!("Antigravity opens no terminal yet; run it headless with `lf -b`")
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{mpsc, AgentConfig, ContextMoment, ConversationEvent, Driver, StreamDriver};
    use crate::harness::HarnessKind;
    use loopflow_model::chat::types::{ConversationItem, Lifecycle};

    /// Put the stand-in `agy` first on PATH; it keeps what it was sent in `home`.
    fn stand_in(home: &std::path::Path) -> crate::test_ambient::EnvGuard {
        use std::os::unix::fs::PermissionsExt;
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("agy");
        std::fs::write(
            &script,
            include_str!("../../tests/support/antigravity_cli.py"),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // The stand-in is Python, found on the PATH the test began with.
        let path = std::env::var_os("PATH").unwrap_or_default();
        let guard = crate::test_ambient::EnvGuard::clear(&["PATH", "LF_BIN"]);
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(&path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        guard
    }

    async fn drive_turn(
        rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    ) -> (Lifecycle, String, Vec<String>) {
        let (mut text, mut errors) = (String::new(), Vec::new());
        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(180), rx.recv())
                .await
                .expect("a turn settles")
                .expect("the driver is alive");
            match event {
                ConversationEvent::ItemCompleted {
                    item: ConversationItem::Message { text: t, phase, .. },
                    ..
                } if phase.as_deref() == Some("final") => text = t,
                ConversationEvent::Error { message, .. } => errors.push(message),
                ConversationEvent::TurnCompleted { status, .. } => return (status, text, errors),
                _ => {}
            }
        }
    }

    fn received(home: &std::path::Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(home.join("received.json")).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn each_turn_is_one_run_that_continues_the_conversation_agy_named() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _path = stand_in(ledger.home());
        let config = AgentConfig {
            agent: crate::config::Agent::parse("antigravity:gemini-fixture").unwrap(),
            task_prompt: "the task".into(),
            system_prompt: "Answer in French.".into(),
            cwd: Some(ledger.home().to_owned()),
            ..AgentConfig::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(HarnessKind::Antigravity, &config, &recording),
            tx,
        );
        harness.start().await.unwrap();
        assert_eq!(harness.agent_session(), None, "agy names the conversation");

        harness.send_input("and this").await.unwrap();
        let (status, text, errors) = drive_turn(&mut rx).await;
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            (status, text.as_str()),
            (Lifecycle::Completed, "answer to: the task\n\nand this")
        );
        let thread = harness.agent_session().expect("the run named it");
        let first = received(ledger.home());
        let args: Vec<&str> = first["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap())
            .collect();
        assert_eq!(
            args[..8],
            [
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--dangerously-skip-permissions",
                "--model",
                "gemini-fixture",
                "--add-dir"
            ]
        );
        // The instructions ride in a folder of the launch's own, under its directory.
        let folder = std::path::Path::new(args[8]);
        assert!(folder.starts_with(ledger.home().join(".lf/prompts")));
        assert_eq!(
            std::fs::read_to_string(folder.join(".agents/AGENTS.md")).unwrap(),
            "Answer in French."
        );
        assert!(
            !folder.join(".agents/hooks.json").exists(),
            "a launch with no conversation context declares no hooks"
        );

        harness.send_input("again").await.unwrap();
        let (status, text, _) = drive_turn(&mut rx).await;
        assert_eq!(
            (status, text.as_str()),
            (Lifecycle::Completed, "answer to: again"),
            "the task seeds only the first turn"
        );
        let second = received(ledger.home());
        let args = second["args"].as_array().unwrap();
        let at = args.iter().position(|arg| arg == "--conversation").unwrap();
        assert_eq!(args[at + 1], thread.as_str());
        assert_eq!(harness.agent_session(), Some(thread));
        harness.stop().await.unwrap();
    }

    #[tokio::test]
    async fn a_refused_run_fails_the_turn_with_what_agy_said() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _path = stand_in(ledger.home());
        let config = AgentConfig {
            agent: crate::config::Agent::parse("antigravity:no-such-model").unwrap(),
            cwd: Some(ledger.home().to_owned()),
            ..AgentConfig::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(HarnessKind::Antigravity, &config, &recording),
            tx,
        );
        harness.start().await.unwrap();
        harness.send_input("go").await.unwrap();
        let (status, _, errors) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Failed);
        assert_eq!(
            errors,
            ["invalid model selection (--model \"no-such-model\" --effort \"\"): model no-such-model is not recognized as a known model or custom model in settings"]
        );
        assert_eq!(harness.agent_session(), None, "nothing to resume");
        harness.stop().await.unwrap();
    }

    fn callback(
        file: &std::path::Path,
        declared: ContextMoment,
        payload: serde_json::Value,
    ) -> Option<(ContextMoment, String, bool)> {
        use crate::harness::Harness;
        super::Antigravity
            .hook_callback(payload, declared, Some(file))
            .map(|callback| {
                (
                    callback.moment,
                    format!(
                        "{} {} {}",
                        callback.payload["session_id"].as_str().unwrap(),
                        callback.payload["hook_event_name"].as_str().unwrap(),
                        callback.payload["source"].as_str().unwrap()
                    ),
                    callback.prompt,
                )
            })
    }

    #[test]
    fn a_launch_reads_only_its_own_conversations_callbacks() {
        use ContextMoment::{Start, Steer, Stop, Tool};
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("launch.thread");
        let call = |id: &str, event: &str, number: i64, steps: i64| {
            serde_json::json!({"conversationId": id, "hookEventName": event,
                "invocationNum": number, "initialNumSteps": steps, "lastUserInput": "hi"})
        };
        // The launch's first model call is its start and its first prompt.
        assert_eq!(
            callback(&file, Tool, call("parent", "PreInvocation", 0, 1)),
            Some((Start, "parent SessionStart startup".into(), true))
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "parent");
        // A subagent runs the same hooks under its own conversation.
        for event in ["PreInvocation", "Stop"] {
            assert_eq!(callback(&file, Tool, call("child", event, 0, 0)), None);
        }
        assert_eq!(
            callback(&file, Tool, call("parent", "PreInvocation", 1, 3)),
            Some((Steer, "parent PostToolUse resume".into(), false))
        );
        // Call 0 of a later turn is that turn's prompt, as its hook declared it.
        for declared in [Tool, Steer] {
            assert_eq!(
                callback(&file, declared, call("parent", "PreInvocation", 0, 8)),
                Some((declared, "parent UserPromptSubmit resume".into(), false))
            );
        }
        assert_eq!(
            callback(&file, Stop, call("parent", "Stop", 0, 0)),
            Some((Stop, "parent Stop startup".into(), false))
        );
        // A resumed conversation's first call in a new launch starts it again.
        let resumed = directory.path().join("resumed.thread");
        assert_eq!(
            callback(&resumed, Steer, call("parent", "PreInvocation", 0, 8)),
            Some((Start, "parent SessionStart resume".into(), true))
        );
        // A payload that names no conversation is nobody's.
        assert_eq!(
            callback(&file, Tool, serde_json::json!({"invocationNum": 0})),
            None
        );
    }

    /// The script beside a launch's hooks, with a stand-in `lf` that counts
    /// its starts and answers `reply`.
    fn run_hook(home: &std::path::Path, reply: &str, moment: &str, payload: &str) -> String {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let lf = home.join("lf");
        std::fs::write(
            &lf,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncat > /dev/null\n{reply}\n",
                home.join("calls").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("LF_BIN", &lf);
        let context = crate::context_block::ContextDelivery {
            identity: None,
            repository: None,
            repo: home.to_owned(),
            home: home.to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let script = home.join("hook.sh");
        std::fs::write(&script, super::hook_script(&context).unwrap()).unwrap();
        let mut hook = std::process::Command::new("/bin/sh")
            .arg(&script)
            .arg(moment)
            .env(crate::steer::STEER_SEEN_ENV, home.join("seen"))
            .env_remove(crate::steer::STEER_SESSION_ENV)
            .env_remove(crate::steer::STEER_TASK_ENV)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        hook.stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let output = hook.wait_with_output().unwrap();
        assert!(output.status.success(), "the hook always exits 0");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn the_hook_script_starts_lf_inside_a_turn_only_when_a_steer_may_wait() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        std::env::set_var("LF_HOME", home);
        let calls = || std::fs::read_to_string(home.join("calls")).unwrap_or_default();
        let inside = r#"{"conversationId":"c","invocationNum":2,"lastUserInput":"x"}"#;
        // A prompt whose text spells the pattern is still call 0: quotes are escaped.
        let prompt =
            r#"{"conversationId":"c","invocationNum":0,"lastUserInput":"\"invocationNum\":3"}"#;

        // Nothing has checked for steers yet, so a call inside a turn asks.
        let block = "echo '{\"injectSteps\":[]}'";
        assert_eq!(
            run_hook(home, block, "tool", inside),
            r#"{"injectSteps":[]}"#
        );
        // A check newer than every steer: lf stays unstarted and the reply is empty.
        std::fs::write(home.join("seen"), "").unwrap();
        assert_eq!(run_hook(home, block, "tool", inside), "{}");
        assert_eq!(calls().lines().count(), 1, "{}", calls());
        // A prompt and a turn end always start lf.
        assert_eq!(
            run_hook(home, block, "tool", prompt),
            r#"{"injectSteps":[]}"#
        );
        // lf answering nothing, or failing, is still a reply `agy` accepts.
        assert_eq!(run_hook(home, "", "stop", inside), "{}");
        assert_eq!(run_hook(home, "exit 3", "tool", prompt), "{}");
        assert_eq!(
            calls().lines().collect::<Vec<_>>(),
            [
                "__context-block --moment tool --harness antigravity",
                "__context-block --moment tool --harness antigravity",
                "__context-block --moment stop --harness antigravity",
                "__context-block --moment tool --harness antigravity",
            ]
        );
    }

    #[test]
    fn only_a_managed_launch_reports_its_turn_ends() {
        let script = std::path::Path::new("/launch/hook.sh");
        let bare = super::hooks(script, crate::harness::LaunchKind::Batch);
        assert_eq!(
            bare,
            serde_json::json!({"loopflow": {"PreInvocation": [
                {"type": "command", "command": "/bin/sh '/launch/hook.sh' steer"}
            ]}})
        );
        let managed = super::hooks(script, crate::harness::LaunchKind::Worker);
        assert_eq!(
            managed["loopflow"]["Stop"][0]["command"],
            "/bin/sh '/launch/hook.sh' stop"
        );
        assert_eq!(
            managed["loopflow"]["PreInvocation"][0]["command"],
            "/bin/sh '/launch/hook.sh' tool"
        );
        // Any other handler would have to answer, and `{}` denies a tool.
        assert_eq!(managed["loopflow"].as_object().unwrap().len(), 2);
    }

    /// Needs `agy` on PATH, signed in:
    ///   cargo test -p loopflow-execution --lib antigravity::tests::live_ -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "drives the real agy; needs the antigravity-cli and its sign-in"]
    async fn live_basic_turn_completes() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let config = AgentConfig {
            agent: crate::config::Agent::parse("antigravity").unwrap(),
            cwd: Some(std::env::temp_dir()),
            ..AgentConfig::default()
        };
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(HarnessKind::Antigravity, &config, &recording),
            tx,
        );
        harness.start().await.expect("start");
        harness
            .send_input("Reply with exactly: ALPHA")
            .await
            .expect("seed turn");
        let (status, text, errors) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed, "{errors:?}");
        assert!(text.to_uppercase().contains("ALPHA"), "turn text: {text:?}");
        assert!(harness.agent_session().is_some());
        harness.stop().await.expect("stop");
    }
}
