//! Observe running owners before synchronous retention compares and ends them.
use super::{sqlite::retention, StoreResult};
use std::path::Path;

pub fn prune(database: &Path, now: i64) -> StoreResult<retention::RetentionReport> {
    let booted_at = crate::process::os_process::machine_booted_at();
    let mut dead = Vec::new();
    for owner in retention::running_owners(database)? {
        let before_boot = owner
            .started_at
            .zip(booted_at)
            .is_some_and(|(started_at, booted_at)| started_at < booted_at);
        let gone = before_boot
            || owner.pid.is_some_and(|pid| match owner.os_started_at {
                Some(started_at) => {
                    crate::process::os_process::process_identity_evidence(pid, started_at)
                        == crate::store::ProcessIdentityEvidence::Dead
                }
                // Without its OS start, only an absent pid says it exited.
                None => matches!(crate::process::OsProcess::read(pid), Ok(None)),
            });
        if !gone {
            continue;
        }
        let at = booted_at.filter(|_| before_boot).unwrap_or(now);
        dead.push((owner, at));
    }
    retention::prune(database, &dead, now)
}
