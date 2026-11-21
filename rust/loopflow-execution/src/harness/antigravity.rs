//! Antigravity, whose CLI is `agy`. A headless turn is one
//! `agy --input-format stream-json --output-format stream-json` process that
//! reads one message and exits; `agy` names the conversation in its first
//! line, and the next turn continues it with `--conversation`. A terminal is
//! `agy -i <prompt>`, whose first hook callback names the conversation.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::common::{Run, Stderr};

use crate::agent::{AgentConfig, AgentFailure};
use crate::context_block::ContextMoment;
use crate::harness::fact_writer::FactWriter;
use crate::harness::{Driver as StreamDriver, HarnessError, HookCallback, LaunchKind, Recording};
use loopflow_harness::antigravity_decode::Turn;
use loopflow_harness::Harness;
use loopflow_model::chat::types::ConversationEvent;
use loopflow_model::id::AgentSessionId;

#[derive(Debug, Default)]
struct State {
    /// The conversation, once lf resumed it or `agy` named it.
    thread: Option<AgentSessionId>,
    turn: Option<Turn>,
    /// Writes what the current process's turn establishes.
    writer: Option<FactWriter>,
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
    run: Run,
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
            run: Run::default(),
        }
    }

    /// The run for one turn, and where its stderr goes.
    fn command(&self, thread: Option<&AgentSessionId>) -> Result<(Command, Stderr)> {
        let config = &self.config;
        let mut command = Command::new(loopflow_harness::HarnessKind::Antigravity.as_str());
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
        let stderr = Stderr::pipe(
            &mut command,
            config,
            loopflow_harness::HarnessKind::Antigravity.as_str(),
        )?;
        Ok((command, stderr))
    }

    fn spawn_reader(
        &self,
        mut child: Child,
        stdout: ChildStdout,
        stderr: Stderr,
        stopped: Arc<AtomicBool>,
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
                    super::common::publish(state.writer.as_mut(), decoded, &events);
                }
            }
            let _ = child.wait().await;
            let mut state = state.lock().expect("Antigravity state lock poisoned");
            let Some(mut turn) = state.turn.take() else {
                return;
            };
            // Stopped by this driver, or with the lf process a signal is ending.
            let interrupted = stopped.load(Ordering::SeqCst) || crate::agent::interrupted();
            let decoded = turn.finish(interrupted, &stderr.read());
            super::common::publish(state.writer.as_mut(), decoded, &events);
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
        .suffix("-agy")
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
    // A managed prompt is a boundary the server records.
    let managed = kind.is_managed();
    let mut hooks =
        serde_json::json!({"PreInvocation": run(if managed { "tool" } else { "steer" })});
    if managed {
        hooks["Stop"] = run("stop");
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
reply=$(printf '%s' "$payload" | {command} "$1" --harness agy)
if [ -n "$reply" ]; then printf '%s\n' "$reply"; else echo '{{}}'; fi
exit 0
"#,
        gate = context.steer_gate(),
        command = context.hook_command()?,
    ))
}

/// Point `thread_file` at the conversation `id` and say whether that moved
/// it: the launch's first callback, and the first after `/new`, `/fork` or
/// `/resume` in the terminal.
fn follow_thread(thread_file: &Path, id: &str) -> bool {
    if std::fs::read_to_string(thread_file).is_ok_and(|current| current.trim() == id) {
        return false;
    }
    if let Err(error) = std::fs::write(thread_file, id) {
        tracing::warn!(%error, "cannot keep the Antigravity conversation this launch is in");
    }
    true
}

#[async_trait]
impl StreamDriver for Driver {
    fn set_runtime(&mut self, runtime: Option<crate::process::LfProcessRuntime>) {
        self.runtime = runtime;
    }

    fn pid(&self) -> Option<u32> {
        self.run.pid
    }

    fn process_group_id(&self) -> Option<u32> {
        self.run.group()
    }

