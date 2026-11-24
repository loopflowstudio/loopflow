use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::{LfProcessId, TraceId, WaveId};
use loopflow_storage::execution::GraphProcessFilter;
use loopflow_storage::graph_process::RunOwner;
use loopflow_storage::session::SessionFilter;
use rusqlite::params;
use std::num::NonZeroU32;

#[test]
fn defer_holds_the_lifecycle_and_a_run_is_moving_whatever_its_auto_setting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let task = TaskId::new();
    let wave = WaveId::new();
    let project = ProjectId::new();
    let by = LfProcessId::new();
    let carrier = loopflow_storage::graph_process::ExecutionStart {
        started_at: 1,
        pid: Some(std::process::id()),
        os_started_at: None,
        caller_session: None,
    };
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo/task',1)",
            params![by, loopflow_model::id::TraceId::new()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
            params![project.as_str(), wave],
        )
        .unwrap();
    }
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE waves SET current_project_id=?1 WHERE id=?2",
            params![project.as_str(), wave],
        )
        .unwrap();
    store
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: loopflow_storage::test_fixture::repository(&store, "/repo"),
            id: task.clone(),
            project_id: Some(project),
            title: "Retained Task".into(),
            description: "Brief".into(),
            due_date: None,
        })
        .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/task')",
            [task.as_str()],
        )
        .unwrap();
    let definition = crate::task_graph::CapturedGraph {
        name: "proof".into(),
        steps: vec![crate::graph::ConcreteStep::Skill(
            crate::graph::ConcreteSkill {
                skill: crate::graph::Skill::named("pursue"),
                id: None,
                interactive: false,
                interactive_name: None,
                returns: None,
                sources: Vec::new(),
            },
        )],
    };
    crate::store::task_work::take_up_task_graph(&store, &task, &definition).unwrap();
    let before = crate::store::task_work::task_graph(&store, &task)
        .unwrap()
        .unwrap();
    let row = crate::store::task_work::task_graph_row(&store, &task)
        .unwrap()
        .unwrap();
    let start = crate::graph::runner::ExecutionCursor::default();
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    assert_eq!(
        store.task_state(&task).unwrap(),
        crate::durable::TaskState::Idle
    );
    assert_eq!(
        crate::store::task_work::task_graph(&store, &task)
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        store.task(&task).unwrap().unwrap().worktree,
        Some(std::path::PathBuf::from("/repo/task"))
    );

    // A run moves the Task.
    assert!(crate::store::task_work::start_task_graph_run(
        &store,
        &task,
        &row,
        &start,
        (&by, &carrier),
    )
    .unwrap());
    // A second run reading the same position cannot also set out.
    assert!(!crate::store::task_work::start_task_graph_run(
        &store,
        &task,
        &row,
        &start,
        (&LfProcessId::new(), &carrier),
    )
    .unwrap());
    let state = || store.task_state(&task).unwrap();
    assert_eq!(state(), crate::durable::TaskState::Moving);
    let mark = |state| loopflow_storage::sqlite::Auto {
        state,
        at: loopflow_storage::rows::now_unix(),
        by: crate::process::Starter::Person { name: None },
    };
    // State says what is happening, whoever takes the interactive steps:
    // the running Task is moving with auto off and with it on.
    for setting in [
        loopflow_storage::sqlite::AutoState::Off,
        loopflow_storage::sqlite::AutoState::On,
    ] {
        store.set_task_auto(&task, &mark(setting), false).unwrap();
        assert_eq!(state(), crate::durable::TaskState::Moving);
    }
    // The carrier exits and nothing runs.
    store
        .finish_run(RunOwner::TaskCarrier, &by, 2, Some(0), None)
        .unwrap();
    assert_eq!(state(), crate::durable::TaskState::Idle);
    // Recovery can hold an observation from before the carrier cleared its pid.
    assert!(!store.count_task_run_restart(&task, &by, 0).unwrap());
}

#[test]
fn reaching_end_without_remaining_steps_requests_completion_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    let task = TaskId::new();
    let by = LfProcessId::new();
    store
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: loopflow_storage::test_fixture::repository(&store, "/repo"),
            id: task.clone(),
            project_id: None,
            title: "Finish review".into(),
            description: "No steps remain".into(),
            due_date: None,
        })
        .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
            params![by, TraceId::new()],
        )
        .unwrap();
    crate::store::task_work::take_up_task_graph(
        &store,
        &task,
        &crate::task_graph::CapturedGraph {
            name: "review".into(),
            steps: Vec::new(),
        },
    )
    .unwrap();
    store.set_task_graph_step(&task, "review").unwrap();
    assert!(store.end_task_graph(&task).unwrap());
    assert!(store.task_completion_pending(&task).unwrap().is_some());
    assert_eq!(store.task_auto(&task).unwrap(), None);
    assert!(!store.end_task_graph(&task).unwrap());
}

