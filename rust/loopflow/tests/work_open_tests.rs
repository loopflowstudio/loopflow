use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::{register_task_without_pr, EnvGuard};

/// The built CLI with no inherited Loopflow or host context, and `bin` first on PATH.
fn lf(home: &Path, cwd: &Path, bin: Option<&Path>, args: &[&str]) -> Output {
    lf_with(home, cwd, bin, &[], args)
}

fn lf_with(
    home: &Path,
    cwd: &Path,
    bin: Option<&Path>,
    env: &[(&str, &str)],
    args: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if ["LF_", "CMUX_", "HERDR_", "TMUX"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            command.env_remove(name.as_ref());
        }
    }
    if let Some(bin) = bin {
        command.env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        );
    }
    command
        .envs(env.iter().copied())
        .env("LF_HOME", home)
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn folder(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// A host CLI stand-in that records its argv and prints `reply`.
fn host(bin: &Path, name: &str, reply: &str, status: i32) -> PathBuf {
    let log = bin.join(format!("{name}.log"));
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncat <<'REPLY'\n{reply}\nREPLY\nexit {status}\n",
        log.display()
    );
    let path = bin.join(name);
    std::fs::write(&path, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    log
}

fn sessions(home: &Path) -> i64 {
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn task_link_resolves_by_folder_name_without_preparing_anything() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let links = json(&lf(
        home.path(),
        elsewhere.path(),
        None,
        &["open", &url, "--link", "--json"],
    ));
    let link = &links[0];
    assert_eq!(link["url"], url.as_str());
    assert_eq!(link["label"], "Prove Task PR transitions · INF-123");
    assert_eq!(
        link["markdown"],
        format!("[Prove Task PR transitions · INF-123]({url})").as_str()
    );
    assert_eq!(
        link["fallback_argv"],
        serde_json::json!(["lf", "open", url])
    );
    assert_eq!(link["target"]["task_id"], registered.task.id.as_str());
    assert_eq!(link["conversation"], "unknown");
    assert_eq!(sessions(home.path()), 0, "a link never creates a Session");

    let text = lf(
        home.path(),
        elsewhere.path(),
        None,
        &["open", &url, "--link"],
    );
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(
        !text.contains('\x1b'),
        "piped text carries no escapes: {text:?}"
    );
    assert!(text.starts_with("Prove Task PR transitions · INF-123  lf open "));
}

#[test]
fn standalone_task_link_resolves_by_folder_name() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let created = json(&lf(
        home.path(),
        repo.path(),
        None,
        &["task", "create", "--title", "Stand alone", "--json"],
    ));
    let identifier = created["identifier"].as_str().unwrap();
    let url = format!("loopflow://{}/task/{identifier}", folder(repo.path()));
    let links = json(&lf(
        home.path(),
        elsewhere.path(),
        None,
        &["open", &url, "--link", "--json"],
    ));
    assert_eq!(links[0]["url"], url.as_str());
    assert_eq!(links[0]["target"]["task_id"], created["id"]);
    assert_eq!(
        links[0]["label"],
        format!("Stand alone · {identifier}").as_str()
    );
}

