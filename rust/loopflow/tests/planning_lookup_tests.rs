use std::process::Command;

use loopflow::ops::task::TaskStatus;
use loopflow::store::{open_ephemeral_store, PlanningState, PmTaskRecord, StorageConfig};
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
        .block_on(store.put_pm_task(
            &scope.to_string_lossy(),
            "linear",
            record.clone(),
            None,
            None,
        ))
        .unwrap();
    let mut stale = record.clone();
    stale.item.id = "issue-2".into();
    stale.item.identifier = "FIX-2".into();
    stale.observed_at -= 7 * 86400 + 1;
    runtime
        .block_on(store.put_pm_task(
            &scope.to_string_lossy(),
            "linear",
            stale.clone(),
            None,
            None,
        ))
        .unwrap();
    let mut invalid = record.clone();
    invalid.item.id = "issue-3".into();
    invalid.item.identifier = "FIX-3".into();
    runtime
        .block_on(store.put_pm_task(
            &scope.to_string_lossy(),
            "linear",
            invalid.clone(),
            None,
            None,
        ))
        .unwrap();
    runtime
        .block_on(store.invalidate_pm_task(
            &scope.to_string_lossy(),
            "linear",
            invalid.clone(),
            None,
        ))
        .unwrap();
    let mut removed = record.clone();
    removed.item.id = "issue-4".into();
    removed.item.identifier = "FIX-4".into();
    runtime
        .block_on(store.put_pm_task(
            &scope.to_string_lossy(),
            "linear",
            removed.clone(),
            None,
            None,
        ))
        .unwrap();
    runtime
        .block_on(store.observe_pm_issue_change("issue-4", None, true))
        .unwrap();
    runtime
        .block_on(store.observe_pm_issue_change("uncached-removal", None, true))
        .unwrap();
    for (selector, expected, state, error) in [
        ("FIX-1", Some(&record), PlanningState::Available, false),
        ("issue-1", Some(&record), PlanningState::Available, false),
        ("FIX-2", Some(&stale), PlanningState::Unavailable, true),
        ("FIX-3", Some(&invalid), PlanningState::Invalid, true),
        ("FIX-4", Some(&removed), PlanningState::Removed, false),
        ("uncached-removal", None, PlanningState::Removed, false),
        ("UNKNOWN", None, PlanningState::Unavailable, true),
    ] {
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
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status: TaskStatus = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(status.planning.as_ref(), expected);
        assert_eq!(status.planning_state, state);
        assert!(status.execution.is_none());
        assert_eq!(status.planning_error.is_some(), error);
        assert_eq!(status.planning_stale, state != PlanningState::Available);
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
