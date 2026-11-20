//! A server whose store is a sealed copy of the Machine serves reads and
//! subscriptions. It starts nothing that acts on the Machine and admits no
//! change: copied rows name real processes, checkouts and tracker records.
use std::time::SystemTime;

use loopflow_execution::engine::{Error, ErrorCode};

/// When this server's store was copied; `None` on a Machine. The store's file
/// mode is the one fact, read at each use.
pub(super) fn taken() -> Option<SystemTime> {
    loopflow_storage::copy::sealed_at(&loopflow_execution::store::default_db_path())
}

/// What answers a change here, before any command code runs.
pub(super) fn refusal() -> Option<Error> {
    taken().map(|at| Error::new(ErrorCode::Invalid, sentence(at)))
}

fn sentence(taken: SystemTime) -> String {
    let at = chrono::DateTime::<chrono::Local>::from(taken).format("%H:%M");
    format!("This is a copy of the Machine from {at}; make changes in Loopflow or with lf.")
}

#[cfg(test)]
mod tests {
    use super::sentence;

    #[test]
    fn the_refusal_names_when_the_copy_was_taken() {
        let noon = chrono::NaiveDate::from_ymd_opt(2026, 11, 20)
            .unwrap()
            .and_hms_opt(14, 2, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .unwrap();
        assert_eq!(
            sentence(noon.into()),
            "This is a copy of the Machine from 14:02; make changes in Loopflow or with lf."
        );
    }
}
