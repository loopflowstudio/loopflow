//! Saved records, atomic SQLite operations and storage-owned files.

pub mod accounts;
pub mod address;
pub mod canonical_ids;
pub mod changes;
pub mod child;
pub mod copy;
pub mod error;
pub mod execution;
pub mod graph;
pub mod graph_process;
pub mod history;
pub mod instrumentation;
#[cfg(test)]
mod leaving_guard;
pub mod lock_file;
pub mod machine_records;
#[cfg(test)]
pub mod migration_build;
pub mod migration_catalog;
pub mod migration_runner;
pub mod migration_schema;
pub mod people;
pub mod planning;
pub mod planning_sync;
pub(crate) mod pool;
pub mod pr_landing;
pub mod process;
pub mod profile;
pub mod provider;
pub mod recording_gap;
pub mod records;
pub mod rental;
pub mod rows;
pub mod session;
pub mod session_record;
pub mod sqlite;
pub mod steer;
pub mod steer_markers;
pub mod task_session;
pub mod token_crypto;
pub mod transcript;
pub mod work;
pub mod write_holds;

#[cfg(any(test, feature = "test-support"))]
pub mod query_plans;
#[cfg(any(test, feature = "test-support"))]
pub mod test_database;
#[cfg(any(test, feature = "test-support"))]
pub mod test_fixture;
pub use accounts::*;
pub use error::{StoreError, StoreResult};
pub use recording_gap::{GapSpan, RecordingGap};
pub use records::*;
pub const TURN_USAGE_LIVE_RETENTION_SECONDS: i64 = 86_400;

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn isolate_fixture_home() {
    let home = std::env::temp_dir().join(format!("lf-storage-test-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("create the storage fixture home");
    for variable in ["CODEX_HOME", "CLAUDE_CONFIG_DIR"] {
        std::env::remove_var(variable);
    }
    std::env::set_var("HOME", home);
}
