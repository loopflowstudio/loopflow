// Temporary measurement-only callback, installed after observe_sqlite.
// SQL uses placeholders; neither bound arguments nor result values are recorded.
fn session_list_trace(connection: &rusqlite::Connection) {
    if std::env::var_os("SESSION_LIST_TRACE").is_none() { return; }
    unsafe extern "C" fn trace(event: u32, _: *mut std::ffi::c_void, pointer: *mut std::ffi::c_void, elapsed: *mut std::ffi::c_void) -> std::ffi::c_int {
        use rusqlite::ffi;
        thread_local! { static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        if INSIDE.with(|inside| inside.replace(true)) { return 0; }
        if event == ffi::SQLITE_TRACE_PROFILE {
            // SAFETY: SQLite owns these live statement and elapsed pointers for this callback.
            let stmt = pointer.cast::<ffi::sqlite3_stmt>();
            let sql = unsafe { std::ffi::CStr::from_ptr(ffi::sqlite3_sql(stmt)) }.to_string_lossy().into_owned();
            let ns = unsafe { *elapsed.cast::<u64>() };
            let steps = unsafe { ffi::sqlite3_stmt_status(stmt, ffi::SQLITE_STMTSTATUS_VM_STEP, 1) };
            let scans = unsafe { ffi::sqlite3_stmt_status(stmt, ffi::SQLITE_STMTSTATUS_FULLSCAN_STEP, 1) };
            let sorts = unsafe { ffi::sqlite3_stmt_status(stmt, ffi::SQLITE_STMTSTATUS_SORT, 1) };
            let readonly = unsafe { ffi::sqlite3_stmt_readonly(stmt) } != 0;
            let handle = unsafe { ffi::sqlite3_db_handle(stmt) };
            // SAFETY: from_handle borrows, never closes the live SQLite connection.
            if let Ok(db) = unsafe { rusqlite::Connection::from_handle(handle) } {
                let plan = (|| -> rusqlite::Result<Vec<String>> {
                    let mut q = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
                    let values = vec![rusqlite::types::Value::Null; q.parameter_count()];
                    let rows = q.query_map(rusqlite::params_from_iter(values), |row| row.get::<_,String>(3))?;
                    rows.collect()
                })();
                let value = serde_json::json!({"sql":sql,"ns":ns,"vm":steps,"scan":scans,"sort":sorts,"readonly":readonly,"database":db.path(),"plan":plan.as_ref().ok(),"plan_error":plan.err().map(|e| e.to_string())});
                if let Some(path) = std::env::var_os("SESSION_LIST_TRACE") {
                    use std::io::Write;
                    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(path) { let _ = writeln!(file,"{value}"); }
                }
            }
        }
        INSIDE.with(|inside| inside.set(false));
        0
    }
    // SAFETY: callback retains no SQLite pointers; handle is live at registration.
    unsafe { rusqlite::ffi::sqlite3_trace_v2(connection.handle(), rusqlite::ffi::SQLITE_TRACE_PROFILE, Some(trace), std::ptr::null_mut()); }
}