#[test]
fn reaching_end_leaves_a_task_with_open_children_open() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    let [parent, child] = [TaskId::new(), TaskId::new()];
    for (id, parent_task_id) in [(&parent, None), (&child, Some(parent.clone()))] {
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                parent_task_id,
                repository_id: loopflow_storage::test_fixture::repository(&store, "/repo"),
                id: id.clone(),
                project_id: None,
                title: "Retry requests".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
    }
    crate::store::task_work::take_up_task_graph(
        &store,
        &parent,
        &crate::task_graph::CapturedGraph {
            name: "review".into(),
            steps: Vec::new(),
        },
    )
    .unwrap();
    store.set_task_graph_step(&parent, "review").unwrap();
    assert!(store.end_task_graph(&parent).unwrap());
    assert!(store.task_completion_pending(&parent).unwrap().is_none());
    assert_eq!(store.open_task_child_count(&parent).unwrap(), 1);

    // Once the child is closed, an explicit request settles as before.
    store
        .abandon(&loopflow_model::durable::WorkRef::Task(child), "not needed")
        .unwrap();
    assert!(store
        .request_task_completion(&parent, None)
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn task_checkout_machine_keeps_session_filters_beside_other_machines() {
    let repo = loopflow_test_support::TestRepo::new();
    let other = loopflow_test_support::TestRepo::new();
    let dir = tempfile::tempdir().unwrap();
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(repo.path(), &alias).unwrap();
    std::fs::create_dir(repo.path().join("sub")).unwrap();
    let store = std::sync::Arc::new(
        crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            dir.path().join("db"),
        ))
        .await
        .unwrap(),
    );
    let home = store.local_machine().await.unwrap().id;
    let task = TaskId::new();
    let wave = WaveId::new();
    let project = ProjectId::new();
    {
        let conn = store.sqlite.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'issue','PROOF-1','Retained Task',1)",params![project.as_str(),task.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
            params![task.as_str(), repo.path().to_str().unwrap()],
        )
        .unwrap();
        for (id, path, scope) in [
            ("a-alias", alias.join("sub"), None),
            ("b-subdir", repo.path().join("sub"), None),
            ("c-repo", repo.path().to_path_buf(), Some("repository")),
            ("d-orphan", other.path().to_path_buf(), None),
        ] {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,interactive) VALUES(?1,?1,'human',1,0,?2,?3,1)",params![id,path.to_str().unwrap(),scope]).unwrap();
            loopflow_storage::sqlite::sessions::test_capture(
                &conn,
                id,
                &loopflow_storage::session_record::new_artifact_key(),
            );
        }
    }
    let remote = loopflow_model::durable::MachineId::new();
    crate::store::machines::add_machine(&store.sqlite, &remote, "worker", "worker").unwrap();
    let mut filter = SessionFilter {
        task: Some("PROOF-1".into()),
        limit: 1,
        after: Some(String::new()),
        ..Default::default()
    };
    for expected in ["a-alias", "b-subdir"] {
        let page = crate::ops::human_session::list(&store, &filter)
            .await
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, expected);
        assert_eq!(page[0].task_ids, vec![task.clone()]);
        assert_eq!(page[0].workspace.as_ref().unwrap().machine_id, home);
        filter.after = Some(page[0].id.to_string());
    }
    filter.task = None;
    filter.orphan = true;
    filter.after = Some(String::new());
    for expected in ["c-repo", "d-orphan"] {
        let page = crate::ops::human_session::list(&store, &filter)
            .await
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, expected);
        assert!(page[0].task_ids.is_empty());
        filter.after = Some(page[0].id.to_string());
    }
}

