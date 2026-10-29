use time::OffsetDateTime;

use super::SqliteStore;
use crate::durable::{MachineId, OwnershipKind, TaskId};
use crate::rental::{Hardware, OnDemand, Ownership, Provider, RentalState, Usd};
use crate::work::task::{LinearWritebackState, Observation, Task};

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
    (dir, store)
}

fn usd(text: &str) -> Usd {
    Usd::parse(text).unwrap()
}

fn code(max: u32) -> OnDemand {
    OnDemand {
        name: "code".into(),
        provider: Provider::Daytona,
        hardware: Hardware {
            vcpus: 4,
            memory_gb: 8,
            disk_gb: 10,
            gpu: None,
            spot: false,
        },
        max_machines: max,
        max_usd_per_hour: usd("0.40"),
        hours: 4.0,
        created_at: OffsetDateTime::from_unix_timestamp(1_792_000_000).unwrap(),
    }
}

#[test]
fn pools_and_ondemands_share_one_namespace() {
    let (_dir, store) = store();
    store.add_pool("lab").unwrap();
    store.add_ondemand(&code(1)).unwrap();
    assert_eq!(store.ondemand("code").unwrap(), Some(code(1)));
    assert!(store.add_pool("code").is_err());
    let mut lab = code(1);
    lab.name = "lab".into();
    assert!(store.add_ondemand(&lab).is_err());
}

#[test]
fn admission_records_identity_before_the_create_and_holds_the_limit() {
    let (_dir, store) = store();
    store.add_ondemand(&code(1)).unwrap();
    let rental = store.admit_rental("code", None, None, false).unwrap();
    assert_eq!(rental.state, RentalState::Requested);
    assert_eq!(rental.provider_name, format!("lf-{}", rental.id));
    assert_eq!(rental.cap_usd, usd("1.32696"));
    let refused = store.admit_rental("code", None, None, true).unwrap_err();
    assert!(refused.to_string().contains("already has 1 of 1"));

    // A lost create stops counting against the limit; its history stays.
    store
        .set_rental_state(&rental.id, RentalState::Lost)
        .unwrap();
    store.admit_rental("code", None, None, true).unwrap();
    assert_eq!(store.rentals(Some("code")).unwrap().len(), 2);
}

#[test]
fn a_rental_moves_from_confirmed_to_running_to_ended() {
    let (_dir, store) = store();
    store.add_ondemand(&code(2)).unwrap();
    let rental = store.admit_rental("code", None, None, false).unwrap();
    store
        .set_rental_state(&rental.id, RentalState::Unconfirmed)
        .unwrap();
    let booting = store
        .confirm_rental(&rental.id, "pod1", usd("0.25"))
        .unwrap();
    assert_eq!(booting.state, RentalState::Booting);
    assert_eq!(booting.usd_per_hour, usd("0.25"));
    assert!(store
        .confirm_rental(&rental.id, "pod2", usd("0.25"))
        .is_err());

    let machine = MachineId::new();
    store
        .add_machine(&machine, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    let running = store.attach_rental(&rental.id, &machine).unwrap();
    assert_eq!(running.state, RentalState::Running);
    assert_eq!(
        store.rental_for_machine(&machine).unwrap().unwrap().id,
        rental.id
    );
    // A rented machine is never pooled.
    store.add_pool("lab").unwrap();
    let pooled = Ownership {
        pool: Some("lab".into()),
        cost_usd: Usd::ZERO,
        expires_at: None,
    };
    assert!(store.set_machine_ownership(&machine, &pooled).is_err());

    assert!(store.remove_ondemand("code").is_err());
    store.set_rental_cost(&rental.id, usd("0.25")).unwrap();
    let (ended, returned) = store
        .set_rental_state(&rental.id, RentalState::Ended)
        .unwrap();
    assert!(returned.is_empty());
    assert!(ended.ended_at.is_some());
    store.remove_ondemand("code").unwrap();
    assert_eq!(store.rentals(None).unwrap()[0].cost_usd, Some(usd("0.25")));
}

#[test]
fn an_owned_machine_joins_a_pool_with_its_cost_and_leaves_when_the_pool_goes() {
    let (_dir, store) = store();
    let mini = MachineId::new();
    store.add_machine(&mini, "jack@mini", "mini").unwrap();
    let ownership = Ownership {
        pool: Some("lab".into()),
        cost_usd: usd("599"),
        expires_at: Some(OffsetDateTime::from_unix_timestamp(1_900_000_000).unwrap()),
    };
    assert!(store.set_machine_ownership(&mini, &ownership).is_err());
    store.add_pool("lab").unwrap();
    store.set_machine_ownership(&mini, &ownership).unwrap();

    let machines = store.pool_machines("lab").unwrap();
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].ownership, ownership);
    assert!(machines[0].tasks.is_empty());

    store.remove_pool("lab").unwrap();
    let left = super::ownership_in(&store.conn.lock().unwrap(), &mini).unwrap();
    assert_eq!(left.pool, None);
    assert_eq!(left.cost_usd, usd("599"));
}

