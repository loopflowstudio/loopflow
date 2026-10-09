mod support;

use std::process::Command;

use clap::Parser;

use loopflow::durable::FlowProcessPage;
use loopflow::store::{open_ephemeral_store, StorageConfig};

#[test]
fn flow_inventory_wire_keeps_unknowns_and_requires_metadata() {
    let json = include_str!("../../../tests/fixtures/dto/flow_page.json");
    let page: FlowProcessPage = serde_json::from_str(json).unwrap();
    assert_eq!(page.entries[0].summary.name, "feature");
    assert_eq!(page.entries[0].repo, None);
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("updated_at");
    assert!(serde_json::from_value::<FlowProcessPage>(missing).is_err());
}

fn command(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let key = name.to_string_lossy();
        if key.starts_with("LF_") || key.starts_with("LOOPFLOW_") {
            cmd.env_remove(name);
        }
    }
    cmd.current_dir(home)
        .args(args)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("RUST_LOG", "off")
        .output()
        .unwrap()
}

#[tokio::test]
async fn public_flow_discovery_reads_saved_detail_without_selecting_work() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .arg(dir.path())
        .status()
        .unwrap()
        .success());
    open_ephemeral_store(&StorageConfig::sqlite(dir.path().join("loopflow.db")))
        .await
        .unwrap();
    let expected_repo = loopflow::repository::CanonicalRepo::discover(dir.path())
        .unwrap()
        .to_string();
    // A Flow whose definition no longer exists: only its Processes describe it.
    let flow = support::record_flow(
        dir.path(),
        std::path::Path::new(&expected_repo),
        "no-template",
        "saved-skill",
        "failed",
    );
    let before = support::recorded_flows(dir.path());
    let output = command(
        dir.path(),
        &[
            "flow",
            "list",
            "--processes",
            "--all",
            "--json",
            "--limit",
            "1",
            "--taskless",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let page: FlowProcessPage = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page.entries[0].summary.id, flow);
    assert_eq!(
        page.entries[0].summary.state,
        loopflow::session::FlowProcessSummaryState::Stopped
    );
    assert_eq!(
        page.entries[0].repo.as_deref(),
        Some(expected_repo.as_str())
    );
    let scoped = command(dir.path(), &["flow", "list", "--processes", "--json"]);
    assert!(
        scoped.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<FlowProcessPage>(&scoped.stdout)
            .unwrap()
            .entries[0]
            .summary
            .id,
        flow
    );
    let detail = command(
        dir.path(),
        &["flow", "show", "--processes", "--json", &flow],
    );
    assert!(
        detail.status.success(),
        "{}",
        String::from_utf8_lossy(&detail.stderr)
    );
    let detail: loopflow::durable::FlowProcessDetail =
        serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(detail.graph.name, "no-template");
    assert_eq!(detail.graph.steps[0].label, "saved-skill");
    assert_eq!(detail.current, Some(0));
    assert_eq!(detail.steps.len(), 1);
    assert_eq!(
        support::recorded_flows(dir.path()),
        before,
        "reading changes nothing"
    );
    let bad = command(dir.path(), &["flow", "list", "--limit", "1"]);
    assert!(!bad.status.success());
    let templates = command(dir.path(), &["flow", "list", "--json"]);
    assert!(templates.status.success());
    assert!(
        serde_json::from_slice::<serde_json::Value>(&templates.stdout)
            .unwrap()
            .is_array()
    );
}

#[test]
fn flow_inventory_flags_do_not_select_task_launch_context() {
    let cli = loopflow::lf::Cli::try_parse_from([
        "lf",
        "flow",
        "list",
        "--processes",
        "--for-task",
        "PROOF-1",
        "--for-wave",
        "wave",
        "--limit",
        "2",
        "--json",
    ])
    .unwrap();
    assert!(cli.task.is_none());
    assert!(cli.wave.is_none());
    match cli.command {
        Some(loopflow::lf::Commands::Flow {
            cmd: loopflow::lf::FlowCommand::List { inventory, json },
        }) => {
            assert_eq!(inventory.for_task.as_deref(), Some("PROOF-1"));
            assert!(json);
        }
        _ => panic!("expected Flow discovery"),
    }
    assert!(
        loopflow::lf::Cli::try_parse_from([
            "lf",
            "flow",
            "list",
            "--processes",
            "--managed",
            "true"
        ])
        .is_err(),
        "no Flow is selected over another"
    );
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "flow",
        "list",
        "--processes",
        "--limit",
        "0"
    ])
    .is_err());
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "flow",
        "list",
        "--processes",
        "--taskless",
        "--for-task",
        "PROOF-1"
    ])
    .is_err());
}

