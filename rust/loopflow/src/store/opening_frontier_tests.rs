use crate::store::FrontierAdvance::{self, Authorized, Forbidden};
use crate::work::wave::Wave;
use loopflow_model::durable::{WorkRef, WorkStatus};
use loopflow_model::id::WaveId;
use loopflow_storage::migration_runner::{
    apply_all_but_head, apply_newer_migration, latest_applied_version_sqlite, latest_known_version,
    prior_known_version,
};
use loopflow_storage::sqlite::SqliteStore;
use std::path::{Path, PathBuf};

/// The machine home whose `.lf/loopflow.db` `may_apply_migrations` treats as
/// the shared release store. The regressions inject it so they never touch a
/// developer's real `~/.lf`.
struct SharedMachine {
    _dir: tempfile::TempDir,
    home: PathBuf,
}

impl SharedMachine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        Self { _dir: dir, home }
    }

    fn shared_db(&self) -> PathBuf {
        self.home.join(".lf/loopflow.db")
    }
}

fn open(
    path: &Path,
    home: &Path,
    advance: FrontierAdvance,
) -> crate::store::StoreResult<SqliteStore> {
    crate::store::opening::open_with(
        path,
        home,
        advance,
        loopflow_storage::sqlite::SQLITE_WRITE_BUSY_TIMEOUT,
    )
}

fn frontier(path: &Path) -> Option<String> {
    let conn = rusqlite::Connection::open(path).unwrap();
    latest_applied_version_sqlite(&conn).unwrap()
}

fn seed_shared_store_at_prior_head(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    apply_all_but_head(
        &loopflow_storage::test_fixture::observers(),
        &loopflow_storage::test_fixture::build(),
        &conn,
    )
    .unwrap();
}

fn seed_current_wave(path: &Path) {
    rusqlite::Connection::open(path).unwrap().execute(
        "INSERT INTO waves(id,name,repo,created_at) VALUES('current-wave','Current','/repo',100)", [],
    ).unwrap();
}

fn current_wave(path: &Path) -> String {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT name FROM waves WHERE id='current-wave'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn any_build_uses_compatible_shared_work_without_migrating() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    open(&path, &shared.home, Authorized).unwrap();
    seed_current_wave(&path);
    let before = frontier(&path);
    let store = open(&path, &shared.home, Forbidden).unwrap();
    let wave = Wave::new(WaveId::new(), "From app".into(), "/repo".into());
    crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
    assert_eq!(current_wave(&path), "Current");
    assert_eq!(frontier(&path), before);
    open(&path, &shared.home, Forbidden).unwrap();
}

/// A newer build leaves this lf reading and writing the shared store.
/// Promotion selecting this build applies nothing to history it does not know.
#[test]
fn an_older_lf_keeps_working_on_a_store_a_newer_build_extended() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    open(&path, &shared.home, Authorized).unwrap();
    seed_current_wave(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    apply_newer_migration(
        &loopflow_storage::test_fixture::observers(),
        &loopflow_storage::test_fixture::build(),
        &conn,
        "ALTER TABLE waves ADD COLUMN colour TEXT;
         CREATE TABLE wave_badges (wave_id TEXT NOT NULL, badge TEXT);",
    )
    .unwrap();
    let newer = frontier(&path);

    let store =
        open(&path, &shared.home, Forbidden).expect("a newer build leaves the store readable");
    let wave = Wave::new(WaveId::new(), "From older lf".into(), "/repo".into());
    crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
    assert_eq!(
        store
            .get_wave(wave.id())
            .unwrap()
            .map(|wave| wave.name().to_string()),
        Some("From older lf".to_string())
    );
    assert_eq!(current_wave(&path), "Current");
    assert_eq!(frontier(&path), newer);
    open(&path, &shared.home, Authorized).expect("selecting an older build applies nothing");
    assert_eq!(frontier(&path), newer);
}

/// A contract migration past its staging window still opens: the one
/// statement that reads what it dropped fails, naming the column.
#[test]
fn a_dropped_column_fails_the_statement_that_reads_it_and_nothing_else() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    open(&path, &shared.home, Authorized).unwrap();
    let running = open(&path, &shared.home, Forbidden).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    apply_newer_migration(
        &loopflow_storage::test_fixture::observers(),
        &loopflow_storage::test_fixture::build(),
        &conn,
        "ALTER TABLE waves DROP COLUMN promoted_at;",
    )
    .unwrap();

    running
        .validate_current_schema(&loopflow_storage::test_fixture::build())
        .expect("the running reader keeps its store");
    let store = open(&path, &shared.home, Forbidden).expect("a fresh open keeps working");

    let error = store.get_wave(&WaveId::new()).unwrap_err().to_string();

    assert!(error.contains("promoted_at"), "{error}");
    assert!(!error.contains("lf install"), "{error}");
    store.health_check().expect("the rest of the store answers");
}

