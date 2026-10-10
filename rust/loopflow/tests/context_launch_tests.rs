//! Real terminal dispatch with providers replaced by argument-limited scripts.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow::engine::prompt::INITIAL_TURN_PROMPT;
use loopflow_test_support::TestRepo;

#[test]
fn terminal_context_uses_files_and_preserves_failed_sessions() {
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join("scratch")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.path().join("wave/fixture")).unwrap();
    fs::create_dir_all(repo.path().join(".claude")).unwrap();
    fs::write(
        repo.path().join(".claude/settings.json"),
        r#"{"permissions":{"defaultMode":"plan"}}"#,
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "diff: false\ndiff_files: false\npaste: false\n",
    )
    .unwrap();
    let memory =
        "Preserve infrastructure configurations, observations, implementation and OpenCode.\n"
            .repeat(1_100);
    let scratch = "Scratch says preserve all context and its source.\n".repeat(900);
    fs::write(repo.path().join("wave/fixture/MEMORY.md"), &memory).unwrap();
    fs::write(repo.path().join("wave/fixture/GOAL.md"), "A fixture goal.").unwrap();
    fs::write(repo.path().join("scratch/plan.md"), &scratch).unwrap();
    fs::write(
        repo.path().join(".lf/skills/probe.md"),
        "Read the supplied context.",
    )
    .unwrap();

    for harness in ["claude", "codex"] {
        fs::write(repo.path().join("wave/fixture/MEMORY.md"), &memory).unwrap();
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("machine/loopflow.db"))
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
        fs::write(&provider, r#"#!/bin/sh
if [ "$1" = --version ]; then echo fixture; exit 0; fi
if [ "$1" = --dangerously-bypass-hook-trust ] && [ "$2" = --model ]; then
    echo "error: a value is required for '--model <MODEL>' but none was supplied" >&2
    exit 2
fi
for arg in "$@"; do
    [ "${#arg}" -lt 122880 ] || exit 99
    printf '%s\n' "$arg" >> "$HOME/argv"
    if [ "$file_next" = yes ]; then context="$arg"; file_next=no; fi
    case "$arg" in
        --append-system-prompt-file) file_next=yes ;;
        model_instructions_file=*) context=${arg#model_instructions_file=}; context=${context#\"}; context=${context%\"} ;;
    esac
done
[ -n "$context" ] || exit 98
cp "$context" "$HOME/received-context"
printf 'provider fixture refused this request\n' >&2
exit 23
"#).unwrap();
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
            .args([
                "-i",
                "-a",
                harness,
                "--wave",
                "fixture",
                "probe",
                "Find the fixture goal.",
            ])
            .env("HOME", home.path())
            .env("LF_HOME", home.path().join("machine"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("CODEX_HOME", home.path().join(".codex"))
            .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("provider fixture refused this request"),
            "{harness}: {stderr}"
        );
        let context = fs::read_to_string(home.path().join("received-context")).unwrap();
        assert!(context.len() > 122_880, "{}", context.len());
        assert!(context.contains(&memory));
        assert!(context.contains(&scratch));
        assert!(context.contains("A fixture goal."));
        assert!(context.contains("<lf:skill:probe>"));
        assert!(context.contains("Find the fixture goal."));
        let argv = fs::read_to_string(home.path().join("argv")).unwrap();
        assert_eq!(argv.matches(INITIAL_TURN_PROMPT).count(), 1, "no retry");
        assert!(!argv.contains("--dangerously-skip-permissions"));
        assert!(
            !argv.contains("--permission-mode"),
            "native plan mode survives"
        );
        assert!(argv.len() < 4096, "{argv}");
        let db = rusqlite::Connection::open(home.path().join("machine/loopflow.db")).unwrap();
        let sessions: i64 = db
            .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sessions, 1, "failed launch keeps the Session");
        fs::write(
            repo.path().join("wave/fixture/MEMORY.md"),
            "New decision from a direct file edit.",
        )
        .unwrap();
        let next = command.output().unwrap();
        assert!(!next.status.success());
        let next_context = fs::read_to_string(home.path().join("received-context")).unwrap();
        assert!(next_context.contains("New decision from a direct file edit."));
        assert!(!next_context.contains(&memory));
    }
}

#[test]
fn headless_codex_reads_context_file_and_keeps_it_for_native_resume() {
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
context=$(printf '%s' "$thread_start" | sed -n 's/.*"model_instructions_file":"\([^"]*\)".*/\1/p')
cp "$context" "$HOME/received-context" || exit 98
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
    assert!(fs::read_to_string(home.path().join("received-context"))
        .unwrap()
        .contains(&content));
    let thread: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("thread-request")).unwrap()).unwrap();
    let path = thread["params"]["config"]["model_instructions_file"]
        .as_str()
        .unwrap();
    assert!(fs::read_to_string(path).unwrap().contains(&content));
    let turn: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("turn-request")).unwrap()).unwrap();
    assert_eq!(turn["params"]["input"][0]["text"], INITIAL_TURN_PROMPT);
}
