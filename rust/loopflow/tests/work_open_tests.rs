use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::rex::IsolatedRex;
use support::{register_task_without_pr, EnvGuard};

/// Native host fixtures own the server and every terminal process it starts.
struct NativeServer(std::process::Child);

impl Drop for NativeServer {
    fn drop(&mut self) {
        // SAFETY: each fixture starts its server as a new process group leader.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGTERM);
        }
        let _ = self.0.wait();
    }
}

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
    lf_command(home, cwd, bin, env, args).output().unwrap()
}

fn lf_command(
    home: &Path,
    cwd: &Path,
    bin: Option<&Path>,
    env: &[(&str, &str)],
    args: &[&str],
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if ["LF_", "CMUX_", "HERDR_", "REX_", "TMUX"]
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
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null());
    command
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
    let reply = reply.replace('\'', "'\"'\"'");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nprintf '%s\\n' '{reply}'\nexit {status}\n",
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
fn files_without_a_host_are_a_noop_even_outside_a_repository() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let result = json(&lf(
        home.path(),
        cwd.path(),
        None,
        &["open", "--file", "not-created.md", "--json"],
    ));
    assert_eq!(result["opening"], false);
    assert!(result["host"].is_null());
    assert_eq!(result["paths"], serde_json::json!(["not-created.md"]));
}

#[test]
fn files_report_unresolved_work_instead_of_an_absent_desktop() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let result = lf(
        home.path(),
        cwd.path(),
        None,
        &["open", "--task", "INF-404", "--file", "guide.md", "--json"],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Task resolution unavailable"));
    assert!(result.stdout.is_empty());
}

#[test]
fn desktop_file_presentation_without_desktop_never_falls_back_to_a_terminal() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let path = repo.path().join("guide.md");
    std::fs::write(&path, "saved correction").unwrap();
    let launches = ["rex", "herdr", "tmux", "cmux", "open"]
        .map(|name| host(bin.path(), name, "unexpected host launch", 1));
    for context in [
        vec![],
        vec![
            ("REX_SESSION", "session:current"),
            ("REX_BLOCK", "block:current"),
        ],
        vec![("HERDR_SOCKET_PATH", "/fixture/herdr.sock")],
        vec![("TMUX", "fixture"), ("TMUX_PANE", "%1")],
        vec![
            ("CMUX_WORKSPACE_ID", "workspace:current"),
            ("CMUX_SOCKET_PATH", "/fixture/cmux.sock"),
        ],
    ] {
        let result = json(&lf_with(
            home.path(),
            repo.path(),
            Some(bin.path()),
            &context,
            &[
                "open", "--host", "desktop", "--task", "INF-123", "--file", "guide.md", "--json",
            ],
        ));
        assert_eq!(result["opening"], false);
        assert!(result["host"].is_null());
        assert_eq!(
            result["paths"],
            serde_json::json!([path.canonicalize().unwrap()])
        );
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), "saved correction");
    assert_eq!(sessions(home.path()), 0);
    assert!(launches.iter().all(|path| !path.exists()));
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

    let repository_url = format!("loopflow://{}", folder(repo.path()));
    for args in [
        vec!["open", &repository_url, "--link", "--json"],
        vec![
            "--repo",
            repo.path().to_str().unwrap(),
            "open",
            "--link",
            "--json",
        ],
    ] {
        let links = json(&lf(home.path(), repo.path(), None, &args));
        assert_eq!(links[0]["url"], repository_url);
        assert!(links[0]["target"]["task_id"].is_null());
    }
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
fn cmux_launches_a_fresh_task_workspace_with_its_splits() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let log = host(bin.path(), "cmux", "OK workspace:7", 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    for _ in 0..2 {
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
    }
    let calls = std::fs::read_to_string(log).unwrap();
    let created: Vec<&str> = calls
        .lines()
        .filter(|line| line.starts_with("new-workspace --name "))
        .collect();
    assert_eq!(created.len(), 2, "each open is a fresh launch: {calls}");
    for call in &created {
        assert!(call.contains(" --layout {"), "{call}");
        assert!(call.contains(" --focus false"), "{call}");
        assert!(
            !call.contains("--group"),
            "a Task joins the caller's window: {call}"
        );
        assert!(
            call.contains(r"'task' 'view'") && call.contains(r"'shell'"),
            "{call}"
        );
        assert!(call.contains("resume"), "{call}");
        // cmux starts surfaces from its own environment; this private Home goes with it.
        assert!(call.contains("'LF_HOME="), "{call}");
    }
    assert_eq!(calls.matches("workspace select workspace:7").count(), 2);
    assert_eq!(sessions(home.path()), 0, "opening creates no conversation");
}