/// A Graph holds its store across steps and revalidates it after each one
/// (`validate_current_schema`). A newer build that adds a slug migration
/// meanwhile leaves that running reader working.
#[test]
fn a_running_older_reader_survives_an_upgrade_under_it() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    open(&path, &shared.home, Authorized).unwrap();
    let running = open(&path, &shared.home, Forbidden).unwrap();
    let before = Wave::new(WaveId::new(), "Before upgrade".into(), "/repo".into());
    crate::work::wave::bootstrap::upsert_wave(&running, &before).unwrap();

    let newer = rusqlite::Connection::open(&path).unwrap();
    apply_newer_migration(
        &loopflow_storage::test_fixture::observers(),
        &loopflow_storage::test_fixture::build(),
        &newer,
        "ALTER TABLE waves ADD COLUMN colour TEXT;
         CREATE TABLE wave_badges (wave_id TEXT NOT NULL, badge TEXT);",
    )
    .unwrap();

    running
        .validate_current_schema(&loopflow_storage::test_fixture::build())
        .expect("a newer migration keeps the running reader valid");
    let after = Wave::new(WaveId::new(), "After upgrade".into(), "/repo".into());
    crate::work::wave::bootstrap::upsert_wave(&running, &after).unwrap();
    for wave in [&before, &after] {
        assert_eq!(
            running
                .get_wave(wave.id())
                .unwrap()
                .map(|w| w.name().to_string()),
            Some(wave.name().to_string())
        );
    }
}

#[test]
fn a_private_store_opens_with_the_current_work_schema() {
    let shared = SharedMachine::new();
    let path = shared.home.join("private/loopflow.db");
    let store = open(&path, &shared.home, Forbidden).expect("a private store migrates itself");
    let wave = Wave::new(
        WaveId::new(),
        "private-development".to_string(),
        "/repo".to_string(),
    );

    crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();

    assert_eq!(
        store
            .work_status(&WorkRef::Wave(wave.id().clone()))
            .unwrap(),
        WorkStatus::Ready
    );
}

/// (a) An ordinary open of an absent shared store must refuse actionably and
/// leave no file behind. Sabotage guard: an `open_with` that creates the
/// SQLite file before deciding authority (or that lets Forbidden bootstrap)
/// would create the path and this fails.
#[test]
fn an_ordinary_open_never_creates_or_initializes_an_absent_shared_store() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();

    let error = open(&path, &shared.home, Forbidden)
        .expect_err("an ordinary open must not initialize the shared store");
    assert!(
        error.to_string().contains("lf self install"),
        "the refusal must name the authorized boundary: {error}"
    );
    assert!(
        !path.exists(),
        "an ordinary open must not create the shared store file"
    );
}

/// (b) An ordinary open of an existing shared store the binary is ahead of
/// must refuse actionably without advancing — it must not hand N+1 code a
/// store still at the N schema — while the old N reader keeps recognizing it.
/// Sabotage guards: a Forbidden open that applied the pending head, or that
/// returned a usable store instead of erroring, fails this test.
#[test]
fn an_ordinary_open_ahead_of_the_shared_frontier_refuses_without_advancing() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    seed_shared_store_at_prior_head(&path);
    let installed_frontier = frontier(&path).unwrap();
    assert_eq!(installed_frontier, prior_known_version());
    assert_ne!(installed_frontier, latest_known_version());

    // The candidate is one migration ahead; its ordinary open refuses rather
    // than reuse a schema older than its own code.
    let error = open(&path, &shared.home, Forbidden)
        .expect_err("an ordinary open ahead of the frontier must refuse");
    // `lf install` finishes an app Sparkle replaced; `--dev` would move a
    // stable Machine to dev.
    let error = error.to_string();
    assert!(
        error.contains(&latest_known_version())
            && (error.contains("run `lf install` to finish") || error.contains("private LF_HOME"))
            && !error.contains("--dev"),
        "the refusal names the pending migration and its fix: {error}"
    );
    assert_eq!(
        frontier(&path).as_deref(),
        Some(installed_frontier.as_str()),
        "a refused open must not advance the shared frontier"
    );

    // The old reader — a build whose head is the store's frontier — still
    // recognizes the untouched store as exactly its own frontier.
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert!(
        loopflow_storage::migration_runner::old_reader_recognizes(&conn),
        "the old installed reader must still recognize the untouched store"
    );
}

