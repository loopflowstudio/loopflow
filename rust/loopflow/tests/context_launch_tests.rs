//! Real terminal dispatch with providers replaced by argument-limited scripts.
#![cfg(unix)]

use crate::support;

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use loopflow_test_support::TestRepo;
use rusqlite::OptionalExtension;

fn native_codex_command(repo: &std::path::Path, home: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", home.join("bin").display()),
        )
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CODEX_HOME", home.join(".codex"))
        .current_dir(repo)
        .args(args)
        .stdin(std::process::Stdio::null());
    command
}

/// A Task with the Machine server running, holding `files` in its own linked
/// checkout. A conversation that names it is Work: without a terminal it runs
/// detached, and Codex runs behind its private app-server, where lf's
/// configuration rides. Adopting a checkout starts no Task conversation beside
/// it, which would share the stand-in's state.
fn codex_task(
    repo: &TestRepo,
    home: &Path,
    files: &[(&str, &str)],
) -> (loopflow_test_support::MachineServer, PathBuf, String) {
    let server = loopflow_test_support::MachineServer::start(env!("CARGO_BIN_EXE_lf"), home);
    let worktree = repo.create_named_worktree("native");
    for (path, content) in files {
        let path = worktree.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    let output = native_codex_command(
        repo.path(),
        home,
        &[
            "task",
            "create",
            "--title",
            "Native Codex",
            "--worktree",
            worktree.to_str().unwrap(),
            "--json",
        ],
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let created: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = created["task"]["id"].as_str().unwrap().to_string();
    (server, worktree, id)
}

fn native_logs(home: &Path) -> String {
    fs::read_dir(home.join("logs"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            fs::read_to_string(entry.path())
                .ok()
                .map(|text| format!("{}: {text}", entry.path().display()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Live agent Processes once their exit records are written. The terminal
/// gets the provider's exit after a short wait; its host lingers to settle the
/// provider, so a reader waits for it.
fn settled_live_agents(database: &rusqlite::Connection) -> i64 {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let live: i64 = database
            .query_row(
                "SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if live == 0 || std::time::Instant::now() >= deadline {
            return live;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
fn codex_service_tier_native_inherits_provider_default() {
    assert_codex_service_tier("-i", None, None);
}

#[test]
fn codex_service_tier_native_uses_standard() {
    assert_codex_service_tier("-i", Some("standard"), Some("default"));
}

#[test]
fn codex_service_tier_native_uses_fast() {
    assert_codex_service_tier("-i", Some("fast"), Some("priority"));
}

#[test]
fn codex_service_tier_headless_inherits_provider_default() {
    assert_codex_service_tier("-b", None, None);
}

#[test]
fn codex_service_tier_headless_uses_standard() {
    assert_codex_service_tier("-b", Some("standard"), Some("default"));
}

#[test]
fn codex_service_tier_headless_uses_fast() {
    assert_codex_service_tier("-b", Some("fast"), Some("priority"));
}

fn assert_codex_service_tier(mode: &str, setting: Option<&str>, expected: Option<&str>) {
    let repo = TestRepo::new();
    let files = [
        (
            ".lf/config.yaml",
            "diff: false\ndiff_files: false\npaste: false\n",
        ),
        (".lf/skills/tier-probe.md", "Tier probe"),
        (".lf/graphs/tier-check.yaml", "- tier-probe\n"),
    ];
    for (path, content) in files {
        repo.create_file(path, content);
    }
    let home = tempfile::tempdir().unwrap();
    support::install_native_codex(&home.path().join("bin"));
    // Only the native Work conversation needs a Task and Machine server.
    let native_task = (mode == "-i").then(|| codex_task(&repo, home.path(), &files));
    let cwd = native_task
        .as_ref()
        .map_or(repo.path(), |(_, worktree, _)| worktree.as_path());
    let run = |args: &[&str]| {
        let output = native_codex_command(cwd, home.path(), args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}/{setting:?} {args:?}: {}\n{}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            native_logs(home.path())
        );
    };
    if let Some(setting) = setting {
        run(&["set", "--global", "codex.service_tier", setting]);
    }
    let native = home.path().join(".codex");
    fs::create_dir_all(&native).unwrap();
    let provider_config = "service_tier = \"priority\"\n";
    fs::write(native.join("config.toml"), provider_config).unwrap();
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&fs::read(home.path().join(name)).unwrap()).unwrap()
    };
    if let Some((_, worktree, task)) = &native_task {
        let session = support::prepare_native_conversation(
            &home.path().join("loopflow.db"),
            worktree,
            "codex",
            None,
            "Tier probe",
        );
        run(&[mode, "--task", task, "-a", "codex", "resume", &session]);
    } else {
        run(&[mode, "-a", "codex", "graph", "tier-check"]);
    }
    let params = read("native-config.json");
    assert_eq!(
        params
            .get("serviceTier")
            .and_then(serde_json::Value::as_str),
        expected
    );
    if mode == "-i" {
        let server = read("native-server-config.json");
        assert_eq!(
            server
                .get("service_tier")
                .and_then(serde_json::Value::as_str),
            expected
        );
    }
    assert_eq!(read("native-tier.json"), expected.unwrap_or("priority"));
    let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    assert_eq!(settled_live_agents(&database), 0);
    if mode == "-b" {
        // Graph Runs are finite; headless resume continues an interactive Session.
        run(&["-i", "-a", "codex", ":", "Resume seed"]);
        assert_eq!(settled_live_agents(&database), 0);
    }
    let session: String = database
        .query_row(
            "SELECT id FROM agent_sessions WHERE interactive=1 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    if mode == "-i" {
        fs::write(home.path().join("check-child-tiers"), "").unwrap();
    }
    let resume_tier = setting.map(|_| "default");
    let mut args = vec![mode];
    if resume_tier.is_some() {
        args.extend(["--setting", "codex.service_tier=default"]);
    }
    if mode == "-b" {
        args.extend(["steer", &session, "Resume tier probe"]);
    } else {
        args.extend(["resume", &session]);
    }
    run(&args);
    assert_eq!(
        read("native-config.json")["serviceTier"].as_str(),
        resume_tier
    );
    if mode == "-i" {
        let probe = read("native-child-tiers.json");
        assert_eq!(probe["new"], resume_tier.unwrap_or("priority"));
        assert_eq!(probe["fork"], resume_tier.unwrap_or("priority"));
        // Give the fixture's private provider an exit signal if it is still live.
        fs::write(home.path().join("release-child-tier-server"), "").unwrap();
    }
    assert_eq!(settled_live_agents(&database), 0);
    assert_eq!(
        fs::read_to_string(native.join("config.toml")).unwrap(),
        provider_config
    );
}

#[test]
fn codex_startup_failures_preserve_input_and_allow_reconnect() {
    for failure in [
        "fail-server",
        "stall-server",
        "fail-terminal",
        "fail-after-submit",
    ] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        crate::support::install_native_codex(&home.path().join("bin"));
        let (_server, worktree, task) = codex_task(
            &repo,
            home.path(),
            &[(
                ".lf/config.yaml",
                "diff: false\ndiff_files: false\npaste: false\n",
            )],
        );
        let native = home.path().join(".codex");
        fs::create_dir(&native).unwrap();
        let config = "model=\"fixture\"\n";
        let unrelated = "{\"session_id\":\"unrelated-native-session\",\"ts\":9999999999}\n";
        fs::write(native.join("config.toml"), config).unwrap();
        fs::write(native.join("history.jsonl"), unrelated).unwrap();
        fs::write(home.path().join(failure), "").unwrap();
        let session = support::prepare_native_conversation(
            &home.path().join("loopflow.db"),
            &worktree,
            "codex",
            None,
            "Preserve this request",
        );
        let start = std::time::Instant::now();
        let output = native_codex_command(
            &worktree,
            home.path(),
            &["-i", "--task", &task, "resume", &session],
        )
        .output()
        .unwrap();
        assert!(!output.status.success(), "{failure}");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "{failure} must fail before the opener's deadline"
        );
        // A terminal's exit reads as the terminal's own output, on stdout.
        let stderr = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // A quit owner still writing its records returns the terminal's own
        // exit (the fixture's 24 or 25) instead of its own message.
        let terminal_exit = match failure {
            "fail-terminal" => Some(24),
            "fail-after-submit" => Some(25),
            _ => None,
        };
        assert!(
            (terminal_exit.is_some() && output.status.code() == terminal_exit)
                || (failure == "fail-after-submit"
                    && stderr.contains("native terminal connection"))
                || stderr.contains(if failure == "fail-terminal" {
                    "exited with"
                } else if failure == "fail-after-submit" {
                    "status"
                } else {
                    "app-server"
                }),
            "{failure}: {stderr}"
        );
        let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        assert_eq!(
            settled_live_agents(&database),
            0,
            "{failure} must settle the owned provider"
        );
        assert_eq!(
            fs::read_to_string(native.join("config.toml")).unwrap(),
            config
        );
        assert_eq!(
            fs::read_to_string(native.join("history.jsonl")).unwrap(),
            unrelated
        );
        let adopted: i64 = database.query_row(
            "SELECT count(*) FROM session_events WHERE kind='captured' AND provider_thread='unrelated-native-session'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(
            adopted, 0,
            "failed startup must not adopt unrelated history"
        );
        fs::remove_file(home.path().join(failure)).unwrap();
        let output = native_codex_command(&worktree, home.path(), &["session", "resume", &session])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{failure}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let turns: serde_json::Value =
            serde_json::from_slice(&fs::read(home.path().join("native-turns.json")).unwrap())
                .unwrap();
        // The first turn goes in before the terminal opens, so a terminal that
        // fails leaves it in history, sent once, and the next one follows it.
        let submitted = failure.starts_with("fail-") && failure != "fail-server";
        assert_eq!(
            turns.as_array().unwrap().len(),
            if submitted { 2 } else { 1 },
            "{failure}"
        );
        if submitted {
            assert_eq!(
                turns[1]["items"][0]["content"][0]["text"],
                "Native follow-up"
            );
        }
        assert!(turns[0]["items"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Preserve this request"));
        let sessions: i64 = database
            .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sessions, 1, "retry keeps the same conversation");
    }
}

#[test]
fn codex_terminal_delivers_large_input_and_approvals_then_reconnects_without_replay() {
    let repo = TestRepo::new();
    let body = "Preserve 雪, \"quotes\", $(shell syntax), and newlines.\n".repeat(8_000);
    let home = tempfile::tempdir().unwrap();
    crate::support::install_native_codex(&home.path().join("bin"));
    let (_server, worktree, task) = codex_task(
        &repo,
        home.path(),
        &[
            (
                ".lf/config.yaml",
                "yolo: false\ndiff: false\ndiff_files: false\npaste: false\n",
            ),
            (".lf/skills/probe.md", &body),
        ],
    );
    let command = |args: &[&str]| native_codex_command(&worktree, home.path(), args);
    let session = support::prepare_native_conversation(
        &home.path().join("loopflow.db"),
        &worktree,
        "codex",
        Some("probe"),
        "Find the bug.",
    );
    let output = command(&["-i", "--task", &task, "resume", &session])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&fs::read(home.path().join(name)).unwrap()).unwrap()
    };
    let turns = read("native-turns.json");
    let input = turns[0]["items"][0]["content"][0]["text"].as_str().unwrap();
    let expected = format!(
        "<lf:skill:probe>\n{body}\n</lf:skill:probe>\n\n<lf:message>\nFind the bug.\n</lf:message>"
    );
    assert!(
        input == expected,
        "first input differs: expected {} bytes, received {}",
        expected.len(),
        input.len()
    );
    assert_eq!(turns.as_array().unwrap().len(), 1);
    assert_eq!(
        read("native-approval.json")["decision"],
        "decline",
        "the terminal must answer, not a headless auto-approver"
    );
    let config = read("native-config.json");
    let server = read("native-server-config.json");
    let capture = fs::read_to_string(home.path().join("launched")).unwrap();
    let key = capture.lines().next().unwrap();
    let shard = &key.strip_prefix("run_").unwrap_or(key)[..2];
    let manifest = read(&format!("runs/{shard}/{key}/manifest.json"));
    // lf's configuration is the private app-server's, so a conversation the
    // person starts in the client has it too; lf's own thread adds none.
    assert_eq!(
        server["developer_instructions"],
        manifest["exec"]["system_prompt"]
    );
    assert!(config.get("developerInstructions").is_none());
    assert!(config.get("config").is_none());
    assert!(config.get("baseInstructions").is_none());
    assert!(config.get("approvalPolicy").is_none());
    assert_eq!(config["threadSource"], "user");
    let hooks = &server["hooks"];
    assert_eq!(hooks["SessionStart"][0]["hooks"][0]["command"], "host-hook");
    let state = hooks["state"].as_object().unwrap();
    assert_eq!(
        state["host-trust"]["trusted_hash"], "unchanged",
        "host trust survives"
    );
    // Codex skips unreviewable thread hooks unless the thread trusts them.
    // Loopflow's own SessionStart groups follow the host group's index.
    assert!(state.contains_key("/<session-flags>/config.toml:session_start:1:0"));
    assert!(!state.contains_key("/<session-flags>/config.toml:session_start:0:0"));
    assert!(hooks["SessionStart"].as_array().unwrap().len() > 1);
    let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    // Closing before subscription must not report the already-recorded first
    // request as unsent. A genuine terminal failure must still fail.
    for marker in ["close-terminal", "fail-terminal"] {
        assert_eq!(settled_live_agents(&database), 0);
        fs::write(home.path().join(marker), "").unwrap();
        let output = command(&["session", "resume", &session]).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.success(),
            marker == "close-terminal",
            "{marker}: {stderr}"
        );
        assert!(!stderr.contains("before submitting"), "{stderr}");
        assert_eq!(read("native-turns.json").as_array().unwrap().len(), 1);
        fs::remove_file(home.path().join(marker)).unwrap();
    }
    assert_eq!(settled_live_agents(&database), 0);
    let output = command(&["session", "resume", &session]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let turns = read("native-turns.json");
    assert_eq!(turns.as_array().unwrap().len(), 2);
    assert_eq!(
        turns[1]["items"][0]["content"][0]["text"],
        "Native follow-up"
    );
    assert_eq!(
        settled_live_agents(&database),
        0,
        "both launches must settle their owned providers"
    );
}

#[test]
fn terminal_context_refreshes_without_replacing_instructions_or_losing_failed_sessions() {
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(".lf/skills/probe.md", "Saved active skill.");
    let large = "UNCHANGED_LARGE_FILE".repeat(12_000);
    repo.create_file("scratch/large.md", &large);
    repo.create_file("scratch/small.md", "CURRENT_SCRATCH");
    let harness = "claude";
    repo.create_file(".lf/wave/fixture/MEMORY.md", "CURRENT_WAVE_MEMORY");
    let home = tempfile::tempdir().unwrap();
    let _server = loopflow_test_support::MachineServer::start(
        env!("CARGO_BIN_EXE_lf"),
        &home.path().join("machine"),
    );
    let store =
        loopflow_execution::store::open_sqlite(&home.path().join("machine/loopflow.db")).unwrap();
    loopflow_execution::work::wave::bootstrap::upsert_wave(
        &store,
        &loopflow_execution::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "fixture".into(),
            loopflow_execution::repository::discover(repo.path())
                .unwrap()
                .to_string(),
        ),
    )
    .unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join(harness);
    fs::write(
        &provider,
        r#"#!/bin/sh
for arg in "$@"; do
    [ "${#arg}" -lt 122880 ] || exit 99
    printf '%s\000' "$arg" >> "$HOME/argv"
done
printf '%s' "$LF_CONTEXT_DELIVERY" > "$HOME/delivery-path"
printf '%s' "$LF_BARE_STEER" > "$HOME/bare-steer"
cat > "$HOME/input"
printf 'provider fixture refused this request\n' >&2
exit 23
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CODEX_HOME", home.path().join(".codex"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .current_dir(repo.path())
        .args([
            "-i",
            "-a",
            harness,
            "--wave",
            "fixture",
            "probe",
            "Find the bug.",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("provider fixture refused"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let raw = fs::read_to_string(home.path().join("argv")).unwrap();
    let args: Vec<_> = raw.split('\0').collect();
    let instructions = {
        let index = args
            .iter()
            .position(|arg| *arg == "--append-system-prompt-file")
            .unwrap();
        fs::read_to_string(args[index + 1]).unwrap()
    };
    assert_eq!(instructions.matches("<lf:loopflow>").count(), 1);
    assert!(!instructions.contains("CURRENT_SCRATCH"));
    assert!(!instructions.contains("CURRENT_WAVE_MEMORY"));
    assert!(!instructions.contains("Saved active skill."));
    assert!(!instructions.contains("This Session:"));
    assert!(!raw.contains("model_instructions_file"));
    assert!(!raw.contains("dangerously-bypass-hook-trust"));
    let input = fs::read_to_string(home.path().join("input")).unwrap();
    assert!(!raw.contains("Saved active skill."));
    assert_eq!(input.matches("<lf:skill:probe>\nSaved active skill.\n</lf:skill:probe>\n\n<lf:message>\nFind the bug.\n</lf:message>").count(), 1);
    let callbacks: Vec<String> = {
        let index = args.iter().position(|arg| *arg == "--settings").unwrap();
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(args[index + 1]).unwrap()).unwrap();
        settings["hooks"]["SessionStart"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| group["hooks"][0]["command"].as_str().unwrap().to_owned())
            .collect()
    };
    // A chat reports to no server; its hooks read steers from the store.
    let bare_steer = fs::read_to_string(home.path().join("bare-steer")).unwrap();
    let adapter: loopflow_execution::steer::SteerAdapter =
        serde_json::from_str(&bare_steer).unwrap();
    let identity = format!("This Session: {}\n", adapter.session);
    assert!(!instructions.contains(&adapter.session));
    let invoke = |moment: &str| {
        let callback = callbacks
            .iter()
            .find(|callback| callback.ends_with(&format!(" {moment}")))
            .unwrap();
        let mut child = Command::new("/bin/sh")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.path())
            .env("LF_HOME", home.path().join("machine"))
            .env("LF_BARE_STEER", &bare_steer)
            .env(
                "LF_CONTEXT_DELIVERY",
                fs::read_to_string(home.path().join("delivery-path")).unwrap(),
            )
            .current_dir(repo.path())
            .args(["-c", callback])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                br#"{"session_id":"native-context-fixture","hook_event_name":"SessionStart"}"#,
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        value["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let first = invoke("start");
    assert!(first.starts_with(&identity), "{first}");
    assert!(first.contains("CURRENT_SCRATCH"));
    assert!(first.contains("CURRENT_WAVE_MEMORY"));
    assert!(!first.contains("Saved active skill.\n"));
    assert!(first.contains("UNCHANGED_LARGE_FILE"));
    assert!(first.contains("source=\"scratch/large.md\" excerpt=\"start\""));
    assert!(!first.contains(&large));
    repo.create_file("scratch/small.md", "FRESH_SCRATCH");
    repo.create_file(".lf/wave/fixture/MEMORY.md", "FRESH_WAVE_MEMORY");
    let compact = invoke("compact");
    assert!(compact.starts_with(&identity), "{compact}");
    assert!(compact.contains("FRESH_WAVE_MEMORY"));
    assert!(!compact.contains("CURRENT_WAVE_MEMORY"));
    assert!(compact.contains("FRESH_SCRATCH"));
    assert!(!compact.contains("CURRENT_SCRATCH"));
    assert!(compact.contains("Saved active skill."));
    assert!(!compact.contains("Find the bug."));
    assert!(compact.len() <= 10_000);
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/large.md")).unwrap(),
        large
    );
    repo.create_file("scratch/small.md", "CURRENT_SCRATCH");
    let db = rusqlite::Connection::open(home.path().join("machine/loopflow.db")).unwrap();
    let sessions: i64 = db
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 1);
}

#[test]
fn headless_codex_keeps_large_first_turn_separate_from_additive_instructions() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let content = "Preserve infrastructure configurations and implementation.\n".repeat(2_500);
    repo.create_file(".lf/skills/probe.md", &content);
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file("scratch/thread-context.md", "ONLY_THREAD_FIXTURE_CONTEXT");
    repo.stage_all();
    repo.commit("Define the probe skill and its context");
    let provider = bin.join("codex");
    let script = support::codex_socket_script(
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo fixture; exit 0; fi
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
printf '%s' "$thread_start" > "$HOME/thread-request"
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-fixture"}}}'
read -r turn_start
printf '%s' "$turn_start" > "$HOME/turn-request"
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-fixture"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread-fixture","turn":{"id":"turn-fixture","status":"inProgress"}}}'
echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread-fixture","turnId":"turn-fixture","itemId":"message-fixture","delta":"Context received."}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-fixture","turn":{"id":"turn-fixture","status":"completed"}}}'
while read -r line; do :; done
"#,
    );
    fs::write(&provider, script).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") {
            command.env_remove(key);
        }
    }
    let output = command
        .current_dir(repo.path())
        .args(["-b", "-a", "codex", "probe"])
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CODEX_HOME", home.path().join(".codex"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let thread: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("thread-request")).unwrap()).unwrap();
    assert!(thread["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains("<lf:loopflow>"));
    assert!(thread["params"]["baseInstructions"].is_null());
    assert!(thread["params"]["config"]["model_instructions_file"].is_null());
    assert!(!thread["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains(&content));
    let database = rusqlite::Connection::open(home.path().join("machine/loopflow.db")).unwrap();
    let session: String = database
        .query_row("SELECT id FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    let instructions = thread["params"]["developerInstructions"].as_str().unwrap();
    assert!(!instructions.contains(&session));
    assert!(!instructions.contains("This Session:"));
    // The actual emitted hook runs with no launch-delivery environment. Codex's
    // app-server supplies native thread identity on stdin, including compaction.
    for (group, expected) in [(0, "start"), (1, "compact")] {
        let callback = thread["params"]["config"]["hooks"]["SessionStart"][group]["hooks"][0]
            ["command"]
            .as_str()
            .unwrap();
        assert!(callback.ends_with(expected));
        let mut child = Command::new("/bin/sh")
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args(["-c", callback])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"session_id":"thread-fixture","hook_event_name":"SessionStart"}"#)
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let context: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let text = context["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            text.starts_with(&format!("This Session: {session}\n")),
            "{text}"
        );
        assert!(!text.contains("This Session's Task:"));
        assert!(text.contains("ONLY_THREAD_FIXTURE_CONTEXT"));
    }
    let turn: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("turn-request")).unwrap()).unwrap();
    assert!(turn["params"]["input"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&content));
}

#[test]
fn claude_terminal_delivers_large_first_turn_without_prompt_arguments() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let content = "Preserve 雪, \"quotes\", $(shell syntax), and newlines.\n".repeat(8_000);
    repo.create_file(".lf/skills/large.md", &content);
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.stage_all();
    repo.commit("Define the large skill");
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        "#!/bin/sh\nfor arg in \"$@\"; do [ \"${#arg}\" -lt 1024 ] || exit 99; done\ncat > \"$HOME/input\"\n",
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") {
            command.env_remove(key);
        }
    }
    let output = command
        .current_dir(repo.path())
        .args(["-i", "-a", "claude", "large"])
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::read_to_string(home.path().join("input"))
        .unwrap()
        .contains(&content));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("\x1b[2K"));
}

