mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use base64::Engine;
use loopflow::engine::flow::{ConcreteStep, Skill, Step};
use loopflow::engine::{compile_flow, load_flow};
use support::codex_app_server_script;
use tempfile::TempDir;

fn session_id_for_capture(home: &Path, artifact: &str) -> String {
    rusqlite::Connection::open_with_flags(
        home.join("loopflow.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .query_row(
        "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
        [artifact],
        |row| row.get(0),
    )
    .unwrap()
}

fn write_skill(repo: &Path, name: &str, content: &str) {
    let skills_dir = repo.join(".lf/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_flow(repo: &Path, name: &str, content: &str) {
    let flows_dir = repo.join(".lf/flows");
    fs::create_dir_all(&flows_dir).unwrap();
    fs::write(flows_dir.join(format!("{name}.yaml")), content).unwrap();
}

fn lf_json(repo: &Path, home: &Path, args: &[&str]) -> serde_json::Value {
    let output = run_lf(repo, home, args, None);
    assert!(
        output.status.success(),
        "lf {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

