use time::OffsetDateTime;

use super::{
    admit, free_machine, give_back, resolve_unconfirmed, GiveBack, Hardware, Holdings, OnDemand,
    Ownership, Pool, PoolMachine, PoolRefusal, Provider, Refusal, Rental, RentalId, RentalState,
    Resolution, Usd, GRACE_SECONDS,
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

fn box4() -> Hardware {
    Hardware {
        vcpus: 4,
        memory_gb: 8,
        disk_gb: 10,
        gpu: None,
        spot: false,
    }
}

fn code(max: u32, max_per_hour: &str) -> OnDemand {
    OnDemand {
        name: "code".into(),
        provider: Provider::Daytona,
        hardware: box4(),
        max_machines: max,
        max_usd_per_hour: usd(max_per_hour),
        hours: 4.0,
        created_at: now(),
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
fn prices_follow_each_providers_published_rates() {
    // Daytona: 4 × $0.0504 + 8 × $0.0162 + 5 GiB of disk past the free 5 × $0.000108.
    assert_eq!(Provider::Daytona.usd_per_hour(&box4()), Ok(usd("0.33174")));
    // Modal sandboxes: 2 cores × $0.141912 + 8 × $0.024012.
    assert_eq!(Provider::Modal.usd_per_hour(&box4()), Ok(usd("0.475920")));
}

#[test]
fn fly_reads_as_history_and_rents_nothing() {
    assert_eq!(Provider::parse("fly"), Some(Provider::Fly));
    assert!(Provider::Fly
        .offer(&box4())
        .unwrap_err()
        .contains("no longer rents"));
    assert!(Provider::Fly.missing_credentials().is_some());
    assert!(<Provider as clap::ValueEnum>::from_str("fly", false).is_err());
}

fn gpu(name: &str) -> Hardware {
    Hardware {
        gpu: Some(name.into()),
        ..box4()
    }
}

#[test]
fn fixed_shapes_take_the_smallest_that_holds_the_hardware() {
    let runpod = Provider::Runpod.offer(&box4()).unwrap();
    assert_eq!(runpod.machine_type.as_deref(), Some("cpu3c"));
    let aws = Provider::Aws
        .offer(&Hardware {
            vcpus: 3,
            memory_gb: 12,
            ..box4()
        })
        .unwrap();
    assert_eq!(aws.machine_type.as_deref(), Some("m7i.xlarge"));
    assert_eq!(aws.usd_per_hour, usd("0.2016"));
    let l4 = Provider::Aws.offer(&gpu("L4")).unwrap();
    assert_eq!(l4.machine_type.as_deref(), Some("g6.xlarge"));
}

#[test]
fn gpus_rent_where_the_provider_offers_them() {
    let a40 = Provider::Runpod.offer(&gpu("A40")).unwrap();
    assert_eq!(a40.machine_type.as_deref(), Some("NVIDIA A40"));
    assert_eq!(a40.usd_per_hour, usd("0.59"));
    let modal = Provider::Modal.offer(&gpu("A100")).unwrap();
    assert_eq!(modal.machine_type.as_deref(), Some("A100-80GB"));
    assert!(Provider::Daytona
        .offer(&gpu("A40"))
        .unwrap_err()
        .contains("--gpu works on runpod, aws and modal"));
    assert!(Provider::Aws
        .offer(&gpu("H100"))
        .unwrap_err()
        .contains("it rents L4, A10G"));
}

#[test]
fn gpu_hardware_is_a_floor_the_shape_must_meet() {
    let big = Provider::Aws
        .offer(&Hardware {
            vcpus: 8,
            memory_gb: 20,
            ..gpu("L4")
        })
        .unwrap();
    assert_eq!(big.machine_type.as_deref(), Some("g6.2xlarge"));
    let refused = Provider::Aws
        .offer(&Hardware {
            vcpus: 32,
            ..gpu("A10G")
        })
        .unwrap_err();
    assert_eq!(refused, "aws rents nothing with 32 vCPU 8 GB + A10G");
}

#[test]
fn spot_rents_only_where_the_provider_sells_it() {
    let spot = Hardware {
        spot: true,
        ..box4()
    };
    assert!(Provider::Aws.offer(&spot).is_ok());
    assert!(Provider::Runpod.offer(&spot).is_ok());
    for provider in [Provider::Daytona, Provider::Modal] {
        let refused = provider.offer(&spot).unwrap_err();
        assert!(refused.starts_with(provider.as_str()), "{refused}");
        assert!(
            refused.contains("--spot works on aws and runpod"),
            "{refused}"
        );
    }
}

#[test]
fn admission_refuses_at_the_limit_and_over_price() {
    assert!(matches!(
        admit(&code(1, "0.40"), 1, None, None, now()),
        Err(Refusal::AtLimit {
            live: 1,
            max: 1,
            ..
        })
    ));
    assert!(matches!(
        admit(&code(0, "0.40"), 0, None, None, now()),
        Err(Refusal::AtLimit { .. })
    ));
    let refused = admit(&code(1, "0.30"), 0, None, None, now()).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "code: 4 vCPU 8 GB costs $0.33174/h, above its $0.30/h limit"
    );
}

#[test]
fn admission_caps_spend_by_hours_a_lower_budget_or_the_longest_window() {
    let admitted = admit(&code(1, "0.40"), 0, None, None, now()).unwrap();
    assert_eq!(admitted.cap_usd, usd("1.32696"));
    assert_eq!(admitted.deadline, now() + time::Duration::hours(4));

    let capped = admit(&code(1, "0.40"), 0, Some(2.0), Some(usd("0.33174")), now()).unwrap();
    assert_eq!(capped.cap_usd, usd("0.33174"));
    assert_eq!(capped.deadline, now() + time::Duration::hours(1));

    let longest = admit(&code(1, "0.40"), 0, Some(48.0), None, now()).unwrap();
    assert_eq!(longest.deadline, now() + time::Duration::hours(24));
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
        resolve_unconfirmed::<u8>(None, now(), now() + time::Duration::minutes(20)),
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

pub(super) fn rented(by_hand: bool) -> Rental {
    let id = RentalId::new();
    Rental {
        provider_name: Rental::provider_name_for(&id),
        id,
        ondemand: "code".into(),
        provider: Provider::Daytona,
        provider_id: Some("sb-1".into()),
        machine_id: Some(MachineId::new()),
        by_hand,
        hardware: box4(),
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
fn a_task_rental_waits_out_its_grace_after_the_task_finishes() {
    let finished = Holdings {
        placed: true,
        finished_at: Some(now()),
        ..Holdings::default()
    };
    let rental = rented(false);
    assert!(matches!(
        give_back(&rental, &finished, false, false, now()),
        GiveBack::Keep(_)
    ));
    let later = now() + time::Duration::seconds(GRACE_SECONDS);
    assert_eq!(
        give_back(&rental, &finished, false, false, later),
        GiveBack::End
    );
    // An explicit end does not wait.
    assert_eq!(
        give_back(&rental, &finished, true, false, now()),
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
