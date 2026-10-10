// Linux honors the fixture CA through SSL_CERT_FILE without changing system trust.
#![cfg(unix)]

mod support;

use loopflow::store::{CredentialType, ProviderToken};
use loopflow_test_support::TestRepo;
use std::path::Path;
use std::process::Command;
use support::{register_task_without_pr, EnvGuard};

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_associated_creation_readback_settles_only_the_exact_origin() {
    associated_creation_fixture(false);
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_associated_creation_readback_preserves_private_origins_and_independent_work() {
    associated_creation_fixture(true);
}

fn associated_creation_fixture(private: bool) {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let key = home.path().join("provider.key");
    std::fs::write(&key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    let wave = loopflow::work::wave::Wave::new(
        loopflow::id::WaveId::new(),
        "task-pr-tests".into(),
        repo.path().to_str().unwrap().into(),
    );
    let private_wave = loopflow::work::wave::Wave::new(
        loopflow::id::WaveId::new(),
        "private-origins".into(),
        repo.path().to_str().unwrap().into(),
    );
    let independent_wave = loopflow::work::wave::Wave::new(
        loopflow::id::WaveId::new(),
        "independent".into(),
        repo.path().to_str().unwrap().into(),
    );
    let tasks = [
        loopflow::durable::TaskId::new(),
        loopflow::durable::TaskId::new(),
    ];
    let projects = [
        loopflow::durable::ProjectId::new(),
        loopflow::durable::ProjectId::new(),
    ];
    let provider_id = |id: &str| {
        uuid::Uuid::parse_str(id.split_once('_').unwrap().1)
            .unwrap()
            .to_string()
    };
    let issue = provider_id(tasks[0].as_str());
    let project = provider_id(projects[0].as_str());
    let peer_home = home.path().join("peer-home");
    std::fs::create_dir(&peer_home).unwrap();
    for (side, directory) in [home.path(), peer_home.as_path()].into_iter().enumerate() {
        let store = runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(directory.join("loopflow.db")),
            ))
            .unwrap();
        let wave = if private && side == 1 {
            &private_wave
        } else {
            &wave
        };
        runtime.block_on(store.create_wave(wave)).unwrap();
        if private && side == 0 {
            runtime
                .block_on(store.create_wave(&independent_wave))
                .unwrap();
        }
        seed_linear_token(&runtime, &store, &key);
        let db = rusqlite::Connection::open(directory.join("loopflow.db")).unwrap();
        // Historical divergent Work: both mappings name the same provider object,
        // but each retained attempted creation has its own UUID and input.
        db.execute(r#"INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,project_prompt_context,workflow,external_project_id,planning_initiatives,planning_teams)
            VALUES(?1,?2,1,1,'task-pr-tests','Task PR tests','workflow: feature','feature',?3,'["initiative-task-pr-tests"]','["team-task-pr-tests"]')"#,
            rusqlite::params![projects[side].as_str(),wave.id(),project]).unwrap();
        db.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug,external_issue_id,planning_team_id,planning_state)
            VALUES(?1,?2,'INF-123','Prove Task PR transitions','Exercise the persisted lifecycle.',1,1,'',?3,'team-task-pr-tests','unstarted')",
            rusqlite::params![tasks[side].as_str(),projects[side].as_str(),issue]).unwrap();
        let reader =
            loopflow::store::sqlite::SqliteStore::new(&directory.join("loopflow.db")).unwrap();
        reader
            .update_project_content(
                &projects[side],
                &loopflow::pm::ProjectContent {
                    workflow: "feature".into(),
                    krs: vec![],
                    metric_targets: vec![],
                },
            )
            .unwrap();
        let record = reader.planning_task(&tasks[side]).unwrap().record.unwrap();
        let mut task_model = record.item;
        task_model.id = tasks[side].to_string();
        let mut project_model = record.project.unwrap();
        project_model.id = projects[side].to_string();
        let content = loopflow::pm::render_project_content(&loopflow::pm::ProjectContent {
            workflow: project_model.workflow.clone(),
            krs: project_model.krs.clone(),
            metric_targets: project_model.metric_targets.clone(),
        });
        for (kind, origin, parent, model, input) in [
            (
                "project",
                projects[side].as_str(),
                wave.id().as_str(),
                serde_json::to_value(&project_model).unwrap(),
                serde_json::json!({"id":provider_id(projects[side].as_str()),"teamIds":["team-task-pr-tests"],"name":project_model.name,"description":project_model.summary,"content":content,"useDefaultTemplate":false}),
            ),
            (
                "task",
                tasks[side].as_str(),
                projects[side].as_str(),
                serde_json::to_value(&task_model).unwrap(),
                serde_json::json!({"id":provider_id(tasks[side].as_str()),"teamId":"team-task-pr-tests","projectId":project,"title":task_model.name,"description":task_model.description,"assigneeId":null,"dueDate":null}),
            ),
        ] {
            let captured = db.prepare("SELECT 'peer:'||id||':'||CASE field WHEN 'issue_title' THEN 'name' WHEN 'project_name' THEN 'name' WHEN 'project_summary' THEN 'summary' ELSE field END FROM planning_peer_heads WHERE kind=?1 AND object_id=?2 AND field IN ('issue_title','project_name','project_summary','workflow','krs','metric_targets','status') ORDER BY id")
                .unwrap().query_map(rusqlite::params![kind,origin], |row| row.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
            let export = serde_json::json!({"id":provider_id(origin),"model":model,"parent":parent,"captured":captured,"input":input,"initiative":"initiative-task-pr-tests","link_id":uuid::Uuid::new_v4().to_string()});
            db.execute("INSERT INTO planning_creations(kind,origin_id,task_id,project_id,export_json,export_attempted,export_link_attempted,export_error)
                VALUES(?1,?2,CASE WHEN ?1='task' THEN ?2 END,CASE WHEN ?1='project' THEN ?2 END,?3,1,?1='project','lost historical response')",
                rusqlite::params![kind,origin,export.to_string()]).unwrap();
        }
        db.execute("UPDATE tasks SET worktree=?2,workspace_slug='retained',branch='main',base_commit=?3 WHERE id=?1",
            rusqlite::params![tasks[side].as_str(),repo.path().to_str().unwrap(),repo.head_sha()]).unwrap();
        db.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
            SELECT ?1,id,1,workspace_slug,branch,base_commit,1,1 FROM tasks WHERE id=?2",
            rusqlite::params![loopflow::work::task::TaskPrId::new().as_str(),tasks[side].as_str()]).unwrap();
        db.execute(
            "INSERT INTO work_placements(task_id,machine_id,placed_at) VALUES(?1,?2,1)",
            rusqlite::params![
                tasks[side].as_str(),
                reader.local_machine().unwrap().id.as_str()
            ],
        )
        .unwrap();
        retain_execution(directory, repo.path(), &tasks[side], wave.id());
    }
    let fixture = serde_json::json!({
        "lf":env!("CARGO_BIN_EXE_lf"),"repo":repo.path(),"home":home.path(),
        "issue":issue,"project":project,"task":tasks[0],"local_project":projects[0],"wave":wave.id(),
        "remote":repo.bare_path(),"private":private,"independent_wave":independent_wave.id(),
        "peer":{"home":peer_home,"task":tasks[1],"project":projects[1],"wave":if private { private_wave.id() } else { wave.id() }},
    });
    run_reconnect_fixture(home.path(), &fixture, "associated-creations");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_git_linear_creation_origins_recover_without_duplicate_effects() {
    planning_reconnect_fixture("creation-origins");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_git_linear_association_keeps_private_origins_held() {
    planning_reconnect_fixture("association-private");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_git_linear_association_round_trip() {
    planning_reconnect_fixture("associations");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_watch_exports_peer_born_plans_and_recovers_mapped_receipts() {
    planning_reconnect_fixture("exports");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_delivery_defers_rejected_peer_effects_while_acquisition_continues() {
    planning_reconnect_fixture("effects");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn work_watch_reconnects_repository_planning() {
    planning_reconnect_fixture("watch");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_flow_reconnects_planning_without_another_turn() {
    planning_reconnect_fixture("flow");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "fixture TLS requires Linux SSL_CERT_FILE"
)]
fn public_git_linear_removal_preserves_partial_order_and_later_saves() {
    planning_reconnect_fixture("ordering");
}

fn planning_reconnect_fixture(mode: &str) {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let key = home.path().join("provider.key");
    std::fs::write(&key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    seed_linear_token(&runtime, &registered.store, &key);
    let project = runtime
        .block_on(registered.store.get_project(&registered.task.project_id))
        .unwrap()
        .unwrap();
    let mut fixture = serde_json::json!({
        "lf": env!("CARGO_BIN_EXE_lf"), "repo": repo.path(), "home": home.path(),
        "issue": registered.task.plan.linear_id.as_ref().unwrap().as_str(), "task": registered.task.id.as_str(),
        "project": project.plan.linear_id.as_ref().unwrap().as_str(), "wave": registered.task.wave_id.as_str(),
    });
    if mode == "ordering" {
        let peer_home = home.path().join("peer-home");
        std::fs::create_dir(&peer_home).unwrap();
        let peer = runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(peer_home.join("loopflow.db")),
            ))
            .unwrap();
        seed_linear_token(&runtime, &peer, &key);
        fixture["peer"] = serde_json::json!({"home":peer_home});
        fixture["remote"] = serde_json::json!(repo.bare_path());
        fixture["local_project"] = serde_json::json!(registered.task.project_id.as_str());
        retain_execution(
            home.path(),
            repo.path(),
            &registered.task.id,
            &registered.task.wave_id,
        );
    }
    let creation_repo = (mode == "creation-origins").then(TestRepo::new);
    if let Some(creation_repo) = &creation_repo {
        fixture["peer"] = prepare_creation_peer(creation_repo, home.path(), &key);
        fixture["remote"] = serde_json::json!(repo.bare_path());
    }
    if mode == "exports" {
        fixture["peer"] = import_unprepared_plans(repo.path(), home.path());
    }
    if mode == "effects" {
        fixture["effects"] =
            import_rejected_deletion(repo.path(), home.path(), &registered.task.id);
    }
    if mode == "associations" || mode == "association-private" {
        let peer_home = home.path().join("peer-home");
        std::fs::create_dir(&peer_home).unwrap();
        let peer = runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(peer_home.join("loopflow.db")),
            ))
            .unwrap();
        let wave = runtime
            .block_on(registered.store.get_wave(&registered.task.wave_id))
            .unwrap()
            .unwrap();
        let peer_wave = if mode == "association-private" {
            loopflow::work::wave::Wave::new(
                loopflow::id::WaveId::new(),
                "private".into(),
                repo.path().to_str().unwrap().into(),
            )
        } else {
            wave.clone()
        };
        runtime.block_on(peer.create_wave(&peer_wave)).unwrap();
        let mut snapshot = runtime
            .block_on(registered.store.pm_snapshot(wave.id()))
            .unwrap()
            .unwrap();
        snapshot.wave_id = peer_wave.id().clone();
        runtime
            .block_on(peer.put_pm_snapshot(snapshot, None))
            .unwrap();
        let peer_task = runtime
            .block_on(
                peer.get_task_by_issue(registered.task.plan.linear_id.as_ref().unwrap().as_str()),
            )
            .unwrap()
            .unwrap();
        assert_ne!(peer_task.id, registered.task.id);
        assert_ne!(peer_task.project_id, registered.task.project_id);
        // Both stores use a disposable key and a synthetic token; no native login.
        seed_linear_token(&runtime, &peer, &key);
        fixture["peer"] = serde_json::json!({"home":peer_home,"task":peer_task.id.as_str(),"project":peer_task.project_id.as_str(),"wave":peer_wave.id().as_str()});
        fixture["private"] = serde_json::json!(mode == "association-private");
        fixture["local_project"] = serde_json::json!(registered.task.project_id.as_str());
        fixture["remote"] = serde_json::json!(repo.bare_path());
        // Populate retained execution on both sides. Public inspection adds its
        // own Processes, but none of these original records may move or settle.
        for (directory, task, wave) in [
            (home.path(), &registered.task.id, wave.id()),
            (peer_home.as_path(), &peer_task.id, peer_wave.id()),
        ] {
            retain_execution(directory, repo.path(), task, wave);
        }
    }
    run_reconnect_fixture(home.path(), &fixture, mode);
}

fn run_reconnect_fixture(home: &Path, fixture: &serde_json::Value, mode: &str) {
    let input = home.join("fixture.json");
    std::fs::write(&input, serde_json::to_vec(fixture).unwrap()).unwrap();
    let output = Command::new("uv")
        .args(["run", "python"])
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/e2e/planning_reconnect.py"),
        )
        .arg(input)
        .arg(mode)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn seed_linear_token(
    runtime: &tokio::runtime::Runtime,
    store: &loopflow::store::Store,
    key: &Path,
) {
    let previous_key = std::env::var_os("LF_PROVIDER_TOKEN_KEY_PATH");
    std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", key);
    let result = runtime.block_on(store.upsert_provider_token(&ProviderToken {
        provider: "linear".into(),
        access_token: "synthetic-planning-token".into(),
        refresh_token: None,
        oauth_client_id: None,
        expires_at: None,
        login: None,
        updated_at: 1,
        credential_type: CredentialType::OAuth,
    }));
    match previous_key {
        Some(value) => std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", value),
        None => std::env::remove_var("LF_PROVIDER_TOKEN_KEY_PATH"),
    }
    result.unwrap();
}

fn retain_execution(
    home: &Path,
    repo: &Path,
    task: &loopflow::durable::TaskId,
    wave: &loopflow::id::WaveId,
) {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    db.execute("INSERT INTO processes(lfid,trace_id,started_at) VALUES('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002',1)", []).unwrap();
    db.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('retained','Retained','human',1,0,?1,?2,?3)", rusqlite::params![repo.to_str().unwrap(),task.as_str(),wave.as_str()]).unwrap();
    let graph = serde_json::json!({"name":"review","nodes":[{"name":"review","skill":"review","description":null}],"edges":[]});
    db.execute(
        "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,'review',1)",
        rusqlite::params![task.as_str(), graph.to_string()],
    )
    .unwrap();
    db.execute("INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,at) VALUES(?1,?2,'set','start','review',1)", rusqlite::params![task.as_str(),graph.to_string()]).unwrap();
}

// Create on the source through the CLI before connecting Linear. The receiver
// learns these identities only through public Git exchange, never a seeded import.
fn prepare_creation_peer(repo: &TestRepo, home: &Path, key: &Path) -> serde_json::Value {
    let peer_home = home.join("creation-peer");
    std::fs::create_dir(&peer_home).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &peer_home)
        .env("LF_HOME", &peer_home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(repo.path())
        .args(["task", "create", "--title", "Creation origin", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let task: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(peer_home.join("loopflow.db")),
        ))
        .unwrap();
    let saved = runtime
        .block_on(store.get_task(&task["id"].as_str().unwrap().parse().unwrap()))
        .unwrap()
        .unwrap();
    let wave = runtime
        .block_on(store.get_wave(&saved.wave_id))
        .unwrap()
        .unwrap();
    seed_linear_token(&runtime, &store, key);
    retain_execution(&peer_home, repo.path(), &saved.id, &saved.wave_id);
    serde_json::json!({"home":peer_home,"repo":repo.path(),"task":saved.id.as_str(),"project":saved.project_id.as_str(),"wave":saved.wave_id.as_str(),"wave_name":wave.slug()})
}

// Import portable plans before connecting Linear. Mixed-provider Git exchange
// is covered separately; this exercises the common foreground owner after import.
fn import_unprepared_plans(repo: &Path, home: &Path) -> serde_json::Value {
    use loopflow::engine::planning_git::PlanningDestination;
    use loopflow::store::sqlite::SqliteStore;

    let source_repo = TestRepo::new();
    let source_home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", source_home.path())
        .env("LF_HOME", source_home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(source_repo.path())
        .args(["task", "create", "--title", "Peer-born task", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let task: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task = task["id"].as_str().unwrap();
    let source = SqliteStore::new(&source_home.path().join("loopflow.db")).unwrap();
    let target = SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let saved = source.task(&task.parse().unwrap()).unwrap().unwrap();
    let source_path = source_repo.path().to_str().unwrap();
    let target_path = repo.to_str().unwrap();
    let binding = PlanningDestination::new(
        source_repo.bare_path().to_str().unwrap(),
        "refs/loopflow/planning/shared/fixture",
    )
    .unwrap();
    source.bind_peer_planning(source_path, &binding).unwrap();
    source
        .select_peer_waves(
            source_path,
            &binding.id(),
            std::slice::from_ref(&saved.wave_id),
        )
        .unwrap();
    target.bind_peer_planning(target_path, &binding).unwrap();
    let snapshot = source
        .export_peer_planning(source_path, &binding.id())
        .unwrap();
    target
        .import_peer_planning(target_path, &binding.id(), "unprepared", &snapshot)
        .unwrap();
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let name: String = db
        .query_row(
            "SELECT name FROM waves WHERE id=?1",
            [saved.wave_id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let wave_dir = repo.join("wave").join(name);
    std::fs::create_dir_all(&wave_dir).unwrap();
    std::fs::write(
        wave_dir.join("GOAL.md"),
        format!(
            "---\nid: {}\npm:\n  linear_initiative: initiative-peer\n---\nPeer export fixture\n",
            saved.wave_id
        ),
    )
    .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM task_creation_intents WHERE task_id=?1",
            [task],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM project_transitions WHERE successor_id=?1",
            [saved.project_id.as_str()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    serde_json::json!({"task":task,"project":saved.project_id.as_str(),"wave":saved.wave_id.as_str()})
}

// Seed an independently retained cache frontier, then use the actual peer
// importer. The public foreground must not mistake the rolled-back receipt for
// permission to send another deletion. No mixed-provider transport is enabled.
fn import_rejected_deletion(
    repo: &Path,
    home: &Path,
    task: &loopflow::durable::TaskId,
) -> serde_json::Value {
    use loopflow::engine::planning_git::PlanningDestination;
    use loopflow::store::sqlite::SqliteStore;
    use rusqlite::params;

    let target = SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let source_home = tempfile::tempdir().unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(source_home.path().join("loopflow.db")),
        ))
        .unwrap();
    let source = SqliteStore::new(&source_home.path().join("loopflow.db")).unwrap();
    let repo = repo.to_str().unwrap();
    let wave = target.task(task).unwrap().unwrap().wave_id;
    let binding = PlanningDestination::new(
        "/synthetic/unavailable",
        "refs/loopflow/planning/shared/effects",
    )
    .unwrap();
    target.bind_peer_planning(repo, &binding).unwrap();
    target
        .select_peer_waves(repo, &binding.id(), std::slice::from_ref(&wave))
        .unwrap();
    source.bind_peer_planning("/source", &binding).unwrap();
    source
        .import_peer_planning(
            "/source",
            &binding.id(),
            "base",
            &target.export_peer_planning(repo, &binding.id()).unwrap(),
        )
        .unwrap();
    let mut record = source.planning_task(task).unwrap().record.unwrap();
    record.item.revision = Some("2026-10-08T10:00:00Z".into());
    record.item.state = Some("unstarted".into());
    record.item.url = None;
    source
        .put_pm_task(
            "/source",
            "linear",
            &record,
            Some((&wave, "initiative-task-pr-tests")),
        )
        .unwrap();
    source.delete_task(task).unwrap();
    let db = rusqlite::Connection::open(source_home.path().join("loopflow.db")).unwrap();
    db.execute("UPDATE task_changes SET attempted=1,error='lost reply' WHERE task_id=?1 AND field='deleted'", [task.as_str()]).unwrap();
    let receipt: String = db
        .query_row(
            "SELECT id FROM task_changes WHERE task_id=?1 AND field='deleted'",
            [task.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let incoming = source
        .export_peer_planning("/source", &binding.id())
        .unwrap();
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    record.item.revision = Some("2026-10-08T12:00:00Z".into());
    // Cache-only setup models a retained migration/alternate acquisition frontier.
    db.execute(
        "UPDATE pm_items SET body=?2 WHERE id=?1",
        params![record.item.id, serde_json::to_string(&record.item).unwrap()],
    )
    .unwrap();
    target
        .import_peer_planning(repo, &binding.id(), "stale-with-effect", &incoming)
        .unwrap();
    assert!(target
        .peer_planning_status(repo)
        .unwrap()
        .into_iter()
        .flat_map(|status| status.conflicts)
        .any(|c| c.object.id == task.as_str() && c.reason.contains("older than retained")));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM task_changes WHERE id=?1",
            [&receipt],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    retain_execution(home, Path::new(repo), task, &wave);
    serde_json::json!({"receipt":receipt})
}
