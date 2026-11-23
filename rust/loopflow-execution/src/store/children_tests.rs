use crate::process::attachment;
use loopflow_model::durable::{ProjectId, RepositoryId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::migration_runner::migration_section_sql;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::edit::WorkValue::{ProjectName, ProjectSummary};
use loopflow_storage::work::task::{NewTask, TaskUpdate};
use rusqlite::params;

fn local_repository(store: &SqliteStore) -> RepositoryId {
    let repo = store.home_dir().unwrap();
    // Fence Git discovery before registering the fixture repository.
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    crate::repository::location::ensure_repository(store, repo.to_str().unwrap()).unwrap()
}

fn local_repository_path(store: &SqliteStore) -> String {
    local_repository(store);
    store
        .home_dir()
        .unwrap()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn local_project(store: &SqliteStore) -> ProjectId {
    local_repository(store);
    crate::test_support::seed_wave_project(
        store,
        store.home_dir().unwrap().to_str().unwrap(),
        "inbox",
    )
    .unwrap()
    .id
}

/// Take `edits` in as another Machine's values, the way a carrier brings them.
fn import_edits(
    store: &SqliteStore,
    edits: &[loopflow_storage::work::edit::WorkEdit],
) -> crate::store::StoreResult<()> {
    let repo = store.home_dir()?.canonicalize().unwrap();
    loopflow_storage::test_fixture::arrive(store, repo.to_str().unwrap(), edits)
}

#[test]
fn task_identifier_repairs_all_generated_titles_from_a_replaced_editor_with_foreign_capture() {
    use crate::process::{AgentCaller, AGENT_CALLER_ENV};
    use crate::session::TitleSource;
    use loopflow_storage::sqlite::SessionCaller;
    use loopflow_storage::work::edit::{WorkEdit, WorkValue};

    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let path = ledger.home().join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let task = store
        .create_task(&NewTask {
            parent_task_id: None,
            repository_id: local_repository(&store),
            id: TaskId::new(),
            project_id: Some(local_project(&store)),
            title: "Fix login".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let template = store.test_session("template", &crate::session_record::new_artifact_key());
    for (id, source) in [
        ("editor", TitleSource::Generated),
        ("other", TitleSource::Generated),
        ("human", TitleSource::Human),
    ] {
        crate::store::sessions::create_session(
            &store,
            crate::session::LfSession {
                id: id.into(),
                captured: None,
                artifact_key: Some(crate::session_record::new_artifact_key()),
                title: "Fix login".into(),
                title_source: source,
                task_id: Some(task.id.clone()),
                wave_id: task.wave_id.clone(),
                ..template.clone()
            },
            None,
        )
        .unwrap();
    }
    let process = loopflow_model::id::LfProcessId::new();
    store.conn.lock().unwrap().execute(
            "INSERT INTO processes(id,trace_id,started_at,command,cwd) VALUES(?1,'trace',1,'lf session resume','/fixture')", [&process]
        ).unwrap();
    let absent = SessionCaller {
        agent: None,
        capture_key: None,
    };
    let first = store
        .attach_session("editor", None, &process, None, &absent)
        .unwrap();
    store
        .attach_session("editor", Some(&first), &process, None, &absent)
        .unwrap();
    let caller = AgentCaller {
        session_id: "editor".into(),
        agent_process_id: Some(first.agent_process_id),
        origin_lf_process_id: process,
        trace_id: None,
    };
    std::env::set_var(AGENT_CALLER_ENV, serde_json::to_string(&caller).unwrap());
    crate::journal::with_runtime(
        ledger.home(),
        &["lf".into(), "task".into(), "edit".into()],
        || {
            assert_eq!(crate::journal::agent_caller(), Some(caller));
            std::env::set_var(
                crate::session_record::CAPTURE_KEY_ENV,
                crate::session_record::new_artifact_key(),
            );
            import_edits(
                &store,
                &WorkEdit::for_target(
                    task.id.as_str(),
                    &[WorkValue::TaskIdentifier("ENG-123".into())],
                ),
            )?;
            Ok(())
        },
    )
    .unwrap();
    for id in ["editor", "other"] {
        assert_eq!(
            store.session(id).unwrap().unwrap().title,
            "ENG-123 Fix login"
        );
    }
    assert_eq!(store.session("human").unwrap().unwrap().title, "Fix login");
}

#[test]
fn task_labels_and_generated_names_follow_local_identity_and_linear_mapping() {
    use crate::session::{SessionFilter, TitleSource, WorkSource};
    use loopflow_storage::work::edit::{WorkEdit, WorkValue};
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let task = store
        .create_task(&NewTask {
            parent_task_id: None,
            repository_id: local_repository(&store),
            id: TaskId::from_raw("task_abcdef10000000000000000000000001"),
            project_id: Some(local_project(&store)),
            title: "Fix login".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    assert_eq!(task.identifier, "abcdef");
    let template = store.test_session("template", &crate::session_record::new_artifact_key());
    for (id, source, title) in [
        ("generated", TitleSource::Generated, "Fix login"),
        ("human", TitleSource::Human, "My login notes"),
    ] {
        let mut session = template.clone();
        session.id = id.into();
        session.artifact_key = Some(crate::session_record::new_artifact_key());
        session.captured = None;
        session.task_id = Some(task.id.clone());
        session.wave_id = Some(task.wave_id.clone().unwrap());
        session.work_source = Some(WorkSource::Declared);
        session.title = title.into();
        session.title_source = source;
        let saved = crate::store::sessions::create_session(&store, session, None).unwrap();
        assert_eq!(
            saved.title,
            if id == "generated" {
                "abcdef Fix login"
            } else {
                title
            }
        );
    }
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE tasks SET external_issue_id='pending-export' WHERE id=?1",
            [task.id.as_str()],
        )
        .unwrap();
        conn.execute("INSERT INTO task_checkouts(task_id,worktree,branch) VALUES(?1,'/retained','old/branch')", [task.id.as_str()]).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET title=?1 WHERE id='generated'",
            [format!(
                "lf-{} Fix login",
                task.id.as_str().trim_start_matches("task_")
            )],
        )
        .unwrap();
    }
    let summaries =
        crate::store::session_projection::session_summaries(&store, &SessionFilter::default(), 0)
            .unwrap();
    let summary = summaries
        .iter()
        .find(|session| session.id == "generated")
        .unwrap();
    assert_eq!(summary.task_identifier.as_deref(), Some("abcdef"));
    assert!(
        summary.title.starts_with("lf-abcdef1000"),
        "passive reads preserve titles"
    );
    assert_eq!(
        store
            .task_checkout(&task.id)
            .unwrap()
            .unwrap()
            .issue_identifier,
        "abcdef"
    );
    let process = loopflow_model::id::LfProcessId::new();
    store.conn.lock().unwrap().execute(
            "INSERT INTO processes(id,trace_id,started_at,command,cwd) VALUES(?1,'trace',1,'lf session resume','/fixture')",
            [&process],
        ).unwrap();
    attachment::claim_session_attachment(&store, "generated", None, &process).unwrap();
    assert_eq!(
        store.session("generated").unwrap().unwrap().title,
        "abcdef Fix login"
    );
    let history = crate::store::sessions::conversation_history(
        &store,
        None,
        None,
        Some("abcdef"),
        None,
        0,
        true,
    )
    .unwrap();
    assert_eq!(history.len(), 2);
    assert!(history
        .iter()
        .all(|input| input.task_identifier.as_deref() == Some("abcdef")));
    {
        import_edits(
            &store,
            &WorkEdit::for_target(
                task.id.as_str(),
                &[WorkValue::TaskIdentifier("ENG-123".into())],
            ),
        )
        .unwrap();
    }
    let mapped = store.task(&task.id).unwrap().unwrap();
    assert_eq!(mapped.identifier, "ENG-123");
    assert_eq!(mapped.branch, "old/branch");
    assert_eq!(
        store.task_checkouts().unwrap()[0].issue_identifier,
        "ENG-123"
    );
    assert_eq!(
        store.session("generated").unwrap().unwrap().title,
        "ENG-123 Fix login"
    );
    assert_eq!(
        store.session("human").unwrap().unwrap().title,
        "My login notes"
    );
    crate::store::sessions::rename_session(
        &store,
        "generated",
        "Review login",
        TitleSource::Generated,
    )
    .unwrap();
    assert_eq!(
        store.session("generated").unwrap().unwrap().title,
        "ENG-123 Review login"
    );
    let history = crate::store::sessions::conversation_history(
        &store,
        None,
        None,
        Some("eng-123"),
        None,
        0,
        true,
    )
    .unwrap();
    assert_eq!(history.len(), 2);
    assert!(history
        .iter()
        .all(|input| input.task_identifier.as_deref() == Some("ENG-123")));
    let summary = crate::store::session_projection::session_summary(&store, "generated", 0)
        .unwrap()
        .unwrap();
    assert_eq!(summary.task_identifier.as_deref(), Some("ENG-123"));
}

#[test]
fn local_labels_extend_only_for_collisions_including_completed_tasks() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let project = local_project(&store);
    let first = TaskId::from_raw("task_abcdef10000000000000000000000001");
    let second = TaskId::from_raw("task_abcdef20000000000000000000000002");
    for id in [&first, &second] {
        store
            .create_task(&NewTask {
                parent_task_id: None,
                repository_id: local_repository(&store),
                id: id.clone(),
                project_id: Some(project.clone()),
                title: "Fix login".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
    }
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET planning_completed=1,issue_identifier='ENG-9' WHERE id=?1",
            [second.as_str()],
        )
        .unwrap();
    assert_eq!(store.task(&first).unwrap().unwrap().identifier, "abcdef1");
    assert_eq!(store.task(&second).unwrap().unwrap().identifier, "ENG-9");
    assert!(store.task_by_issue("abcdef").is_err());
    assert_eq!(store.task_by_issue("abcdef1").unwrap().unwrap().id, first);
    assert!(store.task(&TaskId::new()).unwrap().is_none());
}

#[test]
fn task_checkout_reads_this_machines_checkout() {
    let directory = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&directory.path().join("db")).unwrap();
    let task = store
        .create_task(&NewTask {
            parent_task_id: None,
            repository_id: local_repository(&store),
            due_date: None,
            id: TaskId::new(),
            project_id: Some(local_project(&store)),
            title: "Locate files".into(),
            description: String::new(),
        })
        .unwrap();
    assert!(store.task_checkout(&TaskId::new()).unwrap().is_none());
    assert!(store.task_checkout(&task.id).unwrap().is_none());
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/retained/checkout')",
            [task.id.as_str()],
        )
        .unwrap();
    let checkout = store.task_checkout(&task.id).unwrap().unwrap();
    assert_eq!(checkout.task_id, task.id);
    assert_eq!(checkout.issue_identifier, task.identifier);
    assert_eq!(
        checkout.worktree,
        std::path::Path::new("/retained/checkout")
    );
}

#[test]
fn canonical_task_fields_survive_restart_and_reach_another_machine_through_the_hub() {
    use loopflow_storage::work::edit::{Disposition, WorkValue};

    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let path = source.path().join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let task = store
        .create_task(&NewTask {
            parent_task_id: None,
            repository_id: local_repository(&store),
            id: TaskId::new(),
            project_id: Some(local_project(&store)),
            title: "Shared Task".into(),
            description: "Keep the local execution".into(),
            due_date: Some("2026-10-15".into()),
        })
        .unwrap();
    assert_eq!(task.due_date.as_deref(), Some("2026-10-15"));
    {
        import_edits(
            &store,
            &loopflow_storage::work::edit::WorkEdit::for_target(
                task.id.as_str(),
                &[
                    WorkValue::TaskRank(7),
                    WorkValue::TaskAssignee(Some("assignee-1".into())),
                    WorkValue::Disposition(Disposition {
                        planning_state: Some("completed".into()),
                        planning_completed: 1,
                        planning_completed_at: Some("2026-10-10T12:00:00Z".into()),
                    }),
                ],
            ),
        )
        .unwrap();
    }
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let saved = store.task(&task.id).unwrap().unwrap();
    assert_eq!(saved.rank, 7);
    assert_eq!(saved.assignee.as_deref(), Some("assignee-1"));
    assert_eq!(saved.state.as_deref(), Some("completed"));
    assert_eq!(saved.completed_at.as_deref(), Some("2026-10-10T12:00:00Z"));
    assert_eq!(saved.due_date, task.due_date);
    assert_eq!(saved.worktree, task.worktree);
    assert_eq!(saved.branch, task.branch);
    assert_eq!(saved.parent_pr_id, task.parent_pr_id);
    let peer = crate::store::opening::open_ephemeral(&target.path().join("loopflow.db")).unwrap();
    let wave = store
        .get_wave(saved.wave_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    let page = store.plan_records_since(wave.repo(), 0, 1 << 20).unwrap();
    assert!(!page.more);
    let target_repo = local_repository_path(&peer);
    for _ in 0..2 {
        peer.apply_plan_records(&target_repo, &page.records, None, &Default::default())
            .unwrap();
        let imported = peer.task(&task.id).unwrap().unwrap();
        assert_eq!(imported.id, saved.id);
        assert_eq!(imported.project_id, saved.project_id);
        assert_eq!(imported.title, saved.title);
        assert_eq!(imported.description, saved.description);
        assert_eq!(imported.rank, saved.rank);
        assert_eq!(imported.assignee, saved.assignee);
        assert_eq!(imported.due_date, saved.due_date);
        assert_eq!(imported.state, saved.state);
        assert_eq!(imported.completed_at, saved.completed_at);
        assert!(imported.worktree.is_none());
        assert!(peer.task_prs(&task.id).unwrap().is_empty());
    }
}

#[test]
fn project_edits_and_health_checks_leave_retired_fields_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let id = local_project(&store);
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE projects SET iteration=7, krs='[{}]' WHERE id=?1",
            [id.as_str()],
        )
        .unwrap();
    store
        .edit_project_fields(
            &id,
            &[
                ProjectName(Some("Renamed body of work".into())),
                ProjectSummary("New summary".into()),
            ],
        )
        .unwrap();
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let project = store.project(&id).unwrap().unwrap();
    assert_eq!(project.name, "Renamed body of work");
    assert_eq!(project.summary, "New summary");
    store.health_check().unwrap();
    let retired: (u32, String) = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT iteration,krs FROM projects WHERE id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(retired, (7, "[{}]".into()));
    let json = serde_json::to_value(project).unwrap();
    assert!(json.get("iteration").is_none());
    assert!(json.get("plan").is_none());
}

