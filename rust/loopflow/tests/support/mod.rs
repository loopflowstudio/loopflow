#[allow(unused_imports)] // Each test binary uses a different subset of these fixtures.
pub use loopflow_execution::test_support::*;

#[ctor::ctor(unsafe)]
fn initialize_application_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
    // An integration binary runs the `lf` Cargo built beside it. Building it
    // again from inside the test would link another feature set over that
    // path while this binary's Machine server still spawns from it.
    if let (Some(built), None) = (
        option_env!("CARGO_BIN_EXE_lf"),
        std::env::var_os("LOOPFLOW_TEST_LF"),
    ) {
        std::env::set_var("LOOPFLOW_TEST_LF", built);
    }
}

#[allow(dead_code)]
pub const CALLER: &str = "run_00000000000000000000000000000002";

/// A prepared conversation, before a provider has started.
#[allow(dead_code)]
pub fn prepare_conversation(
    home: &std::path::Path,
    cwd: &std::path::Path,
    harness: &str,
    title: &str,
) -> (String, String, std::path::PathBuf) {
    let store = loopflow_execution::store::open_sqlite(&home.join("loopflow.db")).unwrap();
    let input = format!("run_{}", uuid::Uuid::new_v4().simple());
    let session = loopflow_execution::store::sessions::create_session(
        &store,
        loopflow_execution::session::LfSession {
            captured: None,
            id: format!("session_{input}").into(),
            artifact_key: Some(input.clone()),
            caller_artifact_key: Some(CALLER.into()),
            input_published: false,
            cwd: cwd.into(),
            skill: None,
            provider: Some(harness.into()),
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            interactive: true,
            repo: None,
            title: title.into(),
            title_source: loopflow_execution::session::TitleSource::Generated,
            request: Some(title.into()),
            completed_at: None,
            created_at: 1,
        },
        None,
    )
    .unwrap();
    let dir = home.join("runs").join(&input[4..6]).join(&input);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "artifact_key": input, "caller_artifact_key": CALLER,
        "created_at": "2026-01-01T00:00:00Z", "harness": harness, "model": null,
        "surface": "tui", "cwd": cwd, "repo": null, "worktree": null,
        "skill": null, "subjects": [], "flow": {"kind":"independent"}, "exec": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "test", "boot_id": null
    });
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("prepared"), "").unwrap();
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET input_published=1 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
    (session.id.to_string(), input, dir)
}

#[allow(dead_code)]
pub fn record_native(home: &std::path::Path, id: &str, input: &str, native: &str) {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
        [id, native],
    )
    .unwrap();
    // The input resumes the native conversation its captured row names.
    db.execute(
        "UPDATE session_events SET provider_thread=?3,provider_account=NULL
         WHERE session_id=?1 AND kind='captured' AND receipt_key=?2",
        [id, input, native],
    )
    .unwrap();
}

/// A stopped Claude conversation with a saved thread, and a stand-in Claude
/// on `bin` that appends each launch's first input to `first-input`.
#[allow(dead_code)]
pub fn stopped_claude_conversation(
    home: &std::path::Path,
    cwd: &std::path::Path,
) -> (String, String) {
    use std::{fs, os::unix::fs::PermissionsExt};
    let (id, input, _) = prepare_conversation(home, cwd, "claude", "Remember the word mango");
    let native = uuid::Uuid::new_v4().to_string();
    record_native(home, &id, &input, &native);
    let transcripts = home.join(".claude/projects/fixture");
    fs::create_dir_all(&transcripts).unwrap();
    fs::write(
        transcripts.join(format!("{native}.jsonl")),
        r#"{"type":"user","userType":"external","message":{"content":"remember the word mango"}}"#,
    )
    .unwrap();
    fs::write(
        home.join(".claude.json"),
        serde_json::json!({"projects": {"/": {"hasTrustDialogAccepted": true}}}).to_string(),
    )
    .unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
if [ "$1 $2" = "auth status" ]; then echo '{"loggedIn":true}'; exit 0; fi
previous=
for arg in "$@"; do
    if [ "$previous" = --resume ]; then echo "$arg" > "$LF_HOME/resumed-thread"; fi
    previous="$arg"
done
{ cat; printf '\n--launch--\n'; } >> "$LF_HOME/first-input"
"#,
    )
    .unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    (id, native)
}

/// What the stand-in has been given once it has launched `launches` times.
#[allow(dead_code)]
pub fn first_inputs(home: &std::path::Path, launches: usize) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let text = std::fs::read_to_string(home.join("first-input")).unwrap_or_default();
        if text.matches("--launch--").count() >= launches {
            return text;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{launches} launches; saw {text:?}\n{}",
            std::fs::read_to_string(home.join("logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
