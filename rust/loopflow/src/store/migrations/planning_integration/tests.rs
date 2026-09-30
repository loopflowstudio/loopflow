use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{
    _applied_development_migrations, _migration_transaction, applied_versions,
    apply_installed_development_sqlite, apply_released_planning_fixture, product_schema,
    validate_foreign_keys, validate_installed_development_sqlite, AppliedDevelopmentMigration,
    MIGRATIONS,
};
use crate::build_info::migration_draft_manifest;

#[derive(Deserialize)]
struct History {
    revision: String,
    drafts: Vec<AppliedDevelopmentMigration>,
}

fn histories() -> Vec<History> {
    serde_json::from_str(include_str!("histories.json")).unwrap()
}

fn draft_sql(name: &str) -> String {
    if let Some(draft) = migration_draft_manifest().iter().find(|d| d.name == name) {
        return draft.sql.to_string();
    }
    let marker = format!("-- draft: {name}\n");
    let (_, body) = MIGRATIONS
        .iter()
        .find_map(|m| m.sql.split_once(&marker))
        .unwrap();
    format!(
        "{}\n",
        body.split("\n\n-- draft: ")
            .next()
            .unwrap()
            .trim_end_matches('\n')
    )
}

fn populated_history(history: Option<&History>) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    apply_released_planning_fixture(&conn);
    conn.execute_batch(
        "INSERT INTO waves(id,name,repo,created_at) VALUES('wave','infra','/repo',1);
         INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at)
             VALUES('project','wave','provider-project',1,2);
         INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,created_at,updated_at)
             VALUES('task','project','issue','FIX-1','/repo.task','task',3,4);
         UPDATE tasks SET issue_title='Retained review' WHERE id='task';
         INSERT INTO task_events(task_id,kind_json,created_at)
             VALUES('task','{\"kind\":\"started\"}',5);
         INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
             VALUES('pr','task',1,'task','jack/task','base',6,7);"
    ).unwrap();
    let mut invocation =
        crate::durable::test_flow_invocation("retained", 0, "review", Some("review"), true);
    invocation.id = "planning-review".to_string();
    conn.execute("INSERT INTO task_flow_positions(task_id,invocation_json,flow,step,node_id,human,
        session_run_id,ready_summary,step_index,iteration,position_version,worker_generation,updated_at)
        VALUES('task',?1,'retained','review','review',1,'run_11111111111111111111111111111111',
        'Saved review feedback',0,7,9,0,100)", [serde_json::to_string(&invocation).unwrap()]).unwrap();
    let project = json!({"id":"provider-project", "slug":"chapter", "name":"Chapter",
        "summary":"Retained plan", "metric_targets":[], "flows":{"recommended":"custom"},
        "krs":[], "initiative_ids":["initiative"], "team_ids":["team"]});
    let item = json!({"id":"issue", "identifier":"FIX-1", "url":null,
        "name":"Retained title", "description":"Retained notes", "rank":2,
        "completed":false, "state":"unstarted", "project_id":"provider-project",
        "project":"chapter", "team_id":"team", "assignee":null});
    conn.execute(
        "INSERT INTO pm_snapshots VALUES('wave','linear','initiative',42,?1)",
        [json!({"projects":[project.clone()],"items":[item]}).to_string()],
    )
    .unwrap();
    let mut predecessor = project;
    predecessor["id"] = json!("archived-project");
    conn.execute(
        "INSERT INTO wave_chapters VALUES('wave','chapter','provider-project',1,?1)",
        [json!({"phase":"complete","created_at":32,"predecessors":[predecessor]}).to_string()],
    )
    .unwrap();
    if let Some(history) = history {
        // Reproduce the original branch, including its SQL order and exact
        // receipts. The ordinary migrator performs the subsequent upgrade.
        _migration_transaction(&conn, |conn| {
            conn.execute_batch(
                "CREATE TABLE development_migrations (
                position INTEGER NOT NULL UNIQUE, id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL
            );",
            )?;
            for (position, draft) in history.drafts.iter().enumerate() {
                let sql = draft_sql(&draft.name);
                assert_eq!(
                    hex::encode(Sha256::digest(sql.as_bytes())),
                    draft.checksum,
                    "historical SQL changed: {}",
                    draft.name
                );
                conn.execute_batch(&sql)?;
                conn.execute(
                    "INSERT INTO development_migrations VALUES(?1,?2,?3,?4,17)",
                    (position as i64, &draft.id, &draft.name, &draft.checksum),
                )?;
            }
            Ok(())
        })
        .unwrap();
    }
    conn
}

