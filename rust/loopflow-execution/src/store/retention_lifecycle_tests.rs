use crate::store::retention::prune;
use loopflow_storage::execution::TaskState;
use loopflow_storage::graph_process::ExecutionResult;
use rusqlite::{params, Connection};

/// A pid that held a process a moment ago and holds none now.
fn exited_pid() -> u32 {
    let mut child = std::process::Command::new("true").spawn().unwrap();
    child.wait().unwrap();
    child.id()
}

/// A Graph runner and a Task carrier killed without writing their ends: the
/// next pass ends the run with no exit, so it reads `unknown`, and clears
/// the carrier, so its Task stops reading moving and its run reads died. Live and pre-boot work is
/// told apart by its own record.
#[test]
fn a_killed_runner_ends_unknown_and_its_task_stops_moving() {
    let (_home, store, task) = loopflow_storage::test_fixture::store_with_task();
    let path = store.path().unwrap();
    let conn = Connection::open(&path).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    crate::process::os_process::set_test_machine_booted_at(Some(now - 3_600));
    let own = std::process::id();
    let own_start = crate::process::os_process::process_started_at(own)
        .unwrap()
        .unwrap();
    let dead = exited_pid();
    let run = |id: &str, pid: Option<u32>, os: Option<i64>, at: i64| {
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id,started_at,pid,os_started_at)
             VALUES(?1,'code','{}',?2,?3,?4,?5)",
            params![id, task.as_str(), at, pid, os],
        )
        .unwrap();
    };
    let killed = "00000000-0000-4000-8000-000000000001";
    let live = "00000000-0000-4000-8000-000000000002";
    let before_boot = "00000000-0000-4000-8000-000000000003";
    run(killed, Some(dead), Some(now - 5), now - 5);
    run(live, Some(own), Some(own_start), now);
    run(before_boot, None, None, now - 7_200);
    let carrier = "00000000-0000-4000-8000-000000000004";
    conn.execute(
        "INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,run_pid,run_os_started_at,updated_at)
         VALUES(?1,'{\"name\":\"test\",\"steps\":[]}','start','{\"index\":0}',?2,?3,?4,1)",
        params![task.as_str(), carrier, dead, now - 5],
    )
    .unwrap();
    conn.execute("DELETE FROM graph_processes WHERE lf_process_id=?1", [live])
        .unwrap();
    assert_eq!(store.task_state(&task).unwrap(), TaskState::Moving);

    run(live, Some(own), Some(own_start), now);
    assert_eq!(prune(&path, now).unwrap().ended_owners, 3);

    let result = |id: &str| {
        conn.query_row(
            "SELECT ended_at,exit_code,error,pid FROM graph_processes WHERE lf_process_id=?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    ExecutionResult::of(
                        row.get(0)?,
                        row.get(1)?,
                        row.get::<_, Option<String>>(2)?.as_deref(),
                    ),
                    row.get::<_, Option<u32>>(3)?,
                ))
            },
        )
        .unwrap()
    };
    assert_eq!(result(killed), (Some(now), ExecutionResult::Unknown, None));
    // A restart bounds when it ended.
    assert_eq!(
        result(before_boot),
        (Some(now - 3_600), ExecutionResult::Unknown, None)
    );
    assert_eq!(result(live), (None, ExecutionResult::Running, Some(own)));
    let carrier_pid: Option<u32> = conn
        .query_row(
            "SELECT run_pid FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(carrier_pid, None);
    conn.execute("DELETE FROM graph_processes WHERE lf_process_id=?1", [live])
        .unwrap();
    assert_eq!(store.task_state(&task).unwrap(), TaskState::Idle);
    // Clearing the carrier kept that it died, so the recovery pass still
    // carries the run again.
    let died = crate::ops::task::run_recovery::died_run(&store, &task)
        .unwrap()
        .expect("a carrier retention ended died");
    assert_eq!(died.carrier.as_str(), carrier);
}