#[test]
fn cmux_task_joins_the_group_its_caller_is_in_now() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let log = bin.path().join("cmux.log");
    // The caller's workspace moved into group G-2 after it launched.
    let groups = r#"{"window_id":"W","groups":[{"id":"G-1","anchor_workspace_id":"A","member_workspace_ids":["A"]},{"id":"G-2","anchor_workspace_id":"B","member_workspace_ids":["B","CALLER"]}]}"#;
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \"$1 $2\" in\n  'workspace-group list') printf '%s\\n' '{groups}' ;;\n  *) echo 'OK workspace:9' ;;\nesac\n",
        log.display()
    );
    let path = bin.path().join("cmux");
    std::fs::write(&path, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let report = json(&lf_with(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[("CMUX_WORKSPACE_ID", "CALLER")],
        &[
            "open",
            "--task",
            "INF-123",
            "--host",
            "cmux",
            "--no-focus",
            "--json",
        ],
    ));
    let receipt = &report["receipts"][0];
    assert_eq!(receipt["destination"], "workspace:9");
    assert_eq!(receipt["container"], "G-2");
    let calls = std::fs::read_to_string(log).unwrap();
    let created = calls
        .lines()
        .find(|line| line.starts_with("new-workspace "))
        .unwrap();
    assert!(
        created.contains("--group G-2 --group-placement end"),
        "{calls}"
    );
    assert!(!calls.contains("workspace-group create"), "{calls}");
}

#[test]
fn herdr_without_server_context_creates_no_session() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let output = lf(
        home.path(),
        repo.path(),
        None,
        &["open", "--task", "INF-123", "--host", "herdr", "--json"],
    );
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["receipts"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("HERDR_SOCKET_PATH"));
    assert_eq!(sessions(home.path()), 0);
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
fn refused_host_reports_its_failure_without_another_host() {
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
    assert!(text.contains("nothing was retried"), "{text}");
    assert!(
        !text.contains("terminal"),
        "no other host was tried: {text}"
    );
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
    // The caller's own host is the only destination; without one, Desktop
    // then a plain terminal.
    let order = |first: &[&str]| {
        let hosts: Vec<&str> = if first.is_empty() {
            desktop.iter().chain(&["terminal"]).copied().collect()
        } else {
            first.to_vec()
        };
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

    let log = host(bin.path(), "tmux", "%1", 0);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", "--available", "--host", "tmux", "--json"],
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
        // A caller inside a Desktop addresses that Desktop by id; nothing here
        // is installed, so the Machine's own Loopflow.app never answers.
        let opened = host(bin.path(), "open", "", 0);
        let output = lf_with(
            home.path(),
            repo.path(),
            Some(bin.path()),
            &[("LF_DESKTOP", "com.loopflow.mac.test")],
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
            format!("-b com.loopflow.mac.test -g {task_url}\n"),
            "one background request"
        );
    }
}

#[test]
fn reopening_a_live_conversation_focuses_its_tmux_pane() {
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
        "UPDATE processes SET terminal_id='tmux:%5' WHERE id=(SELECT attached_lf_process_id FROM processes p JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1)",
        [&primary],
    )
    .unwrap();
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));

    let log = host(bin.path(), "tmux", "", 0);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "tmux", "--json"],
    ));
    assert_eq!(report["receipts"][0]["status"], "usable");
    assert_eq!(report["receipts"][0]["destination"], "%5");
    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(
        calls.starts_with("select-window -t %5"),
        "the existing pane is focused; nothing is created or resent: {calls}"
    );
    assert!(!calls.contains("new-window"), "{calls}");

    // A closed pane leaves the conversation where it is.
    std::fs::remove_file(&log).unwrap();
    host(bin.path(), "tmux", "", 1);
    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", &url, "--host", "tmux", "--json"],
    ));
    assert_eq!(report["receipts"][0]["status"], "elsewhere");
    assert!(!std::fs::read_to_string(&log)
        .unwrap()
        .contains("new-window"));
}