fn assert_preserved(conn: &Connection, archive_known: bool) {
    let retained: (String, String, String, i64) = conn.query_row(
        "SELECT t.id,t.worktree,p.id,t.started_at FROM tasks t JOIN task_prs p ON p.task_id=t.id",
        [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(
        (&retained.0, &retained.1, &retained.2),
        (
            &"task".to_string(),
            &"/repo.task".to_string(),
            &"pr".to_string()
        )
    );
    assert!(retained.3 > 0);
    assert_eq!(
        conn.query_row(
            "SELECT created_at FROM task_events WHERE task_id='task'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        5
    );
    let planning: (String, String, i64) = conn
        .query_row(
            "SELECT json_extract(body,'$.flow'),json_extract(body,'$.status'),observed_at
         FROM pm_projects WHERE id='provider-project'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(planning, ("custom".into(), "started".into(), 42));
    let local: (String,String,i64) = conn.query_row(
        "SELECT id,flow,legacy_current FROM projects WHERE external_project_id='provider-project'", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(local, ("project".into(), "custom".into(), 1));
    assert_eq!(
        conn.query_row(
            "SELECT json_extract(body,'$.description') FROM pm_items WHERE id='issue'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Retained notes"
    );
    let review: (String, i64, i64) = conn.query_row(
        "SELECT historical_ready_summary,iteration,position_version FROM flow_sessions WHERE task_id='task'", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(review, ("Saved review feedback".into(), 7, 9));
    let identity: (String, String, String) = conn
        .query_row(
            "SELECT id,json_extract(invocation_json,'$.id'),historical_session_run_id
         FROM flow_sessions WHERE task_id='task'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        identity,
        (
            "planning-review".into(),
            "planning-review".into(),
            "run_11111111111111111111111111111111".into()
        )
    );
    assert_eq!(conn.query_row("SELECT count(*) FROM agent_sessions WHERE task_id='task' AND ready_summary='Saved review feedback'", [],
        |r| r.get::<_, i64>(0)).unwrap(),1);
    assert!(
        conn.query_row(
            "SELECT count(*) FROM session_events WHERE task_id='task' AND kind='captured'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap()
            > 0
    );
    if archive_known {
        let archive: (i64, i64, String, String) = conn.query_row(
            "SELECT archived,observed_at,json_extract(body,'$.flow'),json_extract(body,'$.status')
             FROM pm_projects WHERE id='archived-project'", [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(archive, (1, 32, "custom".into(), "completed".into()));
    } else {
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM pm_projects WHERE id='archived-project'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0,
            "absent historical receipts establish no archive fact"
        );
    }
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name IN ('pm_snapshots','wave_chapters')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    validate_foreign_keys(conn).unwrap();
}

fn snapshot(conn: &Connection) -> serde_json::Value {
    let mut tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let names = tables
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let rows: Vec<_> = names
        .into_iter()
        .map(|name| {
            let mut query = conn
                .prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))
                .unwrap();
            let columns = query.column_count();
            let mut rows = query
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| row.get::<_, rusqlite::types::Value>(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .map(|row| format!("{:?}", row.unwrap()))
                .collect::<Vec<_>>();
            rows.sort();
            (name, rows)
        })
        .collect();
    json!({"schema":product_schema(conn).unwrap(),"tables":rows})
}

#[test]
fn planning_integration_preserves_released_and_both_branch_histories() {
    let histories = histories();
    for history in [None, Some(&histories[0]), Some(&histories[1])] {
        let conn = populated_history(history);
        let original = _applied_development_migrations(&conn).unwrap();
        let started = conn
            .query_row("SELECT started_at FROM tasks", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten();
        apply_installed_development_sqlite(&conn, migration_draft_manifest()).unwrap();
        validate_installed_development_sqlite(&conn, migration_draft_manifest()).unwrap();
        // The execution branch had already discarded old chapter receipts;
        // no new archive fact may be inferred for that absent predecessor.
        assert_preserved(&conn, history.is_none_or(|h| h.drafts.len() == 3));
        if let Some(started) = started {
            assert_eq!(
                conn.query_row("SELECT started_at FROM tasks", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                started
            );
        }
        if !migration_draft_manifest().is_empty() {
            assert_eq!(
                &_applied_development_migrations(&conn).unwrap()[..original.len()],
                original
            );
            if !original.is_empty() {
                assert_eq!(
                    conn.query_row(
                        "SELECT count(*) FROM development_migrations WHERE applied_at=17",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                    original.len() as i64
                );
            }
        }
        let schema = product_schema(&conn).unwrap();
        let canonical = applied_versions(&conn).unwrap();
        apply_installed_development_sqlite(&conn, migration_draft_manifest()).unwrap();
        assert_eq!(product_schema(&conn).unwrap(), schema);
        assert_eq!(applied_versions(&conn).unwrap(), canonical);
    }
}

#[test]
fn planning_integration_rejects_changed_history_and_schema_without_writes() {
    for history in histories() {
        for mutation in [
            "UPDATE development_migrations SET checksum='changed' WHERE position=0",
            "UPDATE development_migrations SET id='changed' WHERE position=0",
            "UPDATE development_migrations SET position=position+100 WHERE position=0",
            "ALTER TABLE projects ADD COLUMN unrecorded TEXT",
        ] {
            let conn = populated_history(Some(&history));
            conn.execute_batch(mutation).unwrap();
            let before = snapshot(&conn);
            assert!(
                apply_installed_development_sqlite(&conn, migration_draft_manifest()).is_err(),
                "{}: {mutation}",
                history.revision
            );
            assert_eq!(snapshot(&conn), before);
        }
    }
}

#[test]
fn planning_integration_rolls_back_if_the_next_draft_fails() {
    let sql = "CREATE TABLE interrupted_upgrade(id TEXT); INSERT INTO missing_upgrade VALUES(1);";
    let mut drafts = migration_draft_manifest().to_vec();
    drafts.push(crate::build_info::MigrationDraft {
        id: "11111111111111111111111111111111",
        name: "interrupted_planning_upgrade",
        dependencies: &["finish_planning_integration"],
        sql,
        checksum: Box::leak(hex::encode(Sha256::digest(sql.as_bytes())).into_boxed_str()),
    });
    let histories = histories();
    for history in [None, Some(&histories[0]), Some(&histories[1])] {
        let conn = populated_history(history);
        let before = snapshot(&conn);
        let error = apply_installed_development_sqlite(&conn, &drafts).unwrap_err();
        assert!(error.to_string().contains("missing_upgrade"), "{error}");
        assert_eq!(snapshot(&conn), before);
        assert!(conn.is_autocommit());
    }
}
