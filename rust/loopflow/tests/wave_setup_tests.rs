//! Set up each area of work once: a Wave's committed `config.yaml` and your
//! own `lf set` values choose how its work runs, with no flags.
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use loopflow::store::sqlite::SqliteStore;
use loopflow::store::ProviderAccountId;
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn lf(repo: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    command(repo, home, args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    // A routed launch signs the shared provider home in, which is the Keychain
    // for Claude on macOS; keep it out of the real one.
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("security"), crate::support::FAKE_SECURITY).unwrap();
    std::fs::set_permissions(
        bin.join("security"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("USER", "fixture")
        .env("NO_COLOR", "1")
        .current_dir(repo)
        .args(args);
    command
}

struct Subscription(Child);

impl Drop for Subscription {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn configuration_edits_reach_an_open_wave_watch_without_store_writes() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/config.yaml", "agent: claude\n");
    repo.create_file(".lf/wave/infra/GOAL.md", "Keep it boring.\n");
    repo.create_file(".git/info/exclude", "/lf.config.local.yaml\n");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .ensure_wave(repo.path().to_str().unwrap(), "infra")
        .unwrap();
    let mut subscription = Subscription(
        command(
            repo.path(),
            home.path(),
            &["wave", "status", "infra", "--json", "--subscribe"],
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap(),
    );
    let stdout = subscription.0.stdout.take().unwrap();
    let (sender, frames) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let frame: Value = serde_json::from_str(&line.unwrap()).unwrap();
            if sender.send(frame).is_err() {
                break;
            }
        }
    });
    let agent = |value: &str, source: &str| {
        let frame = frames.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(frame["unavailable"].is_null(), "{frame}");
        assert_eq!(frame["body"]["setup"]["agent"]["value"], value, "{frame}");
        assert_eq!(frame["body"]["setup"]["agent"]["source"], source);
    };
    agent("claude", "repository");
    let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let revision = || {
        connection
            .query_row("PRAGMA data_version", [], |row| row.get::<_, i64>(0))
            .unwrap()
    };
    let before = revision();
    repo.create_file("lf.config.local.yaml", "agent: codex\n");
    agent("codex", "personal");
    repo.create_file("lf.config.local.yaml", "agent: pi\n");
    agent("pi", "personal");
    std::fs::remove_file(repo.path().join("lf.config.local.yaml")).unwrap();
    agent("claude", "repository");
    std::fs::write(home.path().join("config.yaml"), "agent: opencode\n").unwrap();
    agent("opencode", "personal");
    assert_eq!(revision(), before);
    drop(subscription);
    reader.join().unwrap();
}

