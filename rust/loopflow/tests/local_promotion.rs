use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loopflow::lf::commands::install::{
    build_preview, CandidateCompatibility, CaptureCompatibility, CaptureFailureKind, Verdict,
};

fn initialize_store(path: &std::path::Path) {
    let connection = rusqlite::Connection::open(path).unwrap();
    loopflow::store::migrations::apply_sqlite(&connection).unwrap();
}

fn account_home_interposer(directory: &Path) -> PathBuf {
    let source = directory.join("account_home.c");
    let library = if cfg!(target_os = "macos") {
        directory.join("libaccount_home.dylib")
    } else {
        directory.join("libaccount_home.so")
    };
    let function_name = if cfg!(target_os = "macos") {
        "lf_test_getpwuid_r"
    } else {
        "getpwuid_r"
    };
    let interpose = if cfg!(target_os = "macos") {
        r#"__attribute__((used)) static struct {
    const void *replacement;
    const void *replacee;
} lf_test_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)(unsigned long)&lf_test_getpwuid_r,
    (const void *)(unsigned long)&getpwuid_r
};"#
    } else {
        ""
    };
    fs::write(
        &source,
        format!(
            r#"#include <errno.h>
#include <pwd.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

int {function_name}(uid_t uid, struct passwd *pwd, char *buf, size_t buflen,
                    struct passwd **result) {{
    const char *home = getenv("LF_TEST_ACCOUNT_HOME");
    const char *name = "loopflow-test";
    size_t home_len = strlen(home) + 1;
    size_t name_len = strlen(name) + 1;
    if (buflen < home_len + name_len) return ERANGE;
    memset(pwd, 0, sizeof(*pwd));
    memcpy(buf, home, home_len);
    memcpy(buf + home_len, name, name_len);
    pwd->pw_dir = buf;
    pwd->pw_name = buf + home_len;
    pwd->pw_uid = uid;
    *result = pwd;
    return 0;
}}

{interpose}
"#
        ),
    )
    .unwrap();
    let mut compiler = Command::new("cc");
    if cfg!(target_os = "macos") {
        compiler.args(["-dynamiclib", "-o"]);
    } else {
        compiler.args(["-shared", "-fPIC", "-o"]);
    }
    let status = compiler.arg(&library).arg(&source).status().unwrap();
    assert!(
        status.success(),
        "failed to compile account-home interposer"
    );
    library
}

fn isolated_command(binary: &Path, account_home: &Path, interposer: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .env("LF_TEST_ACCOUNT_HOME", account_home)
        .env_remove("LF_HOME")
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID");
    if cfg!(target_os = "macos") {
        command
            .env("DYLD_INSERT_LIBRARIES", interposer)
            .env("DYLD_FORCE_FLAT_NAMESPACE", "1");
    } else {
        command.env("LD_PRELOAD", interposer);
    }
    command
}

#[test]
fn promotion_preview_is_schema_evidence_not_run_control() {
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("loopflow.db");
    initialize_store(&store);

    let preview = build_preview(&store);
    let json = serde_json::to_value(preview).unwrap();
    assert!(json.get("compatibility").is_some());
    assert!(json.get("candidate_compatibility").is_some());
    assert!(json.get("active_runs").is_none());
}

#[test]
fn candidate_preflight_bypasses_unreadable_machine_switch_state() {
    let directory = tempfile::tempdir().unwrap();
    let account_home = directory.path().join("account");
    let machine_root = account_home.join(".lf-machine/install");
    fs::create_dir_all(&machine_root).unwrap();
    fs::write(machine_root.join("switch.json"), "{}").unwrap();
    let database = account_home.join(".lf/loopflow.db");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    initialize_store(&database);
    let interposer = account_home_interposer(directory.path());

    let output = isolated_command(
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &account_home,
        &interposer,
    )
    .args(["install", "preflight", "--json"])
    .output()
    .unwrap();

    assert!(
        !output.status.success(),
        "the validation-only candidate must still refuse promotion"
    );
    let preview: loopflow::lf::commands::install::PromotionPreview =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "preflight did not reach the read-only candidate gate: {error}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    assert_eq!(
        preview.candidate_compatibility,
        CandidateCompatibility::Checked {
            executable: loopflow::lf::commands::install::ExecutableCompatibility::Compatible {
                references: 0,
            },
            captures: CaptureCompatibility::Compatible {
                complete_captures: 0,
                partial_captures: 0,
            },
        }
    );
}

