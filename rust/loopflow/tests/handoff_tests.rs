use std::path::Path;
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffAttach, InteractiveHandoffStatus,
};
use loopflow::store::sqlite::SqliteStore;
use loopflow::wave::Wave;
use loopflow_test_support::TestRepo;

fn run_lf(repo: &Path, home: &Path, arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(arguments)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_PROJECT_SESSION_ID")
        .env_remove("LF_TASK_SESSION_ID")
        .output()
        .expect("run lf handoff");
    assert!(
        output.status.success(),
        "lf {:?} failed: {}",
        arguments,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn handoff_cli_opens_attaches_and_hands_the_same_session_back() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let wave = Wave::new(
        WaveId::new(),
        "product".to_string(),
        repo.path().display().to_string(),
    );
    store.create_wave(&wave).unwrap();
    drop(store);

    let parent = format!("wave:{}", wave.id());
    let cwd = repo.path().display().to_string();
    let opened = run_lf(
        repo.path(),
        home.path(),
        &[
            "handoff",
            "open",
            "--parent",
            &parent,
            "--home",
            "jack@local",
            "--cwd",
            &cwd,
            "--provider",
            "codex",
            "--generation",
            "1",
            "--reason",
            "OAuth login requires a human",
            "--env",
            "LF_HOME=/tmp/lf",
            "--json",
            "--",
            "tmux",
            "attach-session",
            "-t",
            "lf-auth-interactive",
        ],
    );
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    assert_eq!(opened["created"], true);
    let session_id = opened["handoff"]["id"].as_str().unwrap();

    let attached = run_lf(
        repo.path(),
        home.path(),
        &["handoff", "attach", session_id, "--json"],
    );
    let attached: InteractiveHandoffAttach = serde_json::from_slice(&attached.stdout).unwrap();
    assert_eq!(attached.status, InteractiveHandoffStatus::Attached);
    assert_eq!(attached.cwd, repo.path());
    assert_eq!(
        attached.argv,
        ["tmux", "attach-session", "-t", "lf-auth-interactive"]
    );

    let handed_back = run_lf(
        repo.path(),
        home.path(),
        &[
            "handoff",
            "back",
            session_id,
            "--summary",
            "finish the review fixes headlessly",
            "--json",
        ],
    );
    let handed_back: InteractiveHandoff = serde_json::from_slice(&handed_back.stdout).unwrap();
    assert_eq!(handed_back.id.as_str(), session_id);
    assert_eq!(handed_back.status, InteractiveHandoffStatus::HandedBack);
}
