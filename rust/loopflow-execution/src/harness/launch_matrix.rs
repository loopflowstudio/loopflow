//! Launch contracts exercised through the real native runner and stream loop.
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::agent::{AgentConfig, ProcessConfig};
use crate::harness::LaunchKind;
use crate::harness::{Launch, Mode, Recording, Start};
use crate::process::LfProcessRuntime;
use crate::session_record::{SessionCaptureSpec, SessionGraphMembership, SessionRecorder};
use loopflow_harness::HarnessKind;

fn executable(path: &Path, source: &str) {
    std::fs::write(path, source).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The value the matrix gives every caller-session marker in its own
/// environment, as if `lf` ran from another conversation's shell.
const CALLER: &str = "caller";

fn providers(home: &Path) {
    let bin = home.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let caller_session = serde_json::to_string(crate::agent::CALLER_SESSION_ENV).unwrap();
    let record = format!(
        r#"
import json, os, sys, uuid
from pathlib import Path
with (Path(os.environ["LF_HOME"]) / "matrix-argv.jsonl").open("a") as log:
    log.write(json.dumps({{"args":sys.argv[1:], "managed":"LF_PROVIDER_CONTEXT" in os.environ, "delivery":"LF_CONTEXT_DELIVERY" in os.environ, "opencode":os.environ.get("OPENCODE_CONFIG_CONTENT"), "inherited":[name for name in {caller_session} if os.environ.get(name) == "{CALLER}"], "config_dir":os.environ.get("CLAUDE_CONFIG_DIR")}}) + "\n")
"#
    );
    executable(
        &bin.join("claude"),
        &format!(
            r#"#!/usr/bin/python3
{record}
if "--version" in sys.argv: print("fixture"); sys.exit(0)
if "-p" not in sys.argv: sys.stdin.read(); sys.exit(0)
thread = "matrix-" + uuid.uuid4().hex
for line in sys.stdin:
    json.loads(line)
    print(json.dumps({{"type":"system","subtype":"init","session_id":thread}}), flush=True)
    print(json.dumps({{"type":"result","subtype":"success","result":"matrix answer"}}), flush=True)
"#
        ),
    );
    std::fs::write(
        bin.join("codex_socket.py"),
        include_str!("../../tests/support/codex_socket.py"),
    )
    .unwrap();
    executable(
        &bin.join("codex"),
        &format!(
            "#!/usr/bin/python3\n{record}\n{}",
            include_str!("../../tests/support/native_codex.py")
                .replace("args = sys.argv[1:]", "args = sys.argv[1:]\n    if 'app-server' not in args and '--remote' not in args and os.environ.get('LF_CONTEXT_DELIVERY'):\n        Path(os.environ['LF_CONTEXT_DELIVERY']).with_suffix('.thread').write_text('matrix-bare-' + uuid.uuid4().hex)")
                .replace("from __future__ import annotations", "")
        ),
    );
    executable(
        &bin.join("opencode"),
        &format!(
            "#!/usr/bin/python3\n{record}\n{}",
            include_str!("../../tests/support/opencode_server.py").replace("__WAIT__", "False").replace("if \"serve\" not in sys.argv:\n    _record_launch()", "if \"serve\" not in sys.argv:\n    print('native diagnostic', file=sys.stderr)\n    _record_launch()")
        ),
    );
    // Bare Pi's extension uses the launch snapshot; only managed Pi reports
    // its native conversation to the server. The RPC fixture is shared unchanged.
    executable(
        &bin.join("pi"),
        &format!(
            r#"#!/usr/bin/python3
{record}
if "--mode" not in sys.argv and "LF_PROVIDER_CONTEXT" not in os.environ: sys.exit(0)
{}
"#,
            include_str!("testdata/pi_rpc.py")
        ),
    );
}

#[test]
fn every_harness_runs_all_four_launch_kinds() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    providers(home);
    let binary = loopflow_test_support::lf_binary();
    let _environment = crate::test_ambient::EnvGuard::clear(&[
        "LF_BIN",
        "PATH",
        "CODEX_HOME",
        "OPENCODE_CONFIG_CONTENT",
        "CLAUDE_CONFIG_DIR",
    ]);
    std::env::set_var("LF_BIN", binary);
    std::env::set_var(
        "PATH",
        format!("{}:/usr/bin:/bin", home.join("bin").display()),
    );
    let _server = loopflow_test_support::MachineServer::start(binary, home);
    // From here the test runs as if inside a Claude Code or Codex shell: their
    // markers must not reach a provider, while a person's own setting does.
    // lf's markers stay unset: this process is lf and reads them itself.
    let provider_markers: Vec<&str> = crate::agent::CALLER_SESSION_ENV
        .iter()
        .copied()
        .filter(|name| !name.starts_with("LF_"))
        .collect();
    let _caller = crate::test_ambient::EnvGuard::clear(&provider_markers);
    for name in &provider_markers {
        std::env::set_var(name, CALLER);
    }
    let config_dir = home.join("claude-config");
    std::env::set_var("CLAUDE_CONFIG_DIR", &config_dir);

    for harness in HarnessKind::ALL {
        for (kind, mode, reports) in [
            (LaunchKind::Chat, Mode::Interactive, false),
            (LaunchKind::Batch, Mode::Headless, false),
            (LaunchKind::WorkSession, Mode::Interactive, true),
            (LaunchKind::Worker, Mode::Headless, true),
        ] {
            assert_eq!(kind.mode(), mode, "{kind:?}");
            let row = format!("{harness:?} {kind:?}");
            let directory = tempfile::tempdir_in(home).unwrap();
            let cwd = directory.path();
            crate::journal::with_runtime(home, &["lf".into(), "matrix".into()], || {
                let capture = SessionRecorder::begin_at(
                    home,
                    SessionCaptureSpec {
                        harness: harness.as_str().into(),
                        model: None,
                        surface: if mode == Mode::Interactive {
                            "tui"
                        } else {
                            "headless"
                        }
                        .into(),
                        cwd: cwd.into(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: vec![],
                        graph: SessionGraphMembership::Independent,
                        work: None,
                    },
                )
                .map(LfProcessRuntime::new)?;
                capture.claim_session_attachment()?;
                let recording = Recording::from_runtime(&capture)?;
                let mut config = AgentConfig {
                    agent: crate::config::Agent::parse(harness.as_str()).unwrap(),
                    cwd: Some(cwd.into()),
                    system_prompt: "fixed instructions".into(),
                    chrome: Some(true),
                    skip_permissions: true,
                    write_scope: if harness == HarnessKind::Pi {
                        crate::agent::AgentWriteScope::Configured
                    } else {
                        crate::agent::AgentWriteScope::Worktree
                    },
                    task_prompt: "say hi".into(),
                    conversation_context: Some(crate::context_block::ContextDelivery {
                        identity: None,
                        repository: None,
                        repo: home.into(),
                        home: home.into(),
                        wave: None,
                        skill_file: None,
                        submitted_docs: None,
                        references: vec![],
                    }),
                    env: capture.environment(),
                    ..crate::agent::AgentConfig::default()
                };
                config.env.insert("LF_BIN".into(), binary.into());
                let launch = Launch {
                    config: &config,
                    harness: crate::harness::launcher(harness),
                    kind,
                    recording: &recording,
                    start: Start::New,
                    context_file: None,
                };
                let before = std::fs::read_to_string(home.join("matrix-argv.jsonl"))
                    .unwrap_or_default()
                    .lines()
                    .count();
                let thread = match mode {
                    Mode::Interactive => {
                        let outcome = super::native_session::run(&launch, None, Some(&capture))?;
                        anyhow::ensure!(!outcome.failed(), "{row}: {:?}", outcome.status);
                        if harness == HarnessKind::OpenCode {
                            let log = std::fs::read_to_string(
                                capture.artifact_dir().join("opencode.stderr.log"),
                            )?;
                            anyhow::ensure!(
                                log.contains("native diagnostic"),
                                "{row}: native diagnostics left their log"
                            );
                        }
                        crate::session_record::read_provider_session(&capture.artifact_dir())?
                            .map(|session| session.agent_session)
                    }
                    Mode::Headless => {
                        let outcome = super::drive::run(
                            &launch,
                            &ProcessConfig {
                                runtime: Some(capture.clone()),
                                timeout: Some(std::time::Duration::from_secs(20)),
                                ..Default::default()
                            },
                        )?;
                        anyhow::ensure!(outcome.exit_code == 0, "{row}: {}", outcome.stderr);
                        outcome.agent_session
                    }
                };
                anyhow::ensure!(thread.is_some(), "{row}: no recorded provider thread");
                let recorded = std::fs::read_to_string(home.join("matrix-argv.jsonl"))?;
                let commands: Vec<serde_json::Value> = recorded
                    .lines()
                    .skip(before)
                    .map(serde_json::from_str)
                    .collect::<Result<_, _>>()?;
                anyhow::ensure!(!commands.is_empty(), "{row}: provider did not run");
                let flag = |name: &str| {
                    commands.iter().any(|command| {
                        command["args"]
                            .as_array()
                            .is_some_and(|args| args.iter().any(|arg| arg == name))
                    })
                };
                anyhow::ensure!(
                    flag("--chrome") == (harness == HarnessKind::Claude),
                    "{row}: Chrome flag ownership"
                );
                anyhow::ensure!(
                    !flag("--dangerously-skip-permissions")
                        && !flag("--dangerously-bypass-approvals-and-sandbox"),
                    "{row}: yolo escaped worktree policy"
                );
                if harness == HarnessKind::OpenCode {
                    let settings = commands
                        .iter()
                        .find_map(|command| command["opencode"].as_str())
                        .ok_or_else(|| anyhow::anyhow!("{row}: no server settings"))?;
                    let settings: serde_json::Value = serde_json::from_str(settings)?;
                    anyhow::ensure!(
                        settings["permissions"][1]["action"] == "external_directory"
                            && settings["permissions"][1]["effect"] == "deny",
                        "{row}: worktree permissions lost"
                    );
                }
                for command in &commands {
                    anyhow::ensure!(
                        command["inherited"].as_array().is_some_and(Vec::is_empty),
                        "{row}: provider inherited the caller's session: {command}"
                    );
                    anyhow::ensure!(
                        command["config_dir"] == config_dir.to_string_lossy().as_ref(),
                        "{row}: CLAUDE_CONFIG_DIR did not pass through: {command}"
                    );
                }
                let managed = commands.iter().any(|command| command["managed"] == true);
                anyhow::ensure!(
                    managed == reports,
                    "{row}: wrong hook ownership: {commands:?}"
                );
                println!(
                    "{row}: waited; thread={thread:?}; argv/hooks={commands}",
                    commands = serde_json::to_string(&commands)?
                );
                capture.finish("completed")?;
                Ok(())
            })
            .unwrap_or_else(|error| panic!("{row}: {error:#}"));
        }
    }
}

#[test]
fn held_codex_with_a_saved_empty_thread_sends_its_first_prompt() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    providers(home);
    let binary = loopflow_test_support::lf_binary();
    let _environment = crate::test_ambient::EnvGuard::clear(&["LF_BIN", "PATH", "CODEX_HOME"]);
    std::env::set_var("LF_BIN", binary);
    std::env::set_var(
        "PATH",
        format!("{}:/usr/bin:/bin", home.join("bin").display()),
    );
    let _server = loopflow_test_support::MachineServer::start(binary, home);
    std::fs::write(home.join("native-thread"), "saved-empty-thread").unwrap();
    crate::journal::with_runtime(
        home,
        &["lf".into(), "resume".into()],
        || -> anyhow::Result<()> {
            let capture = SessionRecorder::begin_at(
                home,
                SessionCaptureSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    graph: SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .map(LfProcessRuntime::new)?;
            capture.claim_session_attachment()?;
            let recording = Recording::from_runtime(&capture)?;
            recording.store.record_session_agent_session(
                &recording.session,
                &recording.attachment,
                &"saved-empty-thread".into(),
            )?;
            let config = AgentConfig {
                agent: crate::config::Agent::parse("codex").unwrap(),
                cwd: Some(home.into()),
                task_prompt: "First prompt on the saved thread".into(),
                env: capture.environment(),
                ..crate::agent::AgentConfig::default()
            };
            // Native launch recovers the stored thread even when the entry has no explicit resume token.
            let launch = Launch {
                harness: crate::harness::launcher(HarnessKind::Codex),
                config: &config,
                kind: LaunchKind::WorkSession,
                recording: &recording,
                start: Start::New,
                context_file: None,
            };
            let outcome = super::native_session::run(&launch, None, Some(&capture))?;
            anyhow::ensure!(!outcome.failed(), "{:?}", outcome.status);
            let turns: serde_json::Value =
                serde_json::from_slice(&std::fs::read(home.join("native-turns.json"))?)?;
            assert_eq!(
                turns[0]["items"][0]["content"][0]["text"], config.task_prompt,
                "the first prompt must reach the saved thread"
            );
            assert_eq!(
                recording.store.session_thread(&recording.session)?,
                Some("saved-empty-thread".into())
            );
            capture.finish("completed")?;
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn native_limits_and_schemas_require_a_headless_run() {
    let home = tempfile::tempdir().unwrap();
    let recording = super::admit_for_test(home.path());
    for harness in HarnessKind::ALL {
        for (max_turns, output_schema) in [
            (Some(7), None),
            (None, Some(serde_json::json!({"type":"object"}))),
        ] {
            let config = AgentConfig {
                agent: crate::config::Agent::parse(harness.as_str()).unwrap(),
                max_turns,
                output_schema,
                ..crate::agent::AgentConfig::default()
            };
            let launch = Launch {
                config: &config,
                harness: crate::harness::launcher(harness),
                kind: LaunchKind::Batch,
                recording: &recording,
                start: Start::New,
                context_file: None,
            };
            let error = crate::harness::launcher(harness).open(&launch).unwrap_err();
            assert!(
                error.to_string().contains("require a headless Run"),
                "{harness:?}: {error}"
            );
        }
    }
}
