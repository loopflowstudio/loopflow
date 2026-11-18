use crate::store::retention::prune;
use rusqlite::Connection;

#[test]
fn prune_moves_a_headless_run_with_a_dead_agent_process_to_history() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let exited = std::process::Command::new("true")
        .spawn()
        .and_then(|mut child| child.wait().map(|_| child.id()))
        .unwrap();
    for (session, process, interactive) in [
        ("run", "00000000-0000-4000-8000-000000000001", false),
        ("conversation", "00000000-0000-4000-8000-000000000002", true),
    ] {
        store.test_session(
            session,
            &loopflow_storage::session_record::new_artifact_key(),
        );
        conn.execute(
            "UPDATE agent_sessions SET agent_process_id=?2,attachment_token=?2,interactive=?3,agent_started_at=?4,
                agent_pid=?5,agent_spawn_state='spawn_requested' WHERE id=?1",
            rusqlite::params![session, process, interactive, now, exited],
        )
        .unwrap();
    }

    assert_eq!(prune(&path, now).unwrap().ended_owners, 2);
    let completed: Vec<(String, Option<i64>)> = conn
        .prepare("SELECT id,completed_at FROM agent_sessions ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        completed,
        [("conversation".into(), None), ("run".into(), Some(now))]
    );
}
