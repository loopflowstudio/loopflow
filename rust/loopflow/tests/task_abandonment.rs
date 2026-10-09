mod support;

use std::process::Command;

use loopflow::id::ProcessLfid;
use loopflow::work::task::{GithubPr, PrPublication};
use loopflow_test_support::TestRepo;
use rusqlite::params;
use support::{register_unrun_task, EnvGuard};

#[test]
fn task_abandonment_cli_saves_offline_and_preserves_unresolved_work() {
    for connected in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let _env = EnvGuard::with_lf_home(&[], home.path());
        let repo = TestRepo::new();
        let mut registered =
            register_unrun_task(home.path(), repo.path(), "main", &repo.head_sha());
        std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
        std::fs::write(
            repo.path().join(".lf/config.yaml"),
            if connected {
                "pm:\n  provider: linear\n  linear_team: offline-fixture\n"
            } else {
                "{}\n"
            },
        )
        .unwrap();
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if !connected {
            db.execute("UPDATE tasks SET external_issue_id=NULL", [])
                .unwrap();
            db.execute("UPDATE projects SET external_project_id=NULL", [])
                .unwrap();
        }
        let process = ProcessLfid::new();
        db.execute(
            "INSERT INTO processes(lfid,trace_id,cwd,started_at) VALUES(?1,?1,?2,1)",
            params![process.as_str(), repo.path().to_str().unwrap()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,
                interactive,task_id,wave_id,cwd)
             VALUES('retained-session','retained','generated',1,0,0,?1,?2,?3)",
            params![
                registered.task.id.as_str(),
                registered.task.wave_id.as_str(),
                repo.path().to_str().unwrap()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
             VALUES('retained-session','captured',?1,1,'{}')",
            [uuid::Uuid::new_v4().simple().to_string()],
        )
        .unwrap();
        db.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id='retained-session'",
            [db.last_insert_rowid()],
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = &registered.store;
        let session = runtime
            .block_on(store.session("retained-session"))
            .unwrap()
            .unwrap();
        let unresolved = runtime.block_on(store.process(&process)).unwrap().unwrap();
        let authored = repo.path().join("unfinished.txt");
        std::fs::write(&authored, "Retain unfinished work\n").unwrap();
        registered.pr.publication = Some(PrPublication {
            requested_at: time::OffsetDateTime::now_utc(),
            presentation: None,
            github: Some(GithubPr {
                number: 1,
                url: "https://github.com/loopflowstudio/fixture/pull/1".into(),
                head_sha: None,
            }),
            merge: None,
        });
        runtime
            .block_on(registered.store.update_task_pr(&registered.pr))
            .unwrap();
        let prs = runtime
            .block_on(registered.store.task_prs(&registered.task.id))
            .unwrap();
        let mut receipt = None;
        for _ in 0..2 {
            let output = Command::new(env!("CARGO_BIN_EXE_lf"))
                .current_dir(repo.path())
                .env("LF_HOME", home.path())
                .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
                .args(["task", "abandon", registered.task.id.as_str(), "--json"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stderr).contains("retained checkout/PR"));
            let saved: (String, String, bool, bool) = db.query_row(
                "SELECT id,target,attempted,settled FROM task_state_deliveries WHERE task_id=?1",
                [registered.task.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            ).unwrap();
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM task_state_deliveries WHERE task_id=?1",
                    [registered.task.id.as_str()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
            assert_eq!(saved.1, "canceled");
            assert!(!saved.2 && !saved.3);
            if let Some(prior) = &receipt {
                assert_eq!(prior, &saved);
            }
            receipt = Some(saved);
            let abandoned: bool = db
                .query_row("SELECT abandoned_at IS NOT NULL FROM tasks", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(abandoned);
            assert_eq!(
                runtime.block_on(store.process(&process)).unwrap().unwrap(),
                unresolved
            );
            assert_eq!(
                runtime
                    .block_on(store.session("retained-session"))
                    .unwrap()
                    .unwrap(),
                session
            );
            assert_eq!(
                runtime
                    .block_on(registered.store.task_prs(&registered.task.id))
                    .unwrap(),
                prs
            );
            assert_eq!(
                std::fs::read_to_string(&authored).unwrap(),
                "Retain unfinished work\n"
            );
        }
    }
}