#[test]
fn flow_and_workflow_catalogs_keep_same_name_sources_separate() {
    let repo = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_lf"));
        for (name, _) in
            std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
        {
            cmd.env_remove(name);
        }
        cmd.current_dir(repo.path())
            .env("LF_HOME", home.path())
            .args(args)
            .output()
            .unwrap()
    };
    std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
    std::fs::write(repo.path().join(".lf/flows/feature.yaml"), "- cmd: help\n").unwrap();
    let flows = run(&["flow", "list", "--json"]);
    assert!(flows.status.success());
    let workflows = run(&["project", "workflow", "list", "--json"]);
    assert!(workflows.status.success());
    let entry = |output: &std::process::Output| {
        serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout)
            .unwrap()
            .into_iter()
            .find(|entry| entry["name"] == "feature")
            .unwrap()
    };
    assert_eq!(entry(&flows)["source"], ".lf/flows/feature.yaml");
    assert!(entry(&workflows)["workflow"].is_object());
    assert!(entry(&workflows)["source"].is_null());
    let shown = run(&["flow", "show", "feature", "--json"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shown.stdout).unwrap(),
        entry(&flows)
    );
    let customized = run(&["flow", "customize", "feature"]);
    assert!(customized.status.success());
    assert_eq!(
        String::from_utf8(customized.stdout).unwrap().trim(),
        repo.path()
            .canonicalize()
            .unwrap()
            .join(".lf/flows/feature.yaml")
            .to_str()
            .unwrap()
    );
    // Workflow edits are stored through project workflow set --file. Local
    // definition discovery still reports malformed authored candidates.
    std::fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
    std::fs::write(
        repo.path().join(".lf/workflows/feature.yaml"),
        "invalid: true\n",
    )
    .unwrap();
    let invalid = entry(&run(&["project", "workflow", "list", "--json"]));
    assert!(invalid["workflow"].is_null());
    assert!(invalid["unavailable"].is_string());
    assert_eq!(entry(&run(&["flow", "list", "--json"])), entry(&flows));
    // A directory read failure cannot claim only builtins exist.
    std::fs::remove_dir_all(repo.path().join(".lf/workflows")).unwrap();
    std::fs::write(repo.path().join(".lf/workflows"), "not a directory").unwrap();
    assert!(!run(&["project", "workflow", "list", "--json"])
        .status
        .success());
}

#[test]
fn showing_one_definition_does_not_require_catalog_enumeration() {
    let repo = tempfile::tempdir().unwrap();
    let run = |name: &str| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .current_dir(repo.path())
            .args(["flow", "show", name, "--json"])
            .output()
            .unwrap()
    };
    std::fs::create_dir(repo.path().join(".lf")).unwrap();
    std::fs::write(repo.path().join(".lf/flows"), "not a directory").unwrap();
    let shown = run("pursue");
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    let entry: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(entry["name"], "pursue");
    assert!(entry["graph"].is_object());
    assert!(!run("no-such-flow").status.success());
    assert!(
        !run("implement").status.success(),
        "skills are not Flow definitions"
    );

    std::fs::remove_file(repo.path().join(".lf/flows")).unwrap();
    std::fs::create_dir(repo.path().join(".lf/flows")).unwrap();
    std::fs::write(repo.path().join(".lf/flows/pursue.yaml"), "invalid: true\n").unwrap();
    let invalid = run("pursue");
    assert!(invalid.status.success());
    let entry: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert!(entry["graph"].is_null());
    assert!(entry["unavailable"].is_string());
}