fn open_task(store: &SqliteStore) -> TaskId {
    let now = OffsetDateTime::now_utc();
    let task = Task {
        id: TaskId::new(),
        revision: 0,
        identifier: "LOO-570".into(),
        title: "Rent a machine".into(),
        description: String::new(),
        rank: 0,
        assignee: None,
        due_date: None,
        state: None,
        completed_at: None,
        linear_id: None,
        pm_snapshot_synced_at: None,
        pm_writeback: LinearWritebackState::Current,
        wave_id: None,
        repository_id: store.ensure_repository("/repo").unwrap(),
        project_id: None,
        worktree: None,
        workspace_slug: String::new(),
        branch: "jack/rent".into(),
        base_commit: "deadbeef".into(),
        parent_pr_id: None,
        agent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    store.seed_unplaced_task(&task);
    task.id
}

#[test]
fn an_ended_rental_hands_its_open_tasks_back_here() {
    // Claiming reads the global Git identity, which isolating tests replace.
    let _env = crate::journal::test_env_lock();
    let (_dir, store) = store();
    store.add_ondemand(&code(1)).unwrap();
    let rental = store.admit_rental("code", None, None, false).unwrap();
    store
        .confirm_rental(&rental.id, "pod1", usd("0.24"))
        .unwrap();
    let code_1 = MachineId::new();
    store
        .add_machine(&code_1, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    store.attach_rental(&rental.id, &code_1).unwrap();
    let task = open_task(&store);
    store.claim_task(&task).unwrap();
    store.handoff_task(&task, &code_1).unwrap();
    let person = store.task_ownership(&task).unwrap().owner.unwrap().person;

    let (_, returned) = store
        .set_rental_state(&rental.id, RentalState::Ended)
        .unwrap();

    assert_eq!(returned, std::slice::from_ref(&task));
    let ownership = store.task_ownership(&task).unwrap();
    let owner = ownership.owner.unwrap();
    let local = store.local_machine().unwrap().id;
    assert_eq!(owner.machine.id, Some(local.clone()));
    assert_eq!(owner.person, person);
    let history: Vec<_> = ownership
        .log
        .iter()
        .map(|entry| (entry.kind, entry.machine_id.clone().unwrap()))
        .collect();
    assert_eq!(
        history,
        [
            (OwnershipKind::Claim, local.clone()),
            (OwnershipKind::Handoff, code_1),
            (OwnershipKind::Handoff, local),
        ]
    );
    // This Machine writes the Task again: `lf task run` stays here.
    store.unclaim_task(&task).unwrap();
}

#[test]
fn hardware_rented_before_spot_existed_reads_as_on_demand() {
    use crate::store::migrations::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "rental_spot");
    conn.execute_batch(include_str!(
        "../../../tests/fixtures/migrations/rental_spot_released.sql"
    ))
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
    use crate::store::migrations::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "scope_tasks_capacity");
    conn.execute_batch(include_str!(
        "../../../tests/fixtures/migrations/scope_tasks_capacity_released.sql"
    ))
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
    use crate::store::migrations::{apply_before_migration, migration_section_sql};

    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "forget_fly_rentals");
    conn.execute_batch(include_str!(
        "../../../tests/fixtures/migrations/forget_fly_rentals_released.sql"
    ))
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