#[test]
fn concurrent_and_repeated_openings_create_one_destination() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let log = host(bin.path(), "tmux", "%7", 0);
    let url = format!("loopflow://{}/task/INF-123", folder(repo.path()));
    let receipts: Vec<Value> = std::thread::scope(|scope| {
        let openings: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    json(&lf(
                        home.path(),
                        repo.path(),
                        Some(bin.path()),
                        &["open", &url, "--host", "tmux", "--json"],
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
        &["open", &url, "--host", "tmux", "--json"],
    ));
    for report in receipts.iter().chain([&again]) {
        assert_eq!(report["receipts"][0]["status"], "opening");
        assert_eq!(report["receipts"][0]["destination"], "%7");
    }
    let calls = std::fs::read_to_string(log).unwrap();
    assert_eq!(
        calls.matches("new-window").count(),
        1,
        "one conversation destination: {calls}"
    );
    assert_eq!(calls.matches("select-window -t %7").count(), 2, "{calls}");
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

    // A resumed conversation's live viewer outranks its stale launch environment.
    for (placement, expected) in [
        ("rex:{\"server\":\"unix:///current\",\"session\":\"session:current\",\"block\":\"block:current\"}", "rex"),
        ("cmux:current", "cmux"),
    ] {
        db.execute(
            "UPDATE processes SET terminal_id=?2 WHERE id=(SELECT attached_lf_process_id FROM processes p JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1)",
            rusqlite::params![primary, placement],
        ).unwrap();
        let links = json(&lf_with(home.path(), repo.path(), None,
            &[("LF_AGENT_CALLER", &own), ("REX_SESSION", "session:stale"), ("REX_BLOCK", "block:stale")],
            &["open", &url, "--link", "--json"]));
        assert_eq!(links[0]["route"]["hosts"][0], expected);
    }
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

#[test]
fn rex_each_open_launches_a_fresh_task_tab_without_finding_earlier_ones() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    let args = [
        "open",
        "--task",
        "INF-123",
        "--host",
        "rex",
        "--no-focus",
        "--json",
    ];
    let first = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    rex_reply(
        bin.path(),
        "session.new_window",
        serde_json::json!({"window_id":"window:second"}),
    );
    let second = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(first["receipts"][0]["destination"], "window:fixture");
    assert_eq!(second["receipts"][0]["destination"], "window:second");
    let created = std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    assert_eq!(created.lines().count(), 2, "{created}");
    for request in created.lines() {
        assert!(request.contains("resume"));
        assert_eq!(
            request.matches("Task & workflow").count(),
            1,
            "The builtin layout has one workflow pane beside resume and shell"
        );
    }
    // Opening neither searches for nor touches an earlier native workspace.
    assert!(!bin.path().join("session.list.requests").exists());
    assert!(!bin.path().join("session.view.requests").exists());
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_display_selection_ignores_the_listing_cli_that_has_already_exited() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    rex_reply(bin.path(), "session.focus_window", serde_json::json!({}));
    rex_reply(
        bin.path(),
        "client.list",
        serde_json::json!({"clients":[
            {"client_id":"client:listing","info":{"kind":"cli"}},
            {"client_id":"client:app","info":{"kind":"app"}}
        ]}),
    );
    rex_reply(
        bin.path(),
        "client.inspect",
        serde_json::json!({"actions":[{"name":"session.select"}]}),
    );
    rex_reply(bin.path(), "client.perform", serde_json::json!({}));
    let peer = bin.path().join("rex-peer");
    std::fs::rename(bin.path().join("rex"), &peer).unwrap();
    std::fs::write(bin.path().join("rex"), format!(
        "#!/bin/sh\ncase \"$*\" in *client:listing*) echo 'client not found' >&2; exit 1;; esac\nexec '{}' \"$@\"\n",
        peer.display(),
    )).unwrap();
    std::fs::set_permissions(
        bin.path().join("rex"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    let report = json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &["open", "--task", "INF-123", "--host", "rex", "--json"],
    ));
    assert!(
        report["receipts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("Rex accepted display selection"),
        "{report}"
    );
}

#[test]
fn configured_task_layout_shapes_a_launch_without_a_saved_desktop_layout() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    std::fs::write(
        repo.path().join("lf.config.local.yaml"),
        "task_layout:\n  hidden: [conversation]\n  tree:\n    kind: split\n    axis: vertical\n    ratio: 0.5\n    first:\n      kind: split\n      axis: horizontal\n      ratio: 0.5\n      first:\n        kind: pane\n        id: conversation\n        content: {kind: session, session_id: null, location: null}\n      second:\n        kind: pane\n        id: changes\n        content:\n          kind: browser\n          location: null\n          tabs:\n            - kind: changes\n              id: changes\n              comparison: {kind: working_tree, base: parent}\n          selected: changes\n    second:\n      kind: pane\n      id: shell\n      content:\n        kind: shell\n        tabs: [{id: s, location: null, lf_process_id: null}]\n        selected: s\n",
    )
    .unwrap();
    rex_peer(bin.path());
    json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            "open",
            "--task",
            "INF-123",
            "--host",
            "rex",
            "--no-focus",
            "--json",
        ],
    ));
    let created = std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    let request: Value = serde_json::from_str(created.trim()).unwrap();
    let split = &request["layout"]["split"];
    let words = |pane: &Value| pane["block"]["options"]["command"].to_string();
    assert!(words(&split["before"]).contains("\"changes\""), "{request}");
    assert!(words(&split["after"]).contains("\"shell\""), "{request}");
    assert!(
        !created.contains("Task & workflow"),
        "the template replaced the builtin"
    );
}

