use std::path::Path;

use rusqlite::params;

use super::{copies_home, export, import, CopyCounts};
use loopflow_model::durable::{MachineId, TaskId};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::NewTask;

const TASK: &str = "task_abcd1234400080000000000000000001";
const SESSION: &str = "8f0e2b7c4d1a4e3f9b6c5a4d3e2f1a0b";
const ARTIFACT: &str = "64f34281b74d4594ac53ec251cb05619";

fn with_task(home: &Path) -> SqliteStore {
    let store = crate::store::opening::open_ephemeral(&home.join("db")).unwrap();
    let repo = home.canonicalize().unwrap();
    let repo = repo.to_str().unwrap();
    let project = crate::test_support::seed_wave_project(&store, repo, "inbox").unwrap();
    store
        .create_task(&NewTask {
            parent_task_id: None,
            repository_id: crate::repository::location::ensure_repository(&store, repo).unwrap(),
            due_date: None,
            id: TaskId::parse(TASK).unwrap(),
            project_id: Some(project.id),
            title: "Rent a machine".into(),
            description: String::new(),
        })
        .unwrap();
    store
}

/// The worker: a Session on the Task with a capture.
fn worker(home: &Path) -> SqliteStore {
    let store = with_task(home);
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
         VALUES(?1,'Rent pods','human',100,0,'/home/lf/src/loopflow',?2,
                (SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?2))",
        params![SESSION, TASK],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
         VALUES(?1,'captured','r1',100,?2)",
        params![SESSION, format!(r#"{{"artifact_key":"{ARTIFACT}"}}"#)],
    )
    .unwrap();
    drop(conn);
    let capture = crate::session_record::record_dir(home, ARTIFACT).unwrap();
    std::fs::create_dir_all(&capture).unwrap();
    std::fs::write(capture.join("events.jsonl"), "{\"turn\":1}\n").unwrap();
    store
}

/// The laptop, which knows the worker as `code-1`.
fn laptop(home: &Path, origin: &MachineId) -> SqliteStore {
    let store = with_task(home);
    crate::store::machines::add_machine(&store, origin, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    store
}

fn exported(store: &SqliteStore, home: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    export(store, home, 50, None, &mut out).unwrap();
    out
}

#[test]
fn copies_arrive_from_their_origin_without_becoming_local_rows() {
    let (worker_home, laptop_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let pod = worker(worker_home.path());
    let origin = crate::store::machines::local_machine(&pod).unwrap().id;
    let here = laptop(laptop_home.path(), &origin);

    // A released exporter also wrote Process rows; the copy skips them.
    let mut export = exported(&pod, worker_home.path());
    export.extend_from_slice(b"{\"line\":\"row\",\"kind\":\"process\",\"key\":\"p\",\"row\":{}}\n");
    let counts = import(&here, laptop_home.path(), &origin, export.as_slice()).unwrap();
    assert_eq!(counts, CopyCounts { sessions: 1 });

    let copied =
        crate::store::machines::copied_sessions(&here, Some(&TaskId::parse(TASK).unwrap()), None)
            .unwrap();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].id, SESSION);
    assert_eq!(copied[0].title, "Rent pods");
    assert_eq!(copied[0].origin_name(), "code-1");
    assert_eq!(
        crate::store::machines::copied_history(&here, SESSION)
            .unwrap()
            .unwrap()
            .session
            .origin_name(),
        "code-1"
    );
    crate::store::machines::rename_machine(&here, "code-1", "builder").unwrap();
    assert_eq!(
        crate::store::machines::copied_sessions(&here, None, None).unwrap()[0].origin_name(),
        "builder"
    );
    assert_eq!(
        crate::store::machines::copied_history(&here, SESSION)
            .unwrap()
            .unwrap()
            .session
            .origin_name(),
        "builder"
    );

    // No local surface finds them as Sessions to attach.
    let conn = here.conn.lock().unwrap();
    let local: i64 = conn
        .query_row(
            "SELECT count(*) FROM agent_sessions WHERE id=?1",
            [SESSION],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(local, 0);
    drop(conn);

    let capture =
        crate::session_record::record_dir(&copies_home(laptop_home.path(), &origin), ARTIFACT)
            .unwrap();
    assert_eq!(
        std::fs::read_to_string(capture.join("events.jsonl")).unwrap(),
        "{\"turn\":1}\n"
    );
    assert!(
        crate::session_record::record_dir(laptop_home.path(), ARTIFACT)
            .unwrap()
            .metadata()
            .is_err()
    );
}

#[test]
fn copying_again_brings_rows_up_to_date() {
    let (worker_home, laptop_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let pod = worker(worker_home.path());
    let origin = crate::store::machines::local_machine(&pod).unwrap().id;
    let here = laptop(laptop_home.path(), &origin);
    import(
        &here,
        laptop_home.path(),
        &origin,
        exported(&pod, worker_home.path()).as_slice(),
    )
    .unwrap();
    pod.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET completed_at=200 WHERE id=?1",
            [SESSION],
        )
        .unwrap();
    let counts = import(
        &here,
        laptop_home.path(),
        &origin,
        exported(&pod, worker_home.path()).as_slice(),
    )
    .unwrap();
    assert_eq!(counts.sessions, 1);
    let copied = crate::store::machines::copied_sessions(&here, None, None).unwrap();
    assert_eq!(copied[0].completed_at, Some(200));
}

#[test]
fn copies_are_the_final_record_once_the_origin_is_gone() {
    let laptop_home = tempfile::tempdir().unwrap();
    let origin;
    let here;
    {
        let worker_home = tempfile::tempdir().unwrap();
        let pod = worker(worker_home.path());
        origin = crate::store::machines::local_machine(&pod).unwrap().id;
        here = laptop(laptop_home.path(), &origin);
        import(
            &here,
            laptop_home.path(),
            &origin,
            exported(&pod, worker_home.path()).as_slice(),
        )
        .unwrap();
    }
    crate::store::machines::remove_machine(&here, "code-1").unwrap();
    let copied = crate::store::machines::copied_sessions(&here, None, None).unwrap();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].origin_name(), origin.as_str());
    assert_eq!(here.copy_counts(&origin).unwrap().sessions, 1);
}