fn insert_capture(
    connection: &rusqlite::Connection,
    id: &str,
    status: &str,
    conversation_path: &str,
    event_count: i64,
    bytes: i64,
) {
    let incomplete_reason = (status == "partial").then_some("provider stream ended early");
    let outcome = if status == "complete" {
        "completed"
    } else {
        "failed"
    };
    connection
        .execute(
            "INSERT INTO agent_invocations (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 provider, surface, capture_status, incomplete_reason, outcome,
                 artifact_dir, conversation_path, conversation_event_count,
                 conversation_bytes
             ) VALUES (?1, ?2, ?3, 1, 2, '/repo', '/repo', 'codex', 'headless',
                       ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                format!("trace-{id}"),
                format!("process-{id}"),
                status,
                incomplete_reason,
                outcome,
                id,
                conversation_path,
                event_count,
                bytes,
            ],
        )
        .unwrap();
}

#[test]
fn candidate_audits_persisted_capture_schemas_on_the_migrated_home_copy() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("loopflow.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    loopflow::store::migrations::apply_sqlite(&connection).unwrap();
    let trace_root = directory.path().join("traces");

    let historical = include_str!("fixtures/trace/historical_usage_variants.jsonl");
    let unsupported = include_str!("fixtures/trace/unsupported_schema.jsonl");
    let truncated = include_str!("fixtures/trace/truncated_tail.jsonl").trim_end();
    let corrupt = include_str!("fixtures/trace/corrupt_event.jsonl");
    for (id, content, event_count) in [
        ("historical", historical, 6),
        ("unsupported", unsupported, 1),
        ("truncated", truncated, 1),
        ("corrupt", corrupt, 1),
    ] {
        let relative = format!("{id}/conversation.jsonl");
        let path = trace_root.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        insert_capture(
            &connection,
            id,
            "complete",
            &relative,
            event_count,
            content.len() as i64,
        );
    }
    insert_capture(
        &connection,
        "partial",
        "partial",
        "partial/conversation.jsonl",
        0,
        0,
    );
    drop(connection);

    let preview = build_preview(&database);

    let CandidateCompatibility::Checked {
        captures:
            CaptureCompatibility::Incompatible {
                complete_captures,
                partial_captures,
                failures,
            },
        ..
    } = &preview.candidate_compatibility
    else {
        panic!(
            "unsupported, truncated, and corrupt captures must fail closed: {:?}",
            preview.candidate_compatibility
        );
    };
    assert_eq!(*complete_captures, 4);
    assert_eq!(*partial_captures, 1);
    assert_eq!(failures.len(), 3);
    assert!(!failures
        .iter()
        .any(|failure| matches!(failure.invocation_id.as_str(), "historical" | "partial")));
    assert!(failures.iter().any(|failure| {
        failure.invocation_id == "unsupported"
            && failure.kind == CaptureFailureKind::UnsupportedSchema
    }));
    assert!(failures.iter().any(|failure| {
        failure.invocation_id == "truncated" && failure.kind == CaptureFailureKind::Truncated
    }));
    assert!(failures.iter().any(|failure| {
        failure.invocation_id == "corrupt" && failure.kind == CaptureFailureKind::Corrupt
    }));
    let Verdict::Reject { reasons } = preview.verdict else {
        panic!("persisted capture incompatibility must refuse promotion");
    };
    assert!(reasons
        .iter()
        .any(|reason| reason.contains("complete persisted capture")));
}

#[test]
fn promotion_preview_does_not_mutate_the_selected_store() {
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("loopflow.db");
    initialize_store(&store);
    let connection = rusqlite::Connection::open(&store).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE promotion_probe (value TEXT NOT NULL);
             INSERT INTO promotion_probe VALUES ('unchanged');",
        )
        .unwrap();
    drop(connection);

    let _ = build_preview(&store);

    let connection = rusqlite::Connection::open(&store).unwrap();
    let value: String = connection
        .query_row("SELECT value FROM promotion_probe", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "unchanged");
}