#[test]
fn same_named_repositories_need_a_qualifier_and_change_nothing() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let parents = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let mut ids = Vec::new();
    for parent in &parents {
        let path = parent.path().join("same");
        std::fs::create_dir(&path).unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@e",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&path)
                .status()
                .unwrap()
                .success());
        }
        ids.push((
            path.clone(),
            store
                .ensure_repository(&path.display().to_string())
                .unwrap(),
        ));
    }
    let ambiguous = lf(
        home.path(),
        elsewhere.path(),
        None,
        &["open", "loopflow://same", "--link"],
    );
    assert!(!ambiguous.status.success());
    let error = String::from_utf8_lossy(&ambiguous.stderr);
    for (_, id) in &ids {
        assert!(
            error.contains(&format!("loopflow://same?repository={id}")),
            "{error}"
        );
    }
    let (path, id) = &ids[1];
    let qualified = format!("loopflow://same?repository={id}");
    let links = json(&lf(
        home.path(),
        elsewhere.path(),
        None,
        &["open", &qualified, "--link", "--json"],
    ));
    assert_eq!(
        links[0]["url"],
        qualified.as_str(),
        "emitted links keep the qualifier"
    );
    assert_eq!(
        Path::new(links[0]["target"]["repository_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        path.canonicalize().unwrap()
    );
}

#[test]
fn cmux_opens_the_task_conversation_through_session_resume() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    // The Task waits at explore; its workspace name stays the Task title.
    let graph = serde_json::json!({"name": "feature", "steps": [{"Skill": {"skill": {"name": "explore"}, "id": null, "interactive": true, "returns": null, "sources": []}}]});
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'explore',1)",
            rusqlite::params![registered.task.id.as_str(), graph.to_string()],
        )
        .unwrap();
    let log = host(bin.path(), "cmux", "OK workspace:7", 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "cmux", "--json"],
    ));
    let receipt = &report["receipts"][0];
    assert_eq!(receipt["status"], "opening");
    assert_eq!(receipt["host"], "cmux");
    assert_eq!(receipt["destination"], "workspace:7");
    let call = std::fs::read_to_string(log).unwrap();
    assert!(
        call.starts_with("workspace create --name Prove Task PR transitions · INF-123 --cwd "),
        "{call}"
    );
    assert!(
        call.contains(" session resume --task INF-123 --focus true"),
        "{call}"
    );
    assert!(!call.contains("--fresh"), "{call}");
    // cmux starts the workspace from its own environment; this private Home goes with it.
    let home_arg = format!(
        " --command env 'LF_HOME={}' ",
        home.path().canonicalize().unwrap().display()
    );
    assert!(call.contains(&home_arg), "{call}");
    assert_eq!(
        sessions(home.path()),
        0,
        "the destination's connect owns admission"
    );
}

#[test]
fn herdr_types_connect_into_a_newly_created_pane() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let reply = r#"{"result":{"workspace":{"workspace_id":"w9"},"root_pane":{"pane_id":"w9:p1"}}}"#;
    let log = host(bin.path(), "herdr", reply, 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "herdr", "--json"],
    ));
    assert_eq!(report["receipts"][0]["status"], "opening");
    assert_eq!(report["receipts"][0]["destination"], "w9");
    let calls = std::fs::read_to_string(log).unwrap();
    let calls: Vec<&str> = calls.lines().collect();
    assert!(calls[0].starts_with("workspace create --cwd "), "{calls:?}");
    assert!(
        calls[0].ends_with("--label Prove Task PR transitions · INF-123 --focus"),
        "{calls:?}"
    );
    assert!(calls[1].starts_with("pane run w9:p1 "), "{calls:?}");
    assert!(
        calls[1].ends_with(" session resume --task INF-123"),
        "{calls:?}"
    );
}

#[test]
fn tmux_opens_a_named_window_after_the_callers() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let log = host(bin.path(), "tmux", "%7", 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let report = json(&lf_with(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[("TMUX", "/tmp/tmux-1000/default,1,0"), ("TMUX_PANE", "%3")],
        &["open", &url, "--json"],
    ));
    assert_eq!(report["receipts"][0]["host"], "tmux");
    assert_eq!(report["receipts"][0]["status"], "opening");
    assert_eq!(report["receipts"][0]["destination"], "%7");
    let call = std::fs::read_to_string(log).unwrap();
    assert!(
        call.starts_with(
            "new-window -a -t %3 -P -F #{pane_id} -n Prove Task PR transitions · INF-123 -c "
        ),
        "{call}"
    );
    assert!(
        call.trim_end().ends_with(" session resume --task INF-123"),
        "{call}"
    );
}

#[test]
fn refused_host_reports_cause_and_the_usable_command() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    host(bin.path(), "cmux", "", 1);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let output = lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "cmux"],
    );
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("Could not open Prove Task PR transitions · INF-123: cmux"),
        "{text}"
    );
    assert!(text.contains(&format!("Run: lf open {url}")), "{text}");
}

