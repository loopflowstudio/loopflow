#[allow(unused_imports)] // Each test binary uses a different subset of these fixtures.
pub use loopflow_execution::test_support::*;

#[ctor::ctor(unsafe)]
fn initialize_application_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
}

#[allow(dead_code)] // Session fixtures serve the integration binary only.
pub const CALLER: &str = "run_00000000000000000000000000000002";

/// A prepared conversation, before a provider has started.
#[allow(dead_code)]
pub fn prepare_conversation(
    home: &std::path::Path,
    cwd: &std::path::Path,
    harness: &str,
    title: &str,
) -> (String, String, std::path::PathBuf) {
    let store = loopflow_execution::store::open_sqlite(&home.join("loopflow.db")).unwrap();
    let input = format!("run_{}", uuid::Uuid::new_v4().simple());
    let session = loopflow_execution::store::sessions::create_session(
        &store,
        loopflow_execution::session::LfSession {
            captured: None,
            id: format!("session_{input}").into(),
            artifact_key: Some(input.clone()),
            caller_artifact_key: Some(CALLER.into()),
            input_published: false,
            cwd: cwd.into(),
            skill: None,
            provider: Some(harness.into()),
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            interactive: true,
            repo: None,
            title: title.into(),
            title_source: loopflow_execution::session::TitleSource::Generated,
            request: Some(title.into()),
            ready_summary: None,
            completed_at: None,
            created_at: 1,
        },
        None,
    )
    .unwrap();
    let dir = home.join("runs").join(&input[4..6]).join(&input);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "artifact_key": input, "caller_artifact_key": CALLER,
        "created_at": "2026-01-01T00:00:00Z", "harness": harness, "model": null,
        "surface": "tui", "cwd": cwd, "repo": null, "worktree": null,
        "skill": null, "subjects": [], "flow": {"kind":"independent"}, "exec": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "test", "boot_id": null
    });
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("prepared"), "").unwrap();
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET input_published=1 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
    (session.id.to_string(), input, dir)
}

/// The provider thread `native` that `input` ran on, as its hooks record it.
#[allow(dead_code)]
pub fn record_native(home: &std::path::Path, id: &str, input: &str, native: &str) {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
        [id, native],
    )
    .unwrap();
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
        SELECT id,'observed',?2||':fixture-native',created_at,json_object('input_id',?2,'source','provider-session:fixture','evidence',json_object('schema_version',1,'provider_session_id',?3,'account_id',NULL)),current_capture
        FROM agent_sessions WHERE id=?1", [id,input,native]).unwrap();
}