#[test]
fn task_work_includes_checkout_binding_without_granting_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    let task = TaskId::new();
    let wave = WaveId::new();
    let project = ProjectId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'issue','PROOF-1',1)",params![project.as_str(),task.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/missing/task%_')",
            params![task.as_str()],
        )
        .unwrap();
        for (id, cwd, bound, complete) in [
            ("manual", "/missing/task%_/src", false, false),
            ("conversation", "/missing/task%_", false, false),
            ("history", "/elsewhere", true, true),
            ("sibling", "/missing/task%_-other", false, false),
            ("wildcard", "/missing/taskAB", false, false),
        ] {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id,completed_at)
                VALUES(?1,?1,'human',1,0,?2,?3,?4,?5)", params![id,cwd,bound.then(|| task.as_str()),bound.then_some(wave.as_str()),complete.then_some(2)]).unwrap();
        }
        for id in ["manual", "conversation", "history", "sibling", "wildcard"] {
            loopflow_storage::sqlite::sessions::test_capture(
                &conn,
                id,
                &loopflow_storage::session_record::new_artifact_key(),
            );
        }
    }
    // A Graph is the Task's work when it ran in the checkout.
    let independent = store.test_graph(
        "independent",
        "/missing/task%_/sub",
        &[("implement", None)],
        None,
    );
    store.test_graph("unrelated", "/other", &[("implement", None)], None);
    let work = crate::store::task_work::task_work(&store, &task).unwrap();
    assert_eq!(
        work.sessions
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        ["conversation", "history", "manual"]
    );
    assert_eq!(
        work.graph_processes
            .iter()
            .map(|f| f.summary.id.as_str())
            .collect::<Vec<_>>(),
        [independent.as_str()]
    );
    assert_eq!(
        work.graph_processes[0].summary.task_id.as_ref(),
        Some(&task)
    );
    // Open work is the Task's unended Graph runs and live agents, never a
    // Process row: the Graph still runs, no Session's agent started.
    let open: Vec<_> = store
        .task_open_work(&task)
        .unwrap()
        .iter()
        .map(|work| work.label())
        .collect();
    assert_eq!(open, [format!("Graph run {independent}")]);
    assert_eq!(
        store.session_task_ids("manual").unwrap(),
        std::slice::from_ref(&task)
    );
    assert!(store.session_task_ids("sibling").unwrap().is_empty());
    let summaries = crate::store::session_projection::session_summaries(
        &store,
        &SessionFilter {
            task: Some("PROOF-1".into()),
            headless: true,
            ..Default::default()
        },
        0,
    )
    .unwrap();
    assert_eq!(
        summaries.len(),
        work.sessions
            .iter()
            .filter(|session| session.completed_at.is_none())
            .count()
    );
    assert!(summaries
        .iter()
        .all(|session| session.task_ids == std::slice::from_ref(&task)));
    let graphs = store
        .graph_inventory(
            &GraphProcessFilter {
                task_id: Some(task.clone()),
                ..Default::default()
            },
            None,
            NonZeroU32::new(100).unwrap(),
            loopflow_storage::graph_process::Execution::result,
        )
        .unwrap();
    assert_eq!(graphs.entries, work.graph_processes);

    let child = TaskId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'child','PROOF-2',1)",params![project.as_str(),child.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/missing/task%_/child')",
            params![child.as_str()],
        )
        .unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('child-session','Child','human',1,0,'/missing/task%_/child',?1,?2)", params![child.as_str(), wave]).unwrap();
    }
    // A Graph in a nested checkout started for the deepest Task, as its
    // launch binds it; the enclosing Task keeps the nested Session.
    let child_graph = store.test_graph(
        "child",
        "/missing/task%_/child",
        &[("implement", None)],
        None,
    );
    let graph_ids = |task: &TaskId| {
        crate::store::task_work::task_work(&store, task)
            .unwrap()
            .graph_processes
            .into_iter()
            .map(|graph| graph.summary.id)
            .collect::<Vec<_>>()
    };
    let enclosing = graph_ids(&task);
    assert!(!enclosing.contains(&child_graph.to_string()));
    assert_eq!(graph_ids(&child), [child_graph.to_string()]);
    let work = crate::store::task_work::task_work(&store, &task).unwrap();
    assert!(work
        .sessions
        .iter()
        .any(|session| session.id == "child-session"));
    let memberships = store.session_task_ids("child-session").unwrap();
    assert!(memberships.contains(&task) && memberships.contains(&child));
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM task_checkouts WHERE task_id=?1",
            [task.as_str()],
        )
        .unwrap();
    }
    // Without its checkout the Task keeps the Graphs started for it.
    let work = crate::store::task_work::task_work(&store, &task).unwrap();
    assert_eq!(
        work.sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        ["history"]
    );
    assert_eq!(graph_ids(&task), enclosing);

    // A Session that later names a taskless Graph does not make it the
    // Task's: the Graph's Task is what its launch recorded.
    let taskless = store.test_graph("taskless", "/elsewhere", &[], None);
    store.execute_history(
        "INSERT INTO session_history(session_id,receipt_key,lf_process_id,observed_at,payload) VALUES('history','taskless-graph',?1,3,'{}')", [&taskless]).unwrap();
    assert_eq!(graph_ids(&task), enclosing);
    let (_, entry) = store.graph_process(taskless.as_str()).unwrap().unwrap();
    assert_eq!(entry.summary.task_id, None);
}