#[test]
fn claude_opens_lf_created_worktree_already_trusted() {
    let repo = TestRepo::new();
    let worktree = repo.path().with_file_name(format!(
        "{}.agent-trust",
        repo.path().file_name().unwrap().to_string_lossy()
    ));
    loopflow_execution::git::worktree_add(
        repo.path(),
        &worktree,
        "agent-trust",
        loopflow_execution::git::WorktreeBranch::New {
            start_point: "HEAD",
        },
    )
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        "#!/bin/sh\ncp \"$CLAUDE_CONFIG_DIR/.claude.json\" \"$HOME/seen.json\"\n",
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .current_dir(&worktree)
        .args(["-i", "-a", "claude", ":", "hello"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let seen: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.path().join("seen.json")).unwrap()).unwrap();
    let key = worktree.canonicalize().unwrap().display().to_string();
    assert_eq!(
        seen,
        serde_json::json!({"projects": {key: {"hasTrustDialogAccepted": true}}})
    );
}

/// A stand-in Claude that keeps what it saw under `$HOME`, for one launch environment.
fn claude_launch(home: &Path, script: &str) -> Command {
    install_claude_fixture(home, script);
    claude_command(home)
}

fn install_claude_fixture(home: &Path, script: &str) {
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(&provider, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
}

fn claude_command(home: &Path) -> Command {
    let bin = home.join("bin");
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", home)
        .env("LF_HOME", home.join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .stdin(std::process::Stdio::null());
    command
}

fn succeed(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn saved_claude_model_is_used_by_new_sessions_but_not_changed_on_resume() {
    for (agent, saved, expected) in [
        ("claude", None, None),
        ("claude", Some("opus"), Some("opus")),
        ("claude:fable", Some("opus"), Some("fable")),
    ] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let script = "printf '%s\\n' \"$@\" > \"$HOME/args\"\ncat >/dev/null";
        let run = |args: &[&str]| {
            succeed(
                claude_launch(home.path(), script)
                    .current_dir(repo.path())
                    .args(args),
            )
        };
        if let Some(saved) = saved {
            run(&["set", "--global", "claude.model", saved]);
        }
        run(&["set", "--global", "default_agent", "claude"]);
        let launched_model = || {
            let args = fs::read_to_string(home.path().join("args")).unwrap();
            let args: Vec<_> = args.lines().collect();
            args.windows(2)
                .find(|pair| pair[0] == "--model")
                .map(|pair| pair[1].to_owned())
        };
        run(&["-i", "-a", agent, ":", "model probe"]);
        assert_eq!(launched_model().as_deref(), expected);
        let database = rusqlite::Connection::open(home.path().join("machine/loopflow.db")).unwrap();
        assert_eq!(settled_live_agents(&database), 0);
        let (session, model): (String, Option<String>) = database
            .query_row(
                "SELECT id, model FROM agent_sessions WHERE interactive=1 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(model.as_deref(), expected);
        let args = fs::read_to_string(home.path().join("args")).unwrap();
        let thread = args
            .lines()
            .skip_while(|arg| *arg != "--session-id")
            .nth(1)
            .unwrap();
        let project = home.path().join(".claude/projects/fixture");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(format!("{thread}.jsonl")), "{}\n").unwrap();
        run(&["set", "--global", "claude.model", "sonnet"]);
        run(&["-i", "resume", &session]);
        assert_eq!(launched_model().as_deref(), expected);
        assert_eq!(settled_live_agents(&database), 0);
        run(&["-i", "-a", "claude", "resume", &session]);
        assert_eq!(launched_model().as_deref(), Some("sonnet"));
        assert_eq!(settled_live_agents(&database), 0);
    }
}

#[test]
fn claude_opens_a_task_checkout_already_trusted() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _server = loopflow_test_support::MachineServer::start(
        env!("CARGO_BIN_EXE_lf"),
        &home.path().join("machine"),
    );
    let script = "cp \"$CLAUDE_CONFIG_DIR/.claude.json\" \"$HOME/seen.json\"";
    let created: serde_json::Value = serde_json::from_str::<serde_json::Value>(&succeed(
        claude_launch(home.path(), script)
            .current_dir(repo.path())
            .args(["task", "create", "--title", "Trust", "--json"]),
    ))
    .unwrap()["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let placed: serde_json::Value = serde_json::from_str(&succeed(
        claude_launch(home.path(), script)
            .current_dir(repo.path())
            .args(["checkout", id, "--json"]),
    ))
    .unwrap();
    let worktree = PathBuf::from(placed["worktree"].as_str().unwrap());

    succeed(
        claude_launch(home.path(), script)
            .current_dir(&worktree)
            .args(["-i", "-a", "claude", ":", "hello"]),
    );

    let seen: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.path().join("seen.json")).unwrap()).unwrap();
    let key = worktree.canonicalize().unwrap().display().to_string();
    assert_eq!(seen["projects"][&key]["hasTrustDialogAccepted"], true);
}

