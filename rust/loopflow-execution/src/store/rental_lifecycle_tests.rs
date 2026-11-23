//! Rental declarations, observed hand-back and stale-command recovery.
use loopflow_model::durable::{MachineId, OwnershipKind, TaskId};
use loopflow_storage::rental::{Hardware, OnDemand, Ownership, Provider, RentalState, Usd};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::{LinearWritebackState, Observation, Task};
use time::OffsetDateTime;
fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
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
            modal_sizing: None,
        },
        max_machines: max,
        max_usd_per_hour: usd("0.40"),
        hours: 4.0,
    }
}

#[test]
fn file_edits_change_admission_without_rewriting_live_rentals() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let (home, store) = store();
    let _home =
        crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
    crate::store::rentals::add_ondemand(&store, &code(1)).unwrap();
    let admitted = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    crate::engine::config::set(
        &crate::config::files::Destination::PersonalGlobal {},
        "ondemands.code.max_machines",
        "0",
    )
    .unwrap();
    assert_eq!(
        crate::store::rentals::ondemand(&store, "code")
            .unwrap()
            .unwrap()
            .max_machines,
        0
    );
    assert!(crate::store::rentals::admit_rental(&store, "code", None, None, false).is_err());
    assert_eq!(
        store.rental(&admitted.id).unwrap().cap_usd,
        admitted.cap_usd
    );
}

#[test]
fn pools_and_ondemands_share_one_namespace() {
    let (_dir, store) = store();
    crate::store::rentals::add_pool(&store, "lab.west").unwrap();
    crate::store::rentals::add_ondemand(&store, &code(1)).unwrap();
    assert_eq!(
        crate::store::rentals::ondemand(&store, "code").unwrap(),
        Some(code(1))
    );
    assert!(crate::store::rentals::add_pool(&store, "code").is_err());
    let mut lab = code(1);
    lab.name = "lab.west".into();
    assert!(crate::store::rentals::add_ondemand(&store, &lab).is_err());
    crate::store::rentals::set_pool_off(&store, "lab.west", true).unwrap();
    assert!(
        crate::store::rentals::pool(&store, "lab.west")
            .unwrap()
            .unwrap()
            .off
    );
    crate::store::rentals::remove_pool(&store, "lab.west").unwrap();
    assert!(crate::store::rentals::pool(&store, "lab.west")
        .unwrap()
        .is_none());
    assert!(crate::store::rentals::remove_pool(&store, "lab.west").is_err());
}

#[test]
fn admission_records_identity_before_the_create_and_holds_the_limit() {
    let (_dir, store) = store();
    crate::store::rentals::add_ondemand(&store, &code(1)).unwrap();
    let rental = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    assert_eq!(rental.state, RentalState::Requested);
    assert_eq!(rental.provider_name, format!("lf-{}", rental.id));
    assert_eq!(rental.cap_usd, usd("1.32696"));
    let refused =
        crate::store::rentals::admit_rental(&store, "code", None, None, true).unwrap_err();
    assert!(refused.to_string().contains("already has 1 of 1"));

    // A lost create stops counting against the limit; its history stays.
    crate::store::ownership::set_rental_state(&store, &rental.id, RentalState::Lost).unwrap();
    crate::store::rentals::admit_rental(&store, "code", None, None, true).unwrap();
    assert_eq!(store.rentals(Some("code")).unwrap().len(), 2);
}

#[test]
fn a_rental_moves_from_confirmed_to_running_to_ended() {
    let (_dir, store) = store();
    crate::store::rentals::add_ondemand(&store, &code(2)).unwrap();
    let rental = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    crate::store::ownership::set_rental_state(&store, &rental.id, RentalState::Unconfirmed)
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
    crate::store::machines::add_machine(&store, &machine, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    let running = store.attach_rental(&rental.id, &machine).unwrap();
    assert_eq!(running.state, RentalState::Running);
    assert_eq!(
        store.rental_for_machine(&machine).unwrap().unwrap().id,
        rental.id
    );
    // A rented machine is never pooled.
    crate::store::rentals::add_pool(&store, "lab").unwrap();
    let pooled = Ownership {
        pool: Some("lab".into()),
        cost_usd: Usd::ZERO,
        expires_at: None,
    };
    assert!(crate::store::rentals::set_machine_ownership(&store, &machine, &pooled).is_err());

    assert!(crate::store::rentals::remove_ondemand(&store, "code").is_err());
    store.set_rental_cost(&rental.id, usd("0.25")).unwrap();
    let (ended, returned) =
        crate::store::ownership::set_rental_state(&store, &rental.id, RentalState::Ended).unwrap();
    assert!(returned.is_empty());
    assert!(ended.ended_at.is_some());
    crate::store::rentals::remove_ondemand(&store, "code").unwrap();
    assert_eq!(
        store.rentals(None).unwrap()[0].cost,
        loopflow_storage::rental::RentalCost::CostUsd(Some(usd("0.25")))
    );
}

#[test]
fn an_owned_machine_joins_a_pool_with_its_cost_and_leaves_when_the_pool_goes() {
    let (_dir, store) = store();
    let mini = MachineId::new();
    crate::store::machines::add_machine(&store, &mini, "jack@mini", "mini").unwrap();
    let ownership = Ownership {
        pool: Some("lab".into()),
        cost_usd: usd("599"),
        expires_at: Some(OffsetDateTime::from_unix_timestamp(1_900_000_000).unwrap()),
    };
    assert!(crate::store::rentals::set_machine_ownership(&store, &mini, &ownership).is_err());
    crate::store::rentals::add_pool(&store, "lab").unwrap();
    crate::store::rentals::set_machine_ownership(&store, &mini, &ownership).unwrap();

    let machines = crate::store::machines::pool_machines(&store, "lab").unwrap();
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].ownership, ownership);
    assert!(machines[0].tasks.is_empty());

    crate::store::rentals::remove_pool(&store, "lab").unwrap();
    assert!(crate::store::machines::pool_machines(&store, "lab")
        .unwrap()
        .is_empty());
}