/// Automatic opening goes where the caller is, before installed Desktop.
#[test]
fn automatic_route_opens_where_the_caller_is() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let url = format!("loopflow://{}", folder(repo.path()));
    let route = |env: &[(&str, &str)]| {
        let links = json(&lf_with(
            home.path(),
            repo.path(),
            None,
            env,
            &["open", &url, "--link", "--json"],
        ));
        links[0]["route"].clone()
    };
    let desktop: &[&str] = if cfg!(target_os = "macos") {
        &["desktop"]
    } else {
        &[]
    };
    let order = |first: &[&str]| {
        let hosts: Vec<&str> = first
            .iter()
            .chain(desktop)
            .chain(&["terminal"])
            .copied()
            .collect();
        serde_json::json!(hosts)
    };
    let herdr = [
        ("HERDR_ENV", "1"),
        ("HERDR_SOCKET_PATH", "/tmp/herdr.sock"),
        ("HERDR_PANE_ID", "w1:p1"),
    ];
    assert_eq!(route(&herdr)["hosts"], order(&["herdr"]));
    let tmux = [("TMUX", "/tmp/tmux-1000/default,1,0"), ("TMUX_PANE", "%0")];
    assert_eq!(route(&tmux)["hosts"], order(&["tmux"]));
    assert_eq!(route(&[])["hosts"], order(&[]));
    if cfg!(target_os = "macos") {
        let cmux = [
            ("CMUX_WORKSPACE_ID", "w"),
            ("CMUX_SOCKET_PATH", "/tmp/cmux.sock"),
        ];
        assert_eq!(route(&cmux)["hosts"], order(&["cmux"]));
        let inside = route(&[("LF_DESKTOP", "com.loopflow.mac")]);
        assert_eq!(inside["hosts"], order(&[]));
        assert!(
            inside["reason"]
                .as_str()
                .unwrap()
                .contains("this Desktop window"),
            "{inside}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_automatic_route_never_selects_desktop() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let url = format!("loopflow://{}", folder(repo.path()));
    let links = json(&lf(
        home.path(),
        repo.path(),
        None,
        &["open", &url, "--link", "--json"],
    ));
    assert_eq!(links[0]["route"]["hosts"], serde_json::json!(["terminal"]));
    let output = lf(
        home.path(),
        repo.path(),
        None,
        &["open", &url, "--host", "desktop"],
    );
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Loopflow Desktop requires macOS"), "{text}");
    assert!(text.contains(&format!("Run: lf open {url}")), "{text}");
}

/// Seed an interactive conversation whose provider is alive, with the given activity.
fn live_conversation(
    home: &Path,
    provider: &std::process::Child,
    started: i64,
    cwd: &Path,
    title: &str,
    open_tools: i64,
    yielded: i64,
) -> String {
    let store = loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let input = format!("run_{}", uuid::Uuid::new_v4().simple());
    let id = format!("session_{input}");
    store
        .create_session(
            loopflow::session::LfSession {
                captured: None,
                id: id.clone().into(),
                artifact_key: Some(input),
                caller_artifact_key: None,
                input_published: false,
                cwd: cwd.into(),
                skill: None,
                provider: Some("codex".into()),
                model: None,
                node: None,
                iterations: None,
                task_id: None,
                wave_id: None,
                graph_lf_process_id: None,
                work_source: None,
                bound_at: None,
                interactive: true,
                repo: Some(cwd.display().to_string()),
                title: title.into(),
                title_source: loopflow::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
        )
        .unwrap();
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let parent = loopflow::id::LfProcessId::new();
    db.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [&parent],
    )
    .unwrap();
    let attachment = store
        .claim_session_attachment(&id, None, &parent, true)
        .unwrap();
    db.execute(
        "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
        rusqlite::params![attachment.agent_process_id, provider.id(), started],
    )
    .unwrap();
    db.execute(
        "INSERT INTO session_activity(session_id,attachment_token,agent_process_id,observed_at,open_tools,pending_input,yielded)
         SELECT s.id,p.attachment_token,p.id,unixepoch(),?2,0,?3 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
        rusqlite::params![id, open_tools, yielded],
    )
    .unwrap();
    id
}

#[test]
fn available_conversations_need_no_question_and_are_never_taken_over() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let provider = Child(Command::new("/bin/sleep").arg("3600").spawn().unwrap());
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let empty = lf(home.path(), repo.path(), None, &["open", "--available"]);
    assert!(empty.status.success(), "{empty:?}");
    assert_eq!(
        String::from_utf8_lossy(&empty.stdout),
        "No available conversations.\n"
    );

    // A finished turn with no question, the Task's primary.
    let primary = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Design",
        0,
        1,
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
        rusqlite::params![
            primary,
            registered.task.id.as_str(),
            registered.task.wave_id.as_ref().unwrap().as_str()
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
        rusqlite::params![registered.task.id.as_str(), primary],
    )
    .unwrap();
    let secondary = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Side question",
        0,
        1,
    );
    live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Running a tool",
        1,
        0,
    );

    let text = lf(
        home.path(),
        repo.path(),
        None,
        &["open", "--available", "--link"],
    );
    assert!(
        !String::from_utf8_lossy(&text.stdout).contains("Unknown:"),
        "{text:?}"
    );
    let links = json(&lf(
        home.path(),
        repo.path(),
        None,
        &["open", "--available", "--link", "--json"],
    ));
    let links = links.as_array().unwrap();
    assert_eq!(links.len(), 2, "{links:?}");
    let task_url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let task = links
        .iter()
        .find(|link| link["url"] == task_url.as_str())
        .unwrap();
    assert!(
        task["target"]["session"].is_null(),
        "a primary opens as its Work"
    );
    let side = links
        .iter()
        .find(|link| link["target"]["session"] == secondary.as_str())
        .unwrap();
    assert_eq!(side["label"], "Side question");
    assert_eq!(
        side["fallback_argv"],
        serde_json::json!(["lf", "session", "resume", secondary])
    );

    let log = host(bin.path(), "cmux", "OK workspace:1", 0);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", "--available", "--host", "cmux", "--json"],
    ));
    let statuses: Vec<&str> = report["receipts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|receipt| receipt["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["elsewhere", "elsewhere"]);
    assert!(
        !log.exists(),
        "a live conversation never gets a second terminal"
    );

    // Desktop receives the batch in one request. Here the final item, an
    // exact secondary Session, cannot open there, so nothing activates Desktop.
    if cfg!(target_os = "macos") {
        host(bin.path(), "osascript", "/Applications/Loopflow.app", 0);
        let opened = host(bin.path(), "open", "", 0);
        let output = lf(
            home.path(),
            repo.path(),
            Some(bin.path()),
            &["open", "--available", "--host", "desktop", "--json"],
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let statuses: Vec<&str> = report["receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|receipt| receipt["status"].as_str().unwrap())
            .collect();
        assert_eq!(statuses, ["opening", "failed"], "{report}");
        assert_eq!(
            std::fs::read_to_string(opened).unwrap(),
            format!("-b com.loopflow.mac -g {task_url}\n"),
            "one background request"
        );
    }
}

#[test]
fn reopening_a_live_conversation_focuses_its_cmux_workspace() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let provider = Child(Command::new("/bin/sleep").arg("3600").spawn().unwrap());
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let primary = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Design",
        0,
        1,
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
        rusqlite::params![registered.task.id.as_str(), primary],
    )
    .unwrap();
    db.execute(
        "UPDATE processes SET terminal_id='cmux:WS-1' WHERE id=(SELECT attached_lf_process_id FROM processes p JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1)",
        [&primary],
    )
    .unwrap();
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));

    let log = host(bin.path(), "cmux", "OK", 0);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "cmux", "--json"],
    ));
    assert_eq!(report["receipts"][0]["status"], "usable");
    assert_eq!(report["receipts"][0]["destination"], "WS-1");
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "workspace select WS-1\n",
        "the existing workspace is focused; nothing is created or resent"
    );

    // A closed workspace leaves the conversation where it is.
    std::fs::remove_file(&log).unwrap();
    host(bin.path(), "cmux", "", 1);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "cmux", "--json"],
    ));
    assert_eq!(report["receipts"][0]["status"], "elsewhere");
    assert!(!std::fs::read_to_string(&log)
        .unwrap()
        .contains("workspace create"));
}

