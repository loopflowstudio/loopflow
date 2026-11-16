use crate::store::retention::prune;
use rusqlite::Connection;

#[test]
fn prune_closes_dead_processes_and_forgets_those_nothing_needs() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    crate::store::opening::open_ephemeral(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    crate::journal::set_test_machine_booted_at(Some(now - 3_600));
    let exited = std::process::Command::new("true")
        .spawn()
        .and_then(|mut child| child.wait().map(|_| child.id()))
        .unwrap();
    let own = std::process::id();
    let own_start = crate::journal::process_started_at(own).unwrap();
    let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
    let process = |n: u8, pid: Option<u32>, os: Option<i64>, at: i64, parent: Option<u8>| {
        conn.execute(
            "INSERT INTO processes(id,trace_id,pid,os_started_at,started_at,parent_lf_process_id)
             VALUES(?1,?1,?2,?3,?4,?5)",
            rusqlite::params![id(n), pid, os, at, parent.map(id)],
        )
        .unwrap();
    };
    // Crash leftovers: once closed, only the agent-issued one stays.
    process(1, None, None, now - 7_200, None);
    process(2, Some(exited), None, now, None);
    process(3, Some(own), own_start, now, None);
    process(9, Some(exited), None, now, None);
    conn.execute(
        "UPDATE processes SET via_agent=1 WHERE id IN (?1,?2)",
        [id(1), id(2)],
    )
    .unwrap();
    let finished = |n: u8, parent: Option<u8>| {
        process(n, None, None, now, parent);
        conn.execute(
            "UPDATE processes SET completed_at=?2,outcome='succeeded' WHERE id=?1",
            rusqlite::params![id(n), now],
        )
        .unwrap();
    };
    finished(4, None);
    finished(5, None);
    finished(6, Some(3));

    let report = prune(&path, now).unwrap();
    assert_eq!(
        (report.closed_processes, report.forgotten_processes),
        (3, 3)
    );
    let rows: Vec<(String, Option<i64>, Option<String>)> = conn
        .prepare("SELECT id,completed_at,outcome FROM processes ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [
            // A restart bounds the exit; neither exit was observed.
            (id(1), Some(now - 3_600), None),
            (id(2), Some(now), None),
            (id(3), None, None),
            (id(6), Some(now), Some("succeeded".into())),
        ]
    );
}