/// (c) The promotion boundary owns both first initialization and advancement.
#[test]
fn the_promotion_boundary_initializes_and_advances_the_shared_store() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();

    // Initialization from absent.
    open(&path, &shared.home, Authorized).expect("boundary initializes");
    assert_eq!(
        frontier(&path).as_deref(),
        Some(latest_known_version().as_str())
    );

    // Once the boundary has initialized it, an ordinary open validates the
    // now-existing store read-only and leaves the frontier at the head.
    open(&path, &shared.home, Forbidden).expect("an ordinary open validates the initialized store");
    assert_eq!(
        frontier(&path).as_deref(),
        Some(latest_known_version().as_str())
    );

    // Advancement from a prior-head store.
    let advanced = SharedMachine::new();
    let advanced_path = advanced.shared_db();
    seed_shared_store_at_prior_head(&advanced_path);
    assert_eq!(
        frontier(&advanced_path).as_deref(),
        Some(prior_known_version().as_str())
    );
    open(&advanced_path, &advanced.home, Authorized).expect("boundary advances");
    assert_eq!(
        frontier(&advanced_path).as_deref(),
        Some(latest_known_version().as_str())
    );
}

/// The 2026-07-17 incident shape, exercised as two binary generations: a
/// branch candidate knows one migration the installed release does not.
/// Ordinary candidate use must leave both the shared frontier and existing
/// Wave untouched; explicit promotion advances once and retains the Wave.
#[test]
fn branch_candidate_cannot_advance_shared_store_or_damage_current_state_outside_promotion() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    seed_shared_store_at_prior_head(&path);
    seed_current_wave(&path);
    let installed_frontier = prior_known_version();
    assert_eq!(
        frontier(&path).as_deref(),
        Some(installed_frontier.as_str())
    );
    assert_eq!(current_wave(&path), "Current");

    open(&path, &shared.home, Forbidden)
        .expect_err("ordinary branch candidate must not apply its migration");
    assert_eq!(
        frontier(&path).as_deref(),
        Some(installed_frontier.as_str())
    );
    assert_eq!(current_wave(&path), "Current");
    let installed = rusqlite::Connection::open(&path).unwrap();
    assert!(
        loopflow_storage::migration_runner::old_reader_recognizes(&installed),
        "the installed release must still recognize the candidate's untouched store"
    );
    drop(installed);

    open(&path, &shared.home, Authorized).expect("explicit promotion advances the shared frontier");
    let promoted_frontier = latest_known_version();
    assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
    assert_eq!(current_wave(&path), "Current");

    open(&path, &shared.home, Authorized)
        .expect("repeating promotion at the same frontier is a no-op");
    assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
    assert_eq!(current_wave(&path), "Current");
    open(&path, &shared.home, Forbidden).expect("ordinary current binary opens after promotion");
}

/// Opening the shared store reads its schema, never its rows: a dangling
/// reference is installation preflight's and `lf doctor`'s to report.
#[test]
fn an_ordinary_open_of_the_shared_store_does_not_scan_stored_rows() {
    let shared = SharedMachine::new();
    let path = shared.shared_db();
    open(&path, &shared.home, Authorized).expect("boundary initializes");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO projects(id, wave_id, external_project_id, created_at)
         VALUES ('orphan', 'absent-wave', 'external', 100);",
    )
    .unwrap();

    open(&path, &shared.home, Forbidden)
        .expect("an ordinary open validates ledger and schema only");
    loopflow_storage::migration_runner::validate_sqlite(
        &loopflow_storage::test_fixture::build(),
        &conn,
    )
    .expect_err("full diagnosis still reports the dangling reference");
}

/// A private path (not ~/.lf/loopflow.db) initializes and migrates itself.
#[test]
fn a_private_store_initializes_without_the_promotion_boundary() {
    let shared = SharedMachine::new();
    let private = shared.home.join(".lf-dev/branch/loopflow.db");
    open(&private, &shared.home, Forbidden).expect("private DB initializes");
    assert_eq!(
        frontier(&private).as_deref(),
        Some(latest_known_version().as_str())
    );
}