#[test]
fn claude_dialog_in_a_checkout_lf_did_not_create_makes_the_session_available() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let list = "\"$LF_BIN\" session list --available --json > \"$HOME/available.json\"";

    succeed(
        claude_launch(home.path(), list)
            .current_dir(repo.path())
            .args(["-i", "-a", "claude", ":", "hello"]),
    );

    let available: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.path().join("available.json")).unwrap())
            .unwrap();
    assert_eq!(available.as_array().unwrap().len(), 1, "{available}");
    assert!(
        !home.path().join(".claude/.claude.json").exists(),
        "the dialog stays Claude's to answer"
    );
}

/// Lets a holding provider go when the test ends, however it ends.
struct Release(PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = fs::write(&self.0, b"");
    }
}

fn open_task_conversations(db: &Path, task: &str) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM agent_sessions s
             JOIN held_terminals t ON t.id=s.held_terminal_id
             WHERE s.task_id=?1 AND s.interactive=1 AND s.completed_at IS NULL
                 AND t.ended_at IS NULL",
            [task],
            |row| row.get(0),
        )
        .unwrap_or(0)
}

#[test]
fn a_task_checkout_starts_its_conversation_in_the_background() {
    assert_background_conversation(false);
}

#[test]
fn a_task_run_starts_its_conversation_and_abandon_stops_it() {
    assert_background_conversation(true);
}