#[test]
fn concurrent_and_repeated_openings_create_one_destination() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let log = host(bin.path(), "cmux", "OK workspace:7", 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let receipts: Vec<Value> = std::thread::scope(|scope| {
        let openings: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    json(&lf(
                        home.path(),
                        repo.path(),
                        Some(bin.path()),
                        &["open", &url, "--host", "cmux", "--json"],
                    ))
                })
            })
            .collect();
        openings
            .into_iter()
            .map(|opening| opening.join().unwrap())
            .collect()
    });
    let again = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "cmux", "--json"],
    ));
    for report in receipts.iter().chain([&again]) {
        assert_eq!(report["receipts"][0]["status"], "opening");
        assert_eq!(report["receipts"][0]["destination"], "workspace:7");
    }
    let calls = std::fs::read_to_string(log).unwrap();
    assert_eq!(
        calls.matches("workspace create").count(),
        1,
        "one conversation destination: {calls}"
    );
    assert_eq!(calls.matches("workspace select workspace:7").count(), 2);
}

#[test]
fn links_tell_the_current_conversation_from_another_on_the_same_task() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let provider = Child(Command::new("/bin/sleep").arg("3600").spawn().unwrap());
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let primary = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Design",
        0,
        1,
    );
    let step = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Graph step",
        1,
        0,
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
        rusqlite::params![registered.task.id.as_str(), primary],
    )
    .unwrap();
    db.execute(
        "UPDATE agent_sessions SET task_id=?2,wave_id=?3 WHERE id=?1",
        rusqlite::params![
            step,
            registered.task.id.as_str(),
            registered.task.wave_id.as_ref().unwrap().as_str()
        ],
    )
    .unwrap();
    let origin: String = db
        .query_row(
            "SELECT id FROM processes WHERE trace_id='fixture' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let caller = |session: &str| {
        serde_json::json!({"session_id": session, "agent_process_id": null, "origin_exec_id": origin})
            .to_string()
    };
    let relation = |session: &str| {
        let caller = caller(session);
        json(&lf_with(
            home.path(),
            repo.path(),
            None,
            &[("LF_AGENT_CALLER", &caller)],
            &["open", &url, "--link", "--json"],
        ))[0]["conversation"]
            .clone()
    };
    assert_eq!(relation(&primary), "current_conversation");
    assert_eq!(
        relation(&step),
        "other_conversation",
        "sharing a Task does not make a Graph step the Task's conversation"
    );

    let own = caller(&primary);
    let output = lf_with(
        home.path(),
        repo.path(),
        None,
        &[("LF_AGENT_CALLER", &own)],
        &["open", &url],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is this conversation; continue here"));
}

