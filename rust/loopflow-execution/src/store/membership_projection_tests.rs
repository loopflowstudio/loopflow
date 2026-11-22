use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::session::SessionFilter;
use rusqlite::params;

#[test]
fn reverse_session_membership_keeps_lexical_checkout_and_binding_rules() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    let repository = loopflow_storage::test_fixture::repository(&store, "/repo");
    let roots = [
        Some("/"),
        Some("/repo"),
        Some("/repo/task///"),
        Some("/repo/task/nested"),
        Some("/repo//task"),
        Some("relative"),
        Some("relative//nested/"),
        Some("/literal%_"),
        Some(""),
        None,
        Some("/foreign"),
    ];
    let tasks: Vec<_> = roots.iter().map(|_| TaskId::new()).collect();
    {
        let conn = store.conn.lock().unwrap();
        for (index, (id, root)) in tasks.iter().zip(roots).enumerate() {
            conn.execute(
                "INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,created_at)
                 VALUES(?1,?2,?1,'Task',1)",
                params![id.as_str(), repository.as_str()],
            )
            .unwrap();
            // `/foreign` is checked out on another Machine: no row here.
            if let Some(root) = root.filter(|root| !root.is_empty() && index != 10) {
                conn.execute(
                    "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                    params![id.as_str(), root],
                )
                .unwrap();
            }
        }
    }
    let mut case = 0;
    for (cwd, expected) in [
        ("/repo/task/nested/file", vec![0, 1, 2, 3]),
        ("/repo/task///", vec![0, 1, 2]),
        ("/repo/task//nested/file", vec![0, 1, 2]),
        ("/repo//task/file", vec![0, 1, 4]),
        ("/repo/task-other", vec![0, 1]),
        ("/repository", vec![0]),
        ("relative//nested/file", vec![5, 6]),
        ("relative/", vec![5]),
        ("elsewhere", vec![]),
        ("", vec![0]),
        ("///", vec![0]),
        ("/literal%_/file", vec![0, 7]),
        ("/literalAB/file", vec![0]),
        ("/foreign/file", vec![0]),
    ] {
        for binding in [None, Some(2), Some(8), Some(9), Some(10)] {
            for scope in [None, Some("repository"), Some("wave")] {
                let session = format!("lexical-{case}");
                case += 1;
                {
                    let conn = store.conn.lock().unwrap();
                    conn.execute(
                        "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,primary_scope,interactive)
                         VALUES(?1,'Lexical','human',1,0,?2,?3,?4,1)",
                        params![session, cwd, binding.map(|index| tasks[index].as_str()), scope],
                    ).unwrap();
                    loopflow_storage::sqlite::sessions::test_capture(
                        &conn,
                        &session,
                        &loopflow_storage::session_record::new_artifact_key(),
                    );
                    conn.execute(
                        "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
                        params![tasks[2].as_str(), session],
                    )
                    .unwrap();
                }
                let mut ids: Vec<_> = if scope.is_none() {
                    expected
                        .iter()
                        .copied()
                        .chain(binding)
                        .map(|index| tasks[index].clone())
                        .collect()
                } else {
                    Vec::new()
                };
                ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                ids.dedup();
                assert_eq!(
                    store.session_task_ids(&session).unwrap(),
                    ids,
                    "{cwd:?}, {binding:?}, {scope:?}"
                );
                let summary =
                    crate::store::session_projection::session_summary(&store, &session, 0)
                        .unwrap()
                        .unwrap();
                assert_eq!(summary.task_ids, ids);
                assert!(summary.task_primary);
            }
        }
    }
    assert!(store.session_task_ids("missing").unwrap().is_empty());
}

#[test]
fn scoped_sessions_stay_out_of_tasks_and_orphan_pages_include_only_unassociated_sessions() {
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
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'issue','PROOF-1',1)",params![project.as_str(),task.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/task')",
            params![task.as_str()],
        )
        .unwrap();
        for (id, scope, wave_id, cwd) in [
            ("a-task", None, None, "/repo/task/sub"),
            ("b-repo", Some("repository"), None, "/repo/task"),
            ("c-wave", Some("wave"), Some(wave.as_str()), "/repo/task"),
            ("d-orphan", None, None, "/repo/other"),
            (
                "e-wave-attribution",
                None,
                Some(wave.as_str()),
                "/repo/task",
            ),
        ] {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,wave_id,interactive) VALUES(?1,?1,'human',1,0,?2,?3,?4,1)",params![id,cwd,scope,wave_id]).unwrap();
            loopflow_storage::sqlite::sessions::test_capture(
                &conn,
                id,
                &loopflow_storage::session_record::new_artifact_key(),
            );
        }
    }
    assert_eq!(
        store.session_task_ids("a-task").unwrap(),
        vec![task.clone()]
    );
    assert_eq!(
        store.session_task_ids("e-wave-attribution").unwrap(),
        vec![task]
    );
    for id in ["b-repo", "c-wave", "d-orphan"] {
        assert!(store.session_task_ids(id).unwrap().is_empty());
    }
    let mut filter = SessionFilter {
        orphan: true,
        limit: 1,
        after: Some(String::new()),
        ..Default::default()
    };
    // A Wave conversation has a `wave_id`, so it is not an orphan.
    for expected in ["b-repo", "d-orphan"] {
        let page = crate::store::session_projection::session_summaries(&store, &filter, 0).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, expected);
        filter.after = Some(page[0].id.to_string());
    }
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 0)
            .unwrap()
            .is_empty()
    );
}
