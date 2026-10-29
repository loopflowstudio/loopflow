//! Real terminal dispatch with providers replaced by argument-limited scripts.
#![cfg(unix)]

use crate::support;

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow_test_support::TestRepo;

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

#[test]
fn codex_startup_failures_preserve_input_and_allow_reconnect() {
    for failure in [
        "fail-server",
        "stall-server",
        "fail-terminal",
        "fail-after-submit",
    ] {
        let repo = TestRepo::new();
        repo.create_file(
            ".lf/config.yaml",
            "diff: false\ndiff_files: false\npaste: false\n",
        );
        let home = tempfile::tempdir().unwrap();
        crate::support::install_native_codex(&home.path().join("bin"));
        let native = home.path().join(".codex");
        fs::create_dir(&native).unwrap();
        let config = "model=\"fixture\"\n";
        let unrelated = "{\"session_id\":\"unrelated-native-session\",\"ts\":9999999999}\n";
        fs::write(native.join("config.toml"), config).unwrap();
        fs::write(native.join("history.jsonl"), unrelated).unwrap();
        fs::write(home.path().join(failure), "").unwrap();
        let start = std::time::Instant::now();
        let output = native_codex_command(
            repo.path(),
            home.path(),
            &["-i", "-a", "codex", ":", "Preserve this request"],
        )
        .output()
        .unwrap();
        assert!(!output.status.success(), "{failure}");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "{failure} must fail before the opener's deadline"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            (failure == "fail-after-submit" && stderr.contains("native terminal connection"))
                || stderr.contains(if failure == "fail-terminal" {
                    "before submitting"
                } else if failure == "fail-after-submit" {
                    "status"
                } else {
                    "app-server"
                }),
            "{failure}: {stderr}"
        );
        let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let live: i64 = database
            .query_row(
                "SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(live, 0, "{failure} must settle the owned provider");
        assert_eq!(
            fs::read_to_string(native.join("config.toml")).unwrap(),
            config
        );
        assert_eq!(
            fs::read_to_string(native.join("history.jsonl")).unwrap(),
            unrelated
        );
        let adopted: i64 = database.query_row(
            "SELECT count(*) FROM session_events WHERE json_extract(payload,'$.evidence.provider_session_id')='unrelated-native-session'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(
            adopted, 0,
            "failed startup must not adopt unrelated history"
        );
        let session: String = database
            .query_row("SELECT id FROM agent_sessions LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        fs::remove_file(home.path().join(failure)).unwrap();
        let output =
            native_codex_command(repo.path(), home.path(), &["session", "resume", &session])
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
        assert_eq!(
            turns.as_array().unwrap().len(),
            if failure == "fail-after-submit" { 2 } else { 1 },
            "{failure}"
        );
        if failure == "fail-after-submit" {
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
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    let body = "Preserve 雪, \"quotes\", $(shell syntax), and newlines.\n".repeat(8_000);
    repo.create_file(".lf/skills/probe.md", &body);
    let home = tempfile::tempdir().unwrap();
    crate::support::install_native_codex(&home.path().join("bin"));
    let command = |args: &[&str]| native_codex_command(repo.path(), home.path(), args);
    let output = command(&["-i", "-a", "codex", "probe", "Find the bug."])
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
    let capture = fs::read_to_string(home.path().join("launched")).unwrap();
    let key = capture.lines().next().unwrap();
    let shard = &key.strip_prefix("run_").unwrap_or(key)[..2];
    let manifest = read(&format!("runs/{shard}/{key}/manifest.json"));
    assert_eq!(
        config["developerInstructions"],
        manifest["exec"]["system_prompt"]
    );
    assert!(config.get("baseInstructions").is_none());
    assert_eq!(config["approvalPolicy"], "on-request");
    assert_eq!(config["threadSource"], "user");
    assert_eq!(config["config"]["native-setting"], true);
    let hooks = &config["config"]["hooks"];
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
    let session: String = database
        .query_row("SELECT id FROM agent_sessions LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    // Closing before subscription must not report the already-recorded first
    // request as unsent. A genuine terminal failure must still fail.
    for marker in ["close-terminal", "fail-terminal"] {
        fs::write(home.path().join(marker), "").unwrap();
        let output = command(&["session", "resume", &session]).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.success(),
            marker == "close-terminal",
            "{stderr}"
        );
        assert!(!stderr.contains("before submitting"), "{stderr}");
        assert_eq!(read("native-turns.json").as_array().unwrap().len(), 1);
        fs::remove_file(home.path().join(marker)).unwrap();
    }
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
    let live: i64 = database
        .query_row(
            "SELECT count(*) FROM processes WHERE kind='agent' AND completed_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(live, 0, "both launches must settle their owned providers");
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
    let store = loopflow::store::sqlite::SqliteStore::new(&home.path().join("machine/loopflow.db"))
        .unwrap();
    store
        .create_wave(&loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "fixture".into(),
            loopflow::repository::CanonicalRepo::discover(repo.path())
                .unwrap()
                .to_string(),
        ))
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
    assert_eq!(instructions.matches("<lf:work-guidance>").count(), 1);
    assert!(!instructions.contains("CURRENT_SCRATCH"));
    assert!(!instructions.contains("CURRENT_WAVE_MEMORY"));
    assert!(!instructions.contains("Saved active skill."));
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
    let invoke = |moment: &str| {
        let callback = callbacks
            .iter()
            .find(|callback| callback.ends_with(&format!(" {moment}")))
            .unwrap();
        let output = Command::new("/bin/sh")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.path())
            .env(
                "LF_CONTEXT_DELIVERY",
                fs::read_to_string(home.path().join("delivery-path")).unwrap(),
            )
            .current_dir(repo.path())
            .args(["-c", callback])
            .output()
            .unwrap();
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
    assert!(first.contains("CURRENT_SCRATCH"));
    assert!(first.contains("CURRENT_WAVE_MEMORY"));
    assert!(!first.contains("Saved active skill.\n"));
    assert!(first.contains("UNCHANGED_LARGE_FILE"));
    assert!(first.contains("source=\"scratch/large.md\" excerpt=\"start\""));
    assert!(!first.contains(&large));
    repo.create_file("scratch/small.md", "FRESH_SCRATCH");
    repo.create_file(".lf/wave/fixture/MEMORY.md", "FRESH_WAVE_MEMORY");
    let compact = invoke("compact");
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
    assert!(!thread["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains("<lf:work-guidance>"));
    assert!(thread["params"]["baseInstructions"].is_null());
    assert!(thread["params"]["config"]["model_instructions_file"].is_null());
    assert!(!thread["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains(&content));
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
        assert!(context["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("ONLY_THREAD_FIXTURE_CONTEXT"));
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
    loopflow::git::worktree_add(
        repo.path(),
        &worktree,
        "agent-trust",
        loopflow::git::WorktreeBranch::New {
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

#[test]
fn existing_task_checkout_preloads_work_without_enrolling_other_checkouts() {
    let repo = TestRepo::new();
    repo.create_branch("launcher");
    let task_checkout = repo
        .create_named_worktree("managed")
        .canonicalize()
        .unwrap();
    let home = tempfile::tempdir().unwrap();
    let machine = home.path().join("machine");
    fs::create_dir(&machine).unwrap();
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
    for (checkout, work) in [(repo.path(), false), (task_checkout.as_path(), true)] {
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
        assert_eq!(instructions.contains("<lf:work-guidance>"), work);
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
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    repo.create_file(".lf/skills/probe.md", "First input marker.");
    repo.create_file(".lf/skills/decide.md", "Do not assemble future input.");
    repo.create_file(
        ".lf/graphs/inspect.yaml",
        "- probe\n- loop: probe\n  step: decide\n",
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
        .contains("# Using lf"));
    assert!(!home.path().join("machine").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn operator_context_preview_uses_resolved_skills_without_preparing_checkouts() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
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
            repo.create_file(".gitignore", "ignored.txt\n");
            repo.create_file("config.txt", "CONFIG-DOCUMENT-662");
            repo.create_file("frontmatter.txt", "FRONTMATTER-DOCUMENT-662");
            repo.create_file(
                ".lf/config.yaml",
                "diff: false\ndiff_files: false\npaste: false\ndocs: [config.txt]\n",
            );
            if native {
                repo.create_file(
                    ".agents/skills/probe/SKILL.md",
                    "---\nname: probe\ndescription: Probe docs\n---\nRead the supplied reference.",
                );
            } else {
                repo.create_file(
                    ".lf/skills/probe.md",
                    "---\ndocs: [frontmatter.txt]\n---\nRead the supplied reference.",
                );
            }
            let selected = if selection == "relative" {
                "document.txt".to_string()
            } else {
                path.display().to_string()
            };
            let args = [
                if selection == "external" { "-i" } else { "-b" },
                "-a",
                "codex",
                "--docs",
                &selected,
                "skill",
                "probe",
                "Report final marker.",
            ];
            let preview = native_codex_command(repo.path(), home.path(), &args)
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
            assert!(preview["references"].as_array().unwrap().is_empty());
            assert!(!repo.path().join(".lf/prompts").exists());
            assert!(!home.path().join("native-argv.jsonl").exists());
            assert!(!home.path().join("runs").exists());
            assert!(!home.path().join("loopflow.db").exists());
            let output = native_codex_command(repo.path(), home.path(), &args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
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
                            repo.path().join(source).canonicalize().unwrap()
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
        "resume",
        session,
        "--message",
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
        "resume",
        session,
        "--message",
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
