use time::OffsetDateTime;

use super::SqliteStore;
use crate::durable::MachineId;
use crate::rental::{Hardware, OnDemand, Ownership, Provider, Quote, RentalState, Template, Usd};

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
        provider: Provider::Runpod,
        template: Template {
            gpu: None,
            vcpus: 8,
            memory_gb: 16,
            disk_gb: None,
            image: "runpod/base@sha256:ab".into(),
            data_center: None,
        },
        max_machines: max,
        max_usd_per_hour: usd("0.40"),
        hours: 4.0,
        created_at: OffsetDateTime::from_unix_timestamp(1_792_000_000).unwrap(),
    }
}

fn quote() -> Quote {
    Quote {
        hardware: Hardware::Cpu {
            flavor: "cpu3c".into(),
            vcpus: 8,
        },
        usd_per_hour: usd("0.24"),
        available: true,
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
    let rental = store
        .admit_rental("code", &quote(), None, None, false)
        .unwrap();
    assert_eq!(rental.state, RentalState::Requested);
    assert_eq!(rental.provider_name, format!("lf-{}", rental.id));
    assert_eq!(rental.cap_usd, usd("0.96"));
    let refused = store
        .admit_rental("code", &quote(), None, None, true)
        .unwrap_err();
    assert!(refused.to_string().contains("already has 1 of 1"));

    // A lost create stops counting against the limit; its history stays.
    store
        .set_rental_state(&rental.id, RentalState::Lost)
        .unwrap();
    store
        .admit_rental("code", &quote(), None, None, true)
        .unwrap();
    assert_eq!(store.rentals(Some("code")).unwrap().len(), 2);
}

#[test]
fn a_rental_moves_from_confirmed_to_running_to_ended() {
    let (_dir, store) = store();
    store.add_ondemand(&code(2)).unwrap();
    let rental = store
        .admit_rental("code", &quote(), None, None, false)
        .unwrap();
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
    let ended = store
        .set_rental_state(&rental.id, RentalState::Ended)
        .unwrap();
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