#[test]
fn local_planning_creation_retry_keeps_identity_after_edit_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let input = NewTask {
        parent_task_id: None,
        repository_id: local_repository(&store),
        due_date: None,
        id: TaskId::new(),
        project_id: Some(local_project(&store)),
        title: "Fix parser".into(),
        description: "Keep the original input".into(),
    };
    let task = store.create_task(&input).unwrap();
    assert!(task.worktree.is_none());
    assert!(task.tracker_id.is_none());
    assert!(store.task_prs(&task.id).unwrap().is_empty());
    assert!(store.task_checkouts().unwrap().is_empty());
    let edited = store
        .edit_task(
            &task.id,
            task.revision,
            &TaskUpdate {
                title: Some("Fix quoted input".into()),
                description: None,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(edited.revision, task.revision + 1);
    assert_eq!(edited.description, input.description);
    assert!(store
        .edit_task(
            &task.id,
            task.revision,
            &TaskUpdate {
                title: Some("Stale".into()),
                description: None,
                ..Default::default()
            }
        )
        .is_err());
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let retried = store.create_task(&input).unwrap();
    assert_eq!(retried, edited);
    let child_request = NewTask {
        id: TaskId::new(),
        parent_task_id: Some(task.id.clone()),
        ..input.clone()
    };
    let other_repo = loopflow_storage::test_fixture::repository(&store, "/other");
    let cross_repo = NewTask {
        repository_id: other_repo,
        project_id: None,
        ..child_request.clone()
    };
    assert!(store
        .create_task(&cross_repo)
        .unwrap_err()
        .to_string()
        .contains("another repository"));
    let second = store.create_task(&child_request).unwrap();
    let detached = store
        .edit_task(
            &second.id,
            second.revision,
            &TaskUpdate {
                parent: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(store.create_task(&child_request).unwrap(), detached);
    assert_ne!(second.id, task.id);
    assert_eq!(store.list_tasks(None).unwrap().len(), 2);
    assert_eq!(
        store
            .list_tasks(Some(task.wave_id.as_ref().unwrap()))
            .unwrap()
            .len(),
        2
    );
    assert!(store.list_tasks(Some(&WaveId::new())).unwrap().is_empty());
    assert!(store
        .create_task(&NewTask {
            repository_id: local_repository(&store),
            due_date: None,
            title: "Different request".into(),
            ..input
        })
        .is_err());
    assert_eq!(store.task(&task.id).unwrap().unwrap(), edited);
    let conn = store.conn.lock().unwrap();
    for table in ["agent_sessions", "processes", "task_graphs", "task_prs"] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "creation must not allocate {table}");
    }
}

#[test]
fn local_planning_provider_mapping_preserves_identity_and_creation_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
    let input = NewTask {
        parent_task_id: None,
        repository_id: local_repository(&store),
        due_date: None,
        id: TaskId::new(),
        project_id: Some(local_project(&store)),
        title: "Keep identity".into(),
        description: String::new(),
    };
    let local = store.create_task(&input).unwrap();
    assert_eq!(local.tracker_synced_at, None);
    // Export is a separate slice; this proves the schema accepts a different
    // provider UUID without changing local identity or creation history.
    store.conn.lock().unwrap().execute(
            "UPDATE tasks SET external_issue_id='different-provider-uuid',issue_identifier='TEAM-9' WHERE id=?1",
            [local.id.as_str()],
        ).unwrap();
    let mapped = store.task_by_issue("TEAM-9").unwrap().unwrap();
    assert_eq!(mapped.id, local.id);
    for selector in ["team-9", "Team-9", "tEaM-9"] {
        let resolved = store.task_by_issue(selector).unwrap().unwrap();
        assert_eq!(resolved.id, local.id);
        assert_eq!(resolved.identifier, "TEAM-9");
    }
    assert_eq!(
        store.task_by_issue(&local.identifier).unwrap().unwrap().id,
        local.id
    );
    assert_eq!(store.create_task(&input).unwrap(), mapped);
    let edited = store
        .edit_task(
            &local.id,
            mapped.revision,
            &TaskUpdate {
                title: Some("A mapping does not transfer authority".into()),
                description: None,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(edited.title, "A mapping does not transfer authority");
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE tasks SET planning_deleted_at=1 WHERE id=?1",
            [mapped.id.as_str()],
        )
        .unwrap();
    assert!(store.task_deleted(&mapped).unwrap());
    assert!(store.list_tasks(None).unwrap().is_empty());
    let retained = store.task(&mapped.id).unwrap().unwrap();
    assert_eq!(retained.revision, edited.revision);
    assert_eq!(retained.tracker_id, edited.tracker_id);
    assert_eq!(retained.identifier, edited.identifier);
    assert_eq!(retained.title, edited.title);
    assert_eq!(retained.description, edited.description);
    assert_eq!(retained.tracker_synced_at, edited.tracker_synced_at);
    assert_eq!(store.create_task(&input).unwrap(), retained);
}

#[test]
fn local_planning_prefix_collision_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
    let input = NewTask {
        parent_task_id: None,
        repository_id: local_repository(&store),
        due_date: None,
        id: TaskId::parse("task_0123456789ab40008000000000000001").unwrap(),
        project_id: Some(local_project(&store)),
        title: "One".into(),
        description: String::new(),
    };
    let first = store.create_task(&input).unwrap();
    assert_eq!(first.identifier, "012345");
    for selector in ["0123", "0123456789A", "lf-0123", "task_0123"] {
        assert_eq!(store.task_by_issue(selector).unwrap().unwrap().id, first.id);
    }
    for selector in [
        "012",
        "lf-012",
        "task_012",
        "012z",
        "0123%",
        "fffffffffffffffffffffffffffffffff",
    ] {
        assert!(store.task_by_issue(selector).unwrap().is_none());
    }
    assert_eq!(
        store.task_by_issue("lf-0123456789ab").unwrap().unwrap().id,
        first.id
    );
    let second = store
        .create_task(&NewTask {
            repository_id: local_repository(&store),
            due_date: None,
            id: TaskId::parse("task_0123456789ab40008000000000000002").unwrap(),
            ..input
        })
        .unwrap();
    for selector in ["0123", "lf-0123", "task_0123", "lf-0123456789ab"] {
        let error = store.task_by_issue(selector).unwrap_err().to_string();
        assert!(error.contains(first.id.as_str()));
        assert!(error.contains(second.id.as_str()));
        assert!(error.contains("use a longer Task ID"));
    }
    let first = store.task(&first.id).unwrap().unwrap();
    assert_eq!(first.identifier.len(), 32);
    assert_eq!(second.identifier.len(), 32);
    for task in [first, second] {
        assert_eq!(
            store.task_by_issue(task.id.as_str()).unwrap().unwrap().id,
            task.id
        );
        assert_eq!(
            store.task_by_issue(&task.identifier).unwrap().unwrap().id,
            task.id
        );
    }
}

