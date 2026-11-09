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
fn describe_prints_the_protocol_with_no_server_and_no_store() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let described: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["server", "describe", "--json"],
        None,
    ))
    .unwrap();
    assert_eq!(described, loopflow::server::schema::describe());
    let summary = read(repo.path(), home.path(), &["server", "describe"], None);
    assert!(summary.starts_with("Server protocol revision"), "{summary}");
    assert!(!home.path().join("loopflow.db").exists());
}

#[test]
fn a_client_that_closes_at_once_leaves_the_server_listening() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    let mut server = Server::start(repo.path(), home.path());
    let socket = loopflow::server::socket_path().unwrap();
    for _ in 0..2000 {
        // A full listen queue refuses; only the server's exit fails the test.
        if std::os::unix::net::UnixStream::connect(&socket).is_err() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert!(server.0.try_wait().unwrap().is_none(), "the server exited");
    // Closed clients leave as their connections notice; until then it may be `busy`.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match loopflow::server::client::Connection::open() {
            Ok(_) => break,
            Err(error) => {
                let busy = error
                    .downcast_ref::<loopflow::engine::Error>()
                    .is_some_and(|error| error.code == loopflow::engine::ErrorCode::Busy);
                assert!(busy, "{error:#}");
                assert!(std::time::Instant::now() < deadline, "{error:#}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn configuration_edits_use_the_server_without_work_processes() {
    use loopflow::config::files::Destination;
    use loopflow::lf::commands::read::Read;
    use loopflow::lf::commands::write::Write;
    use loopflow::server::wire::Operation;

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
    let mut request = |operation| {
        connection
            .request(operation)
            .map(|response| response.result)
            .map_err(|error| error.downcast::<loopflow::engine::Error>().unwrap())
    };
    let catalog = request(Operation::Read(Read::ConfigCatalog)).unwrap();
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
    let declaration = |value: &str| {
        Operation::Write(Write::ConfigSet {
            destination: Destination::PersonalGlobal {},
            key: "theme".into(),
            value: value.into(),
        })
    };
    assert_eq!(request(declaration("Hearth")).unwrap()["value"], "Hearth");
    let saved = fs::read_to_string(home.path().join("config.yaml")).unwrap();
    assert!(saved.contains("# retained"));
    assert!(saved.contains("Hearth"));
    let refused = request(Operation::Write(Write::ConfigSet {
        destination: Destination::SharedRepository {
            repository: repo.path().to_owned(),
        },
        key: "user.name".into(),
        value: "Wrong scope".into(),
    }))
    .unwrap_err();
    assert!(refused.message.contains("does not apply"), "{refused:?}");
    assert!(!repo.path().join(".lf/config.yaml").exists());
    let theme = request(Operation::Write(Write::ConfigTheme {
        path: home.path().join("config.yaml"),
        value: None,
    }))
    .unwrap();
    assert_eq!(theme["value"], "Hearth");
    assert_eq!(fs::read(&database).unwrap(), before);
}

#[test]
fn account_holds_and_rotation_use_the_same_machine_state_as_the_cli() {
    use loopflow::lf::commands::read::Read;
    use loopflow::lf::commands::write::Write;

    // Codex puts its Unix socket below HOME; macOS temp roots can exceed SUN_LEN.
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    support::register_codex_login(home.path(), "work");
    let repo = TestRepo::new();
    let _server = Server::start(repo.path(), home.path());
    let report = || {
        server_read(Read::AccountList {
            target: Some("codex=work@".into()),
            cached: true,
        })
    };
    let login = "codex=work@".to_owned();
    assert!(server_write(Write::AccountPause {
        login: login.clone()
    })
    .is_null());
    assert_eq!(report()["accounts"][0]["routing"], "explicit_only");
    read(
        repo.path(),
        home.path(),
        &["account", "unpause", &login],
        None,
    );
    assert_eq!(report()["accounts"][0]["routing"], "automatic");
    assert!(server_write(Write::AccountUse {
        login: login.clone()
    })
    .is_null());
    assert_eq!(report()["accounts"][0]["held"], true);
    let cli: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["account", &login, "--cached", "--json"],
        None,
    ))
    .unwrap();
    assert_eq!(report(), cli);
    let next = server_read(Read::AccountNext {
        directory: repo.path().to_owned(),
        repo: None,
        default: true,
    });
    let cli: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["account", "next", "--default", "--json"],
        None,
    ))
    .unwrap();
    assert_eq!(next, cli);
    assert!(server_write(Write::AccountUse {
        login: "auto".into()
    })
    .is_null());
    assert_eq!(report()["accounts"][0]["held"], false);
    read(
        repo.path(),
        home.path(),
        &["account", "pause", &login],
        None,
    );
    assert_eq!(report()["accounts"][0]["routing"], "explicit_only");
    assert!(server_write(Write::AccountUnpause { login }).is_null());
    assert_eq!(report()["accounts"][0]["routing"], "automatic");
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
    let task: Value = serde_json::from_str::<serde_json::Value>(&read(
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
    .unwrap()["task"]
        .clone();
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
        let change = Write::TaskCreate {
            directory: repo.path().to_owned(),
            session: Some(id.into()),
            title: Some("Keep the conversation".into()),
            request_id: format!("adoption-{served_first}"),
            request: None,
            wave: None,
            graph: None,
            run: false,
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
        let first = &first["task"];
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
        let refused = served(Write::TaskCreate {
            directory: repo.path().to_owned(),
            session: Some(id.into()),
            title: Some(" ".into()),
            request_id: "bad-adoption".into(),
            request: None,
            wave: None,
            graph: None,
            run: false,
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

/// A client that knows only what `lf server describe --json` prints, as a
/// phone or a headless agent client would: raw JSON lines, no Rust types.
/// Everything it sends and every line it receives match the description, and
/// each answer matches its operation's declared result.
#[test]
fn a_client_built_from_the_description_alone_speaks_the_protocol() {
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    let schema: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["server", "describe", "--json"],
        None,
    ))
    .unwrap();
    let definition = |name: &str| {
        let mut root = serde_json::json!({ "$ref": format!("#/$defs/{name}") });
        root["$defs"] = schema["$defs"].clone();
        jsonschema::draft202012::new(&root).unwrap()
    };
    let (client, server) = (definition("ClientMessage"), definition("ServerMessage"));
    let declared = |kind: &str| {
        let arm = ["Read", "Write", "ServerOperation"]
            .iter()
            .flat_map(|group| schema["$defs"][*group]["oneOf"].as_array().unwrap())
            .find(|arm| arm["properties"]["kind"]["const"] == kind)
            .unwrap_or_else(|| panic!("{kind} is not described"));
        let mut result = arm["x-result"].clone();
        result["$defs"] = schema["$defs"].clone();
        jsonschema::draft202012::new(&result).unwrap()
    };
    let conforms = |validator: &jsonschema::Validator, value: &Value| {
        let failed: Vec<_> = validator
            .iter_errors(value)
            .map(|e| e.to_string())
            .collect();
        assert!(failed.is_empty(), "{value}: {failed:#?}");
    };

    let _server = Server::start(repo.path(), home.path());
    let socket = UnixStream::connect(loopflow::server::socket_path().unwrap()).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    let mut lines = BufReader::new(socket.try_clone().unwrap()).lines();
    let mut writer = socket;
    // `described` is false only for the line that deliberately names an
    // operation the description lacks.
    let mut write = |message: Value, described: bool| {
        if described {
            conforms(&client, &message);
        }
        writeln!(writer, "{message}").unwrap();
    };
    let mut receive = || -> Option<Value> {
        let line = lines.next()?.unwrap();
        let message: Value = serde_json::from_str(&line).unwrap();
        conforms(&server, &message);
        Some(message)
    };

    let revision = schema["x-revision"].clone();
    write(
        serde_json::json!({"type": "hello", "revision": revision, "min_revision": revision,
        "client": {"kind": "phone", "platform": "ios", "name": "described", "version": "0"}}),
        true,
    );
    assert_eq!(receive().unwrap()["type"], "welcome");

    write(
        serde_json::json!({"type": "request", "id": "catalog", "operation": {"kind": "config.catalog"}, "select": null}),
        true,
    );
    let answer = receive().unwrap();
    assert_eq!(
        (&answer["type"], &answer["id"]),
        (&"response".into(), &"catalog".into())
    );
    conforms(&declared("config.catalog"), &answer["result"]);

    write(
        serde_json::json!({"type": "subscribe", "id": "processes", "operation": {"kind": "process.list"}, "select": null}),
        true,
    );
    let mut subscribed = false;
    let mut answered = false;
    while !(subscribed && answered) {
        let message = receive().unwrap();
        assert_eq!(message["id"], "processes", "{message}");
        match message["type"].as_str().unwrap() {
            "response" => {
                assert!(message["result"].is_null());
                subscribed = true;
                write(
                    serde_json::json!({"type": "checkpoint", "id": "processes", "checkpoint": 1}),
                    true,
                );
            }
            "reading" => {
                conforms(&declared("process.list"), &message["frame"]["body"]);
                answered |= message["frame"]["answers"] == 1;
            }
            other => panic!("unexpected {other}: {message}"),
        }
    }
    write(
        serde_json::json!({"type": "cancel", "id": "processes"}),
        true,
    );

    // An operation this server lacks fails alone; the connection keeps working.
    write(
        serde_json::json!({"type": "request", "id": "newer", "operation": {"kind": "task.teleport"}, "select": null}),
        false,
    );
    let refused = loop {
        let message = receive().unwrap();
        if message["id"] == "newer" {
            break message;
        }
        assert_eq!(
            message["id"], "processes",
            "only the cancelled subscription may still speak"
        );
    };
    assert_eq!(
        (&refused["type"], &refused["code"]),
        (&"error".into(), &"invalid".into())
    );
    assert_eq!(refused["data"]["kind"], "task.teleport");
    write(
        serde_json::json!({"type": "request", "id": "after", "operation": {"kind": "process.list"}, "select": null}),
        true,
    );
    let after = loop {
        let message = receive().unwrap();
        if message["id"] == "after" {
            break message;
        }
    };
    assert_eq!(after["type"], "response", "{after}");
    conforms(&declared("process.list"), &after["result"]);
}

#[test]
fn subscription_git_retention_keeps_task_reads_fresh_on_the_same_connection() {
    use loopflow::lf::commands::read::Read;
    use loopflow::server::wire::Operation;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file("edit.txt", "before\n");
    repo.stage_all();
    repo.commit("Comparison base");
    register_task_with_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let _server = Server::start(repo.path(), home.path());
    let mut connection = loopflow::server::client::Connection::open().unwrap();
    let pane = || Read::TaskGet {
        directory: repo.path().to_owned(),
        issue: "INF-123".into(),
    };
    let id = connection.subscribe(pane()).unwrap();
    let frame = connection.reading(&id).unwrap().unwrap();
    assert!(frame["unavailable"].is_null(), "{frame}");
    assert_eq!(
        frame["body"]["row"]["condition"]["local_progress"]["dirty"], false,
        "{frame}"
    );
    let changes = || {
        Operation::Read(Read::TaskChanges {
            issue: "INF-123".into(),
            base: "parent".into(),
            head: None,
        })
    };
    let before = connection.request(changes()).unwrap().result;
    assert!(before["files"].as_array().unwrap().is_empty(), "{before}");
    repo.create_file("edit.txt", "after\n");
    repo.create_file("new.txt", "new\n");
    let detail = connection.request(Operation::Read(pane())).unwrap().result;
    assert_eq!(
        detail["row"]["condition"]["local_progress"]["dirty"], true,
        "{detail}"
    );
    let after = connection.request(changes()).unwrap().result;
    let files = after["files"].as_array().unwrap();
    assert!(
        files
            .iter()
            .any(|file| file["path"] == "edit.txt" && file["unstaged"] == true),
        "{after}"
    );
    assert!(
        files
            .iter()
            .any(|file| file["path"] == "new.txt" && file["untracked"] == true),
        "{after}"
    );
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
        head: None,
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
    let unpublished: Value = serde_json::from_str::<serde_json::Value>(&read(
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
    .unwrap()["task"]
        .clone();
    let task_id = unpublished["id"].as_str().unwrap().to_owned();
    let unpublished = unpublished["identifier"].as_str().unwrap().to_owned();
    // Unpublished too: abandoning it touches no PR, so needs no GitHub.
    let abandoned: Value = serde_json::from_str::<serde_json::Value>(&read(
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
    .unwrap()["task"]
        .clone();
    let abandoned = abandoned["identifier"].as_str().unwrap().to_owned();
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    // Planning status also serves a Work folder without Git.
    let work_folder = home.path().join("folder");
    fs::create_dir_all(&work_folder).unwrap();
    let history = || {
        Read::SessionHistory(HistoryWindow {
            since: "7d".into(),
            limit: 50,
            wave: None,
            project: None,
            task: Some("INF-123".into()),
        })
    };
    let all_plans = || Read::PlanList {
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
    let task_pane = || Read::TaskGet {
        directory: repo.path().to_owned(),
        issue: "INF-123".into(),
    };
    let plans = || Read::DesktopViewList {
        directory: repo.path().to_owned(),
    };
    // A new Read fails to compile here until it is compared too.
    let (reads, subscriptions) = match task_diff(None, None) {
        Read::ProcessHistory { .. }
        | Read::CreationReceipt { .. }
        | Read::ProcessList
        | Read::DesktopViewList { .. }
        | Read::SessionList { .. }
        | Read::TaskGet { .. }
        | Read::TaskChanges { .. }
        | Read::TaskDiff { .. }
        | Read::TaskFile { .. }
        | Read::WaveList
        | Read::MachineSelf
        | Read::DesktopPortfolio
        | Read::ConfigSettings { .. }
        | Read::ConfigCatalog
        | Read::ConfigGet { .. }
        | Read::ConfigExplain { .. }
        | Read::DesktopLayout { .. }
        | Read::RepoPeople { .. }
        | Read::GraphList { .. }
        | Read::DesktopViewGet { .. }
        | Read::TaskRuns { .. }
        | Read::ProcessGet { .. }
        | Read::SessionUsage { .. }
        | Read::TaskContext { .. }
        | Read::WavePlan { .. }
        | Read::TaskList { .. }
        | Read::MachineList
        | Read::SessionHistory(_)
        | Read::PlanList { .. }
        | Read::RepoPlan { .. }
        | Read::WorkResolve { .. }
        | Read::RepoPath { .. }
        | Read::WaveGraph { .. }
        | Read::RepoTokens { .. }
        | Read::SessionRetention
        | Read::MachineDoctor
        | Read::RepoCi { .. }
        | Read::AccountList { .. }
        | Read::AccountNext { .. }
        | Read::SkillList { .. }
        | Read::PrChecks { .. }
        | Read::PrReviews { .. }
        | Read::TaskWait { .. } => (
            [
                Read::ProcessList,
                plans(),
                sessions(true),
                task_pane(),
                Read::TaskChanges {
                    head: None,
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
                Read::WaveList,
                Read::MachineSelf,
                Read::DesktopPortfolio,
                Read::ConfigSettings {
                    directory: repo.path().to_owned(),
                    wave: None,
                    skill: Some("implement".into()),
                    graph: None,
                    step: None,
                },
                Read::ConfigCatalog,
                Read::ConfigGet {
                    destination: loopflow::config::files::Destination::SharedRepository {
                        repository: repo.path().to_owned(),
                    },
                    key: "agent".into(),
                },
                Read::ConfigExplain {
                    destination: loopflow::config::files::Destination::PersonalRepository {
                        repository: repo.path().to_owned(),
                    },
                    key: "agent".into(),
                },
                Read::DesktopLayout {
                    directory: repo.path().to_owned(),
                    work: "repository".into(),
                },
                Read::RepoPeople {
                    directory: repo.path().to_owned(),
                },
                Read::GraphList {
                    directory: repo.path().to_owned(),
                },
                Read::DesktopViewGet {
                    directory: repo.path().to_owned(),
                    expression: None,
                },
                Read::DesktopViewGet {
                    directory: repo.path().to_owned(),
                    expression: Some("state:backlog".into()),
                },
                Read::TaskRuns {
                    directory: repo.path().to_owned(),
                    task: "INF-123".into(),
                    after: None,
                },
                Read::ProcessHistory { id: graph.clone() },
                Read::CreationReceipt {
                    request_id: "absent-creation".into(),
                },
                Read::ProcessGet { id: graph },
                Read::SessionUsage {
                    days: 0,
                    wave: None,
                    project: None,
                    task: Some("INF-123".into()),
                },
                Read::SessionUsage {
                    days: 30,
                    wave: Some("task-pr-tests".into()),
                    project: None,
                    task: None,
                },
                Read::TaskContext {
                    task: "INF-123".into(),
                },
                Read::WavePlan {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                },
                Read::TaskList {
                    directory: repo.path().to_owned(),
                    wave: "task-pr-tests".into(),
                    after: None,
                    closed: true,
                },
                Read::MachineList,
                history(),
                all_plans(),
                Read::PlanList {
                    directory: repo.path().to_owned(),
                    wave: None,
                    task: Some("INF-123".into()),
                    all: false,
                },
                Read::RepoPlan {
                    directory: repo.path().to_owned(),
                },
                Read::WorkResolve {
                    directory: "/".into(),
                    url: format!("loopflow://{folder}/task/INF-123"),
                },
                Read::RepoPath {
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
                Read::RepoTokens {
                    directory: repo.path().to_owned(),
                    days: None,
                },
                Read::RepoTokens {
                    directory: repo.path().to_owned(),
                    days: Some(1),
                },
                Read::SessionRetention,
                Read::MachineDoctor,
                // An absolute window: a relative one is measured from each run's now.
                Read::RepoCi {
                    since: "2026-01-01T00:00:00Z".into(),
                    wave: None,
                    repo: None,
                },
                Read::TaskWait {
                    issue: "INF-123".into(),
                    until: loopflow::ops::task::TaskWaitUntil::Terminal,
                },
                Read::SkillList {
                    directory: repo.path().to_owned(),
                    path: vec!["graph".into()],
                },
            ],
            [
                history(),
                Read::ProcessList,
                all_plans(),
                plans(),
                sessions(false),
                sessions(true),
                Read::RepoPlan {
                    directory: work_folder,
                },
                task_pane(),
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
            // `config repo --path` prints a path, not JSON: its value is that text.
            assert_eq!(served, Value::String(printed.trim().into()), "{argv:?}");
            continue;
        };
        assert_eq!(without_clock(served), without_clock(printed), "{argv:?}");
    }
    for served in subscriptions {
        let mut argv = served.argv();
        argv.push("--subscribe".into());
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
            Read::PlanList {
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
        Read::DesktopLayout {
            directory: directory.clone(),
            work: work.into(),
        }
        .read()
        .unwrap()
    };
    let revision = |work: &str| layout(work)["revision"].as_i64().unwrap();
    let dashboard = || without_revision(layout("query:served"));
    let pane = |revision: i64| Write::DesktopLayoutSave {
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
        Write::TaskCreate { .. }
        | Write::TaskComplete { .. }
        | Write::TaskReopen { .. }
        | Write::TaskAbandon { .. }
        | Write::TaskUpdate { .. }
        | Write::TaskClaim { .. }
        | Write::TaskUnassign { .. }
        | Write::TaskHold { .. }
        | Write::TaskMove { .. }
        // Starting a run or a ship starts Processes; task_run_waits_at_an_interactive_step
        // compares the run, and comments differ by their saved IDs.
        | Write::TaskRun { .. }
        | Write::TaskShip { .. }
        | Write::TaskSteer { .. }
        | Write::TaskReport { .. }
        | Write::TaskInterrupt { .. }
        | Write::TaskSave { .. }
        | Write::SessionSteer { .. }
        | Write::SessionRename { .. }
        | Write::SessionBind { .. }
        | Write::SessionResume { .. }
        | Write::SessionNew { .. }
        | Write::SessionMove { .. }
        | Write::WaveCreate { .. }
        | Write::WaveRename { .. }
        | Write::WaveRemove { .. }
        | Write::WaveMachine { .. }
        | Write::WaveSync { .. }
        // Binding, connecting and reteaming reach Linear; served_planning_binding_matches_cli
        // compares their spelling.
        | Write::WavePlanBind { .. }
        | Write::WavePlanConnect { .. }
        | Write::WavePlanReteam { .. }
        | Write::GraphCustomize { .. }
        // Releasing, committing, syncing and creating worktrees change the
        // repository and its remote; their spellings are compared below.
        | Write::RepoRelease { .. }
        | Write::CheckoutCommit { .. }
        | Write::CheckoutSync { .. }
        | Write::CheckoutCreate { .. }
        | Write::AccountDisconnect { .. }
        | Write::AccountConnect { .. }
        | Write::AccountSet { .. }
        | Write::AccountRedeemReset { .. }
        | Write::AccountPause { .. }
        | Write::AccountUnpause { .. }
        | Write::AccountUse { .. }
        | Write::GraphRun { .. }
        | Write::SkillRun { .. }
        | Write::PrPublish { .. }
        | Write::PrOpen { .. }
        | Write::PrSubmit { .. }
        | Write::PrArm { .. }
        | Write::PrLand { .. }
        | Write::PrAbandon { .. }
        | Write::PrReconcile { .. }
        | Write::PrRequestReview { .. }
        | Write::DesktopPortfolioSet { .. }
        | Write::DesktopDashboardQuery { .. }
        | Write::DesktopDashboardPin { .. }
        | Write::DesktopDashboardUnpin { .. }
        | Write::DesktopDashboardRename { .. }
        | Write::DesktopDashboardHide { .. }
        | Write::DesktopDashboardShow { .. }
        | Write::DesktopDashboardSlots { .. }
        | Write::DesktopDashboardMove { .. }
        // configuration_edits_use_the_server_without_work_processes covers settings.
        | Write::ConfigSet { .. }
        | Write::ConfigUnset { .. }
        | Write::ConfigTheme { .. }
        | Write::DesktopLayoutSave { .. } => (
            [
                Write::TaskUpdate {
                    directory: directory.clone(),
                    issue: "INF-123".into(),
                    notes: "Served from the app".into(),
                },
                Write::TaskMove {
                    machine: None,
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
                    intent: loopflow::lf::commands::write::SessionOpenIntent::Resume,
                },
                Write::SessionResume {
                    directory: directory.clone(),
                    id: Some(session.clone()),
                    task: None,
                    wave: None,
                    agent: None,
                    intent: loopflow::lf::commands::write::SessionOpenIntent::Resume,
                },
                Write::SessionNew {
                    request_id: "new-thread-parity".into(),
                    directory: directory.clone(),
                    id: session.clone(),
                    agent: None,
                },
                // No fixture login is held back, so nothing is blocked to move.
                Write::SessionMove {
                    directory: directory.clone(),
                    blocked: true,
                    on: None,
                    agent: "codex".into(),
                },
                Write::TaskComplete {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::TaskReopen {
                    directory: directory.clone(),
                    issue: unpublished.clone(),
                },
                Write::WaveCreate {
                    directory: directory.clone(),
                    wave: "task-pr-tests".into(),
                    objective: None,
                },
                Write::WaveSync {
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
                Box::new(|| Write::DesktopDashboardMove {
                    directory: directory.clone(),
                    board: "query:served".into(),
                    member: format!("task:{task_id}"),
                    destination: format!("task:{task_id}"),
                }) as Box<dyn Fn() -> Write>,
                Box::new(|| Write::DesktopDashboardPin {
                    directory: directory.clone(),
                    id: "served".into(),
                    revision: revision("query:served"),
                }),
                Box::new(|| Write::DesktopDashboardRename {
                    directory: directory.clone(),
                    id: "served".into(),
                    name: "Served board".into(),
                    revision: revision("query:served"),
                }),
                // A hidden tile is no longer one to hide: show it before each hide.
                Box::new(|| {
                    let _ = loopflow::server::client::Connection::open().and_then(|mut c| {
                        c.request(loopflow::server::wire::Operation::Write(
                            Write::DesktopDashboardShow {
                                directory: directory.clone(),
                                id: "served".into(),
                                member: format!("task:{task_id}"),
                            },
                        ))
                    });
                    Write::DesktopDashboardHide {
                        directory: directory.clone(),
                        id: "served".into(),
                        member: format!("task:{task_id}"),
                    }
                }),
                Box::new(|| Write::DesktopDashboardSlots {
                    directory: directory.clone(),
                    id: "served".into(),
                    revision: revision("query:served"),
                    slots: "null".into(),
                }),
                Box::new(|| Write::DesktopDashboardQuery {
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
            Write::TaskReport {
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
        Read::TaskGet {
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
    let portfolio = || without_clock(Read::DesktopPortfolio.read().unwrap());
    let manual = Write::DesktopPortfolioSet {
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
    let create = Write::DesktopDashboardQuery {
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
                        intent: loopflow::lf::commands::write::SessionOpenIntent::Resume,
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
        graph: None,
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

/// The first frame `lf <args>` prints, with its stdin held open as a subscription's
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
    let kind = serde_json::to_value(&query).unwrap()["kind"].clone();
    let result = loopflow::server::client::Connection::open()
        .unwrap()
        .request(loopflow::server::wire::Operation::Read(query))
        .unwrap()
        .result;
    assert_declared_result(kind.as_str().unwrap(), &result);
    result
}

/// `result` has the shape `lf server describe` declares for `kind`.
fn assert_declared_result(kind: &str, result: &Value) {
    let schema = loopflow::server::schema::describe();
    let read = schema["$defs"]["Read"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|arm| arm["properties"]["kind"]["const"] == kind)
        .unwrap_or_else(|| panic!("{kind} is not in the schema"));
    let mut declared = read["x-result"].clone();
    declared["$defs"] = schema["$defs"].clone();
    let validator = jsonschema::draft202012::new(&declared).unwrap();
    let failed: Vec<_> = validator
        .iter_errors(result)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(failed.is_empty(), "{kind} answered {result}: {failed:#?}");
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
        Write::TaskCreate {
            directory,
            request_id,
            title,
            request,
            wave,
            graph,
            session,
            run,
        } => {
            let mut args = words(&["task", "create", "--request-id", request_id, "--json"]);
            for (flag, value) in [
                ("--title", title),
                ("--notes", request),
                ("--wave", wave),
                ("--graph", graph),
                ("--session", session),
            ] {
                if let Some(value) = value {
                    args.extend([flag.into(), value.clone()]);
                }
            }
            if *run {
                args.push("--run".into());
            }
            (path(directory), args)
        }
        Write::TaskComplete { directory, issue } => (
            path(directory),
            words(&["task", "complete", issue, "--json"]),
        ),
        Write::TaskReopen { directory, issue } => {
            (path(directory), words(&["task", "reopen", issue, "--json"]))
        }
        Write::TaskUpdate {
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
        Write::TaskHold { directory, issue } => {
            (path(directory), words(&["task", "hold", issue, "--json"]))
        }
        Write::TaskRun {
            directory,
            issue,
            from,
            agent,
            approve,
            graph,
        } => {
            let mut spelled = Vec::new();
            if *approve {
                spelled.push("--batch".to_owned());
            }
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["task", "run", issue]));
            spelled.extend(graph.iter().cloned());
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
            words(&["task", "steer", issue, message, "--json"]),
        ),
        Write::TaskReport {
            directory,
            issue,
            message,
        } => (
            path(directory),
            words(&["task", "report", issue, message, "--json"]),
        ),
        Write::TaskInterrupt { directory, issue } => (
            path(directory),
            words(&["task", "interrupt", issue, "--json"]),
        ),
        Write::TaskMove {
            directory,
            issue,
            wave,
            machine,
        } => {
            let mut spelled = words(&["task", "move", issue]);
            match (wave, machine) {
                (_, Some(machine)) => spelled.extend(words(&["--machine", machine])),
                (Some(wave), _) => spelled.extend(words(&["--wave", wave])),
                (None, _) => spelled.push("--no-wave".into()),
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
        Write::SessionSteer {
            directory,
            session,
            wave,
            message,
        } => {
            let mut spelled = Vec::new();
            if let Some(wave) = wave {
                spelled.extend(words(&["--wave", wave]));
            }
            spelled.extend(words(&["session", "steer"]));
            spelled.extend(session.clone());
            spelled.extend(words(&[message, "--json"]));
            (path(directory), spelled)
        }
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
            intent,
        } => {
            let mut spelled = Vec::new();
            if id.is_none() && task.is_none() && wave.is_none() {
                spelled.extend(words(&["--repo", &path(directory)]));
            }
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["session", "resume"]));
            spelled.extend(intent.flag().map(str::to_owned));
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
        Write::SessionMove {
            directory,
            blocked,
            on,
            agent,
        } => {
            let mut spelled = words(&["--agent", agent, "session", "move", "--all", "--json"]);
            if *blocked {
                spelled.push("--blocked".into());
            }
            if let Some(on) = on {
                spelled.extend(words(&["--on", on]));
            }
            (path(directory), spelled)
        }
        Write::SessionNew {
            directory,
            id,
            agent,
            request_id,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&[
                "session",
                "new",
                id,
                "--prepare",
                "--json",
                "--request-id",
                request_id,
            ]));
            (path(directory), spelled)
        }
        Write::WaveRename {
            directory,
            wave,
            name,
        } => (
            path(directory),
            words(&["wave", "rename", wave, name, "--json"]),
        ),
        Write::WaveRemove {
            directory,
            wave,
            preview,
        } => {
            let mut args = words(&["wave", "remove", wave, "--json"]);
            if *preview {
                args.push("--preview".into());
            }
            (path(directory), args)
        }
        Write::WaveMachine {
            directory,
            wave,
            machine,
        } => (
            path(directory),
            words(&["--wave", wave, "set", "machine", machine, "--json"]),
        ),
        Write::WaveCreate {
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
        Write::WaveSync { directory, wave } => {
            (path(directory), words(&["wave", "sync", wave, "--json"]))
        }
        Write::WavePlanBind {
            directory,
            wave,
            project,
        } => (
            path(directory),
            words(&["wave", "plan", "bind", wave, project, "--json"]),
        ),
        Write::WavePlanConnect {
            directory,
            wave,
            all,
            team_key,
            team_name,
        } => {
            let mut spelled = words(&["wave", "plan", "connect"]);
            spelled.extend(wave.clone());
            if *all {
                spelled.push("--all".into());
            }
            if let Some(key) = team_key {
                spelled.extend(words(&["--team-key", key]));
            }
            if let Some(name) = team_name {
                spelled.extend(words(&["--team-name", name]));
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::WavePlanReteam { directory, apply } => {
            let mut spelled = words(&["wave", "plan", "reteam"]);
            if *apply {
                spelled.push("--apply".into());
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::GraphCustomize { directory, name } => (
            path(directory),
            words(&["graph", "customize", name, "--json"]),
        ),
        Write::RepoRelease {
            directory,
            version,
            target,
        } => {
            let mut spelled = words(&["repo", "release", "run"]);
            spelled.extend(version.clone());
            if let Some(target) = target {
                spelled.extend(words(&["--target", target]));
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::CheckoutCommit {
            directory,
            message,
            push,
            no_add,
            paths,
            agent,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.push("commit".into());
            if let Some(message) = message {
                spelled.extend(words(&["--message", message]));
            }
            if *push {
                spelled.push("--push".into());
            }
            if *no_add {
                spelled.push("--no-add".into());
            }
            spelled.extend(paths.iter().cloned());
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::CheckoutSync {
            directory,
            onto,
            preview,
            manual,
            continue_sync,
            abort,
            adopt,
        } => {
            let mut spelled = words(&["sync"]);
            spelled.extend(onto.clone());
            for (flag, on) in [
                ("--plan", preview),
                ("--manual", manual),
                ("--continue", continue_sync),
                ("--abort", abort),
                ("--adopt", adopt),
            ] {
                if *on {
                    spelled.push(flag.into());
                }
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::AccountDisconnect { provider, email } => {
            let target = email
                .as_ref()
                .map(|email| format!("{provider}={email}"))
                .unwrap_or_else(|| provider.clone());
            let spelled = words(&["account", "disconnect", &target]);
            ("/".into(), spelled)
        }
        Write::AccountConnect {
            provider,
            email,
            chrome_profile,
            import,
            api_key,
        } => {
            let target = email
                .as_ref()
                .map(|email| format!("{provider}={email}"))
                .unwrap_or_else(|| provider.clone());
            let mut spelled = words(&["account", "connect", &target]);
            if let Some(profile) = chrome_profile {
                spelled.extend(words(&["--chrome-profile", profile]));
            }
            for (flag, on) in [("--import", import), ("--api-key", api_key)] {
                if *on {
                    spelled.push(flag.into());
                }
            }
            ("/".into(), spelled)
        }
        Write::AccountSet {
            provider,
            email,
            login_email,
            plan,
            clear_plan,
            paid_through,
            clear_paid_through,
            clear_cooldown,
            chrome_profiles,
            clear_chrome_profiles,
        } => {
            let target = email
                .as_ref()
                .map(|email| format!("{provider}={email}"))
                .unwrap_or_else(|| provider.clone());
            let mut spelled = words(&["account", "set", &target]);
            for (flag, value) in [
                ("--login-email", login_email),
                ("--plan", plan),
                ("--paid-through", paid_through),
            ] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            for (flag, on) in [
                ("--clear-plan", clear_plan),
                ("--clear-paid-through", clear_paid_through),
                ("--clear-cooldown", clear_cooldown),
                ("--clear-chrome-profiles", clear_chrome_profiles),
            ] {
                if *on {
                    spelled.push(flag.into());
                }
            }
            for profile in chrome_profiles {
                spelled.extend(words(&["--chrome-profile", profile]));
            }
            ("/".into(), spelled)
        }
        Write::AccountRedeemReset {
            provider,
            email,
            idempotency_key,
            credit_id,
        } => {
            let mut spelled = words(&["account", "redeem-reset", &format!("{provider}={email}")]);
            for (flag, value) in [
                ("--idempotency-key", idempotency_key),
                ("--credit-id", credit_id),
            ] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            spelled.push("--json".into());
            ("/".into(), spelled)
        }
        Write::AccountUse { login } => ("/".into(), words(&["account", "use", login])),
        Write::AccountPause { login } => ("/".into(), words(&["account", "pause", login])),
        Write::AccountUnpause { login } => ("/".into(), words(&["account", "unpause", login])),
        Write::GraphRun {
            directory,
            name,
            message,
            agent,
        }
        | Write::SkillRun {
            directory,
            name,
            message,
            agent,
        } => {
            let noun = if matches!(write, Write::GraphRun { .. }) {
                "graph"
            } else {
                "skill"
            };
            let mut spelled = words(&["--batch"]);
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&[noun, name]));
            spelled.extend(message.clone());
            (path(directory), spelled)
        }
        Write::PrPublish {
            directory,
            title,
            body,
            agent,
        }
        | Write::PrOpen {
            directory,
            title,
            body,
            agent,
        } => {
            let verb = if matches!(write, Write::PrOpen { .. }) {
                "open"
            } else {
                "publish"
            };
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["pr", verb]));
            for (flag, value) in [("--title", title), ("--body", body)] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PrSubmit {
            directory,
            strict,
            worktree,
            message,
            title,
            body,
            agent,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["pr", "submit"]));
            if *strict {
                spelled.push("--strict".into());
            }
            for (flag, value) in [
                ("--worktree", worktree),
                ("--message", message),
                ("--title", title),
                ("--body", body),
            ] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PrArm {
            directory,
            strict,
            local,
            worktree,
            message,
            title,
            body,
            agent,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["pr", "arm"]));
            for (flag, on) in [("--strict", strict), ("--local", local)] {
                if *on {
                    spelled.push(flag.into());
                }
            }
            for (flag, value) in [
                ("--worktree", worktree),
                ("--message", message),
                ("--title", title),
                ("--body", body),
            ] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PrLand {
            directory,
            wait_and_fix,
            strict,
            local,
            worktree,
            message,
            title,
            body,
            agent,
        } => {
            let mut spelled = Vec::new();
            if let Some(agent) = agent {
                spelled.extend(words(&["--agent", agent]));
            }
            spelled.extend(words(&["pr", "land"]));
            for (flag, on) in [
                ("--wait-and-fix", wait_and_fix),
                ("--strict", strict),
                ("--local", local),
            ] {
                if *on {
                    spelled.push(flag.into());
                }
            }
            for (flag, value) in [
                ("--worktree", worktree),
                ("--message", message),
                ("--title", title),
                ("--body", body),
            ] {
                if let Some(value) = value {
                    spelled.extend(words(&[flag, value]));
                }
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PrAbandon {
            directory,
            branch,
            force,
        } => {
            let mut spelled = words(&["pr", "abandon"]);
            spelled.extend(branch.clone());
            if *force {
                spelled.push("--force".into());
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::PrReconcile { directory } => {
            (path(directory), words(&["pr", "reconcile", "--json"]))
        }
        Write::PrRequestReview {
            directory,
            pr,
            reviewers,
        } => {
            let mut spelled = words(&["pr", "request-review"]);
            spelled.extend(pr.clone());
            for reviewer in reviewers {
                spelled.extend(words(&["-r", reviewer]));
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::CheckoutCreate {
            directory,
            name,
            preview,
            persistent,
        } => {
            let mut spelled = words(&["wt", "create", name]);
            if *preview {
                spelled.push("--plan".into());
            }
            if *persistent {
                spelled.push("--persistent".into());
            }
            spelled.push("--json".into());
            (path(directory), spelled)
        }
        Write::DesktopPortfolioSet { change } => {
            let mut spelled = words(&["desktop", "portfolio"]);
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
        Write::DesktopDashboardQuery {
            directory,
            id,
            expression,
            revision,
        } => {
            let mut spelled = words(&["desktop", "dashboard", "query"]);
            spelled.extend(revision.map(|revision| format!("--revision={revision}")));
            spelled.extend(words(&["--", id, expression]));
            (path(directory), spelled)
        }
        Write::DesktopDashboardPin {
            directory,
            id,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "pin",
                &format!("--revision={revision}"),
                "--",
                id,
            ]),
        ),
        Write::DesktopDashboardUnpin {
            directory,
            id,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "unpin",
                &format!("--revision={revision}"),
                "--",
                id,
            ]),
        ),
        Write::DesktopDashboardRename {
            directory,
            id,
            name,
            revision,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "rename",
                &format!("--revision={revision}"),
                "--",
                &format!("query:{id}"),
                name,
            ]),
        ),
        Write::DesktopDashboardHide {
            directory,
            id,
            member,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "hide",
                &format!("query:{id}/{member}"),
            ]),
        ),
        Write::DesktopDashboardShow {
            directory,
            id,
            member,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "show",
                &format!("query:{id}/{member}"),
            ]),
        ),
        Write::DesktopDashboardSlots {
            directory,
            id,
            revision,
            ..
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "slots",
                &format!("query:{id}"),
                &format!("--revision={revision}"),
            ]),
        ),
        Write::DesktopDashboardMove {
            directory,
            board,
            member,
            destination,
        } => (
            path(directory),
            words(&[
                "desktop",
                "dashboard",
                "move",
                &format!("{board}/{member}"),
                "--destination",
                destination,
            ]),
        ),
        Write::DesktopLayoutSave {
            directory, work, ..
        } => (
            path(directory),
            words(&["desktop", "layout", work, "--save"]),
        ),
        Write::ConfigSet { .. } | Write::ConfigUnset { .. } | Write::ConfigTheme { .. } => {
            unreachable!("settings writes are compared on their own")
        }
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

#[test]
fn served_task_create_survives_a_dropped_connection_and_server_restart() {
    use loopflow::lf::commands::{read::Read, write::Write};
    use loopflow::server::wire::{Operation, REVISION};
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    let server = Server::start(repo.path(), home.path());
    let change = Write::TaskCreate {
        directory: repo.path().to_path_buf(),
        request_id: "lost-answer".into(),
        title: Some("One Task".into()),
        request: None,
        wave: None,
        graph: None,
        session: None,
        run: false,
    };
    let mut socket = UnixStream::connect(loopflow::server::socket_path().unwrap()).unwrap();
    writeln!(
        socket,
        "{}",
        serde_json::json!({"type":"hello","revision":REVISION,"min_revision":REVISION,
        "client":{"kind":"test","platform":"test","name":"lost","version":"0"}})
    )
    .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    let mut welcome = String::new();
    reader.read_line(&mut welcome).unwrap();
    writeln!(
        socket,
        "{}",
        serde_json::json!({"type":"request","id":"first","select":null,"operation":change})
    )
    .unwrap();
    // Deliberately do not read the answer. Admission is observed in the receipt.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(conn) = rusqlite::Connection::open(home.path().join("loopflow.db")) {
            if conn
                .query_row(
                    "SELECT count(*) FROM creation_receipts WHERE request_id='lost-answer'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                == 1
            {
                break;
            }
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(reader);
    drop(socket);
    let mut connection = loopflow::server::client::Connection::open().unwrap();
    let first = connection
        .request(Operation::Write(change.clone()))
        .unwrap()
        .result;
    assert_eq!(first["milestone"], "filed");
    drop(connection);
    drop(server);
    let _server = Server::start(repo.path(), home.path());
    let mut connection = loopflow::server::client::Connection::open().unwrap();
    let second = connection
        .request(Operation::Write(change.clone()))
        .unwrap()
        .result;
    assert_eq!(first, second);
    std::thread::scope(|scope| {
        let concurrent: Vec<_> = (0..4)
            .map(|_| {
                let change = change.clone();
                scope.spawn(move || {
                    loopflow::server::client::Connection::open()
                        .unwrap()
                        .request(Operation::Write(change))
                        .unwrap()
                        .result
                })
            })
            .collect();
        for retry in concurrent {
            assert_eq!(retry.join().unwrap(), first);
        }
    });
    let found = connection
        .request(Operation::Read(Read::CreationReceipt {
            request_id: "lost-answer".into(),
        }))
        .unwrap()
        .result;
    assert_eq!(found["result"], first);
    assert_eq!(found["disposition"], "complete");
    assert_eq!(found["explanation"], "Task created");
    let mut changed = change.clone();
    if let Write::TaskCreate { title, .. } = &mut changed {
        *title = Some("Different".into());
    }
    let error = connection
        .request(Operation::Write(changed.clone()))
        .unwrap_err()
        .downcast::<loopflow::engine::Error>()
        .unwrap();
    assert_eq!(error.code, loopflow::engine::ErrorCode::Conflict);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    // An unrelated Task graph cannot confirm this creation's interrupted launch.
    let input = loopflow::engine::creation::TaskCreation {
        directory: repo.path().into(),
        title: Some("One Task".into()),
        request: None,
        wave: None,
        graph: None,
        session: None,
        worktree: None,
        run: true,
        agent: None,
        approve: false,
    };
    use sha2::{Digest, Sha256};
    let fingerprint = hex::encode(Sha256::digest(
        serde_json::to_vec(&serde_json::to_value(&input).unwrap()).unwrap(),
    ));
    conn.execute("UPDATE creation_receipts SET fingerprint=?1,run=1,milestone='filed' WHERE request_id='lost-answer'", [fingerprint]).unwrap();
    let continuation = connection
        .request(Operation::Read(Read::CreationReceipt {
            request_id: "lost-answer".into(),
        }))
        .unwrap()
        .result;
    assert_eq!(continuation["disposition"], "continue");
    conn.execute(
        "UPDATE creation_receipts SET milestone='run_requested' WHERE request_id='lost-answer'",
        [],
    )
    .unwrap();
    let interrupted = connection
        .request(Operation::Read(Read::CreationReceipt {
            request_id: "lost-answer".into(),
        }))
        .unwrap()
        .result;
    assert_eq!(interrupted["disposition"], "inspect");
    assert_eq!(
        interrupted["explanation"],
        "Task created; start unconfirmed"
    );
    conn.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'demo',1)",
        rusqlite::params![
            first["task"]["id"].as_str().unwrap(),
            r#"{"name":"unrelated","nodes":[]}"#
        ],
    )
    .unwrap();
    if let Write::TaskCreate { run, .. } = &mut changed {
        *run = true;
    }
    if let Write::TaskCreate { title, .. } = &mut changed {
        *title = Some("One Task".into());
    }
    let unknown = connection
        .request(Operation::Write(changed.clone()))
        .unwrap_err()
        .downcast::<loopflow::engine::Error>()
        .unwrap();
    assert_eq!(unknown.code, loopflow::engine::ErrorCode::OutcomeUnknown);
    assert_eq!(unknown.data.unwrap()["milestone"], "run_requested");
    assert_eq!(
        conn.query_row("SELECT count(*) FROM creation_receipts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    conn.execute(
        "UPDATE tasks SET planning_deleted_at=1 WHERE id=?1",
        [first["task"]["id"].as_str().unwrap()],
    )
    .unwrap();
    let removed = connection
        .request(Operation::Read(Read::CreationReceipt {
            request_id: "lost-answer".into(),
        }))
        .unwrap()
        .result;
    assert_eq!(removed["disposition"], "inspect");
    assert_eq!(removed["explanation"], "The created Task was removed");
    assert_eq!(removed["result"], first);
    assert!(connection.request(Operation::Write(changed)).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM tasks WHERE planning_deleted_at IS NULL",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn served_session_new_recovers_its_input_after_a_lost_reply_and_restart() {
    use loopflow::lf::commands::write::Write;
    use loopflow::server::wire::{Operation, REVISION};
    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_home(&[], Some(home.path()));
    std::env::set_var("LF_HOME", home.path().canonicalize().unwrap());
    let repo = TestRepo::new();
    repo.create_file(".lf/config.yaml", "default_agent: codex\n");
    let server = Server::start(repo.path(), home.path());
    let session: Value = serde_json::from_str(&read(
        repo.path(),
        home.path(),
        &["session", "resume", "--prepare", "--json"],
        None,
    ))
    .unwrap();
    let id = session["id"].as_str().unwrap();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let captures = || {
        conn.query_row(
            "SELECT count(*) FROM session_events WHERE session_id=?1 AND kind='captured'",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
    };
    let before = captures();
    let change = Write::SessionNew {
        directory: repo.path().into(),
        id: id.into(),
        request_id: "lost-thread".into(),
        agent: None,
    };
    let mut socket = UnixStream::connect(loopflow::server::socket_path().unwrap()).unwrap();
    writeln!(
        socket,
        "{}",
        serde_json::json!({"type":"hello","revision":REVISION,"min_revision":REVISION,
        "client":{"kind":"test","platform":"test","name":"lost","version":"0"}})
    )
    .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    reader.read_line(&mut String::new()).unwrap();
    writeln!(
        socket,
        "{}",
        serde_json::json!({"type":"request","id":"first","select":null,"operation":change})
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while conn
        .query_row(
            "SELECT count(*) FROM creation_receipts WHERE request_id='lost-thread'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
        == 0
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(reader);
    drop(socket);
    let result = loopflow::server::client::Connection::open()
        .unwrap()
        .request(Operation::Write(change.clone()))
        .unwrap()
        .result;
    assert_eq!(result["id"], id);
    assert_eq!(captures(), before + 1);
    drop(server);
    let _server = Server::start(repo.path(), home.path());
    let again = loopflow::server::client::Connection::open()
        .unwrap()
        .request(Operation::Write(change))
        .unwrap()
        .result;
    assert_eq!(again, result);
    assert_eq!(captures(), before + 1, "recovery appended another input");
}
