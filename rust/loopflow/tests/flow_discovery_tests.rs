use std::process::Command;

use clap::Parser;

use loopflow::durable::{FlowPage, FlowSession};
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::engine::{ConcreteSkill, ConcreteStep, ExecutionCursor, OccurrencePolicy, Skill};
use loopflow::store::{open_ephemeral_store, StorageConfig};

#[test]
fn flow_inventory_wire_keeps_unknowns_and_requires_metadata() {
    let json = include_str!("../../../tests/fixtures/dto/flow_page.json");
    let page: FlowPage = serde_json::from_str(json).unwrap();
    assert_eq!(page.entries[0].summary.name, None);
    assert_eq!(page.entries[0].repo, None);
    assert!(!page.entries[0].managed);
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
    missing["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("managed");
    assert!(serde_json::from_value::<FlowPage>(missing).is_err());
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
        .env("LF_DB_PATH", home.join("db"))
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
    let store = open_ephemeral_store(&StorageConfig::sqlite(dir.path().join("db")))
        .await
        .unwrap();
    let flow = store
        .create_flow(FlowSession {
            invocation: QueuedInvocation::new(
                "no-template",
                vec![ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named("saved-skill"),
                    flow_parents: vec![],
                    policy: OccurrencePolicy::default(),
                })],
            )
            .unwrap(),
            cursor: ExecutionCursor::default(),
            version: 0,
            task_id: None,
            wave_id: None,
            cwd: dir.path().into(),
            message: None,
            model: None,
            current_attempt: None,
            pending_session_id: None,
            ready_summary: None,
            worker_generation: 0,
            claim: None,
            failure: None,
            finished: false,
            updated_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let output = command(
        dir.path(),
        &[
            "flow",
            "list",
            "--sessions",
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
    let page: FlowPage = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page.entries[0].summary.id, flow.id());
    let expected_repo = loopflow::repository::CanonicalRepo::discover(dir.path())
        .unwrap()
        .to_string();
    assert_eq!(
        page.entries[0].repo.as_deref(),
        Some(expected_repo.as_str())
    );
    let scoped = command(dir.path(), &["flow", "list", "--sessions", "--json"]);
    assert!(
        scoped.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<FlowPage>(&scoped.stdout)
            .unwrap()
            .entries[0]
            .summary
            .id,
        flow.id()
    );
    let detail = command(
        dir.path(),
        &["flow", "show", "--sessions", "--json", flow.id()],
    );
    assert!(
        detail.status.success(),
        "{}",
        String::from_utf8_lossy(&detail.stderr)
    );
    let detail: loopflow::durable::FlowDetail = serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(detail.graph.name, "no-template");
    assert_eq!(detail.current, Some(0));
    assert_eq!(store.flow(flow.id()).await.unwrap().unwrap(), flow);
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
        "--sessions",
        "--for-task",
        "PROOF-1",
        "--for-wave",
        "wave",
        "--managed",
        "false",
        "--limit",
        "2",
        "--json",
    ])
    .unwrap();
    assert!(cli.task.is_none());
    assert!(cli.wave.is_none());
    match cli.command {
        Some(loopflow::lf::Commands::Flow {
            inventory,
            json,
            args,
            ..
        }) => {
            assert_eq!(inventory.for_task.as_deref(), Some("PROOF-1"));
            assert_eq!(inventory.managed, Some(false));
            assert!(json);
            assert!(args.is_empty());
        }
        _ => panic!("expected Flow discovery"),
    }
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "flow",
        "list",
        "--sessions",
        "--limit",
        "0"
    ])
    .is_err());
    assert!(loopflow::lf::Cli::try_parse_from([
        "lf",
        "flow",
        "list",
        "--sessions",
        "--taskless",
        "--for-task",
        "PROOF-1"
    ])
    .is_err());
}
