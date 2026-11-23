use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::session::SessionFilter;
use rusqlite::params;

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
        // Only `a-task` recorded the Task; the others opened in its checkout
        // with a scope of their own.
        for (id, scope, wave_id, task_id, cwd) in [
            (
                "a-task",
                None,
                Some(wave.as_str()),
                Some(task.as_str()),
                "/repo/task/sub",
            ),
            ("b-repo", Some("repository"), None, None, "/repo/task"),
            (
                "c-wave",
                Some("wave"),
                Some(wave.as_str()),
                None,
                "/repo/task",
            ),
            ("d-orphan", None, None, None, "/repo/other"),
            (
                "e-wave-attribution",
                None,
                Some(wave.as_str()),
                None,
                "/repo/task",
            ),
        ] {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,wave_id,task_id,interactive) VALUES(?1,?1,'human',1,0,?2,?3,?4,?5,1)",params![id,cwd,scope,wave_id,task_id]).unwrap();
            loopflow_storage::sqlite::sessions::test_capture(
                &conn,
                id,
                &loopflow_storage::session_record::new_artifact_key(),
            );
        }
    }
    assert_eq!(store.session_task("a-task").unwrap(), Some(task));
    for id in ["b-repo", "c-wave", "d-orphan", "e-wave-attribution"] {
        assert!(store.session_task(id).unwrap().is_none());
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
