use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use rusqlite::Connection;
use serde_json::json;

use crate::installation::{
    self, ActivationTargets, ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet,
    InstallSelection, InstallSource, RecoveryOwner, SwitchPhase, SwitchReceipt,
};
use crate::store::sqlite::SqliteStore;

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn running_provider_uses_updated_install_for_commands_hooks_and_graph() {
    assert!(Path::new("/.dockerenv").is_file());
    let account = installation::account_home().unwrap();
    assert_eq!(account, Path::new("/home/lf-task-proof"));
    let root = installation::root().unwrap();
    let home = account.join(".lf");
    assert!(!root.exists());
    assert!(!home.exists());
    let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN", "LF_AGENT_CALLER"]);
    fs::create_dir(&home).unwrap();
    let database = home.join("loopflow.db");
    crate::store::migrations::apply_sqlite(&Connection::open(&database).unwrap()).unwrap();
    let store = SqliteStore::new(&database).unwrap();
    let session = store.test_session(
        "upgrade-session",
        &crate::session_record::new_artifact_key(),
    );
    let steer = store
        .save_session_steer(&session.id, "Fixture", "Keep this pending")
        .unwrap();
    let history = store.session_history(&session.id, 0, 100).unwrap();
    let files = tempfile::tempdir().unwrap();
    let binary = loopflow_test_support::lf_binary();
    let failing = files.path().join("failed-lf");
    fs::write(
        &failing,
        "#!/bin/sh\nprintf 'fixture CLI failure\\n' >&2\nexit 42\n",
    )
    .unwrap();
    fs::set_permissions(&failing, fs::Permissions::from_mode(0o755)).unwrap();
    let replacement = files.path().join("updated/lf");
    fs::create_dir(replacement.parent().unwrap()).unwrap();
    fs::copy(binary, &replacement).unwrap();
    let prior = selection(&failing, &database);
    installation::write_active(&root, &prior).unwrap();
    let gate =
        installation::install_entry_gate(&root, &ArtifactRole::Cli, Path::new(binary)).unwrap();
    std::env::set_var("LF_BIN", &failing);
    assert_eq!(super::resolve_lf_binary().unwrap(), gate);
    let repo = loopflow_test_support::TestRepo::new();
    // The step is an execution command, so it re-enters through the gate.
    repo.create_file(
        ".lf/graphs/entry-proof.yaml",
        "- cmd: graph inner --batch\n",
    );
    repo.create_file(".lf/graphs/inner.yaml", "- cmd: sync --plan\n");
    let delivery = crate::context_block::ContextDelivery {
        repository: None,
        repo: repo.path().to_path_buf(),
        home: home.clone(),
        wave: None,
        skill_file: None,
        submitted_docs: None,
        references: Vec::new(),
    };
    let delivery_path = files.path().join("delivery.json");
    fs::write(&delivery_path, serde_json::to_vec(&delivery).unwrap()).unwrap();
    let hooks = delivery.hook_settings().unwrap();
    let hook = hooks["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
    assert!(hook.contains(gate.to_str().unwrap()));
    let caller = json!({"session_id":session.id,"agent_process_id":null,"origin_exec_id":"e3904e02-0219-45fd-8c4e-c7f289c60552"}).to_string();
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .env("HOME", &account)
        .env("LF_AGENT_CALLER", &caller)
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::harness::configure_vendor_std_env(&mut command).unwrap();
    command.args(["-ec", &format!(
        "if lf session list --all --json; then exit 93; else test \"$?\" -eq 42; fi\nprintf 'retry-ready\\n'\nread ready\nlf session list --all --json\n{hook} --delivery {}\n\"$LF_BIN\" graph entry-proof --batch\n",
        super::shell_escape(&delivery_path.to_string_lossy()),
    )]);
    let mut provider = command.spawn().unwrap();
    let mut stdout = BufReader::new(provider.stdout.take().unwrap());
    let mut ready = String::new();
    stdout.read_line(&mut ready).unwrap();
    assert_eq!(ready, "retry-ready\n");
    provider.stdout = Some(stdout.into_inner());
    // Retry the failed command from this same live provider and checkout.
    let active = selection(&replacement, &database);
    switch(&root, &prior, &active, &gate);
    provider
        .stdin
        .take()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let output = provider.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("fixture CLI failure"));
    assert!(String::from_utf8_lossy(&output.stdout).contains(session.id.as_str()));

    // A direct old CLI or a gate launch must ignore a stale main-Machine pin.
    for entry in [Path::new(binary), &gate] {
        let mut command = Command::new(entry);
        command
            .env_clear()
            .env("HOME", &account)
            .env("LF_HOME", &home)
            .env("LF_BIN", "/missing/old/lf")
            .env("LF_AGENT_CALLER", &caller)
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args(["session", "list", "--all", "--json"]);
        let output = super::bounded_output(&mut command, Duration::from_secs(30)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(session.id.as_str()));
    }
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    assert_eq!(store.session_history(&session.id, 0, 100).unwrap(), history);
    let pending = store.session_steers(&session.id, 0, 100).unwrap();
    assert_eq!(pending.pending, 1);
    assert_eq!(pending.items[0].steer.id, steer.id);
    let connection = Connection::open(&database).unwrap();
    let child_command: String = connection.query_row(
        "SELECT p.command FROM graph_process_steps s JOIN processes p ON p.id=s.lf_process_id WHERE p.exit_code=0",
        [], |row| row.get(0),
    ).unwrap();
    assert!(
        child_command.contains(replacement.to_str().unwrap()),
        "{child_command}"
    );
    let steps: i64 = connection
        .query_row(
            "SELECT count(*) FROM graph_process_steps s JOIN processes p ON p.id=s.lf_process_id WHERE p.exit_code=0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(steps, 2);
    // The gate's exec keeps the Graph's handoff: the step runs inside its owner.
    let redelegated: i64 = connection
        .query_row(
            r#"SELECT count(*) FROM graph_process_steps s JOIN processes c ON c.parent_lf_process_id=s.lf_process_id WHERE c.command LIKE '%"session","serve"%'"#,
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(redelegated, 0);
    // Reads record no Process, so the provider's Graph launch carries attribution.
    let attributed: Vec<String> = connection
        .prepare("SELECT command FROM processes WHERE caller_session_id=?1 AND via_agent=1")
        .unwrap()
        .query_map([&session.id], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        attributed
            .iter()
            .any(|command| command.contains("entry-proof")),
        "{attributed:?}"
    );
    drop(connection);
    drop(store);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(home).unwrap();
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn graph_live_across_switch_runs_next_step_on_selected_install() {
    assert!(Path::new("/.dockerenv").is_file());
    let account = installation::account_home().unwrap();
    let root = installation::root().unwrap();
    let home = account.join(".lf");
    assert!(!root.exists());
    assert!(!home.exists());
    let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN", "LF_AGENT_CALLER"]);
    fs::create_dir(&home).unwrap();
    let database = home.join("loopflow.db");
    crate::store::migrations::apply_sqlite(&Connection::open(&database).unwrap()).unwrap();
    let files = tempfile::tempdir().unwrap();
    let binary = loopflow_test_support::lf_binary();
    // Step 1 holds at its first git call, a fifo, until the switch settles.
    let release = files.path().join("release");
    let held = files.path().join("held");
    assert!(Command::new("mkfifo")
        .arg(&release)
        .status()
        .unwrap()
        .success());
    let tools = files.path().join("tools");
    fs::create_dir(&tools).unwrap();
    fs::write(
        tools.join("git"),
        format!(
            "#!/bin/sh\nif [ -n \"${graph}\" ] && mkdir '{}' 2>/dev/null; then read _ < '{}'; fi\nexec /usr/bin/git \"$@\"\n",
            held.display(),
            release.display(),
            graph = crate::ops::graph_process::GRAPH_ID_ENV,
        ),
    )
    .unwrap();
    fs::set_permissions(tools.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    // Two copies of this lf; trailing bytes give the prior one its own digest.
    let prior_cli = files.path().join("prior/lf");
    fs::create_dir(prior_cli.parent().unwrap()).unwrap();
    fs::copy(binary, &prior_cli).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&prior_cli)
        .unwrap()
        .write_all(b"prior install")
        .unwrap();
    let replacement = files.path().join("updated/lf");
    fs::create_dir(replacement.parent().unwrap()).unwrap();
    fs::copy(binary, &replacement).unwrap();
    let prior = selection(&prior_cli, &database);
    installation::write_active(&root, &prior).unwrap();
    let gate =
        installation::install_entry_gate(&root, &ArtifactRole::Cli, Path::new(binary)).unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    repo.create_file(
        ".lf/graphs/two-steps.yaml",
        "- cmd: sync --plan\n- cmd: sync --plan\n",
    );
    let mut runner = Command::new(&gate);
    runner
        .env_clear()
        .env("HOME", &account)
        .env("LF_HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
        .current_dir(repo.path())
        .args(["graph", "two-steps", "--batch"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut runner = runner.spawn().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while !held.exists() {
        if runner.try_wait().unwrap().is_some() || std::time::Instant::now() > deadline {
            let ps = Command::new("ps")
                .args(["-eo", "pid,ppid,args"])
                .output()
                .unwrap();
            let recorded = Connection::open(&database)
                .unwrap()
                .prepare("SELECT command, exit_code FROM processes ORDER BY rowid")
                .unwrap()
                .query_map([], |row| {
                    Ok(format!(
                        "{} -> {:?}",
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?
                    ))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            let _ = runner.kill();
            let output = runner.wait_with_output().unwrap();
            panic!(
                "step 1 never held on the prior install: {:?}\n{}\n{recorded:#?}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&ps.stdout),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Promote while the Graph's first step is running on the prior install.
    let active = selection(&replacement, &database);
    switch(&root, &prior, &active, &gate);
    fs::write(&release, "go\n").unwrap();
    let output = runner.wait_with_output().unwrap();
    // Exited on its own: the runner was never stopped or relaunched.
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let connection = Connection::open(&database).unwrap();
    let graphs: i64 = connection
        .query_row("SELECT count(*) FROM graph_processes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(graphs, 1, "one Graph, no relaunch");
    let steps = connection
        .prepare(
            "SELECT s.node, p.command, p.exit_code FROM graph_process_steps s
             JOIN processes p ON p.id=s.lf_process_id ORDER BY s.seq",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let nodes = steps.iter().map(|(node, _, _)| *node).collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2, "each node once, no replay: {steps:?}");
    assert_ne!(nodes[0], nodes[1], "{steps:?}");
    assert!(
        steps.iter().all(|(_, _, exit)| *exit == Some(0)),
        "{steps:?}"
    );
    let (previous, updated) = (prior_cli.to_str().unwrap(), replacement.to_str().unwrap());
    assert!(steps[0].1.contains(previous), "{steps:?}");
    assert!(steps[1].1.contains(updated), "{steps:?}");
    let runner: String = connection
        .query_row(
            "SELECT p.command FROM graph_processes g JOIN processes p ON p.id=g.lf_process_id",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        runner.contains(previous),
        "the runner keeps its lf: {runner}"
    );
    drop(connection);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(home).unwrap();
}

fn selection(binary: &Path, database: &Path) -> ActiveInstall {
    let artifact = ArtifactIdentity::capture(ArtifactRole::Cli, binary).unwrap();
    let set = ArtifactSet {
        id: "entry-proof".into(),
        source: InstallSource::Published,
        source_revision: "fixture".into(),
        source_identity: "fixture".into(),
        build_label: None,
        content_sha256: installation::artifact_set_sha256(binary, None, None).unwrap(),
        artifacts: vec![artifact],
    };
    ActiveInstall {
        schema_version: 1,
        selection: InstallSelection {
            installation_id: binary.display().to_string(),
            source: InstallSource::Published,
            artifact_set: set.clone(),
            store: database.to_path_buf(),
        },
        published_fallback: set.clone(),
        retained_published_sets: vec![set],
    }
}

/// Write and settle a compatible switch from `prior` to `active`.
fn switch(root: &Path, prior: &ActiveInstall, active: &ActiveInstall, gate: &Path) {
    let cli = |install: &ActiveInstall| {
        install
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone()
    };
    let receipt = SwitchReceipt {
        schema_version: 1,
        id: "entry-switch".into(),
        prior: Some(prior.selection.clone()),
        target: active.selection.clone(),
        published_fallback: Some(prior.published_fallback.clone()),
        target_published_fallback: Some(active.published_fallback.clone()),
        phase: SwitchPhase::Settled,
        recovery_owner: RecoveryOwner::Candidate,
        target_store_advance_started: true,
        target_store_advanced: true,
        active_selection_committed: true,
        coordinator: cli(prior),
        candidate: cli(active),
        activation: ActivationTargets {
            cli: gate.to_path_buf(),
            daemon: None,
            app: None,
            legacy_app: None,
        },
        app_was_running: false,
        disposable_store_owned: false,
    };
    installation::write_switch(root, &receipt).unwrap();
    installation::settle_switch(root, &receipt, active).unwrap();
}