fn open_task(store: &SqliteStore) -> TaskId {
    let now = OffsetDateTime::now_utc();
    let task = Task {
        parent_task_id: None,
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
        tracker_id: None,
        tracker_synced_at: None,
        tracker_writeback: LinearWritebackState::Current,
        wave_id: None,
        repository_id: crate::repository::location::ensure_repository(store, "/repo").unwrap(),
        project_id: None,
        worktree: None,
        workspace_slug: String::new(),
        branch: "jack/rent".into(),
        base_commit: "deadbeef".into(),
        parent_pr_id: None,
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
    let _home =
        crate::config::environment::EnvVarGuard::set("LF_HOME", _dir.path().to_str().unwrap());
    crate::store::rentals::add_ondemand(&store, &code(1)).unwrap();
    let rental = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    store
        .confirm_rental(&rental.id, "pod1", usd("0.24"))
        .unwrap();
    let code_1 = MachineId::new();
    crate::store::machines::add_machine(&store, &code_1, "ssh://lf@203.0.113.7:40022", "code-1")
        .unwrap();
    store.attach_rental(&rental.id, &code_1).unwrap();
    let task = open_task(&store);
    crate::store::ownership::claim_task(&store, &task).unwrap();
    crate::store::ownership::move_claim(&store, &task, &code_1).unwrap();
    let person = crate::store::ownership::task_ownership(&store, &task)
        .unwrap()
        .owner
        .unwrap()
        .person;

    let (_, returned) =
        crate::store::ownership::set_rental_state(&store, &rental.id, RentalState::Ended).unwrap();

    assert_eq!(returned, std::slice::from_ref(&task));
    let ownership = crate::store::ownership::task_ownership(&store, &task).unwrap();
    let owner = ownership.owner.unwrap();
    let local = crate::store::machines::local_machine(&store).unwrap().id;
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
            (OwnershipKind::Claim, code_1),
            (OwnershipKind::Claim, local),
        ]
    );
    // This Machine writes the Task again: `lf task run` stays here.
    crate::store::ownership::unassign_task(&store, &task).unwrap();
}

#[test]
fn a_stale_rental_hand_back_reacquires_without_losing_claims() {
    let _env = crate::journal::test_env_lock();
    let (dir, store) = store();
    let _home =
        crate::config::environment::EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
    crate::store::rentals::add_ondemand(&store, &code(1)).unwrap();
    let rental = crate::store::rentals::admit_rental(&store, "code", None, None, false).unwrap();
    store
        .confirm_rental(&rental.id, "pod", usd("0.24"))
        .unwrap();
    let machine = MachineId::new();
    crate::store::machines::add_machine(&store, &machine, "lf@pod", "pod").unwrap();
    store.attach_rental(&rental.id, &machine).unwrap();
    let first = open_task(&store);
    crate::store::ownership::claim_task(&store, &first).unwrap();
    crate::store::ownership::move_claim(&store, &first, &machine).unwrap();
    let (scope, planning) = crate::store::ownership::held_planning(&store, &machine).unwrap();

    // Another writer claims a Task after preflight but before BEGIN.
    let second = open_task(&store);
    crate::store::ownership::claim_task(&store, &second).unwrap();
    crate::store::ownership::move_claim(&store, &second, &machine).unwrap();
    assert!(store
        .set_rental_state_prepared(&rental.id, RentalState::Ended, Some(&(scope, planning)))
        .unwrap()
        .is_none());
    assert!(store.rental(&rental.id).unwrap().state.is_live());
    for task in [&first, &second] {
        let ownership = crate::store::ownership::task_ownership(&store, task).unwrap();
        assert_eq!(ownership.owner.unwrap().machine.id, Some(machine.clone()));
        assert_eq!(ownership.log.len(), 2);
    }
    let (_, returned) =
        crate::store::ownership::set_rental_state(&store, &rental.id, RentalState::Ended).unwrap();
    assert_eq!(returned.len(), 2);
    assert!(returned.contains(&first));
    assert!(returned.contains(&second));
    let local = crate::store::machines::local_machine(&store).unwrap().id;
    for task in [&first, &second] {
        let ownership = crate::store::ownership::task_ownership(&store, task).unwrap();
        assert_eq!(ownership.owner.unwrap().machine.id, Some(local.clone()));
        assert_eq!(ownership.log.len(), 3);
    }
}
