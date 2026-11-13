//! Launch and resume policy as table rows.
//!
//! A fresh launch (`lf -i : …`, `lf -b : …`) reaches `run::run` →
//! `build_prompt` → `build_prompt_at`: it selects the command line's settings,
//! loads personal and repository config at the checkout, and
//! `prepare_process_prompt` returns the `AgentConfig`. `launch_session`
//! (`lf/commands/util.rs`) and the Claude stream (`harness/claude.rs`) turn
//! that into argv with `build_session_command`. `lf session resume` selects the
//! command line's settings in `bin/lf.rs`; `resume_session_with_env` then calls
//! `resolve_session_config(session, &Cli::default())` and
//! `build_session_command` with `ProviderInvocation::Resume`.
//!
//! Owners, PTYs, provider processes, native conversation identity and the
//! handler's own refusals stay with the full-Machine tests in `tests/README.md`.

use std::path::Path;

use clap::Parser;

use crate::agent::{
    build_session_command, resolve_session_config, AgentConfig, ProviderInvocation,
};
use crate::lf::Cli;
use crate::ops::human_session::tests::SessionHome;
use crate::session::LfSession;
use crate::session_record::{SessionCaptureSpec, SessionGraphMembership, SessionRecorder};
use crate::store::SharedStore;

struct Table {
    repo: loopflow_test_support::TestRepo,
    store: SharedStore,
    home: SessionHome,
    _lock: std::sync::MutexGuard<'static, ()>,
}

/// What reached the provider: the resolved config and its argv.
struct Launch {
    config: AgentConfig,
    args: Vec<String>,
}

impl Launch {
    fn bypass(&self) -> bool {
        self.args.contains(&"--dangerously-skip-permissions".into())
    }

    fn chrome(&self) -> Vec<&str> {
        self.args
            .iter()
            .map(String::as_str)
            .filter(|arg| *arg == "--chrome" || *arg == "--no-chrome")
            .collect()
    }

    fn pair(&self, flag: &str) -> Option<&str> {
        self.args
            .windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1].as_str())
    }
}

impl Table {
    fn new() -> Self {
        let lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        let store = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(home.store());
        Self {
            repo: loopflow_test_support::TestRepo::new(),
            store,
            home,
            _lock: lock,
        }
    }

    /// Personal config lives in `LF_HOME`; `None` removes it.
    fn config(&self, personal: Option<&str>, repository: &str) {
        let path = self.home.path().join("config.yaml");
        match personal {
            Some(text) => std::fs::write(path, text).unwrap(),
            None => drop(std::fs::remove_file(path)),
        }
        self.repo.create_file(
            ".lf/config.yaml",
            &format!("{repository}\ndiff: false\ndiff_files: false\npaste: false\n"),
        );
    }

    /// `lf FLAGS : hello` in the checkout.
    fn launch(&self, flags: &[&str]) -> anyhow::Result<Launch> {
        let cli = Cli::try_parse_from(std::iter::once("lf").chain(flags.iter().copied()))?;
        let built = super::build_prompt(self.repo.path(), None, Some("hello"), &cli)?;
        let invocation = if built.process.auto && built.harness == "claude" {
            ProviderInvocation::ClaudeStream { session_id: None }
        } else {
            ProviderInvocation::Terminal { session_id: None }
        };
        let command = build_session_command(&built.agent_config, invocation, None)?;
        Ok(Launch {
            config: built.agent_config,
            args: command.args,
        })
    }