#[test]
fn local_planning_migration_preserves_released_links() {
    use crate::store::migrations::{apply_before_migration, upgrade_from_migration};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "local_planning");
    let wave = WaveId::new();
    let orphan_wave = WaveId::new();
    let project = ProjectId::new();
    let task = TaskId::new();
    conn.execute("INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'shared','/repo',1),(?2,'recovery','/repo',1)", params![wave,orphan_wave]).unwrap();
    conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at) VALUES(?1,?2,'linear-project',1,'shared','Shared','Retain KRs',7,7)",params![project.as_str(),wave]).unwrap();
    conn.execute(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../loopflow-storage/tests/fixtures/migrations/local_planning_retained_task.sql"
        )),
        params![task.as_str(), project.as_str()],
    )
    .unwrap();
    conn.execute("INSERT INTO task_issue_identities(wave_id,issue_id,identifier) VALUES(?1,'orphan-issue','LOO-2')",[&orphan_wave]).unwrap();
    let payload: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/dto/task_history_planning.json"
    )))
    .unwrap();
    let mut historical_project = payload["projects"][0].clone();
    historical_project["workflow"] = serde_json::json!("");
    let project_body = historical_project.to_string();
    let item_body = payload["items"][0].to_string();
    conn.execute("INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES('/repo','linear','current',7,?1)",[&project_body]).unwrap();
    conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear','LOO-318','LOO-318','current',7,?1)",[&item_body]).unwrap();
    conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at) VALUES('pr-retained',?1,1,'task','retain/branch','retained-base',1,7)",[task.as_str()]).unwrap();
    conn.execute_batch(
        &include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../loopflow-storage/tests/fixtures/migrations/local_planning_retained_process.sql"
        ))
        .replace("{task}", task.as_str())
        .replace("{wave}", wave.as_str()),
    )
    .unwrap();
    let imported_project = ProjectId::new();
    conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at) VALUES(?1,?2,'current',1,'current','Current','',7,7)",params![imported_project.as_str(),wave]).unwrap();
    conn.execute(
        "INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,'linear',?2,7)",
        params![
            wave,
            payload["projects"][0]["initiative_ids"][0]
                .as_str()
                .unwrap()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,'current',0)",
        [&wave],
    )
    .unwrap();
    let tables = [
        "task_prs",
        "agent_sessions",
        "task_workflows",
        "task_workflow_moves",
        "task_issue_identities",
        "pm_projects",
        "pm_items",
    ];
    let queries: Vec<_> = tables
        .iter()
        .map(|table| {
            let statement = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            format!("SELECT {} FROM {table}", statement.column_names().join(","))
        })
        .collect();
    let rows = |conn: &rusqlite::Connection, sql: &str| {
        let mut query = conn.prepare(sql).unwrap();
        let columns = query.column_count();
        query
            .query_map([], |row| {
                (0..columns)
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(|value| match value {
                                rusqlite::types::Value::Text(text) => {
                                    serde_json::from_str::<serde_json::Value>(&text)
                                        .map(|json| rusqlite::types::Value::Text(json.to_string()))
                                        .unwrap_or(rusqlite::types::Value::Text(text))
                                }
                                other => other,
                            })
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let before: Vec<_> = queries.iter().map(|sql| rows(&conn, sql)).collect();
    conn.execute_batch("PRAGMA foreign_keys=OFF; BEGIN IMMEDIATE;")
        .unwrap();
    conn.execute_batch(&migration_section_sql("local_planning"))
        .unwrap();
    let violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    conn.execute_batch("COMMIT; PRAGMA foreign_keys=ON;")
        .unwrap();
    for ((table, sql), expected) in tables.iter().zip(&queries).zip(before) {
        assert_eq!(rows(&conn, sql), expected, "{table}");
    }
    upgrade_from_migration(&conn, "optional_task_pr");
    let order: String = conn
        .query_row(
            "SELECT task_order_json FROM tracker_projects WHERE id='current'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let store = SqliteStore::from_fixture_connection(conn, crate::store::token_key::encryption_key);
    let observation: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input_in(
            &store.conn.lock().unwrap(),
            "/repo",
            "LOO-318",
        );
    let mut expected = payload["items"][0].clone();
    expected["parent_id"] = serde_json::Value::Null;
    expected["parent_observed"] = serde_json::json!(false);
    assert_eq!(serde_json::to_value(observation.item).unwrap(), expected);
    assert_eq!(
        serde_json::to_value(observation.project.unwrap()).unwrap(),
        payload["projects"][0]
    );
    assert_eq!(observation.observed_at, 7);
    let imported = store.task_by_issue("LOO-318").unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&order).unwrap(),
        serde_json::json!([imported.id.as_str()])
    );
    assert_eq!(imported.project_id, Some(imported_project));
    assert!(imported.worktree.is_none());
    assert!(store.task_prs(&imported.id).unwrap().is_empty());
    assert_eq!(imported.tracker_synced_at, Some(7));
    let stored_state: (Option<String>, bool) = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT planning_state,planning_completed FROM tasks WHERE id=?1",
            [imported.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        stored_state.0.as_deref(),
        payload["items"][0]["state"].as_str()
    );
    assert_eq!(
        stored_state.1,
        payload["items"][0]["completed"].as_bool().unwrap()
    );
    let uuid = uuid::Uuid::parse_str(imported.id.as_str().trim_start_matches("task_")).unwrap();
    assert_eq!(uuid.get_version_num(), 4);
    assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
    let retained = store.task(&task).unwrap().unwrap();
    assert_eq!(retained.id, task);
    assert_eq!(retained.project_id, Some(project.clone()));
    assert_eq!(retained.tracker_id.unwrap().as_str(), "linear-task");
    assert_eq!(retained.identifier, "LOO-1");
    assert_eq!(retained.worktree.unwrap().to_str(), Some("/repo/task"));
    let project = store.project(&project).unwrap().unwrap();
    assert_eq!(project.tracker_id.unwrap().as_str(), "linear-project");
    assert_eq!(project.description, "Retain KRs");
    assert!(store.task_by_issue("LOO-2").unwrap().is_none());
}

#[test]
fn an_admitted_creation_commits_task_graph_claim_and_conversation_together() {
    use loopflow_storage::sqlite::creation::{
        CreationIdentity, RecoveryDisposition, TaskAdmission, TaskCreationRecord,
    };
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
    let repository_id = local_repository(&store);
    let admit = |request: &str, graph: &str| {
        let creation = NewTask {
            parent_task_id: None,
            repository_id: repository_id.clone(),
            due_date: None,
            id: TaskId::new(),
            project_id: None,
            title: "Start in the conversation".into(),
            description: "Start in the conversation".into(),
        };
        let mut session = loopflow_storage::test_fixture::unpublished_conversation(
            Some(creation.id.clone()),
            None,
            1,
        );
        session.interactive = true;
        let record = TaskCreationRecord {
            identity: CreationIdentity {
                request_id: request.into(),
                kind: "task.create",
                fingerprint: "f".into(),
                run: true,
            },
            admission: Some(TaskAdmission {
                graph: Some(graph.into()),
                session: session.clone(),
                caller: None,
            }),
        };
        let result = crate::store::ownership::promote_task(
            &store,
            &creation.id,
            Some(&creation),
            None,
            None,
            None,
            None,
            Some(&record),
        );
        (creation.id, session, result)
    };

    // A write failing inside admission leaves nothing: no Task without its
    // Graph or conversation, and no receipt to answer a repeat with.
    let (failed, _, result) = admit("broken", "not json");
    assert!(result.is_err());
    assert!(store.task(&failed).unwrap().is_none());
    assert!(store.creation_receipt("broken").unwrap().is_none());

    let (task, session, result) = admit("req", r#"{"name":"code","steps":[]}"#);
    result.unwrap();
    // One reading holds all of it.
    assert_eq!(
        store.task_graph_row(&task).unwrap().unwrap().step,
        loopflow_storage::sqlite::task_work::START
    );
    assert!(store.task_claimed_here(&task).unwrap());
    assert_eq!(store.task_primary(&task).unwrap().unwrap().id, session.id);
    let admitted = store.creation_receipt("req").unwrap().unwrap();
    assert_eq!(admitted.milestone, "admitted");
    assert_eq!(admitted.disposition, RecoveryDisposition::Complete);
    let result = admitted.result.unwrap();
    assert_eq!(result["milestone"], "admitted");
    assert_eq!(result["graph"], "code");
    assert_eq!(result["session_id"], session.id.as_str());
}
