use loopflow::lf::commands::install::build_preview;

fn initialize_store(path: &std::path::Path) {
    let connection = rusqlite::Connection::open(path).unwrap();
    loopflow::store::migrations::apply_sqlite(&connection).unwrap();
}

#[test]
fn promotion_preview_is_schema_evidence_not_run_control() {
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("loopflow.db");
    initialize_store(&store);

    let preview = build_preview(&store);
    let json = serde_json::to_value(preview).unwrap();
    assert!(json.get("compatibility").is_some());
    assert!(json.get("executable_compatibility").is_some());
    assert!(json.get("active_runs").is_none());
}

#[test]
fn promotion_preview_does_not_mutate_the_selected_store() {
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("loopflow.db");
    initialize_store(&store);
    let connection = rusqlite::Connection::open(&store).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE promotion_probe (value TEXT NOT NULL);
             INSERT INTO promotion_probe VALUES ('unchanged');",
        )
        .unwrap();
    drop(connection);

    let _ = build_preview(&store);

    let connection = rusqlite::Connection::open(&store).unwrap();
    let value: String = connection
        .query_row("SELECT value FROM promotion_probe", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "unchanged");
}
