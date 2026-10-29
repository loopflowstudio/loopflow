use crate::support;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_with_pr, EnvGuard};

/// What `lf <args>` prints. `doctor` prints its report before exiting 1 on a
/// failed check, so only empty output is a failed read.
fn read(repo: &Path, home: &Path, args: &[&str], input: Option<&str>) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success() || (args == ["doctor", "--json"] && !output.stdout.is_empty()),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn diff_files_preserves_comparison_identity_and_draft_editing() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_file("original.txt", "original content\n");
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    let base = repo.head_sha();
    repo.create_branch("file-inspection");
    let task = register_task_with_pr(home.path(), repo.path(), "file-inspection", &base);
    fs::rename(
        repo.path().join("original.txt"),
        repo.path().join("renamed.txt"),
    )
    .unwrap();
    repo.stage_all();
    repo.commit("Rename file");
    repo.create_file("edit.txt", "staged\n");
    repo.stage_all();
    repo.create_file("edit.txt", "working\n");
    repo.create_file("untracked.txt", "untracked\n");
    let query = |args: &[&str], input| -> Value {
        serde_json::from_str(&read(repo.path(), home.path(), args, input)).unwrap()
    };

    let files = query(&["task", "diff", "INF-123", "--files", "--json"], None);
    assert_eq!(files["task_id"], task.task.id.as_str());
    assert_eq!(files["base_commit"], base);
    assert_eq!(files["head_commit"], repo.head_sha());
    let paths = files["files"].as_array().unwrap();
    let renamed = paths
        .iter()
        .find(|file| file["path"] == "renamed.txt")
        .unwrap();
    assert_eq!(renamed["old_path"], "original.txt");
    assert_eq!(renamed["committed"], true);
    let edited = paths
        .iter()
        .find(|file| file["path"] == "edit.txt")
        .unwrap();
    assert_eq!(edited["staged"], true);
    assert_eq!(edited["unstaged"], true);
    assert!(paths
        .iter()
        .any(|file| file["path"] == "untracked.txt" && file["untracked"] == true));
    assert_eq!(
        files,
        query(
            &["diff", "INF-123", "--files", "--base", &base, "--json"],
            None
        )
    );
    let head = query(
        &["diff", "INF-123", "--files", "--base", "head", "--json"],
        None,
    );
    assert_eq!(head["base_commit"], repo.head_sha());
    assert!(!head["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["path"] == "renamed.txt"));

    let patch = query(
        &["diff", "INF-123", "renamed.txt", "--base", &base, "--json"],
        None,
    );
    assert_eq!(patch["base_commit"], base);
    assert!(patch["patch"]
        .as_str()
        .unwrap()
        .contains("rename from original.txt"));
    let file = query(&["file", "INF-123", "edit.txt", "--json"], None);
    let draft = query(
        &["diff", "INF-123", "edit.txt", "--draft", "--json"],
        Some("draft\n"),
    );
    assert_eq!(draft["base_commit"], base);
    assert!(draft["patch"].as_str().unwrap().contains("+draft"));
    assert_eq!(
        fs::read_to_string(repo.path().join("edit.txt")).unwrap(),
        "working\n"
    );
    assert_eq!(
        file,
        query(&["file", "INF-123", "edit.txt", "--json"], None)
    );
    let saved = query(
        &[
            "save",
            "INF-123",
            "edit.txt",
            "--revision",
            file["revision"].as_str().unwrap(),
            "--json",
        ],
        Some("draft\n"),
    );
    assert_eq!(saved["published"], true);
    assert_eq!(saved["file"]["content"], "draft\n");
    assert_ne!(saved["file"]["revision"], file["revision"]);
}

#[test]
fn a_query_served_in_process_reads_what_its_lf_command_prints() {
    use loopflow::lf::commands::activity::HistoryWindow;
    use loopflow::lf::commands::operation::Query;

    let home = tempfile::tempdir().unwrap();
    // Same HOME as `read`'s children: `portfolio` scans `$HOME/src`.
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    // As the children resolve it: `doctor` prints the store's path.
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    let base = repo.head_sha();
    repo.create_branch("served-reads");
    register_task_with_pr(home.path(), repo.path(), "served-reads", &base);
    repo.create_file("edit.txt", "working\n");
    let graph = support::record_graph(home.path(), repo.path(), "code", "implement", "succeeded");
    let task_diff = |path: Option<&str>, draft: Option<&str>| Query::TaskDiff {
        issue: "INF-123".into(),
        path: path.map(str::to_owned),
        base: "parent".into(),
        draft: draft.map(str::to_owned),
    };
    // A new Query fails to compile here until it is compared too.
    let repository: Value = serde_json::from_str(
        &Query::RepositoryIdentity {
            directory: repo.path().to_owned(),
        }
        .read()
        .unwrap(),
    )
    .unwrap();
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    let queries = match task_diff(None, None) {
        Query::TaskChanges { .. }
        | Query::TaskDiff { .. }
        | Query::TaskFile { .. }
        | Query::RepositoryIdentity { .. }
        | Query::Waves
        | Query::MachineId
        | Query::Portfolio
        | Query::PaneLayout { .. }
        | Query::People { .. }
        | Query::Graphs { .. }
        | Query::ViewPlan { .. }
        | Query::GraphProcesses { .. }
        | Query::GraphProcess { .. }
        | Query::Usage { .. }
        | Query::TaskContext { .. }
        | Query::WaveDetail { .. }
        | Query::ClosedTasks { .. }
        | Query::Machines
        | Query::History(_)
        | Query::Roadmap { .. }
        | Query::PlanningStatus { .. }
        | Query::WorkLink { .. }
        | Query::RepositoryPath { .. }
        | Query::WaveGraph { .. }
        | Query::SessionPage { .. }
        | Query::Tokens { .. }
        | Query::Retention
        | Query::Doctor => [
            Query::TaskChanges {
                issue: "INF-123".into(),
                base: "parent".into(),
            },
            task_diff(None, None),
            task_diff(Some("edit.txt"), None),
            task_diff(Some("edit.txt"), Some("draft\n")),
            Query::TaskFile {
                issue: "INF-123".into(),
                path: "edit.txt".into(),
            },
            Query::RepositoryIdentity {
                directory: repo.path().to_owned(),
            },
            Query::Waves,
            Query::MachineId,
            Query::Portfolio,
            Query::PaneLayout {
                directory: repo.path().to_owned(),
                work: "repository".into(),
            },
            Query::People {
                directory: repo.path().to_owned(),
            },
            Query::Graphs {
                directory: repo.path().to_owned(),
            },
            Query::ViewPlan {
                directory: repo.path().to_owned(),
                expression: None,
            },
            Query::ViewPlan {
                directory: repo.path().to_owned(),
                expression: Some("state:backlog".into()),
            },
            Query::GraphProcesses {
                directory: repo.path().to_owned(),
                task: "INF-123".into(),
                after: None,
            },
            Query::GraphProcess { id: graph },
            Query::Usage {
                days: 0,
                wave: None,
                project: None,
                task: Some("INF-123".into()),
            },
            Query::Usage {
                days: 30,
                wave: Some("task-pr-tests".into()),
                project: None,
                task: None,
            },
            Query::TaskContext {
                task: "INF-123".into(),
            },
            Query::WaveDetail {
                directory: repo.path().to_owned(),
                wave: "task-pr-tests".into(),
            },
            Query::ClosedTasks {
                directory: repo.path().to_owned(),
                wave: "task-pr-tests".into(),
                after: None,
            },
            Query::Machines,
            Query::History(HistoryWindow {
                since: "7d".into(),
                limit: 50,
                wave: None,
                project: None,
                task: Some("INF-123".into()),
            }),
            Query::Roadmap {
                directory: "/".into(),
                wave: None,
                task: None,
                all: true,
            },
            Query::Roadmap {
                directory: repo.path().to_owned(),
                wave: None,
                task: Some("INF-123".into()),
                all: false,
            },
            Query::PlanningStatus {
                directory: repo.path().to_owned(),
            },
            Query::WorkLink {
                directory: "/".into(),
                url: format!("loopflow://{folder}/task/INF-123"),
            },
            Query::RepositoryPath {
                id: serde_json::from_value(repository["id"].clone()).unwrap(),
            },
            Query::WaveGraph {
                directory: repo.path().to_owned(),
                wave: Some("task-pr-tests".into()),
            },
            Query::WaveGraph {
                directory: repo.path().to_owned(),
                wave: None,
            },
            Query::SessionPage {
                directory: repo.path().to_owned(),
                headless: true,
                after: None,
            },
            Query::Tokens {
                directory: repo.path().to_owned(),
                days: None,
            },
            Query::Tokens {
                directory: repo.path().to_owned(),
                days: Some(1),
            },
            Query::Retention,
            Query::Doctor,
        ],
    };
    for query in queries {
        let owned = query.argv();
        let argv: Vec<&str> = owned.iter().map(String::as_str).collect();
        let printed = read(repo.path(), home.path(), &argv, query.input());
        let served = query.read().unwrap();
        let Ok(printed) = serde_json::from_str::<Value>(&printed) else {
            // `config repository --path` prints a path, not JSON.
            assert_eq!(served.trim(), printed.trim(), "{argv:?}");
            continue;
        };
        let served: Value = serde_json::from_str(&served).unwrap();
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
}

/// `value` without the times a reading stamps when it runs.
fn without_clock(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            fields.retain(|key, field| {
                !(key == "evidence_age_secs"
                    || key == "generated_at"
                    || (key == "observed_at" && field.is_string()))
            });
            for field in fields.values_mut() {
                *field = without_clock(field.take());
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = without_clock(item.take());
            }
        }
        _ => {}
    }
    value
}
