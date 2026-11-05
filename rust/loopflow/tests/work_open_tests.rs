use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;
use serde_json::Value;
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
fn rex_reopens_native_task_window_without_creating_a_session_or_replaying_tools() {
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
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture","label":"Renamed"}]}),
    );
    rex_reply(
        bin.path(),
        "session.view",
        serde_json::json!({"windows":[{"window_id":"window:fixture","label":"My tools"}]}),
    );
    let second = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(first["receipts"][0]["destination"], "window:fixture");
    assert_eq!(
        first["receipts"][0]["destination"],
        second["receipts"][0]["destination"]
    );
    let created = std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    assert_eq!(created.lines().count(), 1);
    assert!(!created.contains("resume"));
    assert!(created.contains("task") && created.contains("view"));
    assert_eq!(
        created.matches("Task & workflow").count(),
        1,
        "A Task without a conversation needs one workflow pane, beside its shell"
    );
    assert_eq!(sessions(home.path()), 0);
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

#[test]
fn rex_created_shell_reopens_on_its_server_without_placement_flags() {
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
    let command = request["layout"]["split"]["after"]["block"]["options"]["command"]
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
    let reopened = json(&launch.output().unwrap());
    assert_eq!(reopened["receipts"][0]["destination"], "window:fixture");
    assert_eq!(std::fs::read_to_string(launches).unwrap(), created);
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
fn rex_remote_arrival_is_passive_reused_and_preserves_conversation_input() {
    use std::os::unix::process::CommandExt;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    let provider = NativeServer(
        Command::new("/bin/sleep")
            .arg("3600")
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    let primary = live_conversation(
        home.path(),
        &provider.0,
        time::OffsetDateTime::now_utc().unix_timestamp(),
        repo.path(),
        "Unfinished input",
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
        "UPDATE session_activity SET pending_input=1 WHERE session_id=?1",
        [&primary],
    )
    .unwrap();
    let attachment = || {
        db.query_row(
        "SELECT p.id,p.attachment_token,p.attached_lf_process_id,a.pending_input FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id JOIN session_activity a ON a.session_id=s.id WHERE s.id=?1",
        [&primary], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?))
    ).unwrap()
    };
    let original = attachment();
    let args = [
        "open",
        "--wave",
        "task-pr-tests",
        "--host",
        "rex",
        "--local-workspace",
        "--arrive-task",
        registered.task.id.as_str(),
        "--json",
    ];
    let first = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(first["receipts"][0]["destination"], "window:fixture");
    let launches = bin.path().join("session.new_window.requests");
    let created = std::fs::read_to_string(&launches).unwrap();
    let request: Value = serde_json::from_str(created.trim()).unwrap();
    assert_eq!(request["focus"], false);
    assert_eq!(
        request["layout"]["block"]["options"]["cwd"],
        repo.path().to_str().unwrap()
    );
    let command = request["layout"]["block"]["options"]["command"]
        .as_array()
        .unwrap();
    assert!(command.windows(2).any(|pair| pair == ["task", "view"]));
    assert!(!command.iter().any(|arg| arg == "resume" || arg == "shell"));
    assert_eq!(
        created.lines().count(),
        1,
        "arrival creates no follower or ordinary tools"
    );
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture","label":"Renamed"}]}),
    );
    rex_reply(
        bin.path(),
        "session.view",
        serde_json::json!({"windows":[{"window_id":"window:fixture","label":"My work"}]}),
    );
    let second = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(
        second["receipts"][0]["destination"],
        first["receipts"][0]["destination"]
    );
    assert_eq!(std::fs::read_to_string(&launches).unwrap(), created);
    assert!(!bin.path().join("session.focus_window.requests").exists());
    assert!(!bin.path().join("session.new_split.requests").exists());
    assert_eq!(attachment(), original);
    assert_eq!(sessions(home.path()), 1);

    // Reconciliation respects explicit native closure, even after its caller restarts.
    rex_reply(
        bin.path(),
        "session.view",
        serde_json::json!({"windows":[]}),
    );
    let closed = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(closed["receipts"][0]["status"], "failed");
    assert!(closed["receipts"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("explicitly closed"));
    assert_eq!(std::fs::read_to_string(&launches).unwrap(), created);
    assert_eq!(attachment(), original);
}

