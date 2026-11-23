use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

use loopflow_execution::{durable::TaskId, repository::location::ensure_repository};
use loopflow_model::durable::RepositoryId;
use loopflow_storage::{
    sqlite::SqliteStore,
    work::{edit::Disposition, task::NewTask},
};
use loopflow_test_support::TestRepo;
use serde_json::Value;

use crate::isolated_lf::{isolated_lf, isolated_read};

struct TaskList {
    repo: TestRepo,
    home: tempfile::TempDir,
    store: SqliteStore,
    repository: RepositoryId,
}

impl TaskList {
    fn new() -> Self {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        crate::support::seed_store(home.path());
        let path = home.path().join("loopflow.db");
        let store = loopflow_execution::store::open_sqlite(&path).unwrap();
        let repository = ensure_repository(
            &store,
            repo.path().canonicalize().unwrap().to_str().unwrap(),
        )
        .unwrap();
        Self {
            repo,
            home,
            store,
            repository,
        }
    }

    fn insert(&self, title: &str, days_since_closed: Option<i64>) -> String {
        let key = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, title.as_bytes());
        let task = self
            .store
            .create_task(&NewTask {
                id: TaskId::from_raw(format!("task_{}", key.simple())),
                repository_id: self.repository.clone(),
                project_id: None,
                title: title.into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        if let Some(days) = days_since_closed {
            let closed = time::OffsetDateTime::now_utc() - time::Duration::days(days);
            self.store
                .record_task_disposition(
                    &task.id,
                    Disposition {
                        planning_state: Some("completed".into()),
                        planning_completed: 1,
                        planning_completed_at: Some(
                            closed
                                .format(&time::format_description::well_known::Rfc3339)
                                .unwrap(),
                        ),
                    },
                )
                .unwrap();
        }
        task.identifier
    }

    fn read(&self, args: &[&str]) -> Value {
        serde_json::from_str(&isolated_read(
            self.repo.path(),
            self.home.path(),
            args,
            None,
        ))
        .unwrap()
    }
}

fn names(rows: &Value) -> Vec<&str> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| row["task"]["name"].as_str().unwrap())
        .collect()
}

#[test]
fn open_search_and_done_pages_read_the_same_backlog() {
    let tasks = TaskList::new();
    let issue = tasks.insert("Search Sessions", None);
    tasks.insert("Keyboard shortcuts", None);
    tasks.insert("Search history", Some(2));
    tasks.insert("Older change", Some(12));

    let plan = tasks.read(&["plan", "list", "--all", "--json"]);
    let mut open = names(&plan["tasks"]["items"]);
    open.sort_unstable();
    assert_eq!(open, ["Keyboard shortcuts", "Search Sessions"]);
    for query in ["SEARCH", issue.as_str()] {
        assert_eq!(
            names(&tasks.read(&["task", "list", query, "--json"])),
            ["Search Sessions"]
        );
    }
    let page = tasks.read(&["task", "list", "--closed", "--limit", "1", "--json"]);
    assert_eq!(names(&page["tasks"]), ["Search history"]);
    let rest = tasks.read(&[
        "task",
        "list",
        "--closed",
        "--after",
        page["next"].as_str().unwrap(),
        "--json",
    ]);
    assert_eq!(names(&rest["tasks"]), ["Older change"]);
    assert!(rest["next"].is_null());
    let week = tasks.read(&["task", "list", "--closed", "--since", "7d", "--json"]);
    assert_eq!(names(&week["tasks"]), ["Search history"]);
}

#[test]
fn subscription_sends_changed_rows_and_answers_a_new_query() {
    let tasks = TaskList::new();
    let issue = tasks.insert("Search Sessions", None);
    let mut reader = isolated_lf(
        tasks.repo.path(),
        tasks.home.path(),
        &["task", "list", "--subscribe"],
    );
    let stdout = reader.stdout.take().unwrap();
    let (sender, frames) = mpsc::channel::<Value>();
    let output = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender
                .send(serde_json::from_str(&line.unwrap()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let next = || {
        frames
            .recv_timeout(Duration::from_secs(30))
            .expect("subscription frame")
    };
    let first = next();
    assert!(first["answers"].is_null());
    assert_eq!(names(&first["body"]), ["Search Sessions"]);
    assert_eq!(
        frames.recv_timeout(Duration::from_secs(3)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );

    isolated_read(
        tasks.repo.path(),
        tasks.home.path(),
        &["task", "update", &issue, "--title", "Find Sessions"],
        None,
    );
    assert_eq!(names(&next()["body"]), ["Find Sessions"]);
    reader
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"id\":1,\"args\":{\"query\":\"missing\"}}\n")
        .unwrap();
    let answer = next();
    assert_eq!(answer["answers"], 1);
    assert!(names(&answer["body"]).is_empty());
    assert!(reader.wait().unwrap().success());
    output.join().unwrap();
}

#[test]
fn task_and_session_subscriptions_answer_numbered_requests() {
    let tasks = TaskList::new();
    let issue = tasks.insert("Search Sessions", None);
    for args in [
        ["task", "status", issue.as_str()],
        ["session", "list", "--all"],
    ] {
        let expected = tasks.read(&[args.as_slice(), &["--json"]].concat());
        let stream = isolated_read(
            tasks.repo.path(),
            tasks.home.path(),
            &[args.as_slice(), &["--subscribe"]].concat(),
            Some("{\"id\":7}\n"),
        );
        let answer: Value = serde_json::from_str(stream.lines().last().unwrap()).unwrap();
        assert_eq!(answer["answers"], 7, "{args:?}");
        assert!(answer["unavailable"].is_null(), "{answer}");
        assert_eq!(answer["body"], expected);
    }
}
