use std::process::Command;

use loopflow::ops::task::TaskStatus;
use loopflow::store::{open_ephemeral_store, PmTaskRecord, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::json;

#[test]
fn task_status_reads_projectless_planning_without_allocating_execution() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            database.clone(),
        )))
        .unwrap();
    let record: PmTaskRecord = serde_json::from_value(json!({
        "item":{"id":"issue-1","identifier":"FIX-1","url":null,
            "name":"Inspect before allocating execution","description":"Planning only",
            "rank":0,"completed":false,"state":"unstarted","project_id":null,
            "project":null,"team_id":"team-1","assignee":null},
        "project":null,"observed_at":time::OffsetDateTime::now_utc().unix_timestamp()
    }))
    .unwrap();
    let scope = repo.path().canonicalize().unwrap();
    runtime
        .block_on(store.put_pm_task(&scope.to_string_lossy(), "linear", record.clone()))
        .unwrap();
    for selector in ["FIX-1", "issue-1"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LF_") {
                command.env_remove(name);
            }
        }
        let output = command
            .current_dir(repo.path())
            .args(["task", "status", selector, "--json"])
            .env("LF_HOME", home.path())
            .env("LF_DB_PATH", &database)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status: TaskStatus = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(status.planning, Some(record.clone()));
        assert!(status.execution.is_none());
        assert!(status.planning_error.is_none());
    }
    assert!(runtime.block_on(store.list_tasks(None)).unwrap().is_empty());
    assert!(runtime
        .block_on(store.list_projects(None))
        .unwrap()
        .is_empty());
    assert_eq!(
        loopflow::engine::worktrees::list_worktrees(repo.path())
            .unwrap()
            .len(),
        1
    );
}