    /// A conversation recorded by the writer a launch uses.
    fn conversation(&self, harness: &str, model: Option<&str>) -> LfSession {
        let recorder = SessionRecorder::begin_at(
            self.home.path(),
            SessionCaptureSpec {
                harness: harness.into(),
                model: model.map(Into::into),
                surface: "tui".into(),
                cwd: self.repo.path().canonicalize().unwrap(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: vec![],
                graph: SessionGraphMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        self.store
            .sqlite
            .session_for_artifact(&recorder.artifact_key())
            .unwrap()
            .unwrap()
    }

    /// `lf FLAGS session resume ID`.
    fn resume(&self, session: &LfSession, flags: &[&str]) -> anyhow::Result<Launch> {
        let cli = Cli::try_parse_from(std::iter::once("lf").chain(flags.iter().copied()))?;
        let _settings = crate::config::settings::select_invocation(cli.setting_values()?);
        let config = resolve_session_config(session, &Cli::default())?;
        let command = build_session_command(
            &config,
            ProviderInvocation::Resume {
                session_id: &"saved".into(),
            },
            None,
        )?;
        Ok(Launch {
            config,
            args: command.args,
        })
    }

    fn claude_settings(&self, mode: &str) {
        self.repo.create_file(
            ".claude/settings.json",
            &format!(r#"{{"permissions":{{"defaultMode":"{mode}"}}}}"#),
        );
    }
}

fn assert_path_eq(left: &Path, right: &Path) {
    assert_eq!(left.canonicalize().unwrap(), right.canonicalize().unwrap());
}

#[test]
fn launch_bypass_and_chrome_take_the_command_line_then_personal_then_repository() {
    let table = Table::new();
    // Claude's own attended mode stays in charge unless lf is told to bypass.
    table.claude_settings("auto");
    const OFF: &str = "agent: claude\nyolo: false\nchrome: false";
    for (personal, repository, flags, bypass, chrome) in [
        (
            Some("yolo: true\nchrome: true\n"),
            OFF,
            &["-i"][..],
            true,
            "--chrome",
        ),
        (
            Some("yolo: true\nchrome: true\n"),
            OFF,
            &["-i", "--setting", "yolo=false", "--chrome", "off"],
            false,
            "--no-chrome",
        ),
        (None, OFF, &["-i"], false, "--no-chrome"),
        (
            None,
            OFF,
            &["-i", "--yolo", "--chrome", "on"],
            true,
            "--chrome",
        ),
        (
            None,
            "agent: claude\nyolo: true\nchrome: true",
            &["-i"],
            true,
            "--chrome",
        ),
    ] {
        table.config(personal, repository);
        let launch = table.launch(flags).unwrap();
        let row = format!("{personal:?} {repository:?} {flags:?}: {:?}", launch.args);
        assert_eq!(launch.bypass(), bypass, "{row}");
        assert_eq!(launch.chrome(), [chrome], "{row}");
        assert_path_eq(launch.config.cwd.as_deref().unwrap(), table.repo.path());
    }
    // Every harness reads the same setting; its argv mapping is a row in
    // `agent::session::tests`.
    for harness in ["codex", "opencode", "pi"] {
        for (repository_yolo, flags, expected) in [
            (false, &[][..], false),
            (true, &[], true),
            (false, &["--yolo"], true),
            (true, &["--setting", "yolo=false"], false),
        ] {
            table.config(None, &format!("agent: {harness}\nyolo: {repository_yolo}"));
            let flags = [&["-i"][..], flags].concat();
            let launch = table.launch(&flags).unwrap();
            assert_eq!(launch.config.agent.as_deref(), Some(harness));
            assert_eq!(
                launch.config.skip_permissions, expected,
                "{harness} yolo: {repository_yolo} {flags:?}"
            );
        }
    }
}

#[test]
fn unset_chrome_sends_claude_nothing_until_a_choice_is_explicit() {
    let table = Table::new();
    table.config(None, "agent: claude");
    for (flags, expected) in [
        (&["-i"][..], &[][..]),
        (&["-b"], &[]),
        (&["-i", "--chrome", "off"], &["--no-chrome"]),
        (&["-i", "--chrome", "on"], &["--chrome"]),
        (&["-b", "--chrome", "on"], &["--chrome"]),
    ] {
        let launch = table.launch(flags).unwrap();
        assert_eq!(launch.chrome(), expected, "{flags:?}: {:?}", launch.args);
    }
    let session = table.conversation("claude", None);
    assert!(table.resume(&session, &[]).unwrap().chrome().is_empty());
}

#[test]
fn headless_launch_carries_the_schema_and_terminal_launch_refuses_it() {
    let table = Table::new();
    table.config(None, "agent: claude:sonnet\nyolo: false\nchrome: true");
    let headless = table
        .launch(&["-b", "--output-schema", r#"{"type":"object"}"#])
        .unwrap();
    assert_eq!(headless.pair("--input-format"), Some("stream-json"));
    assert_eq!(headless.pair("--json-schema"), Some(r#"{"type":"object"}"#));
    assert_eq!(headless.pair("--model"), Some("sonnet"));
    assert_eq!(headless.chrome(), ["--chrome"]);
    // Nobody can answer a permission prompt in a headless Run.
    assert!(headless.bypass());

    let error = table
        .launch(&["-i", "--output-schema", "{}"])
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("require a headless Run"),
        "{error}"
    );
}

#[test]
fn resume_keeps_the_provider_and_takes_a_model_only_from_a_matching_choice() {
    let table = Table::new();
    table.config(None, "");
    let session = table.conversation("claude", Some("sonnet"));
    for (flags, expected) in [
        (&[][..], "sonnet"),
        (&["--setting", "default_agent=codex"], "sonnet"),
        (&["--setting", "default_agent=claude:haiku"], "haiku"),
        (&["--setting", "agents=[codex, claude:haiku]"], "sonnet"),
        (&["--setting", "agents=[claude:haiku, codex]"], "haiku"),
    ] {
        let resumed = table.resume(&session, flags).unwrap();
        assert_eq!(
            resumed.config.agent.as_deref(),
            Some(format!("claude:{expected}").as_str()),
            "{flags:?}"
        );
        assert_eq!(resumed.pair("--model"), Some(expected), "{flags:?}");
        assert_path_eq(resumed.config.cwd.as_deref().unwrap(), table.repo.path());
    }
}

#[test]
fn resume_applies_current_policy_and_command_line_overrides() {
    let table = Table::new();
    for (provider, model) in [
        ("claude", "current-model"),
        ("codex", "current-model"),
        ("opencode", "opencode/current-model"),
    ] {
        let session = table.conversation(provider, None);
        table.config(
            None,
            &format!("agent: {provider}:{model}\nyolo: false\nchrome: false"),
        );
        for (flags, bypass, chrome) in [
            (&[][..], false, "--no-chrome"),
            (&["--yolo", "--chrome", "on"], true, "--chrome"),
        ] {
            let resumed = table.resume(&session, flags).unwrap();
            let row = format!("{provider} {flags:?}: {:?}", resumed.args);
            assert_eq!(
                resumed.config.agent.as_deref(),
                Some(format!("{provider}:{model}").as_str()),
                "{row}"
            );
            assert_eq!(resumed.config.skip_permissions, bypass, "{row}");
            if provider == "claude" {
                assert_eq!(resumed.bypass(), bypass, "{row}");
                assert_eq!(resumed.chrome(), [chrome], "{row}");
                assert_eq!(resumed.pair("--model"), Some("current-model"), "{row}");
            }
        }
    }
}
