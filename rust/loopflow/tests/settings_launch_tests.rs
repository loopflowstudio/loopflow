use crate::support;
use loopflow::store::{CredentialState, ProviderAccount, ProviderAccountId, RoutingState};
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

fn write_preferences(home: &Path, text: &str) -> std::io::Result<()> {
    let path = home.join("config.yaml");
    let saved: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&path).unwrap_or_default()).unwrap();
    let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).unwrap();
    if let Some(accounts) = saved.get("accounts") {
        value["accounts"] = accounts.clone();
    }
    fs::write(path, serde_yaml_ng::to_string(&value).unwrap())
}

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
    {
        command.env_remove(name);
    }
    command
        .args(args)
        .stdin(Stdio::null())
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1");
    command
}

/// The shared native Codex stand-in: an app-server that records each turn
/// and a terminal client that attaches to it.
const CODEX_NATIVE: &str = concat!(
    "#!/bin/sh\nexec /usr/bin/python3 '",
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/native_codex.py' \"$@\"\n"
);

#[test]
fn explicit_graph_selection_stays_with_steps_and_child_commands_choose_their_own_agent() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("agent-proof");
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("security", "#!/bin/sh\nexit 1\n"),
            ("codex", CODEX_NATIVE),
            (
                "claude",
                r#"#!/usr/bin/python3
import json
import os
import subprocess
import sys

if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
with open(os.path.join(os.environ['LF_HOME'], 'launches.jsonl'), 'a') as launches:
    launches.write(json.dumps({'args': sys.argv[1:], 'claude_home': os.environ.get('CLAUDE_CONFIG_DIR')}) + '\n')
message = json.loads(sys.stdin.readline())
message['session_id'] = 'agent-fixture'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'agent-fixture'}), flush=True)
if os.environ.get('LF_AGENT_CHILD') != 'yes':
    with open(os.path.join(os.environ['LF_HOME'], 'child-output'), 'w') as output:
        subprocess.run(
            [os.environ['LF_BIN'], '-b', 'skill', 'child', '-a', 'codex'],
            env={**os.environ, 'LF_AGENT_CHILD': 'yes'},
            stdout=output, stderr=subprocess.STDOUT, check=True, timeout=30,
        )
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'agent-fixture', 'subtype': 'success', 'is_error': False, 'result': 'done'}), flush=True)
for line in sys.stdin:
    pass
