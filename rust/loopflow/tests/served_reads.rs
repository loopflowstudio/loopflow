//! Every app read, served in-process as the app's library serves it, prints
//! what its `lf` command prints. Its own binary: the library's read-only store
//! mode is process-wide.

mod isolated_lf;
mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use isolated_lf::{isolated_lf, isolated_read as read};
use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_with_pr, EnvGuard};

#[test]
fn a_read_served_in_process_prints_what_its_lf_command_prints() {
    use loopflow::lf::commands::activity::HistoryWindow;
    use loopflow::lf::commands::read::Read;

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
    let task_diff = |path: Option<&str>, draft: Option<&str>| Read::TaskDiff {
        issue: "INF-123".into(),
        path: path.map(str::to_owned),
        base: "parent".into(),
        draft: draft.map(str::to_owned),
    };
    // Registration is a write: `lf` owns it, as at Desktop's workspace opening.
    let repository: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["config", "repository", "--json"],
        None,
    ))
    .unwrap();
    // Prepared, not started: a Session a bind preview can name.
    let session: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["session", "connect", "--prepare", "--json", "--try"],
        None,
    ))
    .unwrap();
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    // Planning status also serves a Work folder without Git.
    let work_folder = home.path().join("folder");
    fs::create_dir_all(&work_folder).unwrap();
    let history = || {
        Read::History(HistoryWindow {
            since: "7d".into(),
            limit: 50,
            wave: None,
            project: None,
            task: Some("INF-123".into()),
        })
    };
    let all_plans = || Read::Roadmap {
        directory: "/".into(),
        wave: None,
        task: None,
        all: true,
    };
    let sessions = |headless| Read::DesktopSessions {
        directory: repo.path().to_owned(),
        headless,
    };
    let task_pane = || Read::DesktopTask {
        directory: repo.path().to_owned(),
        issue: "INF-123".into(),
    };
    let plans = || Read::DesktopPlans {
        directory: repo.path().to_owned(),
    };
    // A new Read fails to compile here until it is compared too.
    let (reads, watches) = match task_diff(None, None) {
        Read::MonitorPs
        | Read::DesktopPlans { .. }
        | Read::DesktopSessions { .. }
        | Read::DesktopTask { .. }
        | Read::TaskChanges { .. }
        | Read::TaskDiff { .. }
        | Read::TaskFile { .. }
        | Read::Waves
        | Read::MachineId
        | Read::Portfolio
        | Read::PaneLayout { .. }
        | Read::People { .. }
        | Read::Graphs { .. }
        | Read::ViewPlan { .. }
        | Read::GraphProcesses { .. }
        | Read::GraphProcess { .. }
        | Read::Usage { .. }
        | Read::TaskContext { .. }
        | Read::WaveDetail { .. }
        | Read::ClosedTasks { .. }
        | Read::Machines
        | Read::History(_)
        | Read::Roadmap { .. }
        | Read::PlanningStatus { .. }
        | Read::WorkLink { .. }
        | Read::RepositoryPath { .. }
        | Read::WaveGraph { .. }
        | Read::SessionPage { .. }
        | Read::Tokens { .. }
        | Read::Retention
        | Read::Doctor
        | Read::BindPreview { .. } => (
            [
                Read::MonitorPs,
                plans(),
                sessions(true),
                task_pane(),
                Read::TaskChanges {
                    issue: "INF-123".into(),
                    base: "parent".into(),
                },
                task_diff(None, None),
                task_diff(Some("edit.txt"), None),
                task_diff(Some("edit.txt"), Some("draft\n")),
                Read::TaskFile {
                    issue: "INF-123".into(),
                    path: "edit.txt".into(),
                },
                Read::Waves,
                Read::MachineId,
                Read::Portfolio,
                Read::PaneLayout {
                    directory: repo.path().to_owned(),
                    work: "repository".into(),
                },
                Read::People {
                    directory: repo.path().to_owned(),
                },
                Read::Graphs {
                    directory: repo.path().to_owned(),
                },
                Read::ViewPlan {
                    directory: repo.path().to_owned(),
                    expression: None,
                },
                Read::ViewPlan {
                    directory: repo.path().to_owned(),
                    expression: Some("state:backlog".into()),
                },
                Read::GraphProcesses {
                    directory: repo.path().to_owned(),
                    task: "INF-123".into(),
                    after: None,
                },
                Read::GraphProcess { id: graph },
                Read::Usage {
                    days: 0,
                    wave: None,
                    project: None,
                    task: Some("INF-123".into()),
                },
                Read::Usage {
                    days: 30,
                    wave: Some("task-pr-tests".into()),
                    project: None,
                    task: None,
                },
                Read::TaskContext {
                    task: "INF-123".into(),
                },
                Read::WaveDetail {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                },
                Read::ClosedTasks {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                    after: None,
                },
                Read::Machines,
                history(),
                all_plans(),
                Read::Roadmap {
                    directory: repo.path().to_owned(),
                    wave: None,
                    task: Some("INF-123".into()),
                    all: false,
                },
                Read::PlanningStatus {
                    directory: repo.path().to_owned(),
                },
                Read::WorkLink {
                    directory: "/".into(),
                    url: format!("loopflow://{folder}/task/INF-123"),
                },
                Read::RepositoryPath {
                    id: serde_json::from_value(repository["id"].clone()).unwrap(),
                },
                Read::WaveGraph {
                    directory: repo.path().to_owned(),
                    wave: Some("task-pr-tests".into()),
                },
                Read::WaveGraph {
                    directory: repo.path().to_owned(),
                    wave: None,
                },
                Read::SessionPage {
                    directory: repo.path().to_owned(),
                    headless: true,
                    after: None,
                },
                Read::Tokens {
                    directory: repo.path().to_owned(),
                    days: None,
                },
                Read::Tokens {
                    directory: repo.path().to_owned(),
                    days: Some(1),
                },
                Read::Retention,
                Read::Doctor,
                Read::BindPreview {
                    session: session["id"].as_str().unwrap().into(),
                    task: "INF-123".into(),
                },
            ],
            [
                history(),
                Read::MonitorPs,
                all_plans(),
                plans(),
                sessions(false),
                sessions(true),
                Read::PlanningStatus {
                    directory: work_folder,
                },
                task_pane(),
            ],
        ),
    };
    // From here every store this process opens is read-only, as in the app:
    // a read that writes fails.
    loopflow::store::serve_reads_only();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    assert!(store
        .provision_planning_user_key("00000000-0000-4000-8000-000000000000")
        .is_err());
    for served in reads {
        let owned = served.argv();
        let argv: Vec<&str> = owned.iter().map(String::as_str).collect();
        let printed = read(repo.path(), home.path(), &argv, served.input());
        let served = served.read().unwrap();
        let Ok(printed) = serde_json::from_str::<Value>(&printed) else {
            // `config repository --path` prints a path, not JSON.
            assert_eq!(served.trim(), printed.trim(), "{argv:?}");
            continue;
        };
        let served: Value = serde_json::from_str(&served).unwrap();
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
    for served in watches {
        let mut argv = served.argv();
        argv.push("--watch".into());
        let (sender, wakes) = mpsc::channel();
        let sender = std::sync::Mutex::new(sender);
        let subscription = served
            .subscribe(move || {
                let _ = sender.lock().unwrap().send(());
            })
            .unwrap();
        // The app pulls on its own thread after a data-free wakeup.
        wakes.recv_timeout(Duration::from_secs(30)).unwrap();
        let served: Value =
            serde_json::from_slice(&subscription.pull().expect("a first frame")).unwrap();
        drop(subscription);
        assert!(served["unavailable"].is_null(), "{argv:?}: {served}");
        let printed = first_frame(repo.path(), home.path(), &argv);
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
}

/// The first frame `lf <args>` prints, with its stdin held open as a watch's
/// parent holds it.
fn first_frame(repo: &Path, home: &Path, args: &[String]) -> Value {
    let mut child = isolated_lf(repo, home, args);
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    serde_json::from_str(&line).unwrap_or_else(|_| panic!("{args:?}: no frame"))
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
