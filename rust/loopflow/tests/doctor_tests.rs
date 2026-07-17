use std::process::Command;

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = tempfile::tempdir().unwrap();
    let empty_path = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["doctor", "--json"])
        .current_dir(home.path())
        .env("LF_HOME", home.path())
        .env("LF_DB_PATH", home.path().join("loopflow.db"))
        .env("PATH", empty_path.path())
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_PROJECT_SESSION_ID")
        .env_remove("LF_TASK_SESSION_ID")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "lf doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["store"]["build_source_revision"],
        loopflow::build_info::source_revision()
    );
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "binary-freshness"));
}
