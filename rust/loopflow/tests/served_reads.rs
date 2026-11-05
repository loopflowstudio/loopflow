//! Native client operations through the real server match the CLI.

mod isolated_lf;
mod support;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

use isolated_lf::{isolated_lf, isolated_read as read};
use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_with_pr, EnvGuard};

#[test]
fn configuration_readiness_recognizes_an_older_server_for_replacement() {
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let listener = UnixListener::bind(loopflow::server::socket_path().unwrap()).unwrap();
    // A revision-1 listener reads the probe's first bytes as an absurd frame
    // length and closes without answering.
    let older = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut header = [0; 4];
        socket.read_exact(&mut header).unwrap();
        assert_eq!(&header, b"{\"ty");
    });
    let status = loopflow::server::client::status().unwrap();
    older.join().unwrap();
    assert!(!status.ready);
    assert!(status.listening);
    assert!(status.instance.is_none());
    assert!(status.revision.is_none());
    assert!(status.error.unwrap().contains("older protocol"));
    assert!(!home.path().join("loopflow.db").exists());
}

#[test]
fn configuration_edits_use_the_server_without_work_processes() {
    use loopflow::config::api::Request;
    use loopflow::config::files::Destination;
    use loopflow::server::wire::{Operation, ServerOperation};

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    let _server = Server::start(repo.path(), home.path());
    let database = home.path().join("loopflow.db");
    fs::write(&database, "unreadable Work registry").unwrap();
    let before = fs::read(&database).unwrap();
    let status: Value =
        serde_json::from_str(&read(repo.path(), home.path(), &["server", "status"], None)).unwrap();
    assert_eq!(status["ready"], true, "{status}");
    let mut connection = loopflow::server::client::Connection::open().unwrap();
    assert!(
        connection.welcome().machine.is_none(),
        "an unreadable registry leaves the Machine unknown, not the connection refused"
    );
    let mut request = |request| {
        connection
            .request(Operation::Server(ServerOperation::Configuration {
                request,
            }))
            .map(|response| response.result)
            .map_err(|error| error.downcast::<loopflow::engine::Error>().unwrap())
    };
    let catalog = request(Request::Catalog).unwrap();
    assert!(catalog
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["key"] == "theme"));
    fs::write(
        home.path().join("config.yaml"),
        "# retained\ntheme: Linen\n",
    )
    .unwrap();
    let declaration = |value: &str| Request::Declaration {
        destination: Destination::PersonalGlobal {},
        key: "theme".into(),
        value: Some(value.into()),
        unset: false,
    };
    assert_eq!(request(declaration("Hearth")).unwrap()["value"], "Hearth");
    let saved = fs::read_to_string(home.path().join("config.yaml")).unwrap();
    assert!(saved.contains("# retained"));
    assert!(saved.contains("Hearth"));
    let refused = request(Request::Declaration {
        destination: Destination::SharedRepository {
            repository: repo.path().to_owned(),
        },
        key: "user.name".into(),
        value: Some("Wrong scope".into()),
        unset: false,
    })
    .unwrap_err();
    assert!(refused.message.contains("does not apply"), "{refused:?}");
    assert!(!repo.path().join(".lf/config.yaml").exists());
    let theme = request(Request::Theme {
        path: home.path().join("config.yaml"),
        value: None,
    })
    .unwrap();
    assert_eq!(theme["value"], "Hearth");
    assert_eq!(fs::read(&database).unwrap(), before);
}

#[test]
fn served_binding_moves_the_conversation_to_the_task_checkout() {
    use loopflow::lf::commands::write::Write;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file(".lf/config.yaml", "default_agent: codex\n");
    repo.stage_all();
    repo.commit("Binding fixture");
    let destination = repo
        .create_named_worktree("delivery")
        .canonicalize()
        .unwrap();
    let task: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--worktree",
            destination.to_str().unwrap(),
            "--title",
            "Delivery",
            "--json",
        ],
        None,
    ))
    .unwrap();
    let session: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["session", "resume", "--prepare", "--json"],
        None,
    ))
    .unwrap();
    assert_ne!(session["cwd"], destination.to_str().unwrap());
    let _server = Server::start(repo.path(), home.path());
    let before = processes(&home.path().join("loopflow.db"));
    let bound = server_write(Write::SessionBind {
        directory: repo.path().to_owned(),
        id: session["id"].as_str().unwrap().into(),
        task: task["id"].as_str().unwrap().into(),
        preview: false,
    });
    assert_eq!(bound["id"], session["id"]);
    assert_eq!(bound["cwd"], destination.to_str().unwrap());
    assert_eq!(bound["task_primary"], true);
    assert_eq!(processes(&home.path().join("loopflow.db")), before);
}

