use crate::support;

use std::process::Command;

use clap::Parser;

use loopflow::durable::GraphProcessPage;
use loopflow::store::{open_ephemeral_store, StorageConfig};

#[test]
fn graph_inventory_wire_keeps_unknowns_and_requires_metadata() {
    let json = include_str!("../../../tests/fixtures/dto/graph_page.json");
    let page: GraphProcessPage = serde_json::from_str(json).unwrap();
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
    assert!(serde_json::from_value::<GraphProcessPage>(missing).is_err());
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
async fn public_graph_discovery_reads_saved_detail_without_selecting_work() {
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
    // A Graph whose definition no longer exists: only its Processes describe it.
    let graph = support::record_graph(
        dir.path(),
        std::path::Path::new(&expected_repo),
        "no-template",
        "saved-skill",
        "failed",
    );
    let before = support::recorded_graphs(dir.path());
    let output = command(
        dir.path(),
        &[
            "graph",
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
    let page: GraphProcessPage = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page.entries[0].summary.id, graph);
    assert_eq!(
        page.entries[0].summary.state,
        loopflow::session::GraphProcessSummaryState::Stopped
    );
    assert_eq!(
        page.entries[0].repo.as_deref(),
        Some(expected_repo.as_str())
    );
    let scoped = command(dir.path(), &["graph", "list", "--processes", "--json"]);
    assert!(
        scoped.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<GraphProcessPage>(&scoped.stdout)
            .unwrap()
            .entries[0]
            .summary
            .id,
        graph
    );
    let detail = command(
        dir.path(),
        &["graph", "show", "--processes", "--json", &graph],
    );
    assert!(
        detail.status.success(),
        "{}",
        String::from_utf8_lossy(&detail.stderr)
    );
    let detail: loopflow::durable::GraphProcessDetail =
        serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(detail.compiled.name, "no-template");
    assert_eq!(detail.compiled.steps[0].label, "saved-skill");
    assert_eq!(detail.current, Some(0));
    assert_eq!(detail.steps.len(), 1);
    assert_eq!(
        support::recorded_graphs(dir.path()),
        before,
        "reading changes nothing"
    );
    let bad = command(dir.path(), &["graph", "list", "--limit", "1"]);
    assert!(!bad.status.success());
    let templates = command(dir.path(), &["graph", "list", "--json"]);
    assert!(templates.status.success());
    assert!(
        serde_json::from_slice::<serde_json::Value>(&templates.stdout)
            .unwrap()
            .is_array()
    );
}

#[test]
fn graph_inventory_flags_do_not_select_task_launch_context() {
    let cli = loopflow::lf::Cli::try_parse_from([
        "lf",
        "graph",
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
        Some(loopflow::lf::Commands::Graph {
            cmd: loopflow::lf::GraphCommand::List { inventory, json },
        }) => {
            assert_eq!(inventory.for_task.as_deref(), Some("PROOF-1"));
            assert!(json);
        }
        _ => panic!("expected Graph discovery"),
    }
    assert!(
        loopflow::lf::Cli::try_parse_from([
            "lf",
            "graph",
            "list",
            "--processes",
            "--managed",
            "true"
        ])
        .is_err(),
        "no Graph is selected over another"
    );
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "graph",
        "list",
        "--processes",
        "--limit",
        "0"
    ])
    .is_err());
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "graph",
        "list",
        "--processes",
        "--taskless",
        "--for-task",
        "PROOF-1"
    ])
    .is_err());
}

#[test]
fn graph_catalog_and_wave_selection_read_the_same_graphs() {
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
    std::fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    std::fs::write(repo.path().join(".lf/graphs/feature.yaml"), "- cmd: help\n").unwrap();
    let graphs = run(&["graph", "list", "--json"]);
    assert!(graphs.status.success());
    let entry = |output: &std::process::Output| {
        serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout)
            .unwrap()
            .into_iter()
            .find(|entry| entry["name"] == "feature")
            .unwrap()
    };
    assert_eq!(entry(&graphs)["source"], ".lf/graphs/feature.yaml");
    let shown = run(&["graph", "show", "feature", "--json"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shown.stdout).unwrap(),
        entry(&graphs)
    );
    let customized = run(&["graph", "customize", "feature"]);
    assert!(customized.status.success());
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .ensure_wave_project(
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            "proof",
        )
        .unwrap();
    // A Wave selects a graph by name; its definition is the committed graph.
    let definition = repo.path().join(".lf/graphs/feature.yaml");
    let selected = run(&["--wave", "proof", "set", "graph", "feature"]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    let shown = run(&["wave", "graph", "show", "proof", "--json"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shown.stdout).unwrap()["default"]["name"],
        "feature"
    );
    std::fs::write(&definition, "invalid: true\n").unwrap();
    assert!(!run(&["--wave", "proof", "set", "graph", "feature"])
        .status
        .success());
    assert!(entry(&run(&["graph", "list", "--json"]))["unavailable"].is_string());
}

#[test]
fn showing_one_definition_does_not_require_catalog_enumeration() {
    let repo = tempfile::tempdir().unwrap();
    let run = |name: &str| command(repo.path(), &["graph", "show", name, "--json"]);
    std::fs::create_dir(repo.path().join(".lf")).unwrap();
    std::fs::write(repo.path().join(".lf/graphs"), "not a directory").unwrap();
    let shown = run("pursue");
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    let entry: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(entry["name"], "pursue");
    assert!(entry["compiled"].is_object());
    assert!(!run("no-such-graph").status.success());
    assert!(
        !run("implement").status.success(),
        "skills are not Graph definitions"
    );

    std::fs::remove_file(repo.path().join(".lf/graphs")).unwrap();
    std::fs::create_dir(repo.path().join(".lf/graphs")).unwrap();
    std::fs::write(
        repo.path().join(".lf/graphs/pursue.yaml"),
        "invalid: true\n",
    )
    .unwrap();
    let invalid = run("pursue");
    assert!(invalid.status.success());
    let entry: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert!(entry["compiled"].is_null());
    assert!(entry["unavailable"].is_string());
}
