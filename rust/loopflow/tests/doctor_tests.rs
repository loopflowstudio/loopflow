use std::path::Path;
use std::process::{Command, Output};

fn run_lf(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(home)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("NO_COLOR", "1")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_RUN_ID")
        .output()
        .unwrap()
}

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = tempfile::tempdir().unwrap();
    let output = run_lf(home.path(), &["doctor", "--json"]);
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