fn json(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The agent a launch resolves, from the reproducible command `-v` prints
/// before starting it. No provider is installed, so nothing runs.
fn launch_agent(repo: &Path, home: &Path, wave: Option<&str>) -> String {
    let mut args = vec!["-v", "-b"];
    if let Some(wave) = wave {
        args.extend(["--wave", wave]);
    }
    args.extend(["skill", "probe"]);
    let output = lf(repo, home, &args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("lf ")?.split(" -a ").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no launch command in {stderr}"))
        .to_owned()
}

#[test]
fn documented_recipes_share_file_cli_and_inspection_without_a_database() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let wave = loopflow::id::WaveId::new();
    repo.create_file(
        ".lf/skills/design/SKILL.md",
        "---\ndefault_agent: claude\n---\nDesign.",
    );
    repo.create_file(".lf/skills/implement/SKILL.md", "Implement.");
    repo.create_file(".lf/wave/frontend/config.yaml", &format!("id: {wave}\n"));
    repo.create_file(".lf/graphs/release.yaml", "- step: {name: implement, id: write-code}\n- step: {name: implement, id: fix-review, agent: claude}\n");
    let selected = |args: &[&str]| {
        json(lf(repo.path(), home.path(), args))[0]["settings"]["agent"]["value"].clone()
    };
    for args in [
        vec!["set", "--global", "default_agent", "codex"],
        vec!["set", "--global", "skills.design.agent", "claude"],
        vec!["set", "skills.implement.agent", "codex"],
        vec![
            "--wave",
            "frontend",
            "set",
            "skills.implement.agent",
            "claude",
        ],
        vec![
            "--wave",
            "frontend",
            "set",
            "--shared",
            "default_agent",
            "claude",
        ],
        vec!["set", "graphs.release.steps.fix-review.agent", "claude"],
        vec!["set", "account.claude", "[work@example.com]"],
    ] {
        let output = lf(repo.path(), home.path(), &args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        selected(&["config", "show", "--skill", "design", "--json"]),
        "claude"
    );
    assert_eq!(
        selected(&[
            "config",
            "show",
            "--graph",
            "release",
            "--step",
            "write-code",
            "--json"
        ]),
        "codex"
    );
    assert_eq!(
        selected(&[
            "config",
            "show",
            "--graph",
            "release",
            "--step",
            "fix-review",
            "--json"
        ]),
        "claude"
    );
    assert_eq!(
        selected(&[
            "--wave",
            "frontend",
            "config",
            "show",
            "--skill",
            "implement",
            "--json"
        ]),
        "claude"
    );
    assert_eq!(
        selected(&[
            "-a",
            "codex",
            "--wave",
            "frontend",
            "config",
            "show",
            "--skill",
            "implement",
            "--json"
        ]),
        "codex"
    );
    let settings = json(lf(
        repo.path(),
        home.path(),
        &[
            "--setting",
            "skills.implement.yolo=false",
            "config",
            "show",
            "--skill",
            "implement",
            "--json",
        ],
    ));
    assert_eq!(settings[0]["settings"]["yolo"]["value"], false);
    assert_eq!(
        json(lf(repo.path(), home.path(), &["config", "retained"])),
        serde_json::json!([])
    );
    assert!(!home.path().join("loopflow.db").exists());
}

#[test]
fn each_wave_runs_its_own_setup_and_the_repository_is_the_default() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "graph: repo-flow\nagent: codex\ndiff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(".lf/wave/infra/GOAL.md", "Keep it boring.\n");
    repo.create_file(
        ".lf/wave/infra/config.yaml",
        "graph: code\nagent: claude\nskills:\n  probe: {agent: claude:opus}\n",
    );
    repo.create_file(".lf/wave/research/GOAL.md", "Find out.\n");
    repo.create_file(".lf/wave/research/config.yaml", "agent: codex:o3\n");
    repo.create_file(".lf/wave/plain/GOAL.md", "Inherit everything.\n");
    repo.stage_all();
    repo.commit("Set up three areas of work");

    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let checkout = repo.path().to_str().unwrap();
    store.ensure_wave(checkout, "infra").unwrap();
    store.ensure_wave(checkout, "research").unwrap();
    store.ensure_wave(checkout, "plain").unwrap();
    let here = store.local_machine().unwrap().id;
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "codex".into(),
            account_id: ProviderAccountId::parse("work").unwrap(),
            home: None,
            login_email: None,
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
    drop(store);
    // Machines and account routes are yours: `lf set`, never committed.
    for args in [
        vec!["--wave", "infra", "set", "machine.tasks", here.as_str()],
        vec!["--wave", "research", "set", "account.codex", "work"],
    ] {
        let output = lf(repo.path(), home.path(), &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Launches take their Wave's agent, per skill first; no Wave is the repository's.
    assert_eq!(
        launch_agent(repo.path(), home.path(), Some("infra")),
        "claude:opus"
    );
    assert_eq!(
        launch_agent(repo.path(), home.path(), Some("research")),
        "codex:o3"
    );
    assert_eq!(launch_agent(repo.path(), home.path(), None), "codex");

    let setup = |wave: &str| {
        json(lf(
            repo.path(),
            home.path(),
            &["wave", "status", wave, "--json"],
        ))["setup"]
            .clone()
    };
    let infra = setup("infra");
    assert_eq!(infra["graph"]["value"], "code");
    assert_eq!(infra["graph"]["source"], "repository");
    assert_eq!(infra["agent"]["value"], "claude");
    assert_eq!(infra["skill_agents"]["probe"]["value"], "claude:opus");
    assert_eq!(infra["tasks_machine"]["value"], here.as_str());
    assert_eq!(infra["tasks_machine"]["source"], "personal");
    assert_eq!(infra["session_machine"]["source"], "builtin");
    assert_eq!(infra["accounts"]["codex"]["source"], "builtin");

    let research = setup("research");
    assert_eq!(research["graph"]["value"], "repo-flow");
    assert_eq!(research["graph"]["source"], "repository");
    assert_eq!(research["agent"]["value"], "codex:o3");
    assert_eq!(research["agent"]["source"], "repository");
    assert_eq!(research["tasks_machine"]["source"], "builtin");
    assert_eq!(research["accounts"]["codex"]["accounts"][0], "work");
    assert_eq!(research["accounts"]["codex"]["source"], "personal");

    let plain = setup("plain");
    assert_eq!(plain["graph"]["source"], "repository");
    assert_eq!(plain["agent"]["value"], "codex");
    assert_eq!(plain["agent"]["source"], "repository");
}

#[test]
fn your_own_value_wins_for_you_until_you_unset_it() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "agent: codex\ndiff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(".lf/wave/research/GOAL.md", "Find out.\n");
    repo.create_file(".lf/wave/research/config.yaml", "agent: codex:o3\n");
    repo.stage_all();
    repo.commit("Share the research agent");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .ensure_wave(repo.path().to_str().unwrap(), "research")
        .unwrap();
    drop(store);
    let ok = |args: &[&str]| {
        let output = lf(repo.path(), home.path(), args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let agent = || {
        json(lf(
            repo.path(),
            home.path(),
            &["wave", "status", "research", "--json"],
        ))["setup"]["agent"]
            .clone()
    };
    let shared_path = repo.path().join(".lf/wave/research/config.yaml");
    let shared = std::fs::read_to_string(&shared_path).unwrap();

    ok(&["--wave", "research", "set", "agent", "claude:haiku"]);
    assert_eq!(
        launch_agent(repo.path(), home.path(), Some("research")),
        "claude:haiku"
    );
    assert_eq!(agent()["source"], "personal");
    assert_eq!(std::fs::read_to_string(&shared_path).unwrap(), shared);

    ok(&["--wave", "research", "unset", "agent"]);
    assert_eq!(agent()["value"], "codex:o3");
    assert_eq!(agent()["source"], "repository");
    assert!(!lf(repo.path(), home.path(), &["set", "release", "x"])
        .status
        .success());
}

#[test]
fn a_wave_file_skips_repository_settings_with_a_warning() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(".lf/wave/infra/GOAL.md", "Keep it boring.\n");
    repo.create_file(
        ".lf/wave/infra/config.yaml",
        "agent: codex\nrelease:\n  targets: {}\n",
    );
    repo.stage_all();
    repo.commit("Misplace a repository setting");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .ensure_wave(repo.path().to_str().unwrap(), "infra")
        .unwrap();
    drop(store);
    let output = lf(
        repo.path(),
        home.path(),
        &[
            "--wave", "infra", "config", "settings", "--skill", "probe", "--json",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("config.yaml:2: skipping release: setting does not apply here"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let settings: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(settings[0]["settings"]["agent"]["value"], "codex");
}

#[test]
fn settings_scope_by_the_typed_wave_only() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/wave/research/GOAL.md", "Find out.\n");
    repo.stage_all();
    repo.commit("Add a Wave");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let checkout = repo.path().canonicalize().unwrap();
    let wave = store
        .ensure_wave(checkout.to_str().unwrap(), "research")
        .unwrap();
    // An ambient Wave, as in its Task's worktree, does not scope `lf set`.
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("PATH", "/usr/bin:/bin")
        .env("LF_WAVE_ID", wave.as_str())
        .current_dir(repo.path())
        .args(["set", "agent", "claude:haiku"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let typed = lf(
        repo.path(),
        home.path(),
        &["--wave", "research", "set", "skills.probe.agent", "codex"],
    );
    assert!(typed.status.success());
    let settings: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        &std::fs::read_to_string(loopflow::config::files::private_path(&checkout).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(settings["agent"].as_str(), Some("claude:haiku"));
    // Written in the layout released `lf` reads: the Wave, then the skill.
    assert_eq!(
        settings["waves"][wave.as_str()]["skills"]["probe"]["agent"].as_str(),
        Some("codex")
    );
    assert!(settings["waves"][wave.as_str()].get("agent").is_none());
}

#[test]
fn setting_targets_inspect_and_unset_through_the_cli() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/skills/probe.md",
        "---\nagent: claude\nchrome: true\n---\nProbe.",
    );
    repo.create_file(
        ".lf/graphs/release.yaml",
        "- step: {name: probe, id: write-code}\n- step: {name: probe, id: review}\n",
    );
    std::fs::write(home.path().join("config.yaml"), "agent: claude\nskills: {probe: {agent: codex}}\ngraphs: {release: {steps: {write-code: {agent: claude, chrome: true}}}}\n").unwrap();
    let inspect = || {
        json(lf(
            repo.path(),
            home.path(),
            &["config", "show", "--graph", "release", "--json"],
        ))
    };
    let original = inspect();
    assert_eq!(original[0]["settings"]["agent"]["value"], "claude");
    assert_eq!(original[1]["settings"]["agent"]["value"], "codex");
    for (key, value) in [
        ("agent", "codex"),
        ("graphs.release.steps.write-code.chrome", "false"),
    ] {
        let output = lf(repo.path(), home.path(), &["set", "--global", key, value]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let changed = inspect();
    // Editing the general declaration retains this source's occurrence choice.
    assert_eq!(changed[0]["settings"]["agent"]["value"], "claude");
    assert_eq!(changed[0]["settings"]["chrome"]["value"], false);
    assert_eq!(changed[1]["settings"]["chrome"]["value"], true);
    let candidates = changed[0]["settings"]["chrome"]["candidates"]
        .as_array()
        .unwrap();
    assert_eq!(
        candidates.last().unwrap()["target"],
        serde_json::json!({"kind": "graph_occurrence", "graph": "release", "id": "write-code"})
    );
    let output = lf(
        repo.path(),
        home.path(),
        &[
            "unset",
            "--global",
            "graphs.release.steps.write-code.chrome",
        ],
    );
    assert!(output.status.success());
    assert_eq!(inspect()[0]["settings"]["chrome"]["value"], true);
    let selected = json(lf(
        repo.path(),
        home.path(),
        &[
            "--setting",
            "graphs.release.steps.write-code.chrome=false",
            "config",
            "show",
            "--graph",
            "release",
            "--step",
            "write-code",
            "--json",
        ],
    ));
    assert_eq!(selected[0]["settings"]["chrome"]["value"], false);
}

#[test]
fn setting_targets_inspect_the_defining_graph_and_authored_id() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/skills/probe.md", "Probe.");
    repo.create_file(
        ".lf/graphs/inner.yaml",
        "- interactive: {name: probe, id: work}\n- step: {name: probe, id: child-only}\n",
    );
    repo.create_file(
        ".lf/graphs/team/outer.yaml",
        "- graph: inner\n- xor:\n    paths:\n      selected:\n        graph: inner\n        description: Run again\n      skipped:\n        description: Skip\n- interactive: {name: probe, id: work}\n",
    );
    for (key, value) in [
        ("graphs.inner.steps.work.agent", "claude:inner"),
        ("graphs.team/outer.steps.work.agent", "codex:outer"),
    ] {
        let output = lf(repo.path(), home.path(), &["set", "--global", key, value]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let inspect = |graph: &str, step: Option<&str>| {
        let mut args = vec!["config", "show", "--graph", graph, "--json"];
        if let Some(step) = step {
            args.extend(["--step", step]);
        }
        lf(repo.path(), home.path(), &args)
    };
    // Interactive navigation names change during composition; settings IDs do not.
    let whole = json(inspect("team/outer", None));
    let occurrences = whole.as_array().unwrap();
    assert_eq!(occurrences.len(), 6);
    assert_eq!(occurrences[2]["skill"], "xor-route");
    for index in [0, 3] {
        assert_eq!(occurrences[index]["occurrence"]["graph"], "inner");
        assert_eq!(occurrences[index]["occurrence"]["id"], "work");
        assert_eq!(
            occurrences[index]["settings"]["agent"]["value"],
            "claude:inner"
        );
    }
    for index in [1, 4] {
        assert_eq!(occurrences[index]["occurrence"]["graph"], "inner");
        assert_eq!(occurrences[index]["occurrence"]["id"], "child-only");
    }
    assert_eq!(occurrences[5]["occurrence"]["graph"], "team/outer");
    assert_eq!(occurrences[5]["occurrence"]["id"], "work");
    assert_eq!(occurrences[5]["settings"]["agent"]["value"], "codex:outer");

    for graph in ["team/outer", "team-outer"] {
        assert_eq!(
            json(inspect(graph, Some("work"))),
            serde_json::json!([occurrences[5]])
        );
        let missing = inspect(graph, Some("child-only"));
        assert!(!missing.status.success());
        assert!(String::from_utf8_lossy(&missing.stderr).contains("no matching agent steps"));
    }
    assert_eq!(
        json(inspect("inner", Some("work"))),
        serde_json::json!([occurrences[0]])
    );
    assert_eq!(
        json(inspect("inner", Some("child-only"))),
        serde_json::json!([occurrences[1]])
    );
}