#[test]
fn a_task_identifier_connects_and_a_viewerless_terminal_reads_detached() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let provider = Child(Command::new("/bin/sleep").arg("3600").spawn().unwrap());
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let primary = live_conversation(
        home.path(),
        &provider.0,
        started,
        repo.path(),
        "Design",
        0,
        1,
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
        rusqlite::params![registered.task.id.as_str(), primary],
    )
    .unwrap();
    db.execute(
        "UPDATE processes SET terminal_id='cmux:WS-1' WHERE id=(SELECT attached_lf_process_id FROM processes p JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1)",
        [&primary],
    )
    .unwrap();

    // A Task identifier in place of a Session id opens the Task's conversation.
    let opened = json(&lf(
        home.path(),
        repo.path(),
        None,
        &["session", "resume", "INF-123", "--prepare", "--json"],
    ));
    assert_eq!(opened["id"], primary.as_str());
    let missing = lf(
        home.path(),
        repo.path(),
        None,
        &["session", "resume", "NOPE-185", "--prepare"],
    );
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("Session NOPE-185 was not found"));

    // Its execution owner keeps running after the last viewer's terminal closed.
    let owner = loopflow::id::LfProcessId::new();
    db.execute(
        "INSERT INTO processes(id,trace_id,started_at,viewed) VALUES(?1,'fixture',1,0)",
        [&owner],
    )
    .unwrap();
    db.execute(
        "UPDATE agent_sessions SET transport_lf_process_id=?2 WHERE id=?1",
        rusqlite::params![primary, owner],
    )
    .unwrap();
    let listed = |state: &str| {
        let sessions = json(&lf(
            home.path(),
            repo.path(),
            None,
            &["desktop", "sessions", "--json", "--all"],
        ));
        let session = sessions
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == primary.as_str())
            .unwrap()
            .clone();
        assert_eq!(session["state"], state, "{session}");
        session
    };
    let detached = listed("detached");
    assert_eq!(detached["terminal_ids"], serde_json::json!([]));
    assert_eq!(detached["actions"][0]["label"], "Reconnect");
    assert!(detached["actions"][0]["unavailable_reason"].is_null());
    assert_eq!(detached["actions"].as_array().unwrap().len(), 1);

    db.execute("UPDATE processes SET viewed=1 WHERE id=?1", [&owner])
        .unwrap();
    let viewed = listed("active");
    assert_eq!(viewed["terminal_ids"], serde_json::json!(["cmux:WS-1"]));
}