    async fn start(&mut self) -> Result<()> {
        // `agy` names a new conversation in its first line.
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let Some(message) = super::common::turn_content(content, &self.config, self.first_turn)
        else {
            return Ok(());
        };
        let thread = {
            let state = self.state.lock().expect("Antigravity state lock poisoned");
            if state.turn.is_some() {
                return Err(HarnessError::TurnAlreadyInProgress.into());
            }
            state.thread.clone()
        };
        self.run.settle(&self.owner).await?;
        super::common::next_agent_process(
            &mut self.owner,
            self.runtime.as_ref(),
            &mut self.config,
        )?;
        let owner = self.owner.clone();
        let (command, stderr) = self.command(thread.as_ref())?;
        let mut child = super::agent_process::spawn(command, None, &owner, self.kind)?;
        let stopped = self.run.began(child.id());
        let stdin = child.stdin.take().expect("Antigravity stdin was piped");
        let stdout = child.stdout.take().expect("Antigravity stdout was piped");

        {
            let mut state = self.state.lock().expect("Antigravity state lock poisoned");
            state.writer = Some(super::common::writer(&owner));
            state.turn = Some(Turn::new(thread));
        }
        self.run.reader = Some(self.spawn_reader(child, stdout, stderr, stopped));

        // `agy` runs one turn per line of input and exits at end of input.
        let line = format!(
            "{}\n",
            serde_json::json!({"event": "user", "message": {"content": message}})
        );
        if let Err(error) = super::common::send(owner, stdin, line.into_bytes()).await {
            self.run.settle(&self.owner).await?;
            return Err(error);
        }
        self.first_turn = false;
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // A run has no in-band cancel: stopping its process ends the turn, and
        // the next input continues the conversation by its `--conversation`.
        {
            let state = self.state.lock().expect("Antigravity state lock poisoned");
            if state.turn.is_none() {
                return Ok(());
            }
        }
        self.run.stop(&self.owner).await
    }

    async fn stop(&mut self) -> Result<()> {
        self.run.stop(&self.owner).await
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.state
            .lock()
            .expect("Antigravity state lock poisoned")
            .thread
            .clone()
    }
}

