use super::SqliteStore;
use crate::rental::{Hardware, Ownership, Provider, Usd};
use loopflow_model::durable::{MachineId, TaskId};

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
    (dir, store)
}

fn usd(text: &str) -> Usd {
    Usd::parse(text).unwrap()
}

#[test]
fn hand_back_writes_nothing_when_a_claim_lacks_planning_facts() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
    let repo = crate::test_fixture::repository(&store, "/repo");
    let machine = MachineId::new();
    let first = TaskId::new();
    let second = TaskId::new();
    store.register_machine(&machine).unwrap();
    let mut conn = store.conn.lock().unwrap();
    let claim = |task: &TaskId| {
        conn.execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES(?1,?2,?1,1)",
            rusqlite::params![task.as_str(), repo.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_ownership(id,task_id,kind,machine_id,writer_machine_id,at,written_here)
             VALUES(?1,?2,'claim',?3,?3,1,0)",
            rusqlite::params![
                loopflow_model::durable::OwnershipEntryId::new().as_str(),
                task.as_str(),
                machine.as_str(),
            ],
        )
        .unwrap();
    };
    claim(&first);
    let scope = super::super::task_ownership::held_planning_scope_in(&conn, &machine).unwrap();
    let planning = std::collections::HashMap::from([(first.clone(), false)]);
    claim(&second);
    let current_scope =
        super::super::task_ownership::held_planning_scope_in(&conn, &machine).unwrap();
    for scope in [&scope, &current_scope] {
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        assert_eq!(
            super::super::task_ownership::hand_back_in(&tx, &machine, scope, &planning).unwrap(),
            None
        );
        tx.commit().unwrap();
    }
    for task in [&first, &second] {
        let log = super::super::task_ownership::log_in(&conn, task).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].machine_id, Some(machine.clone()));
    }
}

#[test]
fn hardware_rented_before_spot_existed_reads_as_on_demand() {
    use crate::migration_runner::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "rental_spot",
    );
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/rental_spot_released.sql"
    )))
    .unwrap();
    conn.execute_batch(&migration_section_sql("rental_spot"))
        .unwrap();
    for sql in [
        "SELECT template FROM ondemands",
        "SELECT hardware FROM rentals",
    ] {
        let json: String = conn.query_row(sql, [], |row| row.get(0)).unwrap();
        let hardware: Hardware = serde_json::from_str(&json).unwrap();
        assert!(!hardware.spot, "{sql}");
    }
}

#[test]
fn runpod_rows_from_0_14_16_go_and_current_ones_stay() {
    use crate::migration_runner::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "scope_tasks_capacity",
    );
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/scope_tasks_capacity_released.sql"
    )))
    .unwrap();
    conn.execute_batch(&migration_section_sql("scope_tasks_capacity"))
        .unwrap();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM ondemands ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(names, ["gpu"]);
    let rentals: i64 = conn
        .query_row("SELECT count(*) FROM rentals", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rentals, 0);
}

#[test]
fn fly_rows_from_0_14_17_go_and_the_rest_still_read() {
    use crate::migration_runner::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "forget_fly_rentals",
    );
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/forget_fly_rentals_released.sql"
    )))
    .unwrap();
    conn.execute_batch(&migration_section_sql("forget_fly_rentals"))
        .unwrap();
    let providers: Vec<String> = conn
        .prepare("SELECT provider FROM ondemands UNION ALL SELECT provider FROM rentals")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(providers, ["daytona"]);
    assert!(Provider::parse(&providers[0]).is_some());
    let capacity: Option<String> = conn
        .query_row("SELECT tasks_capacity FROM scope_defaults", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(capacity, None);
}

#[test]
fn an_owned_machines_pool_cost_and_expiry_move_to_pool_machines() {
    use crate::migration_runner::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "task_machine_local",
    );
    conn.execute_batch(
        "INSERT INTO pools(name,created_at) VALUES('lab',1);
         INSERT INTO machines(id,is_local,created_at,observed_at,pool,cost_usd,expires_at)
         VALUES('mch_owned',0,1,1,'lab','12.50',99);
         INSERT INTO machines(id,is_local,created_at,observed_at) VALUES('mch_rented',0,1,1);",
    )
    .unwrap();
    conn.execute_batch(&migration_section_sql("task_machine_local"))
        .unwrap();
    type Row = (String, Option<String>, Option<String>, Option<i64>);
    let rows: Vec<Row> = conn
        .prepare("SELECT machine_id,pool,cost_usd,expires_at FROM pool_machines")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [(
            "mch_owned".to_string(),
            Some("lab".to_string()),
            Some("12.50".to_string()),
            Some(99)
        )]
    );
}

#[test]
fn removing_a_pool_keeps_the_machines_cost_and_expiry() {
    let (_dir, store) = store();
    let machine = MachineId::new();
    store.register_machine(&machine).unwrap();
    let ownership = Ownership {
        pool: Some("lab".into()),
        cost_usd: usd("599"),
        expires_at: Some(time::OffsetDateTime::from_unix_timestamp(1_900_000_000).unwrap()),
    };
    store
        .set_machine_ownership_prepared(&machine, &ownership)
        .unwrap();
    store.clear_pool_machines("lab").unwrap();
    assert_eq!(
        super::ownership_in(&store.conn.lock().unwrap(), &machine).unwrap(),
        Ownership {
            pool: None,
            ..ownership
        }
    );
}