#[test]
fn a_copied_session_opens_after_its_origin_is_gone() {
    let laptop_home = tempfile::tempdir().unwrap();
    let origin;
    let here;
    {
        let worker_home = tempfile::tempdir().unwrap();
        let pod = worker(worker_home.path());
        origin = crate::store::machines::local_machine(&pod).unwrap().id;
        here = laptop(laptop_home.path(), &origin);
        import(
            &here,
            laptop_home.path(),
            &origin,
            exported(&pod, worker_home.path()).as_slice(),
        )
        .unwrap();
    }
    crate::store::machines::remove_machine(&here, "code-1").unwrap();
    assert!(
        crate::store::machines::copied_history(&here, "not-a-session")
            .unwrap()
            .is_none()
    );

    let history = crate::store::machines::copied_history(&here, SESSION)
        .unwrap()
        .unwrap();
    assert_eq!(history.task_id.as_deref(), Some(TASK));
    assert_eq!(history.events.len(), 1);
    assert_eq!(history.events[0].kind, "captured");
    let shown = super::render(&history, laptop_home.path());
    assert!(
        shown.contains(&format!("Session {SESSION} · from {origin}")),
        "{shown}"
    );
    assert!(shown.contains("Title: Rent pods"), "{shown}");
    let capture =
        crate::session_record::record_dir(&copies_home(laptop_home.path(), &origin), ARTIFACT)
            .unwrap();
    assert!(shown.contains(&capture.display().to_string()), "{shown}");
}

#[test]
fn an_export_from_another_machine_is_refused() {
    let (worker_home, laptop_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let pod = worker(worker_home.path());
    let expected = MachineId::new();
    let here = laptop(laptop_home.path(), &expected);
    let error = import(
        &here,
        laptop_home.path(),
        &expected,
        exported(&pod, worker_home.path()).as_slice(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("nothing was copied"), "{error}");
    assert!(crate::store::machines::copied_sessions(&here, None, None)
        .unwrap()
        .is_empty());
}

#[test]
fn resuming_an_export_reads_late_usage_without_resending_closed_sessions() {
    let home = tempfile::tempdir().unwrap();
    let pod = worker(home.path());
    let old = "ee47b779adc741ca89d6c3ae288cc68c";
    pod.test_session(old, "75f34281b74d4594ac53ec251cb05619");
    pod.conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET completed_at=100 WHERE id=?1",
            [old],
        )
        .unwrap();
    let before = pod.export_cursor().unwrap();
    pod.record_turn_start(SESSION, &"thread".into(), "late", 10);
    assert!(pod.record_turn_usage(
        SESSION,
        &"thread".into(),
        "late",
        11,
        &serde_json::json!({"usage":{"input_tokens":42}})
    ));
    let mut delta = Vec::new();
    export(&pod, home.path(), 50, Some(before), &mut delta).unwrap();
    let delta = String::from_utf8(delta).unwrap();
    assert!(delta.contains(SESSION) && delta.contains("\"input_tokens\":42"));
    assert!(!delta.contains(old));
}