fn assert_background_conversation(run_graph: bool) {
    let repo = TestRepo::new();
    repo.create_file(".lf/graphs/pause.yaml", "- interactive: demo\n");
    repo.stage_all();
    repo.commit("Add a graph that waits for review");
    repo.push();
    let home = tempfile::tempdir().unwrap();
    // The server's recovery pass starts the conversation, with the stand-in.
    let _server = loopflow_test_support::MachineServer::start_with_path(
        env!("CARGO_BIN_EXE_lf"),
        &home.path().join("machine"),
        &format!("{}:/usr/bin:/bin", home.path().join("bin").display()),
    );
    let stop = home.path().join("stop");
    let _release = Release(stop.clone());
    // The provider holds until released, so the conversation can be seen.
    let script = format!(
        "while [ -d \"$HOME\" ] && [ ! -e '{}' ]; do sleep 0.1; done",
        stop.display()
    );
    // Polling must leave the script intact while the background launch opens it.
    install_claude_fixture(home.path(), &script);
    let command = || claude_command(home.path());
    succeed(command().current_dir(repo.path()).args([
        "set",
        "--global",
        "default_agent",
        "claude",
    ]));
    let created: serde_json::Value =
        serde_json::from_str(&succeed(command().current_dir(repo.path()).args([
            "task",
            "create",
            "--title",
            "Background conversation",
            "--json",
        ])))
        .unwrap();
    let id = created["task"]["id"].as_str().unwrap();
    let db = home.path().join("machine/loopflow.db");
    // Older open Tasks must not prevent this newly checked-out conversation
    // from starting. These receipts deliberately exceed the former count cap.
    let conn = rusqlite::Connection::open(&db).unwrap();
    for _ in 0..16 {
        let task = loopflow_execution::durable::TaskId::new();
        let terminal = loopflow_model::id::TerminalId::new();
        let session = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
            SELECT ?1,repository_id,?1,1 FROM tasks WHERE id=?2",
            rusqlite::params![task.as_str(), id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO held_terminals(id,holder_build,created_at) VALUES(?1,'fixture',1)",
            [&terminal],
        )
        .unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,interactive,held_terminal_id)
            VALUES(?1,'Older conversation','generated',1,1,'/older',?2,1,?3)", rusqlite::params![session,task.as_str(),terminal]).unwrap();
    }
    drop(conn);

    let worktree = if run_graph {
        succeed(
            command()
                .current_dir(repo.path())
                .args(["task", "run", id, "pause"]),
        );
        let path: String = rusqlite::Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT worktree FROM task_checkouts WHERE task_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        PathBuf::from(path)
    } else {
        let placed: serde_json::Value = serde_json::from_str(&succeed(
            command()
                .current_dir(repo.path())
                .args(["checkout", id, "--json"]),
        ))
        .unwrap();
        PathBuf::from(placed["worktree"].as_str().unwrap())
    };

    // The conversation shows up in the worktree once its provider is running.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let listed: serde_json::Value = serde_json::from_str(&succeed(
            command()
                .current_dir(&worktree)
                .args(["session", "list", "--json"]),
        ))
        .unwrap();
        let conversations: Vec<&serde_json::Value> = listed
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["workspace"]["task_id"] == id)
            .collect();
        assert!(conversations.len() <= 1, "{listed}");
        if conversations
            .first()
            .is_some_and(|row| matches!(row["state"].as_str(), Some("active" | "detached")))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the checkout started no conversation: {listed}\n{}",
            fs::read_to_string(home.path().join("machine/logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Checking the Task out again keeps the one conversation.
    succeed(
        command()
            .current_dir(repo.path())
            .args(["checkout", id, "--json"]),
    );
    assert_eq!(open_task_conversations(&db, id), 1);

    succeed(
        command()
            .current_dir(repo.path())
            .args(["task", "abandon", id]),
    );
    // A Task conversation is held; abandoning ends its unviewed terminal.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while open_task_conversations(&db, id) > 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "abandon left the conversation running"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// The Task's conversation: its current agent, that agent's pid, and the
/// recovery pass's count of starts.
fn task_conversation_agent(db: &Path, task: &str) -> Option<(String, Option<i64>, u32)> {
    rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT s.agent_process_id,s.agent_pid,s.starts_since_turn FROM agent_sessions s
             WHERE s.id=(SELECT session_id FROM task_primary_sessions WHERE task_id=?1) AND s.agent_process_id IS NOT NULL AND s.agent_ended_at IS NULL",
            [task],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .unwrap()
}

/// Wait for the Task's conversation to run an agent other than `previous`.
fn next_task_conversation_agent(
    db: &Path,
    home: &Path,
    task: &str,
    previous: Option<&str>,
) -> (String, i64, u32) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if let Some((agent, Some(pid), attempts)) = task_conversation_agent(db, task) {
            if Some(agent.as_str()) != previous {
                return (agent, pid, attempts);
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the conversation did not start after {previous:?}\n{}",
            fs::read_to_string(home.join("machine/logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// The `lf holder run` that `pid`, a provider in a held terminal, runs under.
fn holder_of(mut pid: i64) -> i64 {
    loop {
        let line =
            succeed(Command::new("/bin/ps").args(["-o", "ppid=,command=", "-p", &pid.to_string()]));
        let (parent, command) = line.trim().split_once(' ').unwrap();
        if command.contains("holder run") {
            return pid;
        }
        pid = parent.parse().unwrap();
    }
}

#[test]
fn a_killed_task_conversation_comes_back_once_and_a_quit_one_stays_quit() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let machine = home.path().join("machine");
    let path = format!("{}:/usr/bin:/bin", home.path().join("bin").display());
    let server = || {
        loopflow_test_support::MachineServer::start_with_path(
            env!("CARGO_BIN_EXE_lf"),
            &machine,
            &path,
        )
    };
    let mut running = Some(server());
    let stop = home.path().join("stop");
    let _release = Release(stop.clone());
    // Each launch logs the thread it starts or resumes.
    let threads = home.path().join("threads.log");
    let script = format!(
        "prev=; for a in \"$@\"; do case \"$prev\" in --resume|--session-id) echo \"$prev $a\" >> '{}';; esac; prev=\"$a\"; done\n\
         while [ -d \"$HOME\" ] && [ ! -e '{}' ]; do sleep 0.1; done",
        threads.display(),
        stop.display()
    );
    // The first `count` launches, once the provider has logged them.
    let launches = |count: usize| -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let logged: Vec<String> = fs::read_to_string(&threads)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect();
            if logged.len() >= count {
                return logged;
            }
            assert!(std::time::Instant::now() < deadline, "{logged:?}");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    };
    succeed(
        claude_launch(home.path(), &script)
            .current_dir(repo.path())
            .args(["set", "--global", "default_agent", "claude"]),
    );
    let created: serde_json::Value = serde_json::from_str(&succeed(
        claude_launch(home.path(), &script)
            .current_dir(repo.path())
            .args(["task", "create", "--title", "Comes back", "--json"]),
    ))
    .unwrap();
    let id = created["task"]["id"].as_str().unwrap();
    succeed(
        claude_launch(home.path(), &script)
            .current_dir(repo.path())
            .args(["checkout", id, "--json"]),
    );
    let db = machine.join("loopflow.db");
    let (first, pid, attempts) = next_task_conversation_agent(&db, home.path(), id, None);
    assert_eq!(attempts, 0, "a first start is not a death");
    let thread = launches(1)[0]
        .strip_prefix("--session-id ")
        .expect("a first start opens a thread")
        .to_owned();
    // Claude wrote its transcript, in the home the server's starts use.
    let transcript = machine.join(format!(".claude/projects/-repo/{thread}.jsonl"));
    fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    fs::write(&transcript, "{}\n").unwrap();

    // Killed: it comes back on its thread, counted once. Coming back is what
    // says the death was not read as a quit.
    succeed(Command::new("/bin/kill").args(["-9", &pid.to_string()]));
    let (second, provider, attempts) =
        next_task_conversation_agent(&db, home.path(), id, Some(&first));
    assert_eq!(attempts, 1);
    assert_eq!(launches(2)[1..], [format!("--resume {thread}")]);
    let status = || -> serde_json::Value {
        let detail: serde_json::Value = serde_json::from_str(&succeed(
            claude_launch(home.path(), &script)
                .current_dir(repo.path())
                .args(["task", "status", id, "--json"]),
        ))
        .unwrap();
        let conversation = detail["conversation"].as_str().unwrap().to_string();
        detail["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == conversation.as_str())
            .unwrap()
            .clone()
    };
    let conversation = status();
    assert!(
        ["active", "detached"].contains(&conversation["state"].as_str().unwrap()),
        "{conversation}"
    );
    assert_eq!(conversation["start"]["attempts"], 1, "{conversation}");
    assert_eq!(
        conversation["outlook"],
        serde_json::Value::Null,
        "{conversation}"
    );

    // A finished turn resets the count; then the server stops, the holder
    // dies with it, and the next server starts the conversation once. The
    // crash took the transcript too: it goes on in a fresh thread.
    fs::remove_file(&transcript).unwrap();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET starts_since_turn=0,last_started_at=NULL",
            [],
        )
        .unwrap();
    drop(running.take());
    let holder = holder_of(provider);
    succeed(Command::new("/bin/kill").args(["-9", &holder.to_string()]));
    running = Some(server());
    let (third, _, attempts) = next_task_conversation_agent(&db, home.path(), id, Some(&second));
    assert_eq!(attempts, 1);
    std::thread::sleep(std::time::Duration::from_secs(4));
    assert_eq!(
        task_conversation_agent(&db, id).map(|(agent, _, attempts)| (agent, attempts)),
        Some((third.clone(), 1)),
        "started once"
    );
    let fresh = &launches(3)[2..];
    assert!(
        fresh.len() == 1 && fresh[0].starts_with("--session-id ") && !fresh[0].ends_with(&thread),
        "{fresh:?}"
    );

    // Quit: the provider exits 0 and stays down.
    fs::write(&stop, b"").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while task_conversation_agent(&db, id).is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "the provider did not exit"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    std::thread::sleep(std::time::Duration::from_secs(4));
    let current: (String, u32) = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT s.agent_process_id,s.starts_since_turn FROM agent_sessions s
             WHERE s.id=(SELECT session_id FROM task_primary_sessions WHERE task_id=?1)",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(current, (third, 1), "a quit conversation stays quit");
    let conversation = status();
    assert_eq!(conversation["state"], "closed", "{conversation}");
    assert_eq!(conversation["ended"]["exit_code"], 0, "{conversation}");
    assert_eq!(
        conversation["start"],
        serde_json::Value::Null,
        "{conversation}"
    );
    assert_eq!(conversation["outlook"]["kind"], "quit", "{conversation}");
    assert!(
        conversation["status_line"]
            .as_str()
            .is_some_and(|line| line.starts_with("quit ") && line.ends_with(" opens it")),
        "{conversation}"
    );
    assert_eq!(
        conversation["outlook"]["at"],
        serde_json::Value::Null,
        "{conversation}"
    );
    drop(running);
}

#[test]
fn a_task_conversation_whose_agent_is_not_installed_says_why_it_did_not_start() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    // The server finds no provider on its PATH.
    let _server = loopflow_test_support::MachineServer::start(
        env!("CARGO_BIN_EXE_lf"),
        &home.path().join("machine"),
    );
    succeed(
        claude_launch(home.path(), "exit 0")
            .current_dir(repo.path())
            .args(["set", "--global", "default_agent", "claude"]),
    );
    let created: serde_json::Value = serde_json::from_str(&succeed(
        claude_launch(home.path(), "exit 0")
            .current_dir(repo.path())
            .args(["task", "create", "--title", "Checkout alone", "--json"]),
    ))
    .unwrap();
    let id = created["task"]["id"].as_str().unwrap();
    let placed: serde_json::Value = serde_json::from_str(&succeed(
        claude_launch(home.path(), "exit 0")
            .current_dir(repo.path())
            .args(["checkout", id, "--json"]),
    ))
    .unwrap();
    assert!(Path::new(placed["worktree"].as_str().unwrap()).is_dir());
    let db = home.path().join("machine/loopflow.db");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let failure = loop {
        let failure: Option<String> = rusqlite::Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT s.start_failure FROM agent_sessions s
                 WHERE s.id=(SELECT session_id FROM task_primary_sessions WHERE task_id=?1)",
                [id],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
            .flatten();
        if let Some(failure) = failure {
            break failure;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the server recorded no reason\n{}",
            fs::read_to_string(home.path().join("machine/logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    assert_eq!(failure, "claude is not installed on this Machine's PATH");
    assert_eq!(open_task_conversations(&db, id), 0);
    // It never ran, so it reads as that, never as a death.
    let status = succeed(
        claude_launch(home.path(), "exit 0")
            .current_dir(repo.path())
            .args(["task", "status", id]),
    );
    assert!(
        status.contains(
            "Conversation: could not start: claude is not installed on this Machine's PATH"
        ),
        "{status}"
    );
}

#[test]
fn a_task_checkout_and_any_other_carry_the_same_launcher_text() {
    let repo = TestRepo::new();
    repo.create_branch("launcher");
    let task_checkout = repo
        .create_named_worktree("managed")
        .canonicalize()
        .unwrap();
    let home = tempfile::tempdir().unwrap();
    let machine = home.path().join("machine");
    fs::create_dir(&machine).unwrap();
    let _server = loopflow_test_support::MachineServer::start(env!("CARGO_BIN_EXE_lf"), &machine);
    let registered =
        support::register_task_without_pr(&machine, &task_checkout, "managed", &repo.head_sha());
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo fixture; exit 0; fi
while [ "$#" -gt 0 ]; do
    if [ "$1" = --append-system-prompt-file ]; then
        shift
        cp "$1" "$HOME/instructions"
        exit 0
    fi
    shift
done
exit 17
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    // A Task's checkout and any other carry the same launcher text.
    for checkout in [repo.path(), task_checkout.as_path()] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", home.path())
            .env("LF_HOME", &machine)
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .current_dir(checkout)
            .args(["-i", "-a", "claude", ":", "inspect this checkout"])
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let instructions = fs::read_to_string(home.path().join("instructions")).unwrap();
        assert_eq!(instructions.matches("<lf:loopflow>").count(), 1);
        assert!(!instructions.contains("You are this Task's operator"));
    }
    let db = rusqlite::Connection::open(&registered.database).unwrap();
    let tasks: i64 = db
        .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tasks, 1, "launching never creates another Task");
    fs::remove_dir_all(task_checkout).unwrap();
}

#[test]
fn context_preview_matches_launched_input_without_writing_or_running_provider() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/skills/probe.md", "Read this exact preview marker.");
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\ncontext_budgets:\n  memory_tokens: 300\n",
    );
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
printf 'called\n' >> "$HOME/provider-calls"
if [ "$1" = --version ]; then echo fixture; exit 0; fi
if [ "$1" = --dangerously-bypass-hook-trust ] && [ "$2" = --model ]; then
    echo "error: a value is required for '--model <MODEL>' but none was supplied" >&2
    exit 2
fi
for arg in "$@"; do
    if [ "$next" = yes ]; then cp "$arg" "$HOME/received-context"; next=no; fi
    [ "$arg" = --append-system-prompt-file ] && next=yes
done
printf 'fixture finished\n' >&2
exit 23
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let message = "Exact supplied direction. ".repeat(500);
    let invoke = |preview: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path().join("machine"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("NO_COLOR", "1")
            .current_dir(repo.path())
            .args(["-i", "-a", "claude", "skill", "probe", &message]);
        if preview {
            command.args(["--context", "--json"]);
        }
        command.output().unwrap()
    };
    let preview = invoke(true);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let input: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert!(input["task_prompt"].as_str().unwrap().contains(&message));
    assert!(!repo.path().join(".lf/prompts").exists());
    assert!(!home.path().join("provider-calls").exists());
    assert!(!home.path().join("machine").exists());
    assert!(!repo.path().join(".lf/tmp").exists());

    let launch = invoke(false);
    assert!(!launch.status.success());
    assert!(String::from_utf8_lossy(&launch.stderr).contains("fixture finished"));
    assert_eq!(
        fs::read_to_string(home.path().join("received-context")).unwrap(),
        input["system_prompt"].as_str().unwrap()
    );
}

#[test]
fn graph_context_preview_preserves_absent_store_and_captured_first_skill() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "agent: codex\ncontext: {diff: false, diff_files: false, paste: false}\n",
    );
    repo.create_file(".lf/skills/probe.md", "First input marker.");
    repo.create_file(".lf/skills/decide.md", "Do not assemble future input.");
    repo.create_file(
        ".lf/graphs/inspect.yaml",
        "- loop:\n    - probe\n    - decide\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo.path())
        .args(["graph", "inspect", "literal input", "--context", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["compiled"]["steps"][1]["returns_to"], 0);
    assert_eq!(report["inputs"][0]["node"], 0);
    let prompt = report["inputs"][0]["input"]["task_prompt"]
        .as_str()
        .unwrap();
    assert!(prompt.contains("First input marker."));
    assert!(prompt.contains("literal input"));
    assert!(!prompt.contains("Do not assemble future input."));
    assert_eq!(report["inputs"][1]["state"], "unavailable");
    assert!(!home.path().join("machine").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn graph_context_preview_retains_native_arguments_and_operating_guidance() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(
        ".claude/skills/audit/SKILL.md",
        "---\nname: audit\ndescription: Native audit\n---\nAudit these exact arguments.",
    );
    repo.create_file(".lf/graphs/inspect.yaml", "- audit\n");
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("machine"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo.path())
        .args([
            "-a",
            "claude",
            "graph",
            "inspect",
            "  literal input  ",
            "--context",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let input = &report["inputs"][0]["input"];
    assert_eq!(input["skill_invocation"]["skill"]["name"], "audit");
    assert_eq!(input["skill_invocation"]["arguments"], "  literal input  ");
    assert!(input["system_prompt"]
        .as_str()
        .unwrap()
        .contains("<lf:loopflow>"));
    assert!(!home.path().join("machine").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn operator_context_preview_uses_resolved_skills_without_preparing_checkouts() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "agent: codex\ncontext: {diff: false, diff_files: false, paste: false}\n",
    );
    repo.create_file(".lf/graphs/inspect.yaml", "- repo/operate\n");
    let preview = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path().join("machine"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args(args)
            .args(["--context", "--json"])
            .output()
            .unwrap()
    };
    for args in [
        vec!["operate"],
        vec!["repo/operate"],
        vec!["skill", "repo-operate"],
        vec!["graph", "inspect"],
    ] {
        let output = preview(&args);
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("context unavailable before the operator's scope checkout is prepared"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // A literal hierarchical override is not the builtin operator. Preview must
    // follow the resolved skill, rather than imposing policy on its spelling.
    repo.create_file(
        ".lf/skills/repo/operate.md",
        "Local skill, current checkout.",
    );
    for args in [vec!["repo/operate"], vec!["graph", "inspect"]] {
        let output = preview(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Local skill, current checkout."));
    }
    assert!(!home.path().join("machine").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
    assert!(!repo.path().join(".git/worktrees").exists());
}

#[test]
fn explicit_documents_match_preview_capture_and_codex_input() {
    for (selection, native) in [("relative", false), ("absolute", true), ("external", false)] {
        for large in [false, true] {
            let repo = TestRepo::new();
            let home = tempfile::tempdir().unwrap();
            support::install_native_codex(&home.path().join("bin"));
            let body = format!(
                "FIRST-662\n{}MIDDLE-662\n{}FINAL-662\n",
                "reference snow 雪 $passive\n".repeat(if large { 5_000 } else { 1 }),
                "document data ".repeat(if large { 20_000 } else { 1 })
            );
            let path = if selection == "external" {
                home.path().join("ignored.txt")
            } else {
                repo.path().join("document.txt")
            };
            fs::write(&path, &body).unwrap();
            let skill = if native {
                (
                    ".agents/skills/probe/SKILL.md",
                    "---\nname: probe\ndescription: Probe docs\n---\nRead the supplied reference.",
                )
            } else {
                (
                    ".lf/skills/probe.md",
                    "---\ndocs: [frontmatter.txt]\n---\nRead the supplied reference.",
                )
            };
            let files = [
                (".gitignore", "ignored.txt\n"),
                ("config.txt", "CONFIG-DOCUMENT-662"),
                ("frontmatter.txt", "FRONTMATTER-DOCUMENT-662"),
                (
                    ".lf/config.yaml",
                    "diff: false\ndiff_files: false\npaste: false\ndocs: [config.txt]\n",
                ),
                skill,
            ];
            // The interactive launch is Work, so its first turn rides Codex's
            // app-server rather than a bare terminal's argv.
            let work = (selection == "external").then(|| codex_task(&repo, home.path(), &files));
            let checkout = match &work {
                Some((_, worktree, _)) => worktree.clone(),
                None => {
                    for (file, content) in files {
                        repo.create_file(file, content);
                    }
                    repo.stage_all();
                    repo.commit("Define the probe skill and its documents");
                    repo.path().to_path_buf()
                }
            };
            let selected = if selection == "relative" {
                "document.txt".to_string()
            } else {
                path.display().to_string()
            };
            let mut args = vec![if selection == "external" { "-i" } else { "-b" }];
            if let Some((_, _, task)) = &work {
                args.extend(["--task", task.as_str()]);
            }
            args.extend([
                "-a",
                "codex",
                "--docs",
                &selected,
                "skill",
                "probe",
                "Report final marker.",
            ]);
            let preview = native_codex_command(&checkout, home.path(), &args)
                .args(["--context", "--json"])
                .output()
                .unwrap();
            assert!(
                preview.status.success(),
                "{}",
                String::from_utf8_lossy(&preview.stderr)
            );
            let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
            let expected = preview["task_prompt"].as_str().unwrap();
            assert!(expected.contains(&body.replace('$', "&#36;")));
            assert_eq!(expected.matches("FIRST-662").count(), 1);
            assert!(expected.contains("CONFIG-DOCUMENT-662"));
            assert_eq!(expected.contains("FRONTMATTER-DOCUMENT-662"), !native);
            // The document rides the prompt, never as a reference; a Task's
            // launch references only its own context.
            assert!(!preview["references"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reference| reference.to_string().contains(&selected)));
            assert_eq!(
                preview["references"].as_array().unwrap().is_empty(),
                work.is_none()
            );
            assert!(!checkout.join(".lf/prompts").exists());
            assert!(!home.path().join("native-argv.jsonl").exists());
            assert!(!home.path().join("runs").exists());
            // Creating the Task opened the store; preview still writes nothing.
            assert_eq!(home.path().join("loopflow.db").exists(), work.is_some());
            let managed_session = (selection == "external").then(|| {
                support::prepare_native_conversation(
                    &home.path().join("loopflow.db"),
                    &checkout,
                    "codex",
                    Some("probe"),
                    "Report final marker.",
                )
            });
            if let Some(session) = &managed_session {
                let skill = args.iter().position(|arg| *arg == "skill").unwrap();
                args.truncate(skill);
                args.extend(["resume", session]);
            }
            let output = native_codex_command(&checkout, home.path(), &args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{selection}/{large}: {}\n{}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
                native_logs(home.path())
            );
            let turns: serde_json::Value =
                serde_json::from_slice(&fs::read(home.path().join("native-turns.json")).unwrap())
                    .unwrap();
            let inputs = turns[0]["items"][0]["content"].as_array().unwrap();
            assert_eq!(inputs.last().unwrap()["text"], expected);
            assert_eq!(!preview["skill_invocation"].is_null(), native);
            let contexts = explicit_document_captures(home.path());
            assert_eq!(contexts.len(), 1);
            assert_eq!(contexts[0]["context"]["task"]["text"], expected);
            assert!(contexts[0]["context"]["task"]["assets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|asset| {
                    asset["kind"] == "document"
                        && asset["source_path"].as_str().is_some_and(|source| {
                            checkout.join(source).canonicalize().unwrap()
                                == path.canonicalize().unwrap()
                                && expected.contains(&format!(
                                    "Document: {}",
                                    serde_json::to_string(source).unwrap()
                                ))
                        })
                }));
            fs::write(&path, "Changed after submission").unwrap();
            assert_eq!(explicit_document_captures(home.path()), contexts);
        }
    }
}

fn explicit_document_captures(home: &std::path::Path) -> Vec<serde_json::Value> {
    ignore::WalkBuilder::new(home.join("runs"))
        .hidden(false)
        .build()
        .map(Result::unwrap)
        .filter(|entry| entry.file_name() == "context.json")
        .map(|entry| serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap())
        .collect()
}

#[test]
fn explicit_documents_native_reconnect_supplies_only_the_new_selection() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    support::install_native_codex(&home.path().join("bin"));
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file("first.txt", "FIRST-DOCUMENT-662");
    repo.create_file("next.txt", "NEXT-DOCUMENT-662");
    repo.stage_all();
    repo.commit("Document fixtures");
    // Keep the repository conversation's managed checkout inside this fixture.
    let checkout = home.path().join("checkout");
    assert!(Command::new("git")
        .args(["clone", "--quiet"])
        .arg(repo.path())
        .arg(&checkout)
        .status()
        .unwrap()
        .success());
    for (key, value) in [
        ("user.name", "Fixture Participant"),
        ("user.email", "fixture@example.invalid"),
    ] {
        assert!(Command::new("git")
            .current_dir(&checkout)
            .args(["config", key, value])
            .status()
            .unwrap()
            .success());
    }
    let command = |args: &[&str]| native_codex_command(&checkout, home.path(), args);
    let prepared = command(&["resume", "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let session = prepared["id"].as_str().unwrap();
    let first_path = repo.path().join("first.txt");
    let next_path = repo.path().join("next.txt");
    let first = command(&[
        "-b",
        "-a",
        "codex",
        "--docs",
        first_path.to_str().unwrap(),
        "steer",
        session,
        "FIRST-REQUEST-662",
    ])
    .output()
    .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let next = command(&[
        "-b",
        "--docs",
        next_path.to_str().unwrap(),
        "steer",
        session,
        "Read the new document.",
    ])
    .output()
    .unwrap();
    assert!(
        next.status.success(),
        "{}",
        String::from_utf8_lossy(&next.stderr)
    );
    let turns: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("native-turns.json")).unwrap()).unwrap();
    assert_eq!(turns.as_array().unwrap().len(), 2);
    let old = turns[0]["items"][0]["content"][0]["text"].as_str().unwrap();
    let new = turns[1]["items"][0]["content"][0]["text"].as_str().unwrap();
    assert!(old.contains("FIRST-DOCUMENT-662"));
    assert!(new.contains("NEXT-DOCUMENT-662"));
    assert!(!new.contains("FIRST-DOCUMENT-662"));
    assert!(!new.contains("FIRST-REQUEST-662"));
    let contexts = explicit_document_captures(home.path());
    assert!(contexts
        .iter()
        .any(|context| context["context"]["task"]["text"] == new));
    let live: i64 = db
        .query_row(
            "SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(live, 0);
}