impl super::Launcher for loopflow_harness::Antigravity {
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
            install_hint: "Install it with `brew install --cask antigravity-cli`.",
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
        // A subagent runs the same hooks under its own id, and nobody typed
        // into it. Every other conversation is one the person is in.
        if payload["lastUserInput"].as_str().is_none_or(str::is_empty) {
            return None;
        }
        // A model call comes before a conversation's stop, so a stop moves
        // nothing. Each `agy` process counts its model calls from 0, and call
        // 0 of a later turn in the same process is that turn's prompt.
        let (moment, event) = if declared == ContextMoment::Stop {
            (ContextMoment::Stop, "Stop")
        } else if thread_file.is_some_and(|file| follow_thread(file, conversation)) {
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
    fn recognizes(&self, process: &crate::process::OsProcess) -> bool {
        // `agy` execs a process named `antigravity`.
        matches!(process.command.as_str(), "agy" | "antigravity")
    }
    fn locate_transcript(
        &self,
        _store: &loopflow_storage::sqlite::SqliteStore,
        id: &AgentSessionId,
        _cwd: &std::path::Path,
    ) -> Result<crate::transcript::Located> {
        let home = self.transcript_home();
        let path = loopflow_harness::transcript::antigravity::file(&home, id.as_str());
        Ok(if path.is_file() {
            crate::transcript::Located::Found {
                source: loopflow_harness::transcript::Source::File(path),
                home: None,
            }
        } else {
            crate::transcript::Located::Gone(crate::transcript::Gap::FileMissing { home })
        })
    }
    fn recent_transcripts(
        &self,
        _home: &std::path::Path,
        _cwd: &std::path::Path,
        _since: i64,
        _owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, loopflow_harness::transcript::Source, i64)> {
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
        let (mut args, input) = launch.terminal_input()?;
        let mut client = super::native_session::NativeClient::new(launch)?;
        args.extend(options(launch.config, client.thread.as_ref(), launch.kind)?);
        if launch.config.skips_permissions(launch.kind.mode()) {
            args.push("--dangerously-skip-permissions".into());
        }
        let cwd = client.process.get_current_dir();
        if let Some(git) = cwd.and_then(main_git_dir) {
            args.extend(["--add-dir".into(), git.display().to_string()]);
        }
        if let Some(trust) = cwd.and_then(trust) {
            args.extend(["--settings".into(), trust]);
        }
        if let Some(input) = input.filter(|input| !input.is_empty()) {
            crate::agent::validate_terminal_turn(&input)?;
            args.extend(["-i".into(), input]);
        }
        if let Some(context) = &launch.config.conversation_context {
            let delivery = context.configure_hooks(&mut client.process)?;
            // Hook callbacks keep the conversation the person is in there:
            // a new one, or one `/new`, `/fork` or `/resume` switched to.
            client.reported_thread = Some(crate::context_block::reported_thread_path(&delivery));
        }
        client.process.args(args);
        Ok(client)
    }
}

/// The main repository's Git directory, for a terminal in one of its
/// worktrees. The TUI runs commands in a sandbox that reaches only workspace
/// folders, and a worktree keeps its Git data outside its own: without this
/// every Git command there answers "fatal: not a git repository" (seen live).
/// Left out when it would sort before the launch directory, because `agy`
/// would then run commands in it. A worktree lf created never sorts so. Both
/// are compared as the filesystem resolves them, which is how `agy` names
/// the directory it starts in.
fn main_git_dir(cwd: &Path) -> Option<PathBuf> {
    let main = crate::agent::workspace_add_dirs(cwd).pop()?;
    let git = main.join(".git").canonicalize().ok()?;
    (git.as_os_str() > cwd.canonicalize().ok()?.as_os_str()).then_some(git)
}

/// Settings for one session that let the TUI start in a worktree lf created
/// without asking whether its folder is trusted. Nothing is saved; a main
/// checkout and a hand-made worktree keep the dialog. `agy` was seen to
/// accept the folder with both spellings of its path given, so both go.
fn trust(cwd: &Path) -> Option<String> {
    crate::git::lf_created_worktree(cwd)?;
    let mut folders = vec![cwd.to_owned()];
    folders.extend(cwd.canonicalize().ok().filter(|real| real != cwd));
    Some(serde_json::json!({"trustedWorkspaces": folders}).to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::common::drive_turn;
    use super::{mpsc, AgentConfig, ContextMoment, Driver, StreamDriver};
    use loopflow_harness::HarnessKind;
    use loopflow_model::chat::types::Lifecycle;

    /// Put the stand-in `agy` first on PATH; it keeps what it was sent in `home`.
    fn stand_in(home: &std::path::Path) -> crate::test_ambient::EnvGuard {
        use std::os::unix::fs::PermissionsExt;
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("agy");
        std::fs::write(&script, include_str!("../../tests/support/agy_cli.py")).unwrap();
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

    fn received(home: &std::path::Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(home.join("received.json")).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn each_turn_is_one_run_that_continues_the_conversation_agy_named() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _path = stand_in(ledger.home());
        let config = AgentConfig {
            agent: crate::config::Agent::parse("agy:gemini-fixture").unwrap(),
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
            agent: crate::config::Agent::parse("agy:no-such-model").unwrap(),
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

    fn terminal_args(config: &AgentConfig, recording: &crate::harness::Recording) -> Vec<String> {
        use crate::harness::Launcher;
        let launch = crate::harness::Launch {
            kind: crate::harness::LaunchKind::Chat,
            ..crate::harness::launch_for_test(HarnessKind::Antigravity, config, recording)
        };
        let client = loopflow_harness::Antigravity.open(&launch).unwrap();
        client
            .process
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_terminal_starts_the_prompt_and_resumes_by_conversation() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let cwd = tempfile::tempdir().unwrap();
        let config = AgentConfig {
            agent: crate::config::Agent::parse("agy:gemini-fixture").unwrap(),
            task_prompt: "-start here".into(),
            cwd: Some(cwd.path().to_owned()),
            ..AgentConfig::default()
        };
        // A folder lf did not create keeps the trust dialog, and a person
        // who did not ask to skip permissions is asked.
        assert_eq!(
            terminal_args(&config, &recording),
            ["--model", "gemini-fixture", "-i", "-start here"]
        );
        let resumed = AgentConfig {
            resume_token: Some("conversation-1".into()),
            task_prompt: String::new(),
            yolo: true,
            ..config
        };
        assert_eq!(
            terminal_args(&resumed, &recording),
            [
                "--model",
                "gemini-fixture",
                "--conversation",
                "conversation-1",
                "--dangerously-skip-permissions"
            ]
        );
    }

    #[test]
    fn a_terminal_in_a_worktree_lf_created_is_trusted_for_that_session() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("kata");
        std::fs::create_dir(&repo).unwrap();
        crate::git::git_stdout(&repo, &["init", "-b", "main"]).unwrap();
        crate::git::git_stdout(
            &repo,
            &[
                "-c",
                "user.email=lf@example.com",
                "-c",
                "user.name=lf",
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ],
        )
        .unwrap();
        let worktree = root.path().join("kata.agent-1");
        crate::git::worktree_add(
            &repo,
            &worktree,
            "agent-1",
            crate::git::WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .unwrap();
        let args = |cwd: &std::path::Path| {
            let config = AgentConfig {
                agent: crate::config::Agent::parse("agy").unwrap(),
                cwd: Some(cwd.to_owned()),
                ..AgentConfig::default()
            };
            terminal_args(&config, &recording)
        };
        // The path as given and as the filesystem resolves it.
        let folders = [worktree.clone(), worktree.canonicalize().unwrap()];
        let mut expected = vec![folders[0].clone()];
        expected.extend((folders[1] != folders[0]).then(|| folders[1].clone()));
        // Its Git data lives in the main repository, outside the sandbox.
        let git = repo.join(".git").canonicalize().unwrap();
        assert_eq!(
            args(&worktree),
            [
                "--add-dir".to_string(),
                git.display().to_string(),
                "--settings".to_string(),
                serde_json::json!({"trustedWorkspaces": expected}).to_string()
            ]
        );
        assert!(args(&repo).is_empty(), "a main checkout keeps the dialog");
        // A worktree that sorts after the repository would hand `agy` the
        // Git directory as the folder its commands run in.
        let later = root.path().join("z-agent-2");
        crate::git::worktree_add(
            &repo,
            &later,
            "agent-2",
            crate::git::WorktreeBranch::New {
                start_point: "HEAD",
            },
        )
        .unwrap();
        assert!(!args(&later).contains(&"--add-dir".to_string()));
    }

    fn callback(
        file: &std::path::Path,
        declared: ContextMoment,
        payload: serde_json::Value,
    ) -> Option<(ContextMoment, String, bool)> {
        use crate::harness::Launcher;
        loopflow_harness::Antigravity
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
    fn a_launch_follows_the_conversation_a_person_is_in() {
        use ContextMoment::{Start, Steer, Stop, Tool};
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("launch.thread");
        let said = |id: &str, event: &str, number: i64, steps: i64, input: &str| {
            serde_json::json!({"conversationId": id, "hookEventName": event,
                "invocationNum": number, "initialNumSteps": steps, "lastUserInput": input})
        };
        let call =
            |id: &str, event: &str, number: i64, steps: i64| said(id, event, number, steps, "hi");
        let thread = || std::fs::read_to_string(&file).unwrap();
        // The launch's first model call is its start and its first prompt.
        assert_eq!(
            callback(&file, Tool, call("parent", "PreInvocation", 0, 1)),
            Some((Start, "parent SessionStart startup".into(), true))
        );
        assert_eq!(thread(), "parent");
        // A subagent runs the same hooks under its own conversation, and
        // nobody typed into it.
        for (declared, event) in [(Tool, "PreInvocation"), (Stop, "Stop")] {
            assert_eq!(
                callback(&file, declared, said("child", event, 0, 0, "")),
                None
            );
        }
        assert_eq!(thread(), "parent");
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
        // `/new` continues the same `agy` under a new conversation: a start.
        assert_eq!(
            callback(&file, Tool, call("new", "PreInvocation", 0, 1)),
            Some((Start, "new SessionStart startup".into(), true))
        );
        assert_eq!(thread(), "new");
        assert_eq!(
            callback(&file, Stop, call("new", "Stop", 0, 0)),
            Some((Stop, "new Stop startup".into(), false))
        );
        // `/fork` carries the history along, so its start is a resumed one.
        assert_eq!(
            callback(&file, Tool, call("fork", "PreInvocation", 0, 3)),
            Some((Start, "fork SessionStart resume".into(), true))
        );
        assert_eq!(thread(), "fork");
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
                "__context-block --moment tool --harness agy",
                "__context-block --moment tool --harness agy",
                "__context-block --moment stop --harness agy",
                "__context-block --moment tool --harness agy",
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
            agent: crate::config::Agent::parse("agy").unwrap(),
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