// The peer exposes inventory independently of replies, including writes whose reply is lost.
fn rex_peer(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let script = bin.join("rex");
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
while [ "$1" != "whoami" ] && [ "$1" != "call" ]; do shift; done
if [ "$1" = whoami ]; then
  echo '{{"endpoint":"unix:///fixture/rex.sock","principal":{{"transport":"unix"}}}}'
  exit
fi
method="$2"
printf '%s\n' "$3" >> '{bin}/'"$method.requests"
cat '{bin}/'"$method.reply"
"#,
            bin = bin.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
    for (method, reply) in [
        ("session.list", serde_json::json!({"sessions":[]})),
        ("session.view", serde_json::json!({"windows":[]})),
        (
            "session.create",
            serde_json::json!({"session_id":"session:fixture"}),
        ),
        (
            "session.new_window",
            serde_json::json!({"window_id":"window:fixture"}),
        ),
    ] {
        rex_reply(bin, method, reply);
    }
}

fn rex_reply(bin: &Path, method: &str, reply: Value) {
    std::fs::write(bin.join(format!("{method}.reply")), reply.to_string()).unwrap();
}

/// One follower produces every Machine's arrivals. A Machine with nothing open gets
/// its container on the first arrival; later arrivals append to the handle the
/// follower holds; an uncertain creation is never followed by another.
#[test]
fn rex_follower_places_remote_arrivals_in_one_container_per_machine() {
    use loopflow::durable::MachineId;
    use loopflow::store::sqlite::SqliteStore;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let peer = MachineId::new();
    store
        .add_machine(&peer, "remote.invalid", "Remote")
        .unwrap();
    store.claim_task(&registered.task.id).unwrap();
    store.handoff_task(&registered.task.id, &peer).unwrap();
    // Remote checkout state is not copied here; the peer decides eligibility.
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE tasks SET worktree=NULL WHERE id=?1",
            [registered.task.id.as_str()],
        )
        .unwrap();
    rex_peer(bin.path());
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture"}]}),
    );
    let url = format!("loopflow://{}", folder(repo.path()));
    let receipt = serde_json::json!({"receipts":[{
        "label":"INF-123 · Task","url":format!("{url}/task/INF-123"),"host":"rex",
        "destination":"window:remote","container":"session:remote","status":"opening",
        "reason":"Created","fallback_argv":[]}],"unknown":[]});
    let requests = host(bin.path(), "ssh", &receipt.to_string(), 0);
    let follow = |remote: Option<String>| {
        let mut args = vec![
            "open".to_owned(),
            url.clone(),
            "--host".into(),
            "rex".into(),
            "--server".into(),
            "unix:///fixture/rex.sock".into(),
            "--follow".into(),
            "session:fixture".into(),
        ];
        args.extend(
            remote
                .into_iter()
                .flat_map(|value| ["--remote".into(), value]),
        );
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = lf(home.path(), repo.path(), Some(bin.path()), &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let sent = std::fs::read_to_string(&requests).unwrap_or_default();
        let _ = std::fs::remove_file(&requests);
        (sent, String::from_utf8_lossy(&output.stdout).into_owned())
    };

    let (sent, stdout) = follow(None);
    assert!(
        sent.contains("--arrive-task")
            && sent.contains(registered.task.id.as_str())
            && sent.contains("--local-workspace")
            && sent.contains("--no-focus"),
        "{sent}"
    );
    assert!(
        !sent.contains("--into"),
        "the first arrival creates: {sent}"
    );
    assert!(stdout.contains("Added INF-123"), "{stdout}");

    let (sent, _) = follow(Some(format!("{peer}=session:remote")));
    assert!(sent.contains("'--into' 'session:remote'"), "{sent}");

    let (sent, _) = follow(Some(format!("{peer}=")));
    assert!(
        sent.is_empty(),
        "uncertain creation is not followed: {sent}"
    );

    assert!(
        !bin.path().join("session.new_window.requests").exists(),
        "no local fallback"
    );
    assert!(!bin.path().join("session.create.requests").exists());

    // A broad launch hands the peer's container and opened Tasks to its one
    // follower; the peer starts no follower of its own.
    let launch = lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            "open",
            &url,
            "--host",
            "rex",
            "--server",
            "unix:///fixture/rex.sock",
            "--json",
        ],
    );
    assert!(
        launch.status.success(),
        "{}",
        String::from_utf8_lossy(&launch.stderr)
    );
    let sent = std::fs::read_to_string(&requests).unwrap();
    assert!(
        sent.contains("'--local-workspace'") && !sent.contains("--arrive-task"),
        "{sent}"
    );
    let workspace =
        std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    assert!(
        workspace.contains(&format!("{peer}=session:remote"))
            && workspace.contains(registered.task.id.as_str()),
        "{workspace}"
    );
}

