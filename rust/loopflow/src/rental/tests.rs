use time::OffsetDateTime;

use super::{
    admit, cpu_quote, free_machine, give_back, resolve_unconfirmed, CpuFlavor, GiveBack, Hardware,
    Holdings, OnDemand, Ownership, Pool, PoolMachine, PoolRefusal, Provider, Quote, Refusal,
    Rental, RentalId, RentalState, Resolution, Template, Usd,
};
use crate::durable::{Machine, MachineId, TaskId};

fn now() -> OffsetDateTime {
    OffsetDateTime::parse(
        "2026-10-21T20:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
}

fn usd(text: &str) -> Usd {
    Usd::parse(text).unwrap()
}

fn flavor(id: &str, per_vcpu: &str, ram: f64, available: bool) -> CpuFlavor {
    CpuFlavor {
        id: id.into(),
        min_vcpus: 2,
        max_vcpus: 32,
        ram_gb_per_vcpu: ram,
        usd_per_vcpu_hour: usd(per_vcpu),
        available,
    }
}

fn code(max: u32, max_per_hour: &str) -> OnDemand {
    OnDemand {
        name: "code".into(),
        provider: Provider::Runpod,
        template: Template {
            gpu: None,
            vcpus: 8,
            memory_gb: 16,
            disk_gb: None,
            image: "runpod/base@sha256:00".into(),
            data_center: None,
        },
        max_machines: max,
        max_usd_per_hour: usd(max_per_hour),
        hours: 4.0,
        created_at: now(),
    }
}

fn quote(price: &str, available: bool) -> Quote {
    Quote {
        hardware: Hardware::Cpu {
            flavor: "cpu3c".into(),
            vcpus: 8,
        },
        usd_per_hour: usd(price),
        available,
    }
}

#[test]
fn dollars_parse_exactly_and_print_as_people_read_them() {
    assert_eq!(usd("$0.40"), Usd::from_micros(400_000));
    assert_eq!(usd("0.03").times(8), usd("0.24"));
    assert_eq!(usd("3").to_string(), "$3.00");
    assert_eq!(usd("0.035").to_decimal(), "0.035");
    for bad in ["", "-1", "1.2.3", "0.0000001", "abc"] {
        assert!(Usd::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn cpu_quote_takes_the_cheapest_in_stock_power_of_two_covering_memory() {
    let flavors = [
        flavor("cpu3c", "0.03", 2.0, true),
        flavor("cpu3g", "0.04", 4.0, true),
        flavor("cpu5c", "0.02", 2.0, false),
    ];
    // 6 vCPUs round up to 8; cpu3c at 2 GB/vCPU covers 16 GB at 8.
    let quote = cpu_quote(&flavors, 6, 16).unwrap();
    assert_eq!(
        quote.hardware,
        Hardware::Cpu {
            flavor: "cpu3c".into(),
            vcpus: 8
        }
    );
    assert_eq!(quote.usd_per_hour, usd("0.24"));
    assert!(quote.available);
}

#[test]
fn cpu_quote_grows_for_memory_and_respects_the_flavor_maximum() {
    let mut small = flavor("cpu3c", "0.03", 2.0, true);
    small.max_vcpus = 8;
    let large = flavor("cpu3g", "0.04", 4.0, true);
    // 64 GB needs 32 vCPUs of cpu3c (over its max of 8) or 16 of cpu3g.
    let quote = cpu_quote(&[small.clone(), large], 2, 64).unwrap();
    assert_eq!(quote.hardware.to_string(), "cpu3g×16");
    assert_eq!(cpu_quote(&[small], 2, 64), None);
}

#[test]
fn admission_refuses_at_the_limit_out_of_stock_and_over_price() {
    assert!(matches!(
        admit(&code(1, "0.40"), 1, &quote("0.24", true), None, None, now()),
        Err(Refusal::AtLimit {
            live: 1,
            max: 1,
            ..
        })
    ));
    assert!(matches!(
        admit(&code(0, "0.40"), 0, &quote("0.24", true), None, None, now()),
        Err(Refusal::AtLimit { .. })
    ));
    assert!(matches!(
        admit(
            &code(1, "0.40"),
            0,
            &quote("0.24", false),
            None,
            None,
            now()
        ),
        Err(Refusal::Unavailable { .. })
    ));
    let refused = admit(&code(1, "0.20"), 0, &quote("0.24", true), None, None, now()).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "code: cpu3c×8 costs $0.24/h, above its $0.20/h limit"
    );
}

#[test]
fn admission_caps_spend_by_hours_or_a_lower_budget() {
    let admitted = admit(&code(1, "0.40"), 0, &quote("0.24", true), None, None, now()).unwrap();
    assert_eq!(admitted.cap_usd, usd("0.96"));
    assert_eq!(admitted.deadline, now() + time::Duration::hours(4));

    let capped = admit(
        &code(1, "0.40"),
        0,
        &quote("0.24", true),
        Some(2.0),
        Some(usd("0.12")),
        now(),
    )
    .unwrap();
    assert_eq!(capped.cap_usd, usd("0.12"));
    assert_eq!(capped.deadline, now() + time::Duration::minutes(30));
}

#[test]
fn an_unconfirmed_create_is_adopted_by_name_or_lost_after_its_deadline() {
    assert_eq!(
        resolve_unconfirmed(Some(7), now(), now()),
        Resolution::Adopt(7)
    );
    assert_eq!(
        resolve_unconfirmed::<u8>(None, now(), now() + time::Duration::minutes(1)),
        Resolution::Wait
    );
    assert_eq!(
        resolve_unconfirmed::<u8>(None, now(), now() + time::Duration::minutes(5)),
        Resolution::Lost
    );
}

fn owned(label: &str, expires_at: Option<OffsetDateTime>, tasks: &[&str]) -> PoolMachine {
    PoolMachine {
        machine: Machine {
            label: Some(label.into()),
            id: MachineId::new(),
            route: format!("jack@{label}"),
            created_at: now(),
            observed_at: now(),
        },
        ownership: Ownership {
            pool: Some("lab".into()),
            cost_usd: Usd::ZERO,
            expires_at,
        },
        tasks: tasks.iter().map(|t| TaskId::from_raw(*t)).collect(),
    }
}

fn lab(off: bool) -> Pool {
    Pool {
        name: "lab".into(),
        off,
        created_at: now(),
    }
}

#[test]
fn a_pool_places_work_on_its_first_free_unexpired_machine() {
    let machines = [
        owned("old", Some(now() - time::Duration::days(1)), &[]),
        owned("busy", None, &["task_00000000000000000000000000000001"]),
        owned("mini", None, &[]),
    ];
    let free = free_machine(&lab(false), &machines, now()).unwrap();
    assert_eq!(free.label(), "mini");
}

#[test]
fn a_pool_refuses_when_off_empty_or_busy_and_names_why() {
    assert!(matches!(
        free_machine(&lab(true), &[owned("mini", None, &[])], now()),
        Err(PoolRefusal::Off(_))
    ));
    assert!(matches!(
        free_machine(&lab(false), &[], now()),
        Err(PoolRefusal::Empty(_))
    ));
    let busy = [
        owned("old", Some(now()), &[]),
        owned("mini", None, &["task_00000000000000000000000000000001"]),
    ];
    let refusal = free_machine(&lab(false), &busy, now()).unwrap_err();
    assert_eq!(
        refusal.to_string(),
        "pool lab has no free machine: old expired, mini holds task_00000000000000000000000000000001"
    );
}

fn rented(by_hand: bool) -> Rental {
    let id = RentalId::new();
    Rental {
        provider_name: Rental::provider_name_for(&id),
        id,
        ondemand: "code".into(),
        provider: Provider::Runpod,
        provider_id: Some("pod1".into()),
        machine_id: Some(MachineId::new()),
        by_hand,
        hardware: quote("0.24", true).hardware,
        usd_per_hour: usd("0.24"),
        cap_usd: usd("0.96"),
        deadline: now() + time::Duration::hours(4),
        state: RentalState::Running,
        created_at: now(),
        ended_at: None,
        cost_usd: None,
    }
}

#[test]
fn a_task_rental_ends_when_its_work_drains() {
    let busy = Holdings {
        live_processes: 1,
        ..Holdings::default()
    };
    assert!(matches!(
        give_back(&rented(false), &busy, false, false, now()),
        GiveBack::Keep(_)
    ));
    // Before its handoff a Task rental is being prepared, not drained.
    assert!(matches!(
        give_back(&rented(false), &Holdings::default(), false, false, now()),
        GiveBack::Keep(_)
    ));
    let drained = Holdings {
        placed: true,
        ..Holdings::default()
    };
    assert_eq!(
        give_back(&rented(false), &drained, false, false, now()),
        GiveBack::End
    );
}

#[test]
fn a_rental_by_hand_lasts_until_its_soft_deadline_or_an_explicit_end() {
    let rental = rented(true);
    let idle = Holdings::default();
    assert!(matches!(
        give_back(&rental, &idle, false, false, now()),
        GiveBack::Keep(_)
    ));
    assert_eq!(
        give_back(&rental, &idle, false, false, rental.soft_deadline()),
        GiveBack::End
    );
    assert_eq!(give_back(&rental, &idle, true, false, now()), GiveBack::End);
}

#[test]
fn unpushed_work_refuses_every_give_back_unless_forced() {
    let holdings = Holdings {
        unpushed: vec!["~/src/loopflow 2 commit(s)".into()],
        ..Holdings::default()
    };
    let rental = rented(false);
    assert!(matches!(
        give_back(&rental, &holdings, true, false, rental.deadline),
        GiveBack::Refuse(_)
    ));
    assert_eq!(
        give_back(&rental, &holdings, true, true, now()),
        GiveBack::End
    );
}