"#,
            ),
        ],
        Some(home.path()),
    );
    let registered = support::register_task_without_pr(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        "agent-proof",
        &repo.head_sha(),
    );
    connect_fixture_claude(home.path(), &registered.store);
    support::managed_codex_account(home.path());

    for (path, content) in [
        (".lf/skills/first.md", "---\nagent: codex\n---\nFirst step."),
        (
            ".lf/skills/second.md",
            "---\nagent: codex\n---\nSecond step.",
        ),
        (
            ".lf/skills/child.md",
            "---\nagent: codex\n---\nProvider-issued child.",
        ),
        (
            ".lf/graphs/pursue.yaml",
            "- step: {name: first, agent: codex}\n- graph: inner\n",
        ),
        (
            ".lf/graphs/inner.yaml",
            "- step: {name: second, agent: codex}\n",
        ),
        (
            ".lf/graphs/feature.yaml",
            "- graph: pursue\n- interactive: {name: demo, id: review}\n",
        ),
    ] {
        repo.create_file(path, content);
    }
    repo.stage_all();
    repo.commit("Define agent selection fixtures");
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    support::select_wave_graph(
        repo.path(),
        &store,
        registered.task.wave_id.as_ref().unwrap(),
        "feature",
    );
    // Direct Task binding used to prefer the saved Task agent over -a.
    for args in [
        vec![
            "-b",
            "--yolo",
            "--chrome",
            "on",
            "--isolate",
            "--task",
            "INF-123",
            "skill",
            "first",
            "-a",
            "claude:sonnet",
        ],
        vec![
            "-b",
            "--yolo",
            "--chrome",
            "on",
            "--isolate",
            "-a",
            "claude:sonnet",
            "--task",
            "INF-123",
            "graph",
            "pursue",
        ],
    ] {
        let output = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}\nchild: {}",
            String::from_utf8_lossy(&output.stderr),
            fs::read_to_string(home.path().join("child-output")).unwrap_or_default()
        );
    }
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let sessions = db
        .prepare("SELECT skill, provider FROM agent_sessions ORDER BY skill")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        sessions,
        ["child", "child", "child", "first", "first", "second"].map(|skill| (
            skill.to_owned(),
            if skill == "child" { "codex" } else { "claude" }.to_owned()
        ))
    );
    assert_eq!(support::recorded_graphs(home.path()).len(), 1);
    let launches = fs::read_to_string(home.path().join("launches.jsonl")).unwrap();
    assert_eq!(launches.lines().count(), 3);
    for launch in launches.lines() {
        let received: serde_json::Value = serde_json::from_str(launch).unwrap();
        let args = received["args"].as_array().unwrap();
        assert!(args.contains(&serde_json::json!("--chrome")), "{launch}");
        assert!(
            !args.contains(&serde_json::json!("--no-chrome")),
            "{launch}"
        );
        assert!(
            args.contains(&serde_json::json!("--dangerously-skip-permissions")),
            "{launch}"
        );
        assert!(
            args.windows(2).any(|pair| pair == ["--model", "sonnet"]),
            "{launch}"
        );
        assert_eq!(
            Path::new(received["claude_home"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            home.path()
                .join("accounts/claude/fixture")
                .canonicalize()
                .unwrap()
        );
    }

    // A separate invocation with no -a keeps skill and step declarations.
    let unbound = TestRepo::new();
    for (path, content) in [
        (
            ".lf/skills/default-proof.md",
            "---\nagent: claude:haiku\n---\nUse the declared agent.",
        ),
        (
            ".lf/graphs/default-proof.yaml",
            "- step: {name: default-proof, agent: 'claude:sonnet'}\n",
        ),
    ] {
        unbound.create_file(path, content);
    }
    unbound.stage_all();
    unbound.commit("Define default agent fixtures");
    for owner in ["skill", "graph"] {
        let output = command(
            unbound.path(),
            home.path(),
            &["-b", "--isolate", owner, "default-proof"],
        )
        .env("LF_AGENT_CHILD", "yes")
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let models = db
        .prepare("SELECT model FROM agent_sessions WHERE skill='default-proof' ORDER BY model")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(models, ["haiku", "sonnet"]);
}

fn connect_fixture_claude(home: &Path, store: &loopflow::store::Store) {
    let name = "fixture";
    let account_home = home.join("accounts/claude").join(name);
    fs::create_dir_all(&account_home).unwrap();
    let account_home = account_home.canonicalize().unwrap();
    let credential =
        r#"{"claudeAiOauth":{"accessToken":"fixture-only","expiresAt":4102444800000}}"#;
    fs::write(account_home.join(".credentials.json"), credential).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.upsert_provider_account(&ProviderAccount {
            provider: "claude".into(),
            account_id: ProviderAccountId::parse(name).unwrap(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse(&format!("{name}@example.com")).unwrap(),
            ),
            observed_email: Some(format!("{name}@example.com")),
            observed_subject: Some(name.into()),
            observed_credential_digest: Some(format!(
                "{:x}",
                Sha256::digest(credential.as_bytes())
            )),
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        }))
        .unwrap();
}

// Record the preferences and documents actually delivered across the provider
// boundary. No installed provider, credentials, or network participates.
const CLAUDE_SETTINGS: &str = r#"#!/usr/bin/python3
import json
import os
import sys
import uuid
from pathlib import Path

if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
message = json.loads(sys.stdin.readline())
thread = str(uuid.uuid4())
message['session_id'] = thread
context = json.loads(Path(os.environ['LF_CONTEXT_DELIVERY']).read_text())
with (Path(os.environ['LF_HOME']) / 'settings-launches.jsonl').open('a') as file:
    file.write(json.dumps({'args': sys.argv[1:], 'input': message,
        'docs': [Path(path).name for path in context['submitted_docs']]}) + '\n')
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': thread}), flush=True)
result = {'type': 'result', 'uuid': str(uuid.uuid4()), 'session_id': thread,
    'subtype': 'success', 'is_error': False, 'result': 'done'}
if '--json-schema' in sys.argv:
    schema = json.loads(sys.argv[sys.argv.index('--json-schema') + 1])
    if 'path' in schema['properties']:
        result['structured_output'] = {'path': 'selected'}
    else:
        marker = Path(os.environ['LF_HOME']) / 'iterated'
        decision = 'advance' if marker.exists() else 'iterate'
        marker.touch()
        result['structured_output'] = {'decision': decision, 'summary': 'One repeat proves selection', 'reason': None}
print(json.dumps(result), flush=True)
for line in sys.stdin:
    pass
"#;

fn success(mut command: Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn settings_records(home: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(home.join("settings-launches.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn exported_commands(repo: &Path, graph: &str) -> Vec<Vec<String>> {
    let recipe = fs::read_to_string(repo.join(format!(".agents/skills/{graph}/SKILL.md"))).unwrap();
    recipe
        .lines()
        .filter_map(|line| {
            let (_, command) = line.split_once(". Run `")?;
            Some(shlex::split(command.strip_suffix("`.").unwrap()).unwrap())
        })
        .collect()
}

fn assert_launch_settings(launch: &serde_json::Value, inspected: &serde_json::Value) {
    let settings = &inspected["settings"];
    let args = launch["args"].as_array().unwrap();
    let agent = settings["agent"]["value"].as_str().unwrap();
    let model = agent.strip_prefix("claude:").unwrap();
    assert!(
        args.windows(2).any(|pair| pair == ["--model", model]),
        "{launch}\n{settings}"
    );
    assert_eq!(
        args.contains(&serde_json::json!("--chrome")),
        settings["chrome"]["value"] == true,
        "{launch}\n{settings}"
    );
    // Claude's documented headless floor bypasses prompts independently of yolo.
    // Explicit false is observable here through chrome and cleared documents;
    // terminal yolo negation has separate native launch/resume coverage.
    assert!(args.contains(&serde_json::json!("--dangerously-skip-permissions")));
    let mut expected = settings["context.docs"]["value"]
        .as_array()
        .unwrap()
        .clone();
    expected.sort_by_key(|value| value.as_str().unwrap().to_owned());
    expected.dedup();
    let mut received = launch["docs"].as_array().unwrap().clone();
    received.sort_by_key(|value| value.as_str().unwrap().to_owned());
    assert_eq!(
        received, expected,
        "delivered context differs from inspection"
    );
}

#[test]
fn runtime_and_exported_occurrences_resolve_live_preferences_at_the_same_authority() {
    exercise_live_occurrence_preferences(false);
}

#[test]
fn runtime_and_exported_occurrences_resolve_unsets_and_invocation_overrides() {
    exercise_live_occurrence_preferences(true);
}

fn exercise_live_occurrence_preferences(overrides: bool) {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("claude", CLAUDE_SETTINGS),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("security", "#!/bin/sh\nexit 1\n"),
        ],
        Some(home.path()),
    );
    let store = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    connect_fixture_claude(home.path(), &store);
    for (path, content) in [
        (".lf/config.yaml", "agent: claude:repo\nyolo: true\nchrome: true\ndocs: [repo.txt]\ncontext: [context.txt]\ndiff: false\ndiff_files: false\n"),
        (".lf/skills/probe.md", "---\nagent: claude:skill\ndocs: [skill.txt]\n---\nReport the selected settings."),
        (".lf/graphs/inner.yaml", "- step: {name: probe, id: work, agent: 'claude:authored', docs: [authored.txt]}\n- step: {name: probe, id: check}\n"),
        (".lf/graphs/outer.yaml", "- graph: inner\n- graph: inner\n- step: {name: probe, id: work}\n"),
    ] { repo.create_file(path, content); }
    for name in [
        "repo", "context", "skill", "authored", "personal", "db", "late",
    ] {
        repo.create_file(&format!("{name}.txt"), &format!("{name} document"));
    }
    repo.stage_all();
    repo.commit("Define occurrence preferences");
    write_preferences(
        home.path(),
        r#"
agent: claude:personal
isolate: true
docs: [personal.txt]
skills:
  probe: {agent: 'claude:personal-skill'}
graphs:
  inner:
    steps:
      work: {agent: 'claude:personal-step', chrome: false, yolo: false, docs: [], context: []}
"#,
    )
    .unwrap();
    // The exported captures are generated once. Later personal/DB changes must
    // affect them without re-syncing or promoting the authored agent to -a.
    success(command(
        repo.path(),
        home.path(),
        &["self", "sync-skills", "--repo"],
    ));
    // A synced recipe must still use the repository's current skill defaults.
    repo.create_file(
        ".lf/skills/probe.md",
        "---\nagent: claude:updated-skill\ndocs: [skill.txt, late.txt]\n---\nCurrent skill body.",
    );
    let commands = exported_commands(repo.path(), "outer");
    assert_eq!(commands.len(), 5);
    let inspect = |flags: &[&str], env: Option<&str>| {
        let mut args = flags.to_vec();
        args.extend(["config", "show", "--graph", "outer", "--json"]);
        let mut cmd = command(repo.path(), home.path(), &args);
        if let Some(value) = env {
            cmd.env("LF_AGENT_OVERRIDE", value);
        }
        serde_json::from_slice::<serde_json::Value>(&success(cmd).stdout).unwrap()
    };
    let launch_pair = |flags: &[&str], env: Option<&str>, all: bool| {
        let inspected = inspect(flags, env);
        let start = settings_records(home.path()).len();
        if all {
            let mut args = flags.to_vec();
            args.extend(["-b", "graph", "outer", "Recorded step direction"]);
            let mut cmd = command(repo.path(), home.path(), &args);
            if let Some(value) = env {
                cmd.env("LF_AGENT_OVERRIDE", value);
            }
            success(cmd);
        }
        for argv in commands.iter().take(if all { 5 } else { 1 }) {
            let mut args = flags.to_vec();
            args.extend(argv[1..].iter().map(String::as_str));
            args.push("Recorded step direction");
            let mut cmd = command(repo.path(), home.path(), &args);
            if let Some(value) = env {
                cmd.env("LF_AGENT_OVERRIDE", value);
            }
            success(cmd);
        }
        let records = settings_records(home.path());
        assert_eq!(records.len() - start, if all { 10 } else { 1 });
        for (index, record) in records[start..].iter().enumerate() {
            assert_launch_settings(record, &inspected[index % 5]);
            assert!(
                record["input"]
                    .to_string()
                    .contains("Recorded step direction"),
                "{record}"
            );
        }
        inspected
    };
    if !overrides {
        let initial = launch_pair(&[], None, true);
        for index in [0, 2] {
            assert_eq!(
                initial[index]["settings"]["agent"]["value"],
                "claude:personal-step"
            );
        }
        for index in [1, 3, 4] {
            assert_eq!(
                initial[index]["settings"]["agent"]["value"],
                "claude:personal-skill"
            );
        }
    }
    for (key, value) in [
        ("agent", "claude:edited-general"),
        ("skills.probe.agent", "claude:edited-skill"),
        ("graphs.inner.steps.work.agent", "claude:edited-step"),
        ("graphs.inner.steps.work.chrome", "true"),
        ("graphs.inner.steps.work.context.docs", "[db.txt]"),
    ] {
        success(command(
            repo.path(),
            home.path(),
            &["set", "--global", key, value],
        ));
    }
    if !overrides {
        let edited = launch_pair(&[], None, true);
        assert_eq!(
            edited[0]["settings"]["agent"]["value"],
            "claude:edited-step"
        );
        assert_eq!(
            edited[4]["settings"]["agent"]["value"],
            "claude:edited-skill"
        );
        return;
    }
    for key in [
        "graphs.inner.steps.work.agent",
        "skills.probe.agent",
        "graphs.inner.steps.work.chrome",
        "graphs.inner.steps.work.context.docs",
    ] {
        success(command(
            repo.path(),
            home.path(),
            &["unset", "--global", key],
        ));
    }
    // Unset removes the edited occurrence and skill declarations from the file.
    assert_eq!(
        launch_pair(&[], None, false)[0]["settings"]["agent"]["value"],
        "claude:edited-general"
    );
    success(command(repo.path(), home.path(), &["repository"]));
    success(command(
        repo.path(),
        home.path(),
        &["set", "agent", "claude:edited-repo"],
    ));
    assert_eq!(
        launch_pair(&[], None, false)[0]["settings"]["agent"]["value"],
        "claude:edited-repo"
    );
    let wave = loopflow::work::wave::Wave::new(
        loopflow::id::WaveId::new(),
        "settings".into(),
        repo.path().canonicalize().unwrap().display().to_string(),
    );
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.create_wave(&wave))
        .unwrap();
    repo.create_file(
        ".lf/wave/settings/GOAL.md",
        &format!("---\nid: {}\n---\nExercise settings scope.", wave.id()),
    );
    success(command(
        repo.path(),
        home.path(),
        &["--wave", "settings", "set", "agent", "claude:edited-wave"],
    ));
    assert_eq!(
        launch_pair(&["--wave", "settings"], None, false)[0]["settings"]["agent"]["value"],
        "claude:edited-wave"
    );
    success(command(
        repo.path(),
        home.path(),
        &["--wave", "settings", "unset", "agent"],
    ));
    success(command(repo.path(), home.path(), &["unset", "agent"]));
    success(command(
        repo.path(),
        home.path(),
        &["unset", "--global", "agent"],
    ));
    assert_eq!(
        launch_pair(&[], None, false)[0]["settings"]["agent"]["value"],
        "claude:authored"
    );
    assert_eq!(
        launch_pair(&[], Some("claude:environment"), false)[0]["settings"]["agent"]["value"],
        "claude:environment"
    );
    assert_eq!(
        launch_pair(
            &[
                "-a",
                "claude:cli",
                "--setting",
                "chrome=false",
                "--setting",
                "context.docs=[]"
            ],
            Some("claude:environment"),
            true
        )[0]["settings"]["agent"]["value"],
        "claude:cli"
    );
    write_preferences(home.path(), "isolate: true\n").unwrap();
    repo.create_file(
        ".lf/wave/settings/config.yaml",
        &format!("id: {}\nagent: claude:wave\n", wave.id()),
    );
    assert_eq!(
        launch_pair(&["--wave", "settings"], None, false)[0]["settings"]["agent"]["value"],
        "claude:wave"
    );
    assert_eq!(
        launch_pair(&[], None, false)[0]["settings"]["agent"]["value"],
        "claude:authored"
    );
}

#[test]
fn choices_and_fallbacks_agree_in_inspection_runtime_and_exported_recipes() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("claude", CLAUDE_SETTINGS),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("security", "#!/bin/sh\nexit 1\n"),
        ],
        Some(home.path()),
    );
    let store = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    connect_fixture_claude(home.path(), &store);
    repo.create_file(".lf/config.yaml", "diff: false\ndiff_files: false\n");
    repo.create_file(
        ".lf/skills/probe.md",
        "---\ndefault_agent: claude:skill\n---\nReport settings.",
    );
    repo.create_file(
        ".lf/graphs/fallback.yaml",
        "- step: {name: probe, id: work, default_agent: 'claude:occurrence'}\n",
    );
    write_preferences(
        home.path(),
        "isolate: true\nskills: {probe: {default_agent: claude:personal}}\n",
    )
    .unwrap();
    success(command(
        repo.path(),
        home.path(),
        &["self", "sync-skills", "--repo"],
    ));
    let exported = exported_commands(repo.path(), "fallback");
    let check = |flags: &[&str], environment: Option<&str>, expected: &str, key: &str| {
        let mut args = flags.to_vec();
        args.extend(["config", "show", "--graph", "fallback", "--json"]);
        let prepare = |args: &[&str]| {
            let mut cmd = command(repo.path(), home.path(), args);
            if let Some(value) = environment {
                cmd.env("LF_SETTINGS", value);
            }
            cmd
        };
        let output = success(prepare(&args));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("has no effect"));
        let inspected: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let setting = &inspected[0]["settings"]["agent"];
        assert_eq!(setting["value"], expected);
        assert_eq!(
            setting["candidates"].as_array().unwrap().last().unwrap()["key"],
            key
        );
        let start = settings_records(home.path()).len();
        let mut args = flags.to_vec();
        args.extend(["-b", "graph", "fallback"]);
        success(prepare(&args));
        let mut args = flags.to_vec();
        args.extend(exported[0][1..].iter().map(String::as_str));
        success(prepare(&args));
        let records = settings_records(home.path());
        assert_eq!(records.len() - start, 2);
        for launch in &records[start..] {
            assert_launch_settings(launch, &inspected[0]);
        }
    };
    check(&[], None, "claude:personal", "default_agent");
    success(command(
        repo.path(),
        home.path(),
        &[
            "set",
            "--global",
            "graphs.fallback.steps.work.default_agent",
            "claude:edited",
        ],
    ));
    check(&[], None, "claude:edited", "default_agent");
    check(
        &[],
        Some("default_agent: claude:environment"),
        "claude:environment",
        "default_agent",
    );
    check(
        &["--setting", "default_agent=claude:cli"],
        Some("default_agent: claude:environment"),
        "claude:cli",
        "default_agent",
    );
    repo.create_file(
        ".lf/config.yaml",
        "agent: claude:repo\ndiff: false\ndiff_files: false\n",
    );
    check(
        &["--setting", "default_agent=claude:cli"],
        None,
        "claude:repo",
        "agent",
    );
    check(&["-a", "claude:forced"], None, "claude:forced", "agent");
    success(command(
        repo.path(),
        home.path(),
        &[
            "unset",
            "--global",
            "graphs.fallback.steps.work.default_agent",
        ],
    ));
    repo.create_file(".lf/config.yaml", "diff: false\ndiff_files: false\n");
    check(&[], None, "claude:personal", "default_agent");
    write_preferences(home.path(), "isolate: true\n").unwrap();
    check(&[], None, "claude:occurrence", "default_agent");
}