#[test]
fn served_session_adoption_matches_cli_without_new_execution() {
    use loopflow::lf::commands::write::Write;
    use loopflow::server::wire::Operation;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let host = TestRepo::new();
    let server = Server::start(host.path(), home.path());
    let database = home.path().join("loopflow.db");
    // Exercise both initial creation and repeat adoption through each carrier.
    for served_first in [true, false] {
        let repo = TestRepo::new();
        repo.create_file(".lf/config.yaml", "default_agent: codex\n");
        repo.stage_all();
        repo.commit("Adoption fixture");
        let session: Value = serde_json::from_str(&read(
            repo.path(),
            home.path(),
            &["session", "resume", "--prepare", "--json"],
            None,
        ))
        .unwrap();
        let id = session["id"].as_str().unwrap();
        let change = Write::TaskFromSession {
            directory: repo.path().to_owned(),
            id: id.into(),
            title: "Keep the conversation".into(),
        };
        let cli = || {
            let args = spelled(&change);
            let args: Vec<_> = args.iter().map(String::as_str).collect();
            let before = processes(&database);
            let output = read(repo.path(), home.path(), &args, None);
            assert!(processes(&database) > before, "CLI lost its real Process");
            serde_json::from_str::<Value>(&output).unwrap()
        };
        let served = |change| {
            let before = processes(&database);
            let answer = loopflow::server::client::Connection::open()
                .unwrap()
                .request(Operation::Write(change));
            assert_eq!(processes(&database), before, "request fabricated execution");
            answer
        };
        let adopt = || {
            let answer = served(change.clone()).unwrap();
            let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap();
            assert!(log
                .lines()
                .any(|line| line.contains(answer.trace_id.as_str())
                    && line.contains("request finished")
                    && line.contains(id)
                    && line.contains(&format!("pid={}", server.0.id()))));
            answer.result
        };
        let first = if served_first { adopt() } else { cli() };
        let second = if served_first { cli() } else { adopt() };
        assert_eq!(without_revision(first.clone()), without_revision(second));
        assert_eq!(first["title"], "Keep the conversation");
        assert!(first["project_id"].is_null());
        assert!(
            first["worktree"].is_null(),
            "repository conversation must not adopt the shared checkout"
        );
        let bound: Value = serde_json::from_str(&read(
            repo.path(),
            home.path(),
            &["session", "resume", id, "--prepare", "--json"],
            None,
        ))
        .unwrap();
        assert_eq!(bound["id"], session["id"]);
        assert_eq!(bound["cwd"], session["cwd"]);
        assert_eq!(bound["agent_process_id"], session["agent_process_id"]);
        assert_eq!(bound["task_primary"], true);
        assert_eq!(bound["work"]["id"], first["id"]);
        assert_eq!(bound["task_ids"], serde_json::json!([first["id"]]));
        let connection = rusqlite::Connection::open(&database).unwrap();
        let owner: String = connection
            .query_row(
                "SELECT person_key FROM task_ownership WHERE task_id=?1",
                [first["id"].as_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            owner, "test@test.com",
            "claim must use the Task repository identity"
        );
        let task_count: i64 = connection
            .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        // A failed request has its own trace, changes no Task and launches nothing.
        let refused = served(Write::TaskFromSession {
            directory: repo.path().to_owned(),
            id: id.into(),
            title: " ".into(),
        })
        .unwrap_err()
        .downcast::<loopflow::engine::Error>()
        .unwrap();
        assert!(!refused.message.is_empty());
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            task_count
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM task_graphs", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn server_operations_match_their_lf_commands() {
    use loopflow::lf::commands::activity::HistoryWindow;
    use loopflow::lf::commands::read::Read;
    use loopflow::lf::commands::write::Write;

    let home = tempfile::tempdir().unwrap();
    // Same HOME as `read`'s children: `portfolio` scans `$HOME/src`.
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    // As the children resolve it: `doctor` prints the store's path.
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file(".lf/config.yaml", "default_agent: codex\n");
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
    // The Wave's authored files, seeded and committed once by `lf`: the add
    // compared below then changes nothing, in the app and again through `lf`.
    read(
        repo.path(),
        home.path(),
        &["wave", "add", "task-pr-tests", "--json"],
        None,
    );
    // Prepared, not started: a Session a bind preview can name.
    let session: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["session", "resume", "--prepare", "--json"],
        None,
    ))
    .unwrap();
    // Unpublished, so it can complete: `lf task complete` waits on a PR's merge.
    let unpublished: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Served writes",
            "--json",
        ],
        None,
    ))
    .unwrap();
    let task_id = unpublished["id"].as_str().unwrap().to_owned();
    let unpublished = unpublished["identifier"].as_str().unwrap().to_owned();
    // Unpublished too: abandoning it touches no PR, so needs no GitHub.
    let abandoned: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Abandoned in the app",
            "--json",
        ],
        None,
    ))
    .unwrap();
    let abandoned = abandoned["identifier"].as_str().unwrap().to_owned();
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
    let sessions = |headless| Read::SessionList {
        directory: repo.path().to_owned(),
        headless,
        after: None,
        limit: None,
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
        | Read::SessionList { .. }
        | Read::DesktopTask { .. }
        | Read::TaskChanges { .. }
        | Read::TaskDiff { .. }
        | Read::TaskFile { .. }
        | Read::Waves
        | Read::MachineId
        | Read::Portfolio
        | Read::Settings { .. }
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
        | Read::Tokens { .. }
        | Read::Retention
        | Read::Doctor
        | Read::TaskWait { .. }
        | Read::SessionAsks => (
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
                Read::Settings {
                    directory: repo.path().to_owned(),
                    wave: None,
                    skill: Some("implement".into()),
                    graph: None,
                    step: None,
                },
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
                    closed: true,
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
                Read::SessionList {
                    directory: repo.path().to_owned(),
                    headless: true,
                    after: None,
                    limit: Some(100),
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
                Read::TaskWait {
                    issue: "INF-123".into(),
                    until: loopflow::ops::task::TaskWaitUntil::Terminal,
                },
                Read::SessionAsks,
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
                Read::SessionAsks,
            ],
        ),
    };
    let server = Server::start(repo.path(), home.path());
    let database = home.path().join("loopflow.db");
    // A server read changes no Work and records no Process.
    let unchanged = |argv: &[&str], serve: &mut dyn FnMut()| {
        let before = changes(&database);
        serve();
        assert_eq!(changes(&database), before, "{argv:?} wrote");
    };
    for query in reads {
        let owned = query.argv();
        let argv: Vec<&str> = owned.iter().map(String::as_str).collect();
        let printed = read(repo.path(), home.path(), &argv, query.input());
        let mut served = Value::Null;
        unchanged(&argv, &mut || served = server_read(query.clone()));
        let Ok(printed) = serde_json::from_str::<Value>(&printed) else {
            // `config repository --path` prints a path, not JSON: its value is that text.
            assert_eq!(served, Value::String(printed.trim().into()), "{argv:?}");
            continue;
        };
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
    for served in watches {
        let mut argv = served.argv();
        argv.push("--watch".into());
        let mut frame = Value::Null;
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        unchanged(&words, &mut || {
            let mut connection = loopflow::server::client::Connection::open().unwrap();
            let id = connection.subscribe(served.clone()).unwrap();
            frame = connection.reading(&id).unwrap().expect("a reading");
        });
        let served = frame;
        assert!(served["unavailable"].is_null(), "{argv:?}: {served}");
        let printed = first_frame(repo.path(), home.path(), &argv);
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }

    let directory = repo.path().to_owned();
    let file_revision = || {
        let file = Read::TaskFile {
            issue: "INF-123".into(),
            path: "edit.txt".into(),
        }
        .read()
        .unwrap();
        file["revision"].as_str().unwrap().to_owned()
    };
    let save = |revision: String| Write::TaskSave {
        directory: directory.clone(),
        issue: "INF-123".into(),
        path: "edit.txt".into(),
        revision,
        content: "saved\n".repeat(12_000),
    };
    let session = session["id"].as_str().unwrap().to_owned();
    // What a write changed, as the app reads it.
    let effect = |issue: &str| {
        without_clock(
            Read::Roadmap {
                directory: directory.clone(),
                wave: None,
                task: Some(issue.into()),
                all: false,
            }
            .read()
            .unwrap(),
        )
    };
    let repository_id = repository["id"].as_str().unwrap().to_owned();
    // A saved record as the app reads it, and the revision a write names.
    let layout = |work: &str| -> Value {
        Read::PaneLayout {
            directory: directory.clone(),
            work: work.into(),
        }
        .read()
        .unwrap()
    };
    let revision = |work: &str| layout(work)["revision"].as_i64().unwrap();
    let dashboard = || without_revision(layout("query:served"));
    let pane = |revision: i64| Write::PaneLayout {
        directory: directory.clone(),
        work: "repository".into(),
        layout: serde_json::json!({
            "work": {"repository": repository_id, "kind": "repository", "id": repository_id},
            "revision": revision,
            "tree": {"kind": "pane", "id": "session",
                     "content": {"kind": "session", "session_id": null, "location": null}},
            "focused": null,
            "zoomed": null,
            "hidden": [],
            "drafts": []
        })
        .to_string(),
    };
    let pane_revision = || {
        let saved = layout("repository");
        saved["revision"].as_i64().unwrap_or(0)
    };
    // A new Write fails to compile here until it is compared too.
    let (writes, arrangement) = match (Write::TaskReopen {
        directory: directory.clone(),
        issue: "INF-123".into(),
    }) {
        // Creation has its own two-order comparison in served_session_adoption_matches_cli_without_new_execution.
        Write::TaskFromSession { .. }
        | Write::TaskComplete { .. }
        | Write::TaskReopen { .. }
        | Write::TaskAbandon { .. }
        | Write::TaskNotes { .. }
        | Write::TaskClaim { .. }
        | Write::TaskUnassign { .. }
        | Write::TaskHold { .. }
        | Write::TaskMove { .. }
        // Starting a run or a ship starts Processes; task_run_waits_at_an_interactive_step
        // compares the run, and comments differ by their saved IDs.
        | Write::TaskRun { .. }
        | Write::TaskShip { .. }
        | Write::TaskSteer { .. }
        | Write::TaskComment { .. }
        | Write::TaskInterrupt { .. }
        // An answer types into a live owner; transport_tests proves its delivery.
        | Write::SessionAnswer { .. }
        | Write::TaskSave { .. }
        | Write::SessionRename { .. }
        | Write::SessionBind { .. }
        | Write::SessionResume { .. }
        | Write::SessionNew { .. }
        | Write::WaveAdd { .. }
        | Write::PlanningRefresh { .. }
        | Write::NewChapter { .. }
        | Write::GraphCustomize { .. }
        | Write::Portfolio { .. }
        | Write::DashboardQuery { .. }
        | Write::DashboardPin { .. }
        | Write::DashboardUnpin { .. }
        | Write::DashboardRename { .. }
        | Write::DashboardHide { .. }
        | Write::DashboardShow { .. }
        | Write::DashboardSlots { .. }
        | Write::TileMove { .. }
        | Write::PaneLayout { .. } => (
            [
                Write::TaskNotes {
                    directory: directory.clone(),
                    issue: "INF-123".into(),
                    notes: "Served from the app".into(),
                },
                Write::TaskMove {
                    directory: directory.clone(),
                    issue: "INF-123".into(),
                    wave: None,
                },
                Write::SessionRename {
                    directory: directory.clone(),
                    id: session.clone(),
                    name: "Served name".into(),
                },
                Write::SessionBind {
                    directory: directory.clone(),
                    id: session.clone(),
                    task: "INF-123".into(),
                    preview: false,
                },
                // The repository's conversation, one Session and its fresh
                // thread: prepared, never started.
                Write::SessionResume {
                    directory: directory.clone(),
                    id: None,
                    task: None,
                    wave: None,
                    agent: None,
                },
                Write::SessionResume {
                    directory: directory.clone(),
                    id: Some(session.clone()),
                    task: None,
                    wave: None,
                    agent: None,
                },
                Write::SessionNew {
                    directory: directory.clone(),
                    id: session.clone(),
                    agent: None,
                },
                Write::TaskComplete {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::TaskReopen {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::WaveAdd {
                    directory: directory.clone(),
                    wave: "task-pr-tests".into(),
                    objective: None,
                },
                Write::PlanningRefresh {
                    directory: directory.clone(),
                    wave: "task-pr-tests".into(),
                },
                Write::TaskAbandon {
                    directory: directory.clone(),
                    issue: abandoned.clone(),
                },
            ],
            // Each names the revision its last write left, so it is built again
            // before `lf` repeats it.
            [
                // The fixture's Working Tasks hold no Task; the shared order
                // moves through the same tile writer as a Dashboard's.
                Box::new(|| Write::TileMove {
                    directory: directory.clone(),
                    board: "query:served".into(),
                    member: format!("task:{task_id}"),
                    destination: format!("task:{task_id}"),
                }) as Box<dyn Fn() -> Write>,
                Box::new(|| Write::DashboardPin {
                    directory: directory.clone(),
                    id: "served".into(),
                    revision: revision("query:served"),
                }),
                Box::new(|| Write::DashboardRename {
                    directory: directory.clone(),
                    id: "served".into(),
                    name: "Served board".into(),
                    revision: revision("query:served"),
                }),
                // A hidden tile is no longer one to hide: show it before each hide.
                Box::new(|| {
                    let _ = loopflow::server::client::Connection::open().and_then(|mut c| {
                        c.request(loopflow::server::wire::Operation::Write(
                            Write::DashboardShow {
                                directory: directory.clone(),
                                id: "served".into(),
                                member: format!("task:{task_id}"),
                            },
                        ))
                    });
                    Write::DashboardHide {
                        directory: directory.clone(),
                        id: "served".into(),
                        member: format!("task:{task_id}"),
                    }
                }),
                Box::new(|| Write::DashboardSlots {
                    directory: directory.clone(),
                    id: "served".into(),
                    revision: revision("query:served"),
                    slots: "null".into(),
                }),
                Box::new(|| Write::DashboardQuery {
                    directory: directory.clone(),
                    id: "served".into(),
                    expression: "state:backlog".into(),
                    revision: Some(revision("query:served")),
                }),
                Box::new(|| pane(pane_revision())),
            ],
        ),
    };
    // The same write through the server, then through `lf`: each prints the same,
    // leaves the same Work. Only the CLI is another OS Process; requests do
    // not manufacture Process rows or borrow the server launcher's attribution.
    let compare = |served: Write, again: &dyn Fn() -> Write, effect: &dyn Fn() -> Value| {
        let argv = spelled(&served);
        let before = processes(&database);
        let written = server_write(served);
        assert_eq!(
            processes(&database),
            before,
            "served write fabricated a Process: {argv:?}"
        );
        // Nothing it started outlives it: no provider, no child of the app.
        assert_eq!(children().trim(), server.0.id().to_string(), "{argv:?}");
        let effected = effect();
        let again = again();
        let args = spelled(&again);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let before = processes(&database);
        let printed = read(repo.path(), home.path(), &args, again.input());
        let recorded = processes(&database) - before;
        assert_eq!(effect(), effected, "{argv:?}");
        // The CLI prints with `--json` exactly what the server returned.
        let printed: Value = serde_json::from_str(&printed)
            .unwrap_or_else(|error| panic!("{argv:?} printed no JSON ({error}): {printed}"));
        assert_eq!(
            without_times(without_save_time(without_clock(written))),
            without_times(without_save_time(without_clock(printed))),
            "{argv:?}"
        );
        recorded
    };
    for write in writes {
        let issue = match &write {
            Write::TaskComplete { issue, .. }
            | Write::TaskReopen { issue, .. }
            | Write::TaskAbandon { issue, .. } => issue.clone(),
            _ => "INF-123".to_owned(),
        };
        let recorded = compare(write.clone(), &|| write.clone(), &|| effect(&issue));
        assert!(recorded > 0, "{:?} recorded no Process", spelled(&write));
    }
    // A save names the revision it replaces, which the first save changed.
    compare(save(file_revision()), &|| save(file_revision()), &|| {
        effect("INF-123")
    });
    // Claiming and unassigning leave the same owner, which each prints from the
    // store. Only the CLI records a Process. Every claim appends to the ownership
    // log, so the log itself differs.
    for claimed in [true, false] {
        let (directory, issue) = (directory.clone(), "INF-123".to_owned());
        let ownership = if claimed {
            Write::TaskClaim { directory, issue }
        } else {
            Write::TaskUnassign { directory, issue }
        };
        let before = processes(&database);
        let written = server_write(ownership.clone());
        assert_eq!(
            processes(&database),
            before,
            "served ownership fabricated a Process"
        );
        let args = spelled(&ownership);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let before = processes(&database);
        let printed: Value =
            serde_json::from_str(&read(repo.path(), home.path(), &args, None)).unwrap();
        let recorded = processes(&database) - before;
        assert!(recorded > 0, "{args:?} recorded no Process");
        assert_eq!(written["owner"], printed["owner"], "{args:?}");
        assert_eq!(written["owner"].is_null(), !claimed, "{written}");
    }
    // Stopping a Task that is not shipping saves the stop, which keeps a run
    // from approving it until `lf ship`, retaining who requested the stop. The
    // ship process it kills and the merge request it takes back are the ship
    // tests'.
    // No run is live, so an interrupt reaches nothing, in the app as through `lf`.
    let interrupt = Write::TaskInterrupt {
        directory: directory.clone(),
        issue: "INF-123".into(),
    };
    compare(interrupt.clone(), &|| interrupt.clone(), &|| {
        effect("INF-123")
    });
    // A steer and a comment save the same text by the same author; each save
    // has its own ID.
    for steer in [true, false] {
        let (directory, issue, message) = (
            directory.clone(),
            "INF-123".to_owned(),
            format!("served {steer}"),
        );
        let saved = if steer {
            Write::TaskSteer {
                directory,
                issue,
                message,
            }
        } else {
            Write::TaskComment {
                directory,
                issue,
                message,
            }
        };
        let last = |thread: &Value| {
            let comment = thread["comments"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .clone();
            let body = comment["body"].as_str().unwrap();
            (
                body[..body.find("<!--").unwrap_or(body.len())]
                    .trim()
                    .to_owned(),
                comment["author"].clone(),
            )
        };
        let before = processes(&database);
        let written = server_write(saved.clone());
        assert_eq!(
            processes(&database),
            before,
            "served comment fabricated a Process"
        );
        let args = spelled(&saved);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let printed: Value =
            serde_json::from_str(&read(repo.path(), home.path(), &args, None)).unwrap();
        assert_eq!(last(&written), last(&printed), "{args:?}");
        assert_eq!(last(&written).0, format!("served {steer}"));
    }
    let stop = Write::TaskHold {
        directory: directory.clone(),
        issue: "INF-123".into(),
    };
    let stopped = || {
        Read::DesktopTask {
            directory: directory.clone(),
            issue: "INF-123".into(),
        }
        .read()
        .unwrap()["execution"]["ship_stop"]["by"]
            .clone()
    };
    assert!(compare(stop.clone(), &|| stop.clone(), &stopped) > 0);
    assert_eq!(stopped()["kind"], "person", "no stop saved");

    // Which repositories the app shows, as the app reads them.
    let portfolio = || without_clock(Read::Portfolio.read().unwrap());
    let manual = Write::Portfolio {
        change: loopflow::portfolio::PortfolioCommand::Manual,
    };
    assert!(compare(manual.clone(), &|| manual.clone(), &portfolio) > 0);
    // A customized Graph is the builtin's copy in the repository.
    let customized = || {
        Value::from(
            fs::read_to_string(repo.path().join(".lf/graphs/code.yaml")).unwrap_or_default(),
        )
    };
    let customize = Write::GraphCustomize {
        directory: directory.clone(),
        name: "code".into(),
    };
    // `lf` writes its own copy, not the app's.
    let recustomize = || {
        fs::remove_file(repo.path().join(".lf/graphs/code.yaml")).unwrap();
        customize.clone()
    };
    assert_eq!(compare(customize.clone(), &recustomize, &customized), 0);
    // A chapter preview reads the rotation it would make and records nothing.
    let waves = Read::Waves.read().unwrap();
    let wave_id = waves
        .as_array()
        .unwrap()
        .iter()
        .find(|wave| wave.to_string().contains("\"task-pr-tests\""))
        .and_then(|wave| wave["id"].as_str())
        .unwrap_or_else(|| panic!("no task-pr-tests Wave in {waves}"))
        .to_owned();
    let preview = Write::NewChapter {
        directory: directory.clone(),
        name: "October".into(),
        plan: serde_json::json!({
            "name": "October",
            "waves": [{
                "wave_id": wave_id,
                "successor_id": "proj_4f6c2b8e-3d5a-4c1e-9b7a-2e8d6f0a1c3b",
                "create": true,
                "project_name": "October",
                "krs": [{"text": "Desktop writes through the server", "holds": false}],
                "metric_targets": []
            }]
        })
        .to_string(),
        preview: true,
    };
    assert_eq!(
        compare(preview.clone(), &|| preview.clone(), &|| effect("INF-123")),
        0
    );
    // A bind preview names the Task a bind would assign and assigns nothing.
    let bind_preview = Write::SessionBind {
        directory: directory.clone(),
        id: session.clone(),
        task: "INF-123".into(),
        preview: true,
    };
    assert_eq!(
        compare(bind_preview.clone(), &|| bind_preview.clone(), &|| effect(
            "INF-123"
        )),
        0
    );
    // Arrangement persists without a Process, in the app as through `lf`.
    let create = Write::DashboardQuery {
        directory: directory.clone(),
        id: "served".into(),
        expression: format!("task:{task_id}"),
        revision: None,
    };
    let created = server_write(create);
    assert_eq!(layout("query:served")["id"], "served", "{created}");
    for write in arrangement {
        let recorded = compare(write(), &*write, &dashboard);
        assert_eq!(recorded, 0, "{:?} recorded a Process", spelled(&write()));
    }
    // A refused write names its code and changes nothing; the CLI refuses
    // the same spelling. The default diagnostic log includes the request
    // trace, target, outcome and real server PID.
    use loopflow::engine::ErrorCode;
    use loopflow::server::wire::Operation;
    let failure = Write::SessionRename {
        directory: directory.clone(),
        id: session.clone(),
        name: " ".into(),
    };
    let refused = loopflow::server::client::Connection::open()
        .unwrap()
        .request(Operation::Write(failure.clone()))
        .unwrap_err()
        .downcast::<loopflow::engine::Error>()
        .unwrap();
    assert_eq!(refused.code, ErrorCode::Invalid, "{refused:?}");
    let cli = isolated_lf(repo.path(), home.path(), &spelled(&failure))
        .wait_with_output()
        .unwrap();
    assert_ne!(cli.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&cli.stderr).contains(&refused.message));
    let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap();
    assert!(log.lines().any(|line| line.contains("request finished")
        && line.contains("code=1")
        && line.contains(&session)
        && line.contains(&format!("pid={}", server.0.id()))));

    // Concurrent edits name distinct durable Sessions and never create an OS
    // Process per request. Each returned trace resolves to only its own target.
    let other_repo = TestRepo::new();
    other_repo.create_file(".lf/config.yaml", "default_agent: codex\n");
    let other: Value = serde_json::from_str(&read(
        other_repo.path(),
        home.path(),
        &["session", "resume", "--prepare", "--json"],
        None,
    ))
    .unwrap();
    let other = other["id"].as_str().unwrap().to_owned();
    let before = processes(&database);
    let traces = std::thread::scope(|threads| {
        let requests: Vec<_> = [(&session, "First request"), (&other, "Second request")]
            .into_iter()
            .map(|(id, name)| {
                let change = Write::SessionRename {
                    directory: directory.clone(),
                    id: id.clone(),
                    name: name.into(),
                };
                threads.spawn(move || {
                    let loopflow::server::client::Response {
                        trace_id,
                        result: saved,
                    } = loopflow::server::client::Connection::open()
                        .unwrap()
                        .request(Operation::Write(change))
                        .unwrap();
                    assert_eq!(saved["id"], *id);
                    assert_eq!(saved["title"], name);
                    (trace_id, id.clone())
                })
            })
            .collect();
        requests
            .into_iter()
            .map(|request| request.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(processes(&database), before);
    assert_ne!(traces[0].0, traces[1].0);
    let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap();
    for (trace, target) in &traces {
        let lines: Vec<_> = log
            .lines()
            .filter(|line| line.contains(trace.as_str()))
            .collect();
        assert!(lines
            .iter()
            .any(|line| line.contains("request started") && line.contains(target)));
        assert!(lines
            .iter()
            .any(|line| line.contains("request finished") && line.contains("code=0")));
    }

    // A provider transcript without a usable cwd adopts the request's directory,
    // even when another request prepares a conversation in a different repository.
    let before = processes(&database);
    std::thread::scope(|threads| {
        let requests: Vec<_> = [("codex", repo.path()), ("claude", other_repo.path())]
            .into_iter()
            .map(|(provider, directory)| {
                let id = uuid::Uuid::new_v4().to_string();
                let transcript = if provider == "codex" {
                    home.path().join(format!(
                        ".codex/sessions/2026/11/04/rollout-time-{id}.jsonl"
                    ))
                } else {
                    home.path()
                        .join(format!(".claude/projects/test/{id}.jsonl"))
                };
                fs::create_dir_all(transcript.parent().unwrap()).unwrap();
                fs::write(transcript, "{}\n").unwrap();
                threads.spawn(move || {
                    let saved = server_write(Write::SessionResume {
                        directory: directory.to_owned(),
                        id: Some(id),
                        task: None,
                        wave: None,
                        agent: None,
                    });
                    assert_eq!(
                        Path::new(saved["cwd"].as_str().unwrap())
                            .canonicalize()
                            .unwrap(),
                        directory.canonicalize().unwrap()
                    );
                    assert_eq!(saved["provider"], provider);
                })
            })
            .collect();
        for request in requests {
            request.join().unwrap();
        }
    });
    assert_eq!(processes(&database), before);

    // An admitted request survives a lost reply. Never resend it: inspect the
    // persisted result after dropping the connection while the write is held.
    let lock = rusqlite::Connection::open(&database).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut connection = loopflow::server::client::Connection::open().unwrap();
    connection
        .send(Operation::Write(Write::SessionRename {
            directory: directory.clone(),
            id: other.clone(),
            name: "After disconnect".into(),
        }))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap();
        if log
            .lines()
            .any(|line| line.contains("request started") && line.contains("After disconnect"))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "write was not admitted: {log}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(connection);
    lock.execute_batch("ROLLBACK").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let name: String = lock
            .query_row(
                "SELECT title FROM agent_sessions WHERE id=?1",
                [&other],
                |row| row.get(0),
            )
            .unwrap();
        if name == "After disconnect" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "admitted write did not finish: {}",
            fs::read_to_string(home.path().join("logs/lf.log")).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // Last, since the detached run's Processes finish on their own clock:
    // `task.run` starts `lf task run` detached and answers what it reports.
    // This fixture's Task has no launchable planning, so the run it started
    // reports the refusal `lf task run` prints, and nothing ran.
    let run = Write::TaskRun {
        directory: directory.clone(),
        issue: "INF-123".into(),
        from: Some("demo".into()),
        agent: None,
        approve: false,
    };
    let refused = loopflow::server::client::Connection::open()
        .unwrap()
        .request(loopflow::server::wire::Operation::Write(run))
        .unwrap_err()
        .downcast::<loopflow::engine::Error>()
        .unwrap();
    assert!(
        refused
            .message
            .contains("Task planning has invalid ownership"),
        "{refused:?}"
    );
}

/// The processes this test started that are still running.
fn children() -> String {
    let output = std::process::Command::new("pgrep")
        .args(["-P", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap()
}

/// A saved record without its revision, which each save advances.
fn without_revision(mut value: Value) -> Value {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("revision");
    }
    value
}

/// Each Work domain's revision and the Processes recorded.
fn changes(database: &Path) -> (Vec<(String, i64)>, i64) {
    let conn = rusqlite::Connection::open(database).unwrap();
    let revisions = conn
        .prepare("SELECT domain, revision FROM store_revisions ORDER BY domain")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let processes = conn
        .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
        .unwrap();
    (revisions, processes)
}

/// A save without its recoveries' directories, which each save names by its time.
fn without_save_time(mut value: Value) -> Value {
    if let Some(recoveries) = value
        .pointer_mut("/file/recoveries")
        .and_then(Value::as_array_mut)
    {
        for recovery in recoveries {
            recovery.as_object_mut().unwrap().remove("directory");
        }
    }
    value
}

/// The `processes` revision: every Process recorded or forgotten moves it.
fn processes(database: &Path) -> i64 {
    rusqlite::Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT revision FROM store_revisions WHERE domain='processes'",
            [],
            |row| row.get(0),
        )
        .unwrap()
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

struct Server(std::process::Child);

impl Server {
    fn start(repo: &Path, home: &Path) -> Self {
        let server = Self(isolated_lf(repo, home, &["server", "run"]));
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        // Readiness as a client sees it; `status()` would compare the server's
        // executable with this test binary.
        while loopflow::server::client::Connection::open().is_err() {
            assert!(std::time::Instant::now() < deadline, "server did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        server
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn server_read(query: loopflow::lf::commands::read::Read) -> Value {
    loopflow::server::client::Connection::open()
        .unwrap()
        .request(loopflow::server::wire::Operation::Read(query))
        .unwrap()
        .result
}

fn server_write(change: loopflow::lf::commands::write::Write) -> Value {
    loopflow::server::client::Connection::open()
        .unwrap()
        .request(loopflow::server::wire::Operation::Write(change))
        .unwrap()
        .result
}

/// The `lf` command each write equals, printing with `--json` what the
/// server returns for it.
fn spelled(write: &loopflow::lf::commands::write::Write) -> Vec<String> {
    use loopflow::lf::commands::write::Write;
    use loopflow::portfolio::PortfolioCommand;
    let path = |path: &Path| path.display().to_string();
    let (directory, words): (String, Vec<String>) = match write {
        Write::TaskFromSession {
            directory,
            id,
            title,
        } => (
            path(directory),
            words(&[
                "task",
                "create",
                "--session",
                id,
                "--title",
                title,
                "--json",
            ]),
        ),
        Write::TaskComplete { directory, issue } => (
            path(directory),
            words(&["task", "complete", issue, "--json"]),
        ),
        Write::TaskReopen { directory, issue } => {
            (path(directory), words(&["task", "reopen", issue, "--json"]))
        }
        Write::TaskNotes {
            directory,
            issue,
            notes,
        } => (
            path(directory),
            words(&["task", "edit", issue, &format!("--notes={notes}"), "--json"]),
        ),
        Write::TaskAbandon { directory, issue } => (
            path(directory),
            words(&["task", "abandon", issue, "--json"]),
        ),
        Write::TaskClaim { directory, issue } => {
            (path(directory), words(&["task", "claim", issue, "--json"]))
        }
        Write::TaskUnassign { directory, issue } => (
            path(directory),
            words(&["task", "unassign", issue, "--json"]),
        ),
        Write::TaskHold { directory, issue } => (
            path(directory),
            words(&["--task", issue, "ship", "--stop", "--json"]),
        ),
        Write::TaskRun {
            directory,
            issue,
            from,
            agent,
            approve,
        } => {
            let mut spelled = Vec::new();
            if *approve {
                spelled.push("--batch".to_owned());
            }
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["task", "run", issue]));
            if let Some(from) = from {
                spelled.extend(words(&["--from", from]));
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::TaskShip { directory, issue } => {
            (path(directory), words(&["--task", issue, "ship", "--json"]))
        }
        Write::TaskSteer {
            directory,
            issue,
            message,
        } => (
            path(directory),
            words(&["task", "comment", issue, "--steer", message, "--json"]),
        ),
        Write::TaskComment {
            directory,
            issue,
            message,
        } => (
            path(directory),
            words(&["task", "comment", issue, message, "--json"]),
        ),
        Write::TaskInterrupt { directory, issue } => (
            path(directory),
            words(&["task", "interrupt", issue, "--json"]),
        ),
        Write::TaskMove {
            directory,
            issue,
            wave,
        } => {
            let mut spelled = words(&["task", "move", issue]);
            match wave {
                Some(wave) => spelled.extend(words(&["--wave", wave])),
                None => spelled.push("--no-wave".into()),
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::TaskSave {
            directory,
            issue,
            path: file,
            revision,
            ..
        } => (
            path(directory),
            words(&[
                "task",
                "save",
                issue,
                file,
                "--revision",
                revision,
                "--json",
            ]),
        ),
        Write::SessionRename {
            directory,
            id,
            name,
        } => (
            path(directory),
            words(&["session", "rename", "--json", "--", id, name]),
        ),
        Write::SessionBind {
            directory,
            id,
            task,
            preview,
        } => {
            let mut spelled = words(&["session", "bind", "--json", "--task", task]);
            if *preview {
                spelled.push("--dry-run".into());
            }
            spelled.extend(words(&["--", id]));
            (path(directory), spelled)
        }
        Write::SessionResume {
            directory,
            id,
            task,
            wave,
            agent,
        } => {
            let mut spelled = Vec::new();
            if id.is_none() && task.is_none() && wave.is_none() {
                spelled.extend(words(&["--repo", &path(directory)]));
            }
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["session", "resume"]));
            spelled.extend(id.clone());
            if let Some(task) = task {
                spelled.extend(words(&["--task", task]));
            }
            if let Some(wave) = wave {
                spelled.extend(words(&["--wave", wave]));
            }
            spelled.extend(words(&["--prepare", "--json"]));
            (path(directory), spelled)
        }
        Write::SessionNew {
            directory,
            id,
            agent,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["session", "new", id, "--prepare", "--json"]));
            (path(directory), spelled)
        }
        Write::SessionAnswer {
            session,
            text,
            submit,
        } => {
            let mut spelled = words(&["session", "answer", session, text]);
            if *submit {
                spelled.push("--submit".into());
            }
            spelled.push("--json".into());
            ("/".into(), spelled)
        }
        Write::WaveAdd {
            directory,
            wave,
            objective,
        } => {
            let mut spelled = words(&["wave", "add", wave]);
            if let Some(objective) = objective {
                spelled.extend(words(&["--objective", objective]));
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PlanningRefresh { directory, wave } => (
            path(directory),
            words(&["wave", "status", wave, "--sync", "--json"]),
        ),
        Write::NewChapter {
            directory,
            name,
            preview,
            ..
        } => {
            let mut spelled = words(&[
                "wave",
                "new-chapter",
                "--all",
                name,
                "--plan",
                "/dev/stdin",
                "--json",
            ]);
            if *preview {
                spelled.push("--dry-run".into());
            }
            (path(directory), spelled)
        }
        Write::GraphCustomize { directory, name } => (
            path(directory),
            words(&["graph", "customize", name, "--json"]),
        ),
        Write::Portfolio { change } => {
            let mut spelled = vec!["portfolio".to_owned()];
            match change {
                PortfolioCommand::Folder { path: folder } => {
                    spelled.push("folder".into());
                    spelled.extend(folder.as_deref().map(path));
                }
                PortfolioCommand::Manual => spelled.push("manual".into()),
                PortfolioCommand::Add { paths } | PortfolioCommand::Import { paths } => {
                    let verb = if matches!(change, PortfolioCommand::Add { .. }) {
                        "add"
                    } else {
                        "import"
                    };
                    spelled.push(verb.into());
                    spelled.extend(paths.iter().map(|each| path(each)));
                }
                PortfolioCommand::Remove { path: removed } => {
                    spelled.extend(["remove".into(), path(removed)]);
                }
            }
            spelled.push("--json".into());
            ("/".into(), spelled)
        }
        Write::DashboardQuery {
            directory,
            id,
            expression,
            revision,
        } => {
            let mut spelled = words(&["desktop", "query"]);
            spelled.extend(revision.map(|revision| format!("--revision={revision}")));
            spelled.extend(words(&["--", id, expression]));
            (path(directory), spelled)
        }
        Write::DashboardPin {
            directory,
            id,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "pin",
                &format!("--revision={revision}"),
                "--",
                id,
            ]),
        ),
        Write::DashboardUnpin {
            directory,
            id,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "unpin",
                &format!("--revision={revision}"),
                "--",
                id,
            ]),
        ),
        Write::DashboardRename {
            directory,
            id,
            name,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "rename",
                &format!("--revision={revision}"),
                "--",
                &format!("query:{id}"),
                name,
            ]),
        ),
        Write::DashboardHide {
            directory,
            id,
            member,
        } => (
            path(directory),
            words(&["desktop", "hide", "-p", &format!("query:{id}/{member}")]),
        ),
        Write::DashboardShow {
            directory,
            id,
            member,
        } => (
            path(directory),
            words(&["desktop", "show", "-p", &format!("query:{id}/{member}")]),
        ),
        Write::DashboardSlots {
            directory,
            id,
            revision,
            ..
        } => (
            path(directory),
            words(&[
                "desktop",
                "layout",
                &format!("query:{id}"),
                "--save",
                &format!("--revision={revision}"),
            ]),
        ),
        Write::TileMove {
            directory,
            board,
            member,
            destination,
        } => (
            path(directory),
            words(&[
                "desktop",
                "move",
                "-p",
                &format!("{board}/{member}"),
                "--destination",
                destination,
            ]),
        ),
        Write::PaneLayout {
            directory, work, ..
        } => (
            path(directory),
            words(&["desktop", "layout", work, "--save"]),
        ),
    };
    let mut argv = vec!["--cwd".to_owned(), directory];
    argv.extend(words);
    argv
}

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// `value` without the times each write stamps when it runs.
fn without_times(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            fields.retain(|key, _| !key.ends_with("_at") && key != "revision");
            for field in fields.values_mut() {
                *field = without_times(field.take());
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = without_times(item.take());
            }
        }
        _ => {}
    }
    value
}
