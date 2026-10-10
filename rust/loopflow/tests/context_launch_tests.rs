//! Real terminal dispatch with providers replaced by argument-limited scripts.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow_test_support::TestRepo;

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
    for harness in ["claude", "codex"] {
        let home = tempfile::tempdir().unwrap();
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
            .args(["-i", "-a", harness, "probe", "Find the bug."])
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
        let instructions = if harness == "claude" {
            let index = args
                .iter()
                .position(|arg| *arg == "--append-system-prompt-file")
                .unwrap();
            fs::read_to_string(args[index + 1]).unwrap()
        } else {
            let value = args
                .iter()
                .find_map(|arg| arg.strip_prefix("developer_instructions="))
                .unwrap();
            serde_json::from_str::<String>(value).unwrap()
        };
        assert!(!instructions.contains("CURRENT_SCRATCH"));
        assert!(!instructions.contains("Saved active skill."));
        assert!(!raw.contains("model_instructions_file"));
        assert!(!raw.contains("dangerously-bypass-hook-trust"));
        assert_eq!(raw.matches("<lf:skill:probe>\nSaved active skill.\n</lf:skill:probe>\n\n<lf:message>\nFind the bug.\n</lf:message>").count(), 1);
        let hooks = if harness == "claude" {
            let index = args.iter().position(|arg| *arg == "--settings").unwrap();
            serde_json::from_slice::<serde_json::Value>(&fs::read(args[index + 1]).unwrap())
                .unwrap()["hooks"]
                .clone()
        } else {
            // The same declarations are saved in the capture for native resume.
            // Read the explicit --delivery files from the actual terminal flags.
            let flags = args.iter().find(|arg| arg.starts_with("hooks=")).unwrap();
            assert!(flags.contains("trusted_hash"));
            let delivery = fs::read_dir(repo.path().join(".lf/prompts"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.to_string_lossy().ends_with("-context-delivery.md"))
                .max_by_key(|path| fs::metadata(path).unwrap().modified().unwrap())
                .unwrap();
            let callback = |moment| {
                format!(
                    "env LF_HOME={} {} __context-block --delivery {} --moment {moment}",
                    home.path().join("machine").display(),
                    env!("CARGO_BIN_EXE_lf"),
                    delivery.display()
                )
            };
            // Assert the launch installed exactly these callback arguments, then
            // exercise the callback just as the native provider would.
            assert!(flags.contains(&delivery.display().to_string()));
            serde_json::json!({"SessionStart":[{"hooks":[{"command":callback("start")}]},{"hooks":[{"command":callback("compact")}]}]})
        };
        let invoke = |index: usize| {
            let output = Command::new("/bin/sh")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", home.path())
                .current_dir(repo.path())
                .args([
                    "-c",
                    hooks["SessionStart"][index]["hooks"][0]["command"]
                        .as_str()
                        .unwrap(),
                ])
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
        let first = invoke(0);
        assert!(first.contains("CURRENT_SCRATCH"));
        assert!(!first.contains("Saved active skill.\n"));
        assert!(first.contains("UNCHANGED_LARGE_FILE"));
        assert!(first.contains("source=\"scratch/large.md\" excerpt=\"start\""));
        assert!(!first.contains(&large));
        repo.create_file("scratch/small.md", "FRESH_SCRATCH");
        let compact = invoke(1);
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
    let turn: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("turn-request")).unwrap()).unwrap();
    assert!(turn["params"]["input"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&content));
}

#[test]
fn terminal_rejects_only_the_oversized_first_turn_before_provider_spawn() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/skills/large.md", &"x".repeat(122_880));
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\n",
    );
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        "#!/bin/sh\ntouch \"$HOME/provider-started\"\nexit 91\n",
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
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("first turn is"), "{stderr}");
    assert!(
        stderr.contains("terminal argument cap is 122880 bytes"),
        "{stderr}"
    );
    assert!(!home.path().join("provider-started").exists());
}
