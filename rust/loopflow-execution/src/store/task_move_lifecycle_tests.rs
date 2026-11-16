use loopflow_model::durable::{MachineId, TaskId};
use loopflow_storage::sqlite::task_moves::MoveStage;
use loopflow_storage::work::task::NewTask;

#[test]
fn a_sent_move_stays_until_it_arrives_and_then_keeps_no_payload() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    let repo = dir.path().canonicalize().unwrap();
    let task = store
        .create_task(&NewTask {
            id: TaskId::new(),
            repository_id: crate::repository::location::ensure_repository(
                &store,
                repo.to_str().unwrap(),
            )
            .unwrap(),
            project_id: None,
            title: "Moving".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap()
        .id;
    let (laptop, mini) = (
        crate::store::machines::local_machine(&store).unwrap().id,
        MachineId::new(),
    );
    assert_eq!(store.task_move_in_flight(&task).unwrap(), None);

    // Leaving, then sent: still one move, as old as when it began.
    crate::store::ownership::begin_task_move(&store, &task, &mini).unwrap();
    let began = 1_000;
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE task_moves SET at=?1", [began])
        .unwrap();
    store.record_move_sent(&task, "{\"first\":1}").unwrap();
    let sent = store.task_move_in_flight(&task).unwrap().unwrap();
    assert_eq!(sent.to_machine_id, mini);
    assert_eq!(sent.at, began);
    assert_eq!(
        sent.stage,
        MoveStage::Sent {
            payload: "{\"first\":1}".into()
        }
    );

    store.record_move_arrived(&task, &laptop, &mini).unwrap();
    assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
    let conn = store.conn.lock().unwrap();
    let rows: Vec<(String, Option<String>)> = conn
        .prepare("SELECT state,payload FROM task_moves WHERE task_id=?1")
        .unwrap()
        .query_map([task.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, [("arrived".to_string(), None)]);
    drop(conn);
    assert!(store.task_move_arrived_on(&task, &mini).unwrap());

    // Sent on to a third Machine, whose arrival this one never hears of,
    // then back here: nothing is in flight any more.
    let third = MachineId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO task_moves(task_id,from_machine_id,to_machine_id,state,at,claim_id,payload)
             VALUES(?1,?2,?3,'sent',1,'claim','{}')",
            [task.as_str(), laptop.as_str(), third.as_str()],
        )
        .unwrap();
    assert!(!store.task_move_arrived_on(&task, &third).unwrap());
    store.record_move_arrived(&task, &third, &laptop).unwrap();
    assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
    assert!(store.task_move_arrived_on(&task, &laptop).unwrap());
}