#[test]
fn rex_remote_arrival_rejects_wrong_machine() {
    use loopflow::durable::MachineId;
    use loopflow::store::sqlite::SqliteStore;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let peer = MachineId::new();
    store
        .add_machine(&peer, "remote.invalid", "Remote")
        .unwrap();
    store.claim_task(&registered.task.id).unwrap();
    store.handoff_task(&registered.task.id, &peer).unwrap();
    assert_eq!(
        store
            .task_execution_route(&registered.task.id)
            .unwrap()
            .machine_id,
        peer
    );
    let args = [
        "open",
        "--wave",
        "task-pr-tests",
        "--host",
        "rex",
        "--local-workspace",
        "--arrive-task",
        registered.task.id.as_str(),
        "--json",
    ];
    let wrong = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(wrong["receipts"][0]["status"], "failed");
    assert!(wrong["receipts"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("another Machine"));
    assert!(!bin.path().join("session.create.requests").exists());
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_remote_arrival_requires_a_started_task_and_an_available_checkout() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    rex_peer(bin.path());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let args = [
        "open",
        "--wave",
        "task-pr-tests",
        "--host",
        "rex",
        "--local-workspace",
        "--arrive-task",
        registered.task.id.as_str(),
        "--json",
    ];
    db.execute(
        "UPDATE tasks SET worktree=NULL WHERE id=?1",
        [registered.task.id.as_str()],
    )
    .unwrap();
    let backlog = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert!(backlog["receipts"].as_array().unwrap().is_empty());
    assert!(!bin.path().join("session.create.requests").exists());
    db.execute(
        "UPDATE tasks SET worktree=?2 WHERE id=?1",
        rusqlite::params![
            registered.task.id.as_str(),
            repo.path().join("missing").to_str().unwrap()
        ],
    )
    .unwrap();
    let missing = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
    assert_eq!(missing["receipts"][0]["status"], "failed");
    assert!(missing["receipts"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("checkout is unavailable"));
    assert!(!bin.path().join("session.new_window.requests").exists());
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_remote_follower_routes_locally_unstarted_tasks_without_fallback() {
    use loopflow::durable::MachineId;
    use loopflow::store::sqlite::SqliteStore;
    use loopflow::work::selection::{Selection, SelectionExpression, SelectionId};

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
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    store.claim_task(&registered.task.id).unwrap();
    store.handoff_task(&registered.task.id, &peer).unwrap();
    assert_eq!(
        store
            .task_execution_route(&registered.task.id)
            .unwrap()
            .machine_id,
        peer
    );
    // Remote checkout state is not copied into the local plan.
    db.execute(
        "UPDATE tasks SET worktree=NULL WHERE id=?1",
        [registered.task.id.as_str()],
    )
    .unwrap();
    let selected = Selection {
        id: SelectionId("workspace".into()),
        expression: SelectionExpression::Wave {
            wave: registered.task.wave_id.clone().unwrap(),
        },
    };
    db.execute(
        "INSERT INTO host_workspaces(id,repository_id,host,server,selection_json,copy_name,container_id,creation_pending,updated_at)
         VALUES('follow-remote',?1,'rex','unix:///fixture/rex.sock',?2,'review','session:fixture',0,1)",
        rusqlite::params![registered.task.repository_id.as_str(), serde_json::to_string(&selected).unwrap()],
    ).unwrap();
    rex_peer(bin.path());
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture"}]}),
    );
    let requests = host(bin.path(), "ssh", "", 1);
    let args = [
        "open",
        "--host",
        "rex",
        "--watch-workspace",
        "follow-remote",
    ];
    let output = lf(home.path(), repo.path(), Some(bin.path()), &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sent = std::fs::read_to_string(&requests).unwrap();
    assert!(
        sent.contains("remote.invalid") && sent.contains(peer.as_str()),
        "{sent}"
    );
    assert!(
        sent.contains("--arrive-task") && sent.contains(registered.task.id.as_str()),
        "{sent}"
    );
    assert!(
        sent.contains("--local-workspace") && sent.contains("--copy") && sent.contains("review")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("no local fallback"));
    assert!(!bin.path().join("session.new_window.requests").exists());
    assert_eq!(sessions(home.path()), 0);

    // A destination follower never forwards the request back to another Machine.
    std::fs::remove_file(&requests).unwrap();
    let mut local = args.to_vec();
    local.push("--local-workspace");
    assert!(lf(home.path(), repo.path(), Some(bin.path()), &local)
        .status
        .success());
    assert!(!requests.exists());
}

#[test]
fn rex_remote_follower_reconnects_without_work_changes_and_reuses_a_lost_reply() {
    use std::io::{Read, Seek, Write};
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    use loopflow::durable::MachineId;
    use loopflow::store::sqlite::SqliteStore;
    use loopflow::work::selection::{Selection, SelectionExpression, SelectionId};

    let repo = TestRepo::new();
    let checkout = repo.create_named_worktree("remote-task");
    let home = tempfile::tempdir().unwrap();
    let remote = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let remote_bin = remote.path().join(".local/bin");
    std::fs::create_dir_all(&remote_bin).unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let peer = MachineId::new();
    store
        .add_machine(&peer, "remote.invalid", "Remote")
        .unwrap();
    store.claim_task(&registered.task.id).unwrap();
    store.handoff_task(&registered.task.id, &peer).unwrap();
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    // The two Machines share planning identities, but only the destination has
    // the checkout. No planning import or local Work mutation follows startup.
    db.execute(
        "VACUUM INTO ?1",
        [remote.path().join("loopflow.db").to_str().unwrap()],
    )
    .unwrap();
    let remote_db = rusqlite::Connection::open(remote.path().join("loopflow.db")).unwrap();
    remote_db
        .execute("UPDATE machines SET is_local=0 WHERE is_local=1", [])
        .unwrap();
    remote_db
        .execute(
            "UPDATE machines SET is_local=1 WHERE id=?1",
            [peer.as_str()],
        )
        .unwrap();
    remote_db
        .execute(
            "UPDATE tasks SET worktree=?1,checkout_machine_id=?2 WHERE id=?3",
            rusqlite::params![
                checkout.to_str().unwrap(),
                peer.as_str(),
                registered.task.id.as_str()
            ],
        )
        .unwrap();
    db.execute(
        "UPDATE tasks SET worktree=NULL,checkout_machine_id=NULL WHERE id=?1",
        [registered.task.id.as_str()],
    )
    .unwrap();
    let selected = Selection {
        id: SelectionId("workspace".into()),
        expression: SelectionExpression::Wave {
            wave: registered.task.wave_id.clone().unwrap(),
        },
    };
    db.execute(
        "INSERT INTO host_workspaces(id,repository_id,host,server,selection_json,copy_name,container_id,creation_pending,updated_at)
         VALUES('follow-roundtrip',?1,'rex','unix:///fixture/rex.sock',?2,'review','session:fixture',0,1)",
        rusqlite::params![registered.task.repository_id.as_str(), serde_json::to_string(&selected).unwrap()],
    ).unwrap();
    rex_peer(bin.path());
    rex_peer(&remote_bin);
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture"}]}),
    );
    // Pin the actual receiving CLI inside SSH's remote PATH boundary.
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), remote_bin.join("lf")).unwrap();
    let script = format!(
        r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
printf '%s\n' attempt >> '{bin}/attempts'
if [ ! -f '{bin}/connected' ]; then touch '{bin}/disconnected'; echo disconnected >&2; exit 255; fi
cd '{remote}' || exit 1
env -i HOME='{remote}' LF_HOME='{remote}' LF_BIN='{lf}' PATH='{remote_bin}:/usr/bin:/bin' /bin/bash -c "$remote_command" > '{bin}/receipt' || exit $?
# The native peer retains its view even if SSH loses the successful receipt.
printf '%s' '{{"sessions":[{{"session_id":"session:fixture"}}]}}' > '{remote_bin}/session.list.reply'
printf '%s' '{{"windows":[{{"window_id":"window:fixture","label":"Renamed task"}}]}}' > '{remote_bin}/session.view.reply'
if [ ! -f '{bin}/lost-reply' ]; then touch '{bin}/lost-reply'; echo lost-reply >&2; exit 255; fi
cat '{bin}/receipt'
touch '{bin}/delivered'
"#,
        bin = bin.path().display(),
        remote = remote.path().display(),
        remote_bin = remote_bin.display(),
        lf = env!("CARGO_BIN_EXE_lf")
    );
    std::fs::write(bin.path().join("ssh"), script).unwrap();
    std::fs::set_permissions(
        bin.path().join("ssh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: valid output pointers; null selects default terminal settings.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: openpty returned two fresh independently owned descriptors.
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    // SAFETY: slave is the other fresh descriptor returned by openpty.
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    let stdout = tempfile::tempfile().unwrap();
    let stderr = tempfile::tempfile().unwrap();
    let mut follower = NativeServer(
        lf_command(
            home.path(),
            repo.path(),
            Some(bin.path()),
            &[],
            &[
                "open",
                "--host",
                "rex",
                "--watch-workspace",
                "follow-roundtrip",
            ],
        )
        .stdin(slave)
        .stdout(stdout.try_clone().unwrap())
        .stderr(stderr.try_clone().unwrap())
        .process_group(0)
        .spawn()
        .unwrap(),
    );
    let diagnostics = || {
        let mut log = stderr.try_clone().unwrap();
        log.rewind().unwrap();
        let mut text = String::new();
        log.read_to_string(&mut text).unwrap();
        text
    };
    let deadline = Instant::now() + Duration::from_secs(55);
    while !bin.path().join("delivered").exists() {
        if bin.path().join("disconnected").exists() {
            std::fs::write(bin.path().join("connected"), "").unwrap();
        }
        assert!(
            follower.0.try_wait().unwrap().is_none(),
            "follower exited before delivery: {}",
            diagnostics()
        );
        assert!(
            Instant::now() < deadline,
            "remote reconciliation did not recover: {}",
            diagnostics()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    master.write_all(b"q\n").unwrap();
    while follower.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "follower did not stop");
        std::thread::sleep(Duration::from_millis(20));
    }
    let receipt: Value =
        serde_json::from_str(&std::fs::read_to_string(bin.path().join("receipt")).unwrap())
            .unwrap();
    assert_eq!(receipt["receipts"][0]["destination"], "window:fixture");
    assert!(diagnostics().contains("disconnected"));
    assert!(diagnostics().contains("lost-reply"));
    let launches = std::fs::read_to_string(remote_bin.join("session.new_window.requests")).unwrap();
    assert_eq!(
        launches.lines().count(),
        1,
        "lost replies must reuse the destination"
    );
    let request: Value = serde_json::from_str(launches.trim()).unwrap();
    assert_eq!(request["focus"], false);
    assert_eq!(
        request["layout"]["block"]["options"]["cwd"],
        checkout.to_str().unwrap()
    );
    let command = request["layout"]["block"]["options"]["command"]
        .as_array()
        .unwrap();
    assert!(command.windows(2).any(|pair| pair == ["task", "view"]));
    assert!(!command.iter().any(|arg| arg == "resume" || arg == "shell"));
    assert!(!bin.path().join("session.new_window.requests").exists());
    assert!(!remote_bin.join("session.new_split.requests").exists());
    assert!(!remote_bin.join("session.focus_window.requests").exists());
    assert_eq!(sessions(home.path()), 0);
    assert_eq!(sessions(remote.path()), 0);
    assert_eq!(
        store.task(&registered.task.id).unwrap().unwrap().worktree,
        None
    );
}

#[test]
fn rex_open_keeps_other_tasks_when_a_wave_cannot_be_prepared() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let checkout = repo.create_named_worktree("standalone");
    let standalone =
        support::register_sibling_task(&registered, "INF-124", "standalone", &checkout);
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET project_id=NULL WHERE id=?1",
        [standalone.id.as_str()],
    )
    .unwrap();
    rex_peer(bin.path());
    let peer = bin.path().join("rex-peer");
    std::fs::rename(bin.path().join("rex"), &peer).unwrap();
    std::fs::write(
        bin.path().join("rex"),
        format!(
            "#!/bin/sh\ncase \"$*\" in *session.create*task-pr-tests*) echo 'lost reply'; exit;; esac\nexec '{}' \"$@\"\n",
            peer.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(
        bin.path().join("rex"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let output = lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            "--repo",
            repo.path().to_str().unwrap(),
            "open",
            "--host",
            "rex",
            "--no-focus",
            "--json",
        ],
    );
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let receipts = report["receipts"].as_array().unwrap();
    assert_eq!(receipts.len(), 2, "{report}");
    let failed = receipts
        .iter()
        .find(|item| item["status"] == "failed")
        .unwrap();
    assert!(failed["url"].as_str().unwrap().ends_with("/task/INF-123"));
    assert!(failed["reason"]
        .as_str()
        .unwrap()
        .contains("outcome is unknown"));
    let opened = receipts
        .iter()
        .find(|item| item["status"] == "opening")
        .unwrap();
    assert!(opened["url"].as_str().unwrap().ends_with("/task/INF-124"));
    assert_eq!(opened["destination"], "window:fixture");
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_follower_keeps_other_arrivals_when_a_wave_cannot_be_prepared() {
    use loopflow::work::selection::{Selection, SelectionExpression, SelectionId};

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let registered = register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let checkout = repo.create_named_worktree("standalone");
    let standalone =
        support::register_sibling_task(&registered, "INF-124", "standalone", &checkout);
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE tasks SET project_id=NULL WHERE id=?1",
        [standalone.id.as_str()],
    )
    .unwrap();
    let selected = Selection {
        id: SelectionId("workspace".into()),
        expression: SelectionExpression::Repository,
    };
    db.execute(
        "INSERT INTO host_workspaces(id,repository_id,host,server,selection_json,copy_name,container_id,creation_pending,updated_at)
         VALUES('follow-partial',?1,'rex','unix:///fixture/rex.sock',?2,NULL,'session:fixture',0,1)",
        rusqlite::params![registered.task.repository_id.as_str(), serde_json::to_string(&selected).unwrap()],
    ).unwrap();
    rex_peer(bin.path());
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture"}]}),
    );
    std::fs::write(bin.path().join("session.create.reply"), "lost reply").unwrap();
    let output = lf(
        home.path(),
        repo.path(),
        Some(bin.path()),
        &[
            "open",
            "--host",
            "rex",
            "--watch-workspace",
            "follow-partial",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome is unknown"));
    let created = std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    let request: Value = serde_json::from_str(created.trim()).unwrap();
    assert_eq!(request["session_id"], "session:fixture");
    assert_eq!(
        request["layout"]["block"]["options"]["cwd"],
        checkout.to_str().unwrap()
    );
    assert!(created.contains(standalone.id.as_str()));
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_lost_recreation_reply_never_reuses_a_deleted_locator_as_retry_permission() {
    // Both the container and the Task tab can disappear between explicit opens.
    for method in ["session.create", "session.new_window"] {
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
        json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
        if method == "session.new_window" {
            rex_reply(
                bin.path(),
                "session.list",
                serde_json::json!({"sessions":[{"session_id":"session:fixture","label":"Renamed"}]}),
            );
        }
        // Inventory confirms the old destination is gone; its replacement succeeds
        // natively but the server's reply cannot be read.
        std::fs::write(bin.path().join(format!("{method}.reply")), "lost reply").unwrap();
        let failed = lf(home.path(), repo.path(), Some(bin.path()), &args);
        assert!(!failed.status.success());
        let retry = lf(home.path(), repo.path(), Some(bin.path()), &args);
        assert!(!retry.status.success());
        let report: Value = serde_json::from_slice(&retry.stdout).unwrap();
        assert!(report["receipts"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("unknown"));
        let requests =
            std::fs::read_to_string(bin.path().join(format!("{method}.requests"))).unwrap();
        assert_eq!(
            requests.lines().count(),
            2,
            "{method}: a lost reply must not create a third destination"
        );
        assert_eq!(sessions(home.path()), 0);
    }
}

#[test]
fn rex_recovers_a_lost_creation_by_marker_then_reuses_its_renamed_native_id() {
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
        "--copy",
        "review",
        "--no-focus",
        "--json",
    ];
    std::fs::write(bin.path().join("session.new_window.reply"), "lost reply").unwrap();
    assert!(!lf(home.path(), repo.path(), Some(bin.path()), &args)
        .status
        .success());
    rex_reply(
        bin.path(),
        "session.list",
        serde_json::json!({"sessions":[{"session_id":"session:fixture","label":"Renamed session"}]}),
    );
    let created = std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap();
    let request: Value = serde_json::from_str(created.trim()).unwrap();
    for label in [request["window_label"].as_str().unwrap(), "My arrangement"] {
        rex_reply(
            bin.path(),
            "session.view",
            serde_json::json!({"windows":[{"window_id":"window:recovered","label":label}]}),
        );
        let report = json(&lf(home.path(), repo.path(), Some(bin.path()), &args));
        assert_eq!(report["receipts"][0]["destination"], "window:recovered");
    }
    assert_eq!(
        std::fs::read_to_string(bin.path().join("session.new_window.requests")).unwrap(),
        created
    );
    assert_eq!(sessions(home.path()), 0);
}

#[test]
fn rex_activates_a_named_passive_copy_once_even_when_the_split_reply_is_lost() {
    for reply in ["{}", "lost reply"] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let _env = EnvGuard::with_lf_home(&[], home.path());
        let registered =
            register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
        rex_peer(bin.path());
        let args = [
            "open",
            "--task",
            "INF-123",
            "--host",
            "rex",
            "--copy",
            "review",
            "--new-shells",
            "--no-focus",
            "--json",
        ];
        let mut arrival = args.to_vec();
        arrival.extend([
            "--local-workspace",
            "--arrive-task",
            registered.task.id.as_str(),
        ]);
        let passive = json(&lf(home.path(), repo.path(), Some(bin.path()), &arrival));
        assert_eq!(passive["receipts"][0]["destination"], "window:fixture");
        let windows = bin.path().join("session.new_window.requests");
        let created = std::fs::read_to_string(&windows).unwrap();
        rex_reply(
            bin.path(),
            "session.list",
            serde_json::json!({
                "sessions":[{"session_id":"session:fixture","label":"Renamed copy"}]
            }),
        );
        rex_reply(
            bin.path(),
            "session.view",
            serde_json::json!({
                "windows":[{"window_id":"window:fixture","label":"Native edits"}]
            }),
        );
        std::fs::write(bin.path().join("session.new_split.reply"), reply).unwrap();
        let activated = lf(home.path(), repo.path(), Some(bin.path()), &args);
        let repeated = lf(home.path(), repo.path(), Some(bin.path()), &args);
        let report: Value = serde_json::from_slice(&repeated.stdout).unwrap();
        assert_eq!(activated.status.success(), reply == "{}");
        assert_eq!(repeated.status.success(), reply == "{}");
        if reply == "{}" {
            assert_eq!(report["receipts"][0]["destination"], "window:fixture");
            assert!(report["receipts"][0]["reason"]
                .as_str()
                .unwrap()
                .contains("0 new native shells"));
        } else {
            assert!(report["receipts"][0]["reason"]
                .as_str()
                .unwrap()
                .contains("Tool activation outcome is unknown"));
        }
        let splits =
            std::fs::read_to_string(bin.path().join("session.new_split.requests")).unwrap();
        assert_eq!(
            splits.lines().count(),
            1,
            "activation must not replay tools"
        );
        let split: Value = serde_json::from_str(splits.trim()).unwrap();
        assert_eq!(split["window_id"], "window:fixture");
        assert_eq!(split["focus"], false);
        assert_eq!(std::fs::read_to_string(windows).unwrap(), created);
        assert_eq!(sessions(home.path()), 0);
    }
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

/// Installed-host acceptance stays opt-in; CI and ordinary tests use bounded peers.
#[test]
#[ignore = "requires the installed Rex CLI; creates and destroys an isolated local server"]
fn rex_native_workspace_keeps_window_and_block_identity() {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
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
    let config = root.path().join("empty.lua");
    std::fs::write(&config, "").unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    host(
        &bin,
        "open",
        "Desktop is excluded from this isolated Rex test",
        1,
    );
    let socket = root.path().join("rex.sock");
    let endpoint = format!("unix://{}", socket.display());
    let rex = std::env::var_os("LOOPFLOW_TEST_REX")
        .expect("set LOOPFLOW_TEST_REX to the installed Rex executable");
    // Native children have no REX_SERVER. The caller must discover this server,
    // not Rex's configured default. Refuse any unqualified CLI query so this
    // fixture can never contact the person's server.
    let rex_cli = bin.join("rex");
    std::fs::write(
        &rex_cli,
        format!(
            "#!/bin/sh\ncase \" $* \" in *' -S '*|*' --server '*) ;; *) echo 'missing explicit Rex endpoint' >&2; exit 91;; esac\nexec '{}' \"$@\"\n",
            rex.to_string_lossy().replace('\'', "'\"'\"'"),
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&rex_cli, std::fs::Permissions::from_mode(0o755)).unwrap();
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
    let child = Command::new(&rex)
        .args(["server", "run", "--listen", &endpoint, "--state-dir"])
        .arg(root.path().join("state"))
        .arg("--config")
        .arg(&config)
        .env_clear()
        .env("HOME", root.path())
        .env("LF_BIN", &viewer)
        .env(
            "PATH",
            format!(
                "{}:{}:/opt/homebrew/bin:/usr/bin:/bin",
                bin.display(),
                Path::new(&rex).parent().unwrap().display()
            ),
        )
        .env("TERM", "xterm-256color")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = NativeServer(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket.exists() {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "isolated Rex exited"
        );
        assert!(Instant::now() < deadline, "isolated Rex did not listen");
        std::thread::sleep(Duration::from_millis(20));
    }
    let args = [
        "open", "--task", "INF-123", "--host", "rex", "--server", &endpoint, "--json",
    ];
    let first = json(&lf(&home, repo.path(), None, &args));
    let window = first["receipts"][0]["destination"].as_str().unwrap();
    let call = |method: &str, args: Value| -> Value {
        let output = Command::new(&rex)
            .args([
                "--autostart=false",
                "-S",
                &endpoint,
                "api",
                "call",
                method,
                &args.to_string(),
            ])
            .output()
            .unwrap();
        json(&output)
    };
    let inventory = call("session.list", serde_json::json!({}));
    let container = inventory["sessions"][0]["session_id"].as_str().unwrap();
    call(
        "session.set_window_label",
        serde_json::json!({"session_id":container,"window_id":window,"label":"My retained arrangement"}),
    );
    // Rex's wrapper hides a tool's cwd, so its report is all a split can inherit;
    // a shell has no report to outlive `cd`, and the host reads its process.
    let block_method = |block: &str, method: &str| -> Value {
        json(
            &Command::new(&rex)
                .args(["--autostart=false", "-S", &endpoint, "block", "call"])
                .args(["-s", container, "-b", block, method])
                .output()
                .unwrap(),
        )
    };
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
            let reported = block_method(id, "pwd")["pwd"].as_str().map(str::to_owned);
            let process = block_method(id, "process");
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
    assert_eq!(second["receipts"][0]["destination"], window);
    let after = call("session.view", serde_json::json!({"session_id":container}));
    assert_eq!(
        before, after,
        "reopen must preserve the host's native arrangement and blocks"
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
    assert_eq!(
        automatic["receipts"][0]["destination"], window,
        "{automatic}"
    );
    assert_eq!(automatic["receipts"][0]["host"], "rex");
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
    let wave: String = rusqlite::Connection::open(home.join("loopflow.db")).unwrap()
        .query_row("SELECT container_id FROM host_workspaces WHERE json_extract(selection_json,'$.expression.wave')=?1",
            [registered.task.wave_id.as_ref().unwrap().as_str()], |row| row.get(0))
        .unwrap_or_else(|error| panic!("Wave workspace was not recorded: {error}; native inventory: {inventory}"));
    let before_arrival = call("session.view", serde_json::json!({"session_id":wave}));
    let sibling = repo.create_named_worktree("arriving-task");
    let sibling_task =
        support::register_sibling_task(&registered, "INF-124", "arriving-task", &sibling);
    let deadline = Instant::now() + Duration::from_secs(15);
    let arrived = loop {
        let current = call("session.view", serde_json::json!({"session_id":wave}));
        if current["windows"].as_array().unwrap().len()
            > before_arrival["windows"].as_array().unwrap().len()
        {
            break current;
        }
        assert!(
            Instant::now() < deadline,
            "Started Task did not arrive in the native Wave workspace: {current}"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(
        before_arrival["active_window_id"],
        arrived["active_window_id"]
    );
    let passive = arrived["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| {
            window["label"]
                .as_str()
                .is_some_and(|label| label.contains("INF-124"))
        })
        .unwrap();
    let block_count: usize = passive["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|layer| layer["blocks"].as_array().unwrap().len())
        .sum();
    assert_eq!(
        block_count, 1,
        "Background arrival creates only the passive Task view"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let passive_cwd = loop {
        if let Ok(cwd) =
            std::fs::read_to_string(root.path().join(format!("cwd-{}", sibling_task.id)))
        {
            break cwd;
        }
        assert!(
            Instant::now() < deadline,
            "Passive Task viewer did not record its directory; screen: {}",
            call(
                "com.superlogical.terminal.format",
                serde_json::json!({"session_id":wave,"block_id":passive["layers"][0]["blocks"][0]["block_id"],"args":{"format":"text"}}),
            )
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(
        std::fs::canonicalize(passive_cwd.trim()).unwrap(),
        std::fs::canonicalize(&sibling).unwrap(),
        "The passive Task view must execute in its Task checkout, not the repository root"
    );
    assert_eq!(sessions(&home), 0);
    let repeated = json(&lf(
        &home,
        repo.path(),
        None,
        &[
            "open",
            "--task",
            "INF-124",
            "--host",
            "rex",
            "--server",
            &endpoint,
            "--no-focus",
            "--json",
        ],
    ));
    assert_eq!(repeated["receipts"][0]["destination"], passive["window_id"]);
    let activated = call("session.view", serde_json::json!({"session_id":wave}));
    let task_window = |view: &Value| {
        view["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|window| window["window_id"] == passive["window_id"])
            .unwrap()
            .clone()
    };
    let activated_window = task_window(&activated);
    assert!(
        activated_window["layers"][0]["blocks"]
            .as_array()
            .unwrap()
            .len()
            > block_count
    );
    let passive_id = &passive["layers"][0]["blocks"][0]["block_id"];
    assert!(activated_window["layers"][0]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|block| &block["block_id"] == passive_id));
    json(&lf(
        &home,
        repo.path(),
        None,
        &[
            "open",
            "--task",
            "INF-124",
            "--host",
            "rex",
            "--server",
            &endpoint,
            "--no-focus",
            "--json",
        ],
    ));
    let reopened = call("session.view", serde_json::json!({"session_id":wave}));
    assert_eq!(
        task_window(&reopened),
        activated_window,
        "Reopening must not add or replace tools"
    );
    assert_eq!(reopened["active_window_id"], arrived["active_window_id"]);
    assert_eq!(sessions(&home), 0);

    let follower_window = reopened["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["label"] == "Workspace")
        .unwrap()["window_id"]
        .clone();
    call(
        "session.close_window",
        serde_json::json!({"session_id":wave,"window_id":follower_window}),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
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
        let recovered = call("session.view", serde_json::json!({"session_id":wave}));
        if recovered["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| window["label"] == "Workspace")
        {
            assert_eq!(task_window(&recovered), activated_window);
            break;
        }
        assert!(Instant::now() < deadline, "Follower did not recover");
        std::thread::sleep(Duration::from_millis(50));
    }
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
fn herdr_native_workspace_reuses_its_native_tools() {
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
    let before = call("workspace.list", serde_json::json!({}));
    let second = json(&lf_with(&home, repo.path(), None, &environment, &args));
    assert_eq!(
        first["receipts"][0]["destination"],
        second["receipts"][0]["destination"]
    );
    assert_eq!(before, call("workspace.list", serde_json::json!({})));
    assert_eq!(sessions(&home), 0);
    call(
        "workspace.close",
        serde_json::json!({"workspace_id":first["receipts"][0]["destination"]}),
    );
}
