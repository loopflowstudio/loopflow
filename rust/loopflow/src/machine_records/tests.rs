use std::path::Path;

use rusqlite::params;

use super::{copies_home, export, import, CopyCounts};
use crate::durable::{MachineId, TaskId};
use crate::store::sqlite::SqliteStore;
use crate::work::task::NewTask;

const TASK: &str = "task_abcd1234400080000000000000000001";
const SESSION: &str = "8f0e2b7c4d1a4e3f9b6c5a4d3e2f1a0b";
const PROCESS: &str = "1b2c3d4e-5f60-4718-8293-a4b5c6d7e8f9";
const ARTIFACT: &str = "64f34281b74d4594ac53ec251cb05619";

fn with_task(home: &Path) -> SqliteStore {
    let store = SqliteStore::open_ephemeral(&home.join("db")).unwrap();
    let repo = home.canonicalize().unwrap();
    let repo = repo.to_str().unwrap();
    let project = store.ensure_wave_project(repo, "inbox").unwrap();
    store
        .create_task(&NewTask {
            repository_id: store.ensure_repository(repo).unwrap(),
            due_date: None,
            id: TaskId::parse(TASK).unwrap(),
            project_id: Some(project.id),
            title: "Rent a machine".into(),
            description: String::new(),
        })
        .unwrap();
    store
}

/// The worker: one unfinished Process, and a Session on the Task with a capture.
fn worker(home: &Path) -> SqliteStore {
    let store = with_task(home);
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO processes(id,trace_id,command,started_at) VALUES(?1,?1,'lf task run',100)",
        [PROCESS],
    )
    .unwrap();
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
    store
        .add_machine(origin, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    store
}

fn exported(store: &SqliteStore, home: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    export(store, home, 50, &mut out).unwrap();
    out
}

#[test]
fn copies_arrive_from_their_origin_without_becoming_local_rows() {
    let (worker_home, laptop_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let pod = worker(worker_home.path());
    let origin = pod.local_machine().unwrap().id;
    let here = laptop(laptop_home.path(), &origin);

    let counts = import(
        &here,
        laptop_home.path(),
        &origin,
        exported(&pod, worker_home.path()).as_slice(),
    )
    .unwrap();
    assert_eq!(
        counts,
        CopyCounts {
            sessions: 1,
            processes: 1
        }
    );

    let copied = here
        .copied_sessions(Some(&TaskId::parse(TASK).unwrap()), None)
        .unwrap();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].id, SESSION);
    assert_eq!(copied[0].title, "Rent pods");
    assert_eq!(copied[0].origin_name(), "code-1");

    // No local surface finds them: not as Processes to reconcile, not as Sessions to attach.
    let conn = here.conn.lock().unwrap();
    let local: i64 = conn
        .query_row(
            "SELECT (SELECT count(*) FROM processes WHERE id=?1)
                  + (SELECT count(*) FROM agent_sessions WHERE id=?2)",
            params![PROCESS, SESSION],
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
    let origin = pod.local_machine().unwrap().id;
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
    let copied = here.copied_sessions(None, None).unwrap();
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
        origin = pod.local_machine().unwrap().id;
        here = laptop(laptop_home.path(), &origin);
        import(
            &here,
            laptop_home.path(),
            &origin,
            exported(&pod, worker_home.path()).as_slice(),
        )
        .unwrap();
    }
    here.remove_machine("code-1").unwrap();
    let copied = here.copied_sessions(None, None).unwrap();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].origin_name(), origin.as_str());
    assert_eq!(here.copy_counts(&origin).unwrap().processes, 1);
}

#[test]
fn a_copied_session_opens_after_its_origin_is_gone() {
    let laptop_home = tempfile::tempdir().unwrap();
    let origin;
    let here;
    {
        let worker_home = tempfile::tempdir().unwrap();
        let pod = worker(worker_home.path());
        origin = pod.local_machine().unwrap().id;
        here = laptop(laptop_home.path(), &origin);
        import(
            &here,
            laptop_home.path(),
            &origin,
            exported(&pod, worker_home.path()).as_slice(),
        )
        .unwrap();
    }
    here.remove_machine("code-1").unwrap();
    assert!(here.copied_history("not-a-session").unwrap().is_none());

    let history = here.copied_history(SESSION).unwrap().unwrap();
    assert_eq!(history.task_id.as_deref(), Some(TASK));
    assert_eq!(history.events.len(), 1);
    assert_eq!(history.events[0].kind, "captured");
    let shown = history.render(laptop_home.path());
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
    assert!(here.copied_sessions(None, None).unwrap().is_empty());
}
