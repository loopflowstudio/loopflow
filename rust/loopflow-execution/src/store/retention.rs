//! Observe processes before synchronous retention compares and closes them.
use super::{sqlite::retention, StoreResult};
use std::path::Path;

pub fn prune(database: &Path, now: i64) -> StoreResult<retention::RetentionReport> {
    let unfinished = retention::unfinished_processes(database)?;
    let home = database.parent().unwrap_or(Path::new("."));
    let booted_at = crate::journal::machine_booted_at();
    let mut dead = Vec::new();
    for (process, locator) in unfinished {
        let receipt = match &locator {
            Some(locator) => crate::journal::read_process_receipt(
                &crate::journal::process_receipt_path(home, locator),
            ),
            None => Ok(None),
        };
        let evidence = crate::journal::recorded_process_evidence(
            &process,
            &receipt,
            crate::journal::process_identity_evidence,
        );
        // No process holds the PID at all, so the recorded one has exited.
        let gone = evidence == crate::store::ProcessIdentityEvidence::Dead
            || (evidence == crate::store::ProcessIdentityEvidence::Unknown
                && process
                    .pid()
                    .is_some_and(|pid| matches!(crate::journal::OsProcess::read(pid), Ok(None))));
        if !gone {
            continue;
        }
        let at = booted_at
            .filter(|booted_at| process.started_at() < *booted_at)
            .unwrap_or(now);
        dead.push((process, locator, at));
    }
    retention::prune(database, &dead)
}