#[test]
fn rex_created_shell_opens_in_its_own_session_without_placement_flags() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    json(&lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            "open",
            "--task",
            "INF-123",
            "--host",
            "rex",
            "--server",
            "unix:///fixture/rex.sock",
            "--no-focus",
            "--json",
        ],
    ));
    let launches = bin.path().join("session.new_window.requests");
    let created = std::fs::read_to_string(&launches).unwrap();
    let request: Value = serde_json::from_str(created.trim()).unwrap();
    let command = request["layout"]["split"]["after"]["split"]["after"]["block"]["options"]
        ["command"]
        .as_array()
        .unwrap();
    let args = command
        .iter()
        .map(|word| word.as_str().unwrap())
        .collect::<Vec<_>>();
    // Execute the generated native shell launcher. Rex supplies block/session
    // identity; the launcher must carry the server across task-tool's exec.
    let shell = bin.path().join("fixture-shell");
    std::fs::write(&shell, format!(
        "#!/bin/sh\n[ \"$REX_SERVER\" = unix:///fixture/rex.sock ] || exit 91\nexec '{}' open --no-focus --json\n", env!("CARGO_BIN_EXE_lf")
    )).unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture"}]}),
    );
    rex_reply(
        bin.path(),
        "session.view",
        serde_json::json!({"windows":[{"window_id":"window:fixture","layers":[{"blocks":[{"block_id":"block:shell"}]}]}]}),
    );
    let mut launch = Command::new(args[0]);
    launch
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("SHELL", shell)
        .env("REX_SESSION", "session:fixture")
        .env("REX_BLOCK", "block:shell")
        .current_dir(repo.path())
        .args(&args[1..]);
    rex_reply(
        bin.path(),
        "session.new_window",
        serde_json::json!({"window_id":"window:again"}),
    );
    let reopened = json(&launch.output().unwrap());
    assert_eq!(reopened["receipts"][0]["destination"], "window:again");
    let launched = std::fs::read_to_string(launches).unwrap();
    let again: Value = serde_json::from_str(launched.lines().nth(1).unwrap()).unwrap();
    assert_eq!(
        again["session_id"], "session:fixture",
        "a Task opened inside Rex joins the caller's session"
    );
    let containers = std::fs::read_to_string(bin.path().join("session.create.requests")).unwrap();
    assert_eq!(containers.lines().count(), 1, "{containers}");
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_agent_tool_open_uses_the_moved_viewer_instead_of_startup_context() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let provider = NativeServer(
        Command::new("/bin/sleep")
            .arg("3600")
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let session = live_conversation(
        home.path(),
        &provider.0,
        time::OffsetDateTime::now_utc().unix_timestamp(),
        repo.path(),
        "Agent tool caller",
        1,
        0,
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let (agent, origin, token): (String, String, String) = db
        .query_row(
            "SELECT p.id,p.attached_lf_process_id,p.attachment_token FROM processes p
             JOIN agent_sessions s ON s.agent_process_id=p.id WHERE s.id=?1",
            [&session],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let caller = serde_json::json!({
        "session_id":session, "agent_process_id":agent, "origin_exec_id":origin,
    })
    .to_string();
    rex_peer(bin.path());
    // Fail on the stale/default server, and expose the requested current endpoint.
    let peer = bin.path().join("rex-peer");
    std::fs::rename(bin.path().join("rex"), &peer).unwrap();
    let wrapper = format!(
        r#"#!/bin/sh
while [ "$#" -gt 0 ] && [ "$1" != -S ]; do shift; done
[ "$#" -gt 1 ] || exit 91
endpoint="$2"
case "$endpoint" in unix:///current|unix:///moved) ;; *) exit 92;; esac
shift 2
case " $* " in
  *' whoami '*) printf '{{"endpoint":"%s","principal":{{"transport":"unix"}}}}\n' "$endpoint";;
  *) exec '{}' "$@";;
esac
"#,
        peer.display()
    );
    std::fs::write(bin.path().join("rex"), wrapper).unwrap();
    std::fs::set_permissions(
        bin.path().join("rex"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    for destination in ["current", "moved"] {
        let placement = format!(
            "rex:{}",
            serde_json::json!({
                "server":format!("unix:///{destination}"),
                "session":format!("session:{destination}"),
                "block":format!("block:{destination}"),
            })
        );
        db.execute(
            "UPDATE processes SET terminal_id=?2 WHERE id=?1",
            rusqlite::params![origin, placement],
        )
        .unwrap();
        rex_reply(
            bin.path(),
            "session.view",
            serde_json::json!({
                "windows":[{"window_id":format!("window:{destination}"),"layers":[{
                    "blocks":[{"block_id":format!("block:{destination}")}]
                }]}]
            }),
        );
        rex_reply(
            bin.path(),
            "session.new_window",
            serde_json::json!({
                "window_id":format!("window:new-{destination}")
            }),
        );
        let report = json(&lf_with(
            home.path(),
            repo.path(),
            Some(bin.path()),
            &[
                ("LF_AGENT_CALLER", &caller),
                ("REX_SERVER", "unix:///stale"),
                ("REX_SESSION", "session:stale"),
                ("REX_BLOCK", "block:stale"),
            ],
            &["open", "--task", "INF-123", "--no-focus", "--json"],
        ));
        assert_eq!(report["receipts"][0]["host"], "rex");
        assert_eq!(
            report["receipts"][0]["destination"],
            format!("window:new-{destination}")
        );
    }
    let retained: (String, String) = db
        .query_row(
            "SELECT attached_lf_process_id,attachment_token FROM processes WHERE id=?1",
            [&agent],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(retained, (origin, token));
    assert_eq!(sessions(home.path()), 1);
}

#[test]
fn rex_unavailable_context_never_falls_through_to_desktop() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let rex = host(bin.path(), "rex", "unavailable", 1);
    let desktop = host(bin.path(), "open", "", 0);
    let output = lf_with(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            ("REX_SESSION", "session:missing"),
            ("REX_BLOCK", "block:missing"),
        ],
        &["open", "--task", "INF-123", "--json"],
    );
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    if let Ok(queries) = std::fs::read_to_string(rex) {
        // The test runner itself may descend from Rex. Only that explicit
        // listener may be queried; the stand-in still reports it unavailable.
        assert!(queries.lines().all(|line| line.contains("-S ")));
    } else {
        assert!(report["receipts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("Rex caller server is unavailable from its process ancestry"));
    }
    assert!(!desktop.exists());
    assert_eq!(sessions(home.path()), 0);
}

// Arrangement tests exercise tools only; conversation execution has its own
// isolated native test with a stand-in provider in transport_tests.rs.
fn configure_native_tools(repo: &Path) {
    std::fs::write(repo.join("lf.config.local.yaml"), r#"task_layout:
  hidden: [conversation]
  tree:
    kind: split
    axis: vertical
    ratio: 0.6
    first: {kind: pane, id: browser, content: {kind: browser, location: null, tabs: [], selected: null}}
    second:
      kind: split
      axis: horizontal
      ratio: 0.5
      first: {kind: pane, id: conversation, content: {kind: session, session_id: null, location: null}}
      second: {kind: pane, id: shell, content: {kind: shell, tabs: [], selected: null}}
"#).unwrap();
}

/// Installed-host acceptance stays opt-in; CI and ordinary tests use bounded peers.
#[test]
#[ignore = "requires the installed Rex CLI; creates and destroys an isolated local server"]
fn rex_native_fresh_launches_keep_earlier_arrangements_and_task_directories() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    let root = tempfile::Builder::new()
        .prefix("rex498-")
        .tempdir_in("/tmp")
        .unwrap();
    let repo = TestRepo::new();
    let home = root.path().join("lf");
    std::fs::create_dir(&home).unwrap();
    let _env = EnvGuard::with_lf_home(&[], &home);
    let registered = register_task_without_pr(&home, repo.path(), "main", &repo.head_sha());
    configure_native_tools(repo.path());
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    host(
        &bin,
        "open",
        "Desktop is excluded from this isolated Rex test",
        1,
    );
    // Record the actual launch directory, then run the real viewer. Rex's
    // background process snapshot can retain the initial login process.
    let viewer = bin.join("lf-viewer");
    std::fs::write(
        &viewer,
        format!(
            "#!/bin/sh\nif [ \"$3\" = task ] && [ \"$4\" = view ]; then\n pwd -P > '{}/cwd-'\"$5\".pending\n mv '{}/cwd-'\"$5\".pending '{}/cwd-'\"$5\"\nfi\nexec '{}' \"$@\"\n",
            root.path().display(), root.path().display(), root.path().display(), env!("CARGO_BIN_EXE_lf"),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&viewer, std::fs::Permissions::from_mode(0o755)).unwrap();
    let server = IsolatedRex::start(root.path(), &bin, &viewer);
    let endpoint = server.endpoint.clone();
    let args = [
        "open", "--task", "INF-123", "--host", "rex", "--server", &endpoint, "--json",
    ];
    let first = json(&lf(&home, repo.path(), None, &args));
    let window = first["receipts"][0]["destination"].as_str().unwrap();
    let call = |method: &str, args: Value| server.call(method, args);
    let inventory = call("session.list", serde_json::json!({}));
    let container = inventory["sessions"][0]["session_id"].as_str().unwrap();
    call(
        "session.set_window_label",
        serde_json::json!({"session_id":container,"window_id":window,"label":"My renamed arrangement"}),
    );
    // Rex's wrapper hides a tool's cwd, so its report is all a split can inherit;
    // a shell has no report to outlive `cd`, and the host reads its process.
    let blocks = call(
        "session.list_blocks",
        serde_json::json!({"session_id":container}),
    );
    let checkout = repo.path().canonicalize().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    for block in blocks["blocks"].as_array().unwrap() {
        let id = block["block_id"].as_str().unwrap();
        let is_shell = block["label"] == "shell";
        loop {
            let reported = server.pwd(container, id);
            let process = server.block_call(container, id, "process");
            let cwd = process["foreground"]["cwd"].as_str().map(Path::new);
            let settled = if is_shell {
                reported.is_none() && cwd == Some(checkout.as_path())
            } else {
                reported.is_some_and(|url| url.ends_with(&format!("{}/", repo.path().display())))
            };
            if settled {
                break;
            }
            assert!(Instant::now() < deadline, "{block}: {process}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let before = call("session.view", serde_json::json!({"session_id":container}));
    let second = json(&lf(&home, repo.path(), None, &args));
    assert_ne!(second["receipts"][0]["destination"], window);
    assert_eq!(
        before,
        call("session.view", serde_json::json!({"session_id":container})),
        "another launch leaves the earlier native arrangement untouched"
    );
    assert_eq!(
        sessions(&home),
        0,
        "workspace painting cannot prepare or start providers"
    );
    let reply = root.path().join("caller.json");
    call(
        "session.new_window",
        serde_json::json!({"session_id":container,"focus":false,"layout":{"block":{"flavor":"com.superlogical.terminal.shell","options":{"shell":"none","cwd":repo.path(),"command":["/bin/sh","-c","PATH=\"$5:$PATH\" LF_HOME=\"$1\" \"$2\" --cwd \"$3\" --task INF-123 open --json > \"$4\" 2> \"$4.err\"; printf '%s' \"$?\" > \"$4.exit\"; exec /bin/cat","caller",home,env!("CARGO_BIN_EXE_lf"),repo.path(),reply,bin]}}}}),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let automatic = loop {
        if let Ok(bytes) = std::fs::read(&reply) {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                break value;
            }
        }
        if let Ok(status) = std::fs::read_to_string(reply.with_extension("json.exit")) {
            panic!(
                "Rex caller exited {status} without a receipt: stdout={} stderr={}",
                std::fs::read_to_string(&reply).unwrap_or_default(),
                std::fs::read_to_string(reply.with_extension("json.err")).unwrap_or_default(),
            );
        }
        assert!(
            Instant::now() < deadline,
            "Rex shell did not return its automatic-open receipt: {}",
            std::fs::read_to_string(reply.with_extension("json.err")).unwrap_or_default(),
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(automatic["receipts"][0]["host"], "rex");
    let inside = automatic["receipts"][0]["destination"].as_str().unwrap();
    assert_ne!(inside, window, "{automatic}");
    assert!(
        call("session.view", serde_json::json!({"session_id":container}))["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate["window_id"] == inside),
        "a Task opened inside Rex joins the caller's session"
    );
    let existing: Vec<Value> = call("session.list", serde_json::json!({}))["sessions"]
        .as_array()
        .unwrap()
        .clone();
    let repository_url = format!("loopflow://{}", folder(repo.path()));
    json(&lf(
        &home,
        repo.path(),
        None,
        &[
            "open",
            &repository_url,
            "--host",
            "rex",
            "--server",
            &endpoint,
            "--no-focus",
            "--json",
        ],
    ));
    let inventory = call("session.list", serde_json::json!({}));
    let workspace = inventory["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|session| !existing.contains(session))
        .unwrap_or_else(|| panic!("repository opening created no session: {inventory}"))
        ["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let deadline = Instant::now() + Duration::from_secs(15);
    let before_arrival = loop {
        let view = call("session.view", serde_json::json!({"session_id":workspace}));
        if view["windows"]
            .as_array()
            .is_some_and(|windows| windows.iter().any(|window| window["label"] == "Workspace"))
        {
            break view;
        }
        assert!(
            Instant::now() < deadline,
            "follower tab did not appear: {view}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let sibling = repo.create_named_worktree("arriving-task");
    let sibling_task =
        support::register_sibling_task(&registered, "INF-124", "arriving-task", &sibling);
    let deadline = Instant::now() + Duration::from_secs(15);
    let arrived = loop {
        let current = call("session.view", serde_json::json!({"session_id":workspace}));
        if current["windows"].as_array().unwrap().len()
            > before_arrival["windows"].as_array().unwrap().len()
        {
            break current;
        }
        if Instant::now() >= deadline {
            let follower = current["windows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|window| window["label"] == "Workspace")
                .map(|window| window["layers"][0]["blocks"][0]["block_id"].clone());
            panic!(
                "Started Task did not arrive in the native session: {current}\nfollower: {}",
                follower.map_or(Value::Null, |block| call(
                    "com.superlogical.terminal.format",
                    serde_json::json!({"session_id":workspace,"block_id":block,"args":{"format":"text"}}),
                ))
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(
        before_arrival["active_window_id"], arrived["active_window_id"],
        "arrivals never take focus"
    );
    let arrival = arrived["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| {
            window["label"]
                .as_str()
                .is_some_and(|label| label.contains("INF-124") || label.contains("arriving"))
                || !before_arrival["windows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|old| old["window_id"] == window["window_id"])
        })
        .unwrap()
        .clone();
    let block_count: usize = arrival["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|layer| layer["blocks"].as_array().unwrap().len())
        .sum();
    assert!(
        block_count > 1,
        "an arriving Task gets its normal configured splits: {arrival}"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let arrival_cwd = loop {
        if let Ok(cwd) =
            std::fs::read_to_string(root.path().join(format!("cwd-{}", sibling_task.id)))
        {
            break cwd;
        }
        assert!(
            Instant::now() < deadline,
            "Arriving Task view did not record its directory"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(
        std::fs::canonicalize(arrival_cwd.trim()).unwrap(),
        std::fs::canonicalize(&sibling).unwrap(),
        "The arriving Task view must execute in its Task checkout, not the repository root"
    );
    assert_eq!(sessions(&home), 0);
    // A closed arrival stays closed while the follower runs.
    call(
        "session.close_window",
        serde_json::json!({"session_id":workspace,"window_id":arrival["window_id"]}),
    );
    std::thread::sleep(Duration::from_secs(3));
    let after_close = call("session.view", serde_json::json!({"session_id":workspace}));
    assert_eq!(
        after_close["windows"].as_array().unwrap().len(),
        before_arrival["windows"].as_array().unwrap().len(),
        "{after_close}"
    );
    for session in call("session.list", serde_json::json!({}))["sessions"]
        .as_array()
        .unwrap()
    {
        call(
            "session.destroy",
            serde_json::json!({"session_id":session["session_id"]}),
        );
    }
    assert!(call("session.list", serde_json::json!({}))["sessions"]
        .as_array()
        .is_none_or(|sessions| sessions.is_empty()));
}

#[test]
#[ignore = "requires the installed Herdr CLI; creates and destroys an isolated local server"]
fn herdr_native_fresh_launches_leave_earlier_tabs_alone() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::{net::UnixStream, process::CommandExt};
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let root = tempfile::Builder::new()
        .prefix("h498-")
        .tempdir_in("/tmp")
        .unwrap();
    let home = root.path().join("lf");
    std::fs::create_dir(&home).unwrap();
    let repo = TestRepo::new();
    let _env = EnvGuard::with_lf_home(&[], &home);
    register_task_without_pr(&home, repo.path(), "main", &repo.head_sha());
    configure_native_tools(repo.path());
    let herdr = std::env::var_os("LOOPFLOW_TEST_HERDR")
        .expect("set LOOPFLOW_TEST_HERDR to the installed Herdr executable");
    let config = root.path().join("c");
    let mut server = NativeServer(
        Command::new(herdr)
            .args(["--session", "probe", "server"])
            .env_clear()
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", &config)
            .env("XDG_STATE_HOME", root.path().join("s"))
            .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin")
            .env("TERM", "xterm-256color")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let socket = config.join("herdr/sessions/probe/herdr.sock");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket.exists() {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "isolated Herdr exited"
        );
        assert!(Instant::now() < deadline, "isolated Herdr did not listen");
        std::thread::sleep(Duration::from_millis(20));
    }
    let call = |method: &str, params: Value| -> Value {
        let mut peer = UnixStream::connect(&socket).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        writeln!(
            peer,
            "{}",
            serde_json::json!({"id":"test","method":method,"params":params})
        )
        .unwrap();
        let mut line = String::new();
        BufReader::new(peer).read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        assert!(result.get("error").is_none(), "{result}");
        result["result"].clone()
    };
    let args = ["open", "--task", "INF-123", "--host", "herdr", "--json"];
    let environment = [("HERDR_SOCKET_PATH", socket.to_str().unwrap())];
    let first = json(&lf_with(&home, repo.path(), None, &environment, &args));
    let tab = first["receipts"][0]["destination"]
        .as_str()
        .unwrap()
        .to_owned();
    let workspaces = call("workspace.list", serde_json::json!({}));
    let workspace = workspaces["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|workspace| {
            workspace["label"]
                .as_str()
                .is_some_and(|label| !label.is_empty())
        })
        .unwrap()["workspace_id"]
        .clone();
    // Focus moves to the newest launch; everything else about the tab stays.
    let tabs = || {
        let mut listed = call("tab.list", serde_json::json!({"workspace_id":workspace}));
        for tab in listed["tabs"].as_array_mut().unwrap() {
            tab.as_object_mut().unwrap().remove("focused");
        }
        listed
    };
    let before = tabs();
    assert!(before.to_string().contains(&tab), "{before}");
    let second = json(&lf_with(&home, repo.path(), None, &environment, &args));
    assert_ne!(second["receipts"][0]["destination"].as_str().unwrap(), tab);
    assert_eq!(before, tabs(), "the earlier Task tab is untouched");
    assert_eq!(sessions(&home), 0);
    for workspace in call("workspace.list", serde_json::json!({}))["workspaces"]
        .as_array()
        .unwrap()
    {
        call(
            "workspace.close",
            serde_json::json!({"workspace_id":workspace["workspace_id"]}),
        );
    }
}
