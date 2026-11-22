use loopflow_storage::sqlite::SqliteStore;
fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("db")).unwrap();
    (dir, store)
}

#[test]
fn a_read_snapshot_keeps_joined_queries_and_revisions_at_one_commit() {
    let (dir, writer) = store();
    writer.test_session(
        "first",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    writer.test_session(
        "second",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let reader = SqliteStore::open_read_only(
        &dir.path().join("db"),
        crate::store::token_key::encryption_key,
    )
    .unwrap();
    let snapshot = reader.read_snapshot().unwrap();
    let before = reader.revisions().unwrap();
    assert_eq!(
        crate::store::session_projection::session_summary(&reader, "first", 1)
            .unwrap()
            .unwrap()
            .title,
        "Retained"
    );
    writer
        .conn
        .lock()
        .unwrap()
        .execute_batch("BEGIN; UPDATE agent_sessions SET title='committed'; COMMIT;")
        .unwrap();
    // These readers have their own savepoints, nested inside the pinned read.
    assert_eq!(
        crate::store::session_projection::session_summary(&reader, "second", 1)
            .unwrap()
            .unwrap()
            .title,
        "Retained"
    );
    assert_eq!(reader.revisions().unwrap(), before);
    drop(snapshot);
    assert_eq!(
        crate::store::session_projection::session_summary(&reader, "first", 1)
            .unwrap()
            .unwrap()
            .title,
        "committed"
    );
    assert_eq!(
        crate::store::session_projection::session_summary(&reader, "second", 1)
            .unwrap()
            .unwrap()
            .title,
        "committed"
    );
    assert!(reader.revisions().unwrap().sessions > before.sessions);
    // Failed projection exits release the transaction too.
    let failed = || -> crate::store::StoreResult<()> {
        let _snapshot = reader.read_snapshot()?;
        reader
            .conn
            .lock()
            .unwrap()
            .execute_batch("SELECT absent FROM no_table")?;
        Ok(())
    };
    assert!(failed().is_err());
    assert!(reader.read_snapshot().is_ok());
}
