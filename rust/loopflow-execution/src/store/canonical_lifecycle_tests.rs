use crate::store::migrations::{apply_pending_migrations, apply_sqlite};
use fs2::FileExt;
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::test_fixture::session_fence_file;
use rusqlite::{params, Connection};
const SESSION: &str = "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const PROCESS: &str = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB";
const TRACE: &str = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";
fn released() -> (tempfile::TempDir, Connection) {
    loopflow_storage::migration_runner::canonical_fixture(
        &crate::store::migrations::observers(),
        &crate::store::migrations::build(),
    )
}
#[test]
fn canonical_ids_changed_claim_waits_for_canonical_live_caller() {
    let (_home, conn) = released();
    let pid = std::process::id();
    let started_at = crate::process::os_process::process_started_at(pid)
        .unwrap()
        .unwrap();
    let process = PROCESS.to_ascii_lowercase();
    let trace = TRACE.to_ascii_lowercase();
    let session = SESSION.to_ascii_lowercase();
    let agent = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    conn.execute("UPDATE agent_sessions SET id=?1", [&session])
        .unwrap();
    conn.execute(
        "UPDATE processes SET id=?1,trace_id=?2,started_at=?3,completed_at=NULL,pid=?4,os_started_at=?3",
        params![process, trace, started_at, pid],
    )
    .unwrap();
    conn.execute("INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,attached_lf_process_id,attachment_token,agent_session_id,started_at,completed_at)
        VALUES(?1,?2,'agent',?3,?3,'EEEEEEEE-EEEE-4EEE-8EEE-EEEEEEEEEEEE',?4,1,2)", params![agent,trace,process,session]).unwrap();
    conn.execute("UPDATE agent_sessions SET agent_process_id=?1", [agent])
        .unwrap();
    let error = apply_sqlite(&conn).unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&format!("Process {process} has Live")),
        "{error}"
    );
    conn.execute(
        "UPDATE processes SET completed_at=?1 WHERE id=?2",
        params![started_at + 1, process],
    )
    .unwrap();
    apply_sqlite(&conn).unwrap();
}

#[test]
fn canonical_ids_canonical_live_claim_does_not_defer_upgrade() {
    let (home, conn) = released();
    let pid = std::process::id();
    let started_at = crate::process::os_process::process_started_at(pid)
        .unwrap()
        .unwrap();
    conn.execute("UPDATE agent_sessions SET id=lower(id)", [])
        .unwrap();
    conn.execute("UPDATE processes SET id=lower(id),trace_id=lower(trace_id),pid=?1,started_at=?2,completed_at=NULL", params![pid,started_at]).unwrap();
    let agent = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    let token = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    let session = SESSION.to_ascii_lowercase();
    conn.execute("INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,attached_lf_process_id,attachment_token,agent_session_id,pid,os_started_at,started_at)
        VALUES(?1,?2,'agent',?3,?3,?4,?5,?6,?7,?7)",
        params![agent,TRACE.to_ascii_lowercase(),PROCESS.to_ascii_lowercase(),token,session,pid,started_at]).unwrap();
    conn.execute(
        "UPDATE agent_sessions SET agent_process_id=?1,provider_thread='Native:UPPER'",
        [agent],
    )
    .unwrap();
    let store = home.path().join("loopflow.db");
    // The released schema keeps this claim on the agent's Process row.
    let holder = loopflow_model::id::LfProcessId::parse(&PROCESS.to_ascii_lowercase()).unwrap();
    let original_claim = crate::process::SessionAttachment {
        agent_process_id: loopflow_model::id::AgentProcessId::parse(agent).unwrap(),
        lf_process_id: Some(holder.clone()),
        token: loopflow_model::id::AttachmentToken::parse(token).unwrap(),
        launcher_lf_process_id: holder,
    };
    let fence = session_fence_file(&store, &SESSION.to_ascii_lowercase()).unwrap();
    FileExt::lock_shared(&fence).unwrap();
    apply_sqlite(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT pid FROM processes WHERE kind='agent'", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        pid
    );
    assert!(session_fence_file(&store, &SESSION.to_ascii_lowercase())
        .unwrap()
        .try_lock_exclusive()
        .is_err());
    let store =
        SqliteStore::open_read_only(&store, crate::store::token_key::encryption_key).unwrap();
    let claim = store.session_attachment(&session).unwrap().unwrap();
    assert_eq!(claim, original_claim);
    assert_eq!(claim.token.as_str(), token);
    assert_eq!(claim.agent_process_id.as_str(), agent);
    assert_eq!(
        store.session_thread(&session).unwrap().unwrap().as_str(),
        "Native:UPPER"
    );
    // Exercise the production dispatch boundary with the original live claim.
    assert_eq!(
        store
            .with_session_dispatch(&session, &claim, || Ok("sent"))
            .unwrap(),
        "sent"
    );
}

#[test]
fn canonical_ids_commands_read_the_upgraded_work_and_native_history() {
    let (home, conn) = released();
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../loopflow-storage/tests/fixtures/migrations/canonical_ids_work.sql"
    )))
    .unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    // A resolved path, so the Git locator check runs wherever temp dirs are symlinked.
    let repo_path = repo.path().canonicalize().unwrap();
    let repo_path = repo_path.to_str().unwrap();
    conn.execute_batch(&format!(
        "UPDATE waves SET repo='{repo_path}'; UPDATE repository_plans SET repo='{repo_path}';"
    ))
    .unwrap();
    conn.execute("UPDATE tasks SET worktree=?1", [repo_path])
        .unwrap();
    conn.execute("UPDATE agent_sessions SET repo=?1,cwd=?1", [repo_path])
        .unwrap();
    apply_sqlite(&conn).unwrap();
    apply_pending_migrations(&conn);
    let binary = loopflow_test_support::lf_binary();
    let lf = |args: &[&str]| {
        let mut command = std::process::Command::new(binary);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LF_")
                || name.to_string_lossy().starts_with("LOOPFLOW_")
            {
                command.env_remove(name);
            }
        }
        command
            .args(args)
            .current_dir(repo_path)
            .env("LF_HOME", home.path())
            .env("LF_BIN", binary)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    };
    // The released checkout predates Git locators; establish it as the original.
    let bound = lf(&[
        "config",
        "repository",
        "--bind",
        "REPO_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ]);
    assert!(
        bound.status.success(),
        "{}",
        String::from_utf8_lossy(&bound.stderr)
    );
    for args in [
        vec!["desktop", "task", "lOo-506", "--json"],
        vec![
            "task",
            "status",
            "TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "--json",
        ],
        vec![
            "task",
            "status",
            "FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF",
            "--json",
        ],
        vec!["session", "history", SESSION, "--json"],
        vec!["history", "show", SESSION, "--json"],
    ] {
        let output = lf(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let json = output.to_string();
        assert!(
            !json.contains("TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            "{json}"
        );
        assert!(!json.contains(PROCESS), "{json}");
        if args[1] == "show" {
            let canonical = args[2].to_ascii_lowercase();
            assert!(json.contains(&canonical), "{json}");
        }
        if args[0] == "desktop" {
            assert!(
                json.contains("ffffffff-ffff-4fff-8fff-ffffffffffff"),
                "{json}"
            );
        }
        if args[0] == "task" {
            assert!(
                json.contains("task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                "{json}"
            );
            assert!(
                json.contains("session_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                "{json}"
            );
        }
    }
}