struct TaskCheckouts<'a> {
    home: &'a Path,
    repo: &'a Path,
}

impl Drop for TaskCheckouts<'_> {
    fn drop(&mut self) {
        let db = rusqlite::Connection::open(self.home.join("loopflow.db")).unwrap();
        let mut query = db
            .prepare("SELECT worktree FROM tasks WHERE worktree IS NOT NULL")
            .unwrap();
        let paths = query.query_map([], |row| row.get::<_, String>(0)).unwrap();
        for path in paths.flatten() {
            // All rows in this disposable Machine were created by the fixture.
            if Path::new(&path) != self.repo {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}

#[test]
fn session_and_resume_choices_do_not_select_existing_or_new_task_workers() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let claude = r#"#!/usr/bin/python3
import json
import os
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
args = sys.argv[1:]
assert '--input-format' not in args, 'Task worker wrongly selected Claude'
assert not os.environ.get('LF_AGENT_OVERRIDE'), 'Session choice escaped to child policy'
if 'SETTINGS_TASK' not in os.environ:
    sys.exit(0)  # a Task conversation checkout started in the background, not a launch under test
home = Path(os.environ['LF_HOME'])
db = sqlite3.connect(home / 'loopflow.db')
def run(*args):
    # Conversations the commands below start inherit this environment; keep them quiet.
    env = {name: value for name, value in os.environ.items() if name != 'SETTINGS_TASK'}
    result = subprocess.run([os.environ['LF_BIN'], *args], stdin=subprocess.DEVNULL, env=env,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=90)
    if result.returncode:
        raise RuntimeError(str(args) + result.stdout + result.stderr)
    return result.stdout + result.stderr

def worker(task):
    text = run('task', 'run', task, 'proof', '--from', 'implement')
    graph = text.split('running as Graph process ')[1].split()[0]
    carrier = db.execute('SELECT parent_lf_process_id FROM processes WHERE id=?', (graph,)).fetchone()[0]
    deadline = time.monotonic() + 90
    while True:
        done, code = db.execute('SELECT completed_at, exit_code FROM processes WHERE id=?', (carrier,)).fetchone()
        if done:
            assert code == 0, text + '\n' + str(list(db.execute('SELECT error FROM processes WHERE parent_lf_process_id=?', (carrier,))))
            return
        if time.monotonic() > deadline:
            raise TimeoutError(text)
        time.sleep(.05)

worker(os.environ['SETTINGS_TASK'])
receipt = json.loads(run('task', 'create', '--title', 'Child created by selected Session', '--json'))
child = receipt['id']
worker(child)
with (home / 'session-children.jsonl').open('a') as output:
    output.write(json.dumps({'args': args, 'child': child}) + '\n')
"#;
    let _env = support::EnvGuard::with_home(
        &[
            ("claude", claude),
            ("codex", CODEX_NATIVE),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("security", "#!/bin/sh\nexit 1\n"),
            ("open", "#!/bin/sh\nexit 0\n"),
        ],
        Some(home.path()),
    );
    repo.create_file(".lf/config.yaml", "diff: false\ndiff_files: false\n");
    repo.create_file(
        ".lf/skills/implement.md",
        "Record the Task worker selection.",
    );
    repo.create_file(
        ".lf/graphs/proof.yaml",
        "- implement\n- interactive: {name: demo, id: review}\n",
    );
    repo.stage_all();
    repo.commit("Define Task worker settings fixture");
    // New Task checkouts start at the repository's main branch.
    let pushed = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(pushed.status.success());
    let store = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    let _checkouts = TaskCheckouts {
        home: home.path(),
        repo: repo.path(),
    };
    connect_fixture_claude(home.path(), &store);
    support::managed_codex_account(home.path());
    write_preferences(
        home.path(),
        "agent: claude\nisolate: true\nskills: {implement: {agent: codex}}\n",
    )
    .unwrap();
    let task: serde_json::Value = serde_json::from_slice(
        &success(command(
            repo.path(),
            home.path(),
            &[
                "task",
                "create",
                "--title",
                "Session worker preferences",
                "--json",
            ],
        ))
        .stdout,
    )
    .unwrap();
    let task = task["id"].as_str().unwrap();
    success(command(
        repo.path(),
        home.path(),
        &["task", "checkout", task],
    ));
    let mut first = command(
        repo.path(),
        home.path(),
        &[
            "-i",
            "-a",
            "claude:sonnet",
            "--task",
            task,
            ":",
            "Run the Task and create another",
        ],
    );
    first.env("SETTINGS_TASK", task);
    success(first);
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let session: String = db
        .query_row(
            "SELECT id FROM agent_sessions WHERE provider='claude'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut resumed = command(
        repo.path(),
        home.path(),
        &["-i", "-a", "claude:haiku", "resume", &session],
    );
    resumed.env("SETTINGS_TASK", task);
    success(resumed);
    let children: Vec<serde_json::Value> =
        fs::read_to_string(home.path().join("session-children.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    assert_eq!(children.len(), 2);
    for (record, model) in children.iter().zip(["sonnet", "haiku"]) {
        assert!(record["args"]
            .as_array()
            .unwrap()
            .windows(2)
            .any(|pair| pair == ["--model", model]));
    }
    assert!(children[1]["args"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("--resume")));
    let workers: Vec<String> = db
        .prepare("SELECT provider FROM agent_sessions WHERE skill='implement'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(workers, vec!["codex"; 4]);
    for child in children {
        let id = child["child"].as_str().unwrap();
        let status: serde_json::Value = serde_json::from_slice(
            &success(command(
                repo.path(),
                home.path(),
                &["task", "status", id, "--json"],
            ))
            .stdout,
        )
        .unwrap();
        assert!(!status.to_string().contains("claude:"));
    }
}

#[test]
fn explicit_graph_agent_reaches_composed_routers_and_repeated_steps() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("claude", CLAUDE_SETTINGS),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("security", "#!/bin/sh\nexit 1\n"),
        ],
        Some(home.path()),
    );
    let store = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    connect_fixture_claude(home.path(), &store);
    repo.create_file(
        ".lf/config.yaml",
        "agent: codex\nisolate: true\ndiff: false\ndiff_files: false\n",
    );
    repo.create_file(
        ".lf/skills/probe.md",
        "---\nagent: codex\n---\nRun the probe.",
    );
    repo.create_file(
        ".lf/graphs/inner.yaml",
        "- step: {name: probe, id: work, agent: codex}\n- loop: work\n",
    );
    repo.create_file(".lf/graphs/routed.yaml", "- xor:\n    paths:\n      selected:\n        graph: inner\n        description: Run and repeat\n      skipped:\n        description: Skip\n");
    success(command(
        repo.path(),
        home.path(),
        &["-b", "-a", "claude:explicit", "graph", "routed"],
    ));
    let records = settings_records(home.path());
    assert_eq!(records.len(), 5); // router, work, decision, repeated work, decision
    for record in records {
        assert!(record["args"]
            .as_array()
            .unwrap()
            .windows(2)
            .any(|pair| pair == ["--model", "explicit"]));
    }
    let graphs = support::recorded_graphs(home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(
        graphs[0]
            .1
            .iter()
            .map(|step| step["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "xor-route",
            "probe",
            "loop-or-next",
            "probe",
            "loop-or-next"
        ]
    );
}

#[test]
fn cli_destinations_save_the_declaration_used_by_preparation_and_unset() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path());
    repo.create_file(".lf/skills/probe.md", "Report settings.\n");
    repo.create_file(".lf/config.yaml", "agent: claude:shared\n");
    repo.create_file(
        ".lf/wave/product/config.yaml",
        "id: 8ae21a68-8940-4553-a3ab-135133925e35\n",
    );
    repo.stage_all();
    repo.commit("Authored settings");
    let child = home.path().join("child");
    assert!(Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["worktree", "add", "-b", "child"])
        .arg(&child)
        .output()
        .unwrap()
        .status
        .success());
    let run = |args: &[&str]| success(command(&child, home.path(), args));
    run(&["set", "--global", "default_agent", "codex:global"]);
    let global = fs::read(home.path().join("config.yaml")).unwrap();
    run(&["set", "skills.probe.agent", "codex:project"]);
    run(&[
        "--wave",
        "product",
        "set",
        "skills.probe.agent",
        "claude:wave",
    ]);
    run(&[
        "--wave",
        "8ae21a68-8940-4553-a3ab-135133925e35",
        "set",
        "--shared",
        "chrome",
        "true",
    ]);
    let check = |wave: Option<&str>, expected: &str| {
        let mut args = Vec::new();
        if let Some(wave) = wave {
            args.extend(["--wave", wave]);
        }
        args.extend(["config", "show", "--skill", "probe", "--json"]);
        let inspection: serde_json::Value = serde_json::from_slice(&run(&args).stdout).unwrap();
        let config = loopflow::config::load_wave_config(Some(&child), wave)
            .unwrap()
            .unwrap();
        let prepared = loopflow::prompt::process::prepare_process_prompt(
            &config,
            loopflow::prompt::process::ProcessPromptInput {
                repo_root: child.clone(),
                skill: Some("probe".into()),
                wave: wave.map(str::to_owned),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(prepared.config.agent().unwrap(), expected);
        assert_eq!(prepared.config.chrome, wave.is_some().then_some(true));
        assert_eq!(inspection[0]["settings"]["agent"]["value"], expected);
    };
    check(None, "codex:project");
    check(Some("product"), "claude:wave");
    check(Some("8ae21a68-8940-4553-a3ab-135133925e35"), "claude:wave");
    let private = repo.path().join("lf.config.local.yaml");
    let before = fs::read(&private).unwrap();
    let rejected = command(&child, home.path(), &["set", "user.name", "Wrong scope"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("does not apply"));
    assert_eq!(fs::read(&private).unwrap(), before);
    run(&["--wave", "product", "unset", "skills.probe.agent"]);
    check(Some("product"), "codex:project");
    run(&["unset", "skills.probe.agent"]);
    check(None, "claude:shared");
    run(&["unset", "--shared", "agent"]);
    check(None, "codex:global");
    assert_eq!(fs::read(home.path().join("config.yaml")).unwrap(), global);
    assert!(!child.join("lf.config.local.yaml").exists());
}

#[test]
fn context_namespace_agrees_across_files_flags_and_preparation() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path());
    fs::write(
        home.path().join("config.yaml"),
        "agent: codex\ncontext:\n  docs: [global.md]\n",
    )
    .unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "context:\n  docs: [repo.md]\n  exclude: [skip.md]\n  diff: true\n",
    );
    repo.create_file(
        ".lf/skills/context-proof.md",
        "---\ncontext:\n  docs: [skill.md]\n---\nRead context.\n",
    );
    for name in [
        "global.md",
        "repo.md",
        "skill.md",
        "private.md",
        "skip.md",
        "cli.md",
    ] {
        repo.create_file(name, name);
    }
    let run = |args: &[&str]| {
        let result = command(repo.path(), home.path(), args).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result
    };
    run(&["set", "context.docs", "[private.md, skip.md]"]);
    let show = |prefix: &[&str]| {
        let mut args = prefix.to_vec();
        args.extend(["config", "show", "--skill", "context-proof", "--json"]);
        serde_json::from_slice::<serde_json::Value>(&run(&args).stdout).unwrap()
    };
    let config = loopflow::config::load_config(Some(repo.path()))
        .unwrap()
        .unwrap();
    let skill = loopflow::graph::load_skill("context-proof", repo.path()).unwrap();
    let resolved = loopflow::config::settings::resolve(&config, Some(&skill)).unwrap();
    let prepared = loopflow::prompt::process::prepare_process_prompt(
        &config,
        loopflow::prompt::process::ProcessPromptInput {
            repo_root: repo.path().into(),
            skill: Some("context-proof".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        show(&[])[0]["settings"]["context.docs"]["value"],
        serde_json::json!(resolved.config.context.docs)
    );
    let paths: Vec<_> = prepared
        .components
        .docs
        .iter()
        .map(|doc| doc.path.as_str())
        .collect();
    for name in ["global.md", "repo.md", "skill.md", "private.md"] {
        assert!(paths.contains(&name));
    }
    assert!(!paths.contains(&"skip.md"));
    let flags = show(&["--docs", "cli.md", "--diff", "none"]);
    assert_eq!(flags[0]["settings"]["context.diff"]["value"], false);
    assert!(flags[0]["settings"]["context.docs"]["value"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("cli.md")));
    run(&["set", "skills.context-proof.context.docs", "[]"]);
    assert_eq!(
        show(&[])[0]["settings"]["context.docs"]["value"],
        serde_json::json!([])
    );
    run(&["unset", "skills.context-proof.context.docs"]);
    assert!(show(&[])[0]["settings"]["context.docs"]["value"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("private.md")));
    let saved = fs::read_to_string(repo.path().join("lf.config.local.yaml")).unwrap();
    assert!(saved.contains("context:") && saved.contains("docs:"));
}

#[test]
fn configuration_table_keeps_defaults_and_candidate_details_opt_in() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("config.yaml"),
        "default_agent: codex\nmachine: workstation\naccount:\n  claude: [work]\ntheme: light\nuser:\n  name: Test User\n",
    )
    .unwrap();
    repo.create_file(".lf/skills/config-table-proof.md", "Inspect settings.\n");
    repo.create_file(
        ".lf/config.yaml",
        "skills:\n  config-table-proof:\n    agent: claude\n    chrome: false\n",
    );
    let show = |flags: &[&str]| {
        let mut args = vec!["config", "show", "--skill", "config-table-proof"];
        args.extend_from_slice(flags);
        let output = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let table = show(&[]);
    let headings = [
        "Agents and context",
        "Machines",
        "Accounts",
        "Desktop",
        "Loopflow",
    ];
    let positions: Vec<_> = headings
        .iter()
        .map(|heading| table.find(&format!("\n  {heading}\n")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!table.contains("\n  Release\n"));
    let machines = &table[positions[1]..positions[2]];
    assert!(machines.contains("machine.tasks") && machines.contains("workstation"));
    let accounts = &table[positions[2]..positions[3]];
    assert!(accounts.contains("account.claude") && accounts.contains("work"));
    assert!(table.contains("SETTING") && table.contains("VALUE") && table.contains("FROM"));
    assert!(
        table.contains("claude") && table.contains("Shared project · skill config-table-proof")
    );
    assert!(table.contains("false"));
    assert!(!table.contains("context.budgets.memory_bytes"));
    assert!(!table.contains("\u{1b}[") && !table.contains("{\"kind\""));
    assert!(!table.contains("an applicable agent choice wins"));
    assert!(!table.contains("default_agent"));
    let explanation = show(&["--explain"]);
    assert!(
        explanation.contains("selected") && explanation.contains("an applicable agent choice wins")
    );
    assert!(
        explanation.contains(".lf/config.yaml") && explanation.contains("$LF_HOME/config.yaml")
    );
    assert!(explanation.contains("\n  agent = claude\n"));
    assert!(explanation.contains("\n      default_agent = codex"));
    assert!(!explanation.contains("\n  default_agent"));
    let all = show(&["--all"]);
    assert!(all.contains("context.budgets.memory_bytes"));
    assert!(all.contains("context.docs") && all.contains("context.exclude"));
    assert!(all.contains("context.diff") && all.contains("context.paste"));
    assert!(!all.contains("default_agent") && !all.contains("settings hidden"));
    let json: serde_json::Value = serde_json::from_str(&show(&["--json"])).unwrap();
    assert_eq!(json[0]["settings"]["agent"]["value"], "claude");
    assert_eq!(json[0]["settings"]["default_agent"]["value"], "codex");
    assert_eq!(json[0]["settings"]["chrome"]["value"], false);
    assert!(json[0]["settings"]["context.budgets.memory_bytes"].is_object());
    assert!(json[0]["settings"]["agent"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|candidate| candidate["key"] == "default_agent" && candidate["source"] == "personal"));

    repo.create_file(".lf/config.yaml", "{}\n");
    let fallback = show(&["--explain"]);
    assert!(fallback.contains("\n  agent = codex\n"));
    assert!(fallback.contains("\n      default_agent = codex · Personal global\n"));
    assert!(!fallback.contains("\n  default_agent"));

    repo.create_file(
        ".lf/graphs/config-table-flow.yaml",
        "- config-table-proof\n- step: {name: config-table-proof, id: revision}\n",
    );
    let output = command(
        repo.path(),
        home.path(),
        &[
            "config",
            "show",
            "--graph",
            "config-table-flow",
            "--explain",
        ],
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(graph.contains("\nconfig-table-flow / config-table-proof\n"));
    assert!(graph.contains("\nconfig-table-flow / revision · config-table-proof\n"));
    assert!(!graph.contains("unnamed step"));
}

#[test]
#[cfg(unix)]
fn automatic_agent_selection_requires_exactly_one_installed_harness() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    symlink("/usr/bin/git", bin.path().join("git")).unwrap();
    repo.create_file(".lf/skills/selection-proof.md", "Report the selection.\n");
    let run = |args: &[&str]| {
        command(repo.path(), home.path(), args)
            .env("PATH", bin.path())
            .output()
            .unwrap()
    };
    let inspect = || {
        let output = run(&["config", "show", "--skill", "selection-proof", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()[0]["settings"]["agent"]
            .clone()
    };
    let unresolved = || {
        let agent = inspect();
        assert!(agent["value"].is_null(), "{agent}");
        assert!(agent["candidates"].as_array().unwrap().is_empty());
        let output = run(&["-b", "skill", "selection-proof"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("No agent selected"), "{error}");
        assert!(error.contains("lf set --global default_agent"), "{error}");
        assert!(!home.path().join("provider-started").exists());
    };
    let install = |name: &str| {
        let path = bin.path().join(name);
        fs::write(
            &path,
            "#!/bin/sh\nprintf started > \"$LF_HOME/provider-started\"\nexit 91\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    };

    unresolved();
    for harness in ["claude", "codex", "opencode", "pi"] {
        install(harness);
        let agent = inspect();
        assert_eq!(agent["value"], harness);
        assert_eq!(
            agent["candidates"][0]["locator"],
            "PATH (only installed harness)"
        );
        fs::remove_file(bin.path().join(harness)).unwrap();
    }
    install("claude");
    install("codex");
    unresolved();
    let output = run(&["config", "show", "--skill", "selection-proof", "--explain"]);
    let table = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success() && table.contains("Not selected"));
    assert!(table.contains("lf set --global default_agent"));

    let created = run(&[
        "task",
        "create",
        "--title",
        "Choose agents per step",
        "--json",
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let status = run(&["task", "status", task["id"].as_str().unwrap(), "--json"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    let output = run(&["set", "--global", "default_agent", "claude"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(inspect()["value"], "claude");
    let output = run(&[
        "-a",
        "codex",
        "config",
        "show",
        "--skill",
        "selection-proof",
        "--json",
    ]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["settings"]["agent"]["value"], "codex");
    assert_eq!(inspect()["value"], "claude");
    assert!(!home.path().join("provider-started").exists());
}
