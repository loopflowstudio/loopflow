#![cfg(target_os = "macos")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use rusqlite::Connection;
use serde_json::Value;

fn cli(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_HOME", cwd)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn clone_command(parent: &Path, clone: &Path, args: &[&str]) -> Output {
    let mut command = vec!["clone", "run", clone.to_str().unwrap(), "--"];
    command.extend(args);
    cli(parent, &command)
}

#[test]
fn clone_migrates_only_its_snapshot_and_confines_children() {
    let parent = tempfile::tempdir().unwrap();
    let parent = parent.path().canonicalize().unwrap();
    let source = parent.join("source");
    let clone = parent.join("clone");
    let codex = parent.join("codex");
    let claude = parent.join("claude");
    fs::create_dir(&source).unwrap();
    let conn = Connection::open(source.join("loopflow.db")).unwrap();
    loopflow::store::migrations::apply_sqlite(&conn).unwrap();
    conn.execute_batch("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,provider)
        VALUES('copied-codex','Copied Codex','generated',1,1,'/missing/worktree','codex'),
              ('copied-claude','Copied Claude','generated',1,1,'/missing/worktree','claude');
        INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
            SELECT id,'captured','run_' || lower(hex(randomblob(16))),1,'{}' FROM agent_sessions;
        UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events WHERE session_id=agent_sessions.id);
        INSERT INTO processes(id,trace_id,command,cwd,started_at,pid,os_started_at,kind,agent_session_id,
            endpoint,attached_lf_process_id,attachment_token)
        VALUES('copied-process','trace','codex','/missing/worktree',1,123,456,'agent','copied-codex',
            '/tmp/original.sock','copied-client','copied-token');
        UPDATE agent_sessions SET agent_process_id='copied-process' WHERE id='copied-codex';
    ").unwrap();
    conn.execute(
        "INSERT INTO provider_tokens(provider,access_token,updated_at,credential_type,encrypted)
        VALUES('codex','synthetic-token-must-not-survive',1,'oauth',0)",
        [],
    )
    .unwrap();
    // A live WAL source exercises the backup, not an immutable read that loses WAL rows.
    conn.execute_batch("PRAGMA journal_mode=WAL; UPDATE agent_sessions SET title='WAL title' WHERE id='copied-codex';").unwrap();
    for (root, relative) in [
        (&codex, "sessions/2026/history.jsonl"),
        (&claude, "projects/repo/history.jsonl"),
    ] {
        fs::create_dir_all(root.join(relative).parent().unwrap()).unwrap();
        fs::write(root.join(relative), "{\"message\":\"retained\"}\n").unwrap();
        fs::write(root.join("auth.json"), "synthetic credential sentinel").unwrap();
    }
    fs::create_dir_all(source.join("runs/ab/run_abcdef")).unwrap();
    fs::write(
        source.join("runs/ab/run_abcdef/context.json"),
        "retained capture",
    )
    .unwrap();
    std::os::unix::fs::symlink(codex.join("auth.json"), codex.join("sessions/secret.jsonl"))
        .unwrap();
    let before = fs::read(source.join("loopflow.db")).unwrap();
    let wal = fs::read(source.join("loopflow.db-wal")).unwrap();
    let shm = fs::read(source.join("loopflow.db-shm")).unwrap();
    let manifest: Value = serde_json::from_str(&success(cli(
        &source,
        &[
            "clone",
            "create",
            clone.to_str().unwrap(),
            "--source",
            source.to_str().unwrap(),
            "--codex-home",
            codex.to_str().unwrap(),
            "--claude-home",
            claude.to_str().unwrap(),
        ],
    )))
    .unwrap();
    assert_eq!(manifest["ready"], true);
    assert_eq!(manifest["artifacts"]["codex"], 1);
    assert_eq!(manifest["artifacts"]["claude"], 1);
    assert!(!clone.join("archive/codex/auth.json").exists());
    assert!(!clone.join("archive/codex/sessions/secret.jsonl").exists());
    assert!(!fs::read(clone.join("loopflow.db"))
        .unwrap()
        .windows(b"synthetic-token-must-not-survive".len())
        .any(|bytes| bytes == b"synthetic-token-must-not-survive"));
    let copied = Connection::open(clone.join("loopflow.db")).unwrap();
    assert_eq!(
        copied
            .query_row(
                "SELECT title FROM agent_sessions WHERE id='copied-codex'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "WAL title"
    );
    assert_eq!(copied.query_row("SELECT count(*) FROM processes WHERE pid IS NOT NULL OR endpoint IS NOT NULL OR attachment_token IS NOT NULL", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    let preserved_shm = fs::read(source.join("loopflow.db-shm")).unwrap();
    assert!(
        preserved_shm == shm,
        "clone capture changed source shared memory"
    );
    assert_ne!(
        copied
            .query_row("SELECT id FROM machines WHERE route='local'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        conn.query_row("SELECT id FROM machines WHERE route='local'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap()
    );
    loopflow::store::sqlite::SqliteStore::new(&clone.join("loopflow.db")).unwrap();
    assert_eq!(
        copied
            .query_row("SELECT count(*) FROM schema_migrations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    let lf = clone.join("bin/lf");
    for provider in ["copied-codex", "copied-claude"] {
        for args in [
            vec!["session", "connect", provider],
            vec!["-b", "session", "connect", provider, "--message", "again"],
        ] {
            let mut command = vec![lf.to_str().unwrap()];
            command.extend(args);
            let output = clone_command(&parent, &clone, &command);
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("read-only"),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    let sentinel = parent.join("source-file");
    fs::write(&sentinel, "unchanged").unwrap();
    let output = clone_command(
        &parent,
        &clone,
        &[
            "/bin/sh",
            "-c",
            "printf changed > \"$1\"",
            "sh",
            sentinel.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "unchanged");
    success(clone_command(&parent, &clone, &["/bin/sh", "-c", "printf private > \"$LF_HOME/created\"; test \"$HOME\" = \"$LF_HOME/home\"; test \"$CODEX_HOME\" = \"$HOME/.codex\"; test \"$CLAUDE_CONFIG_DIR\" = \"$HOME/.claude\""]));
    assert_eq!(
        fs::read_to_string(clone.join("created")).unwrap(),
        "private"
    );
    let mut client = Command::new("/bin/sleep").arg("60").spawn().unwrap();
    let signal = clone_command(
        &parent,
        &clone,
        &["/bin/kill", "-TERM", &client.id().to_string()],
    );
    assert!(!signal.status.success());
    assert!(client.try_wait().unwrap().is_none());
    client.kill().unwrap();
    client.wait().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    assert!(!clone_command(
        &parent,
        &clone,
        &["/usr/bin/curl", "--max-time", "1", &address]
    )
    .status
    .success());
    let socket = parent.join("source.sock");
    let socket_listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    socket_listener.set_nonblocking(true).unwrap();
    assert!(!clone_command(
        &parent,
        &clone,
        &["/usr/bin/nc", "-U", socket.to_str().unwrap()]
    )
    .status
    .success());
    assert_eq!(
        socket_listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    std::os::unix::fs::symlink(&sentinel, clone.join("source-alias")).unwrap();
    assert!(!clone_command(
        &parent,
        &clone,
        &[
            "/bin/sh",
            "-c",
            "printf changed > \"$LF_HOME/source-alias\""
        ]
    )
    .status
    .success());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "unchanged");
    // Direct selection also enters isolation and pins nested lf to the clone.
    success(cli(&clone, &["session", "list", "--json"]));
    // Selecting the copied executable must not fall back to the source Machine.
    success(
        Command::new(&lf)
            .env_clear()
            .env("LF_HOME", &source)
            .current_dir(&source)
            .args(["session", "list", "--json"])
            .stdin(Stdio::null())
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read(source.join("loopflow.db")).unwrap(), before);
    assert_eq!(fs::read(source.join("loopflow.db-wal")).unwrap(), wal);

    assert_eq!(
        fs::read_to_string(codex.join("sessions/2026/history.jsonl")).unwrap(),
        "{\"message\":\"retained\"}\n"
    );
    assert_eq!(
        fs::read_to_string(claude.join("projects/repo/history.jsonl")).unwrap(),
        "{\"message\":\"retained\"}\n"
    );
    drop(copied);
    success(cli(&source, &["clone", "dispose", clone.to_str().unwrap()]));
    assert!(!clone.exists());
    assert_eq!(fs::read(source.join("loopflow.db")).unwrap(), before);
}

#[test]
fn clone_runs_a_new_disposable_conversation_without_account_selection() {
    let parent = tempfile::tempdir().unwrap();
    let parent = parent.path().canonicalize().unwrap();
    let source = parent.join("source");
    let clone = parent.join("clone");
    fs::create_dir(&source).unwrap();
    loopflow::store::migrations::apply_sqlite(
        &Connection::open(source.join("loopflow.db")).unwrap(),
    )
    .unwrap();
    success(cli(
        &parent,
        &[
            "clone",
            "create",
            clone.to_str().unwrap(),
            "--source",
            source.to_str().unwrap(),
        ],
    ));
    let lf = clone.join("bin/lf");
    for name in ["claude", "codex"] {
        let provider = clone.join("bin").join(name);
        let native = if name == "codex" {
            crate::support::install_native_codex(&clone.join("bin"));
            fs::read_to_string(&provider).unwrap()
        } else {
            String::new()
        };
        fs::write(
            &provider,
            format!(
                r#"#!/bin/sh
set -e
printf 'offline provider reached\n'
printf '%s\n' "$LF_HOME" > "$LF_HOME/provider-home"
{native}"#
            ),
        )
        .unwrap();
        fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
        let output = success(clone_command(
            &parent,
            &clone,
            &[
                lf.to_str().unwrap(),
                "-i",
                "-a",
                name,
                ":",
                "Disposable conversation",
            ],
        ));
        assert!(output.contains("offline provider reached"), "{output}");
        assert_eq!(
            fs::read_to_string(clone.join("provider-home"))
                .unwrap()
                .trim(),
            clone.to_str().unwrap()
        );
    }
    let sessions: Value = serde_json::from_str(&success(clone_command(
        &parent,
        &clone,
        &[
            lf.to_str().unwrap(),
            "desktop",
            "sessions",
            "--all",
            "--history",
            "--json",
        ],
    )))
    .unwrap();
    assert_eq!(sessions.as_array().unwrap().len(), 2);
    assert!(sessions[0]["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a.as_str() == lf.to_str()));
}

#[test]
fn populated_clone_retains_navigation_and_task_primary_without_source_authority() {
    let parent = tempfile::tempdir().unwrap();
    let parent = parent.path().canonicalize().unwrap();
    let source = parent.join("source");
    let clone = parent.join("clone");
    fs::create_dir(&source).unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let repo_path = repo.path().canonicalize().unwrap();
    let registered =
        crate::support::register_task_without_pr(&source, &repo_path, "main", &repo.head_sha());
    let database = Connection::open(source.join("loopflow.db")).unwrap();
    database.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,provider,task_id,wave_id)
        VALUES('copied-primary','Task conversation','generated',1,1,?1,'claude',?2,?3)",
        rusqlite::params![repo_path.to_str().unwrap(),registered.task.id.as_str(),registered.task.wave_id.as_ref().unwrap().as_str()]).unwrap();
    database
        .execute_batch(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
        VALUES('copied-primary','captured','run_0123456789abcdef0123456789abcdef',1,'{}');
        UPDATE agent_sessions SET current_capture=last_insert_rowid() WHERE id='copied-primary';
        UPDATE tasks SET primary_session_id='copied-primary';",
        )
        .unwrap();
    let head = repo.head_sha();
    let before = fs::read(source.join("loopflow.db")).unwrap();
    let wal = fs::read(source.join("loopflow.db-wal")).unwrap();
    success(cli(
        &parent,
        &[
            "clone",
            "create",
            clone.to_str().unwrap(),
            "--source",
            source.to_str().unwrap(),
        ],
    ));
    let lf = clone.join("bin/lf");
    let run = |args: &[&str]| {
        let mut command = vec![lf.to_str().unwrap(), "--repo", repo_path.to_str().unwrap()];
        command.extend(args);
        serde_json::from_str::<Value>(&success(clone_command(&parent, &clone, &command))).unwrap()
    };
    let waves = run(&["wave", "list", "--json"]);
    assert!(waves.to_string().contains("task-pr-tests"));
    let sessions = run(&["desktop", "sessions", "--task", "INF-123", "--json"]);
    let primary = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["task_primary"] == true)
        .unwrap_or_else(|| {
            panic!("copied Task primary missing from Session inventory: {sessions}")
        });
    assert_eq!(primary["id"], "copied-primary");
    assert_eq!(primary["task_primary"], true);
    assert!(primary["open_argv"].as_array().unwrap().is_empty());
    assert!(primary["actions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|action| action["unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("read-only")));
    assert_eq!(repo.head_sha(), head);
    assert_eq!(fs::read(source.join("loopflow.db")).unwrap(), before);
    assert_eq!(fs::read(source.join("loopflow.db-wal")).unwrap(), wal);
}

#[test]
fn clone_creation_refuses_an_incompatible_parent_sandbox_before_copying_source() {
    let parent = tempfile::tempdir().unwrap();
    let parent = parent.path().canonicalize().unwrap();
    let source = parent.join("source");
    let clone = parent.join("clone");
    fs::create_dir(&source).unwrap();
    let sentinel = b"source database must not be opened";
    fs::write(source.join("loopflow.db"), sentinel).unwrap();
    // macOS cannot tighten a parent profile that grants a no-sandbox exec.
    // This reproduces an incompatible host without weakening the clone policy.
    let output = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", r#"(version 1)(allow default)(allow process-exec (literal "/bin/ps") (with no-sandbox))"#])
        .arg(env!("CARGO_BIN_EXE_lf"))
        .env_clear().env("LF_HOME", &source)
        .current_dir(&parent)
        .args(["clone", "create", clone.to_str().unwrap(), "--source", source.to_str().unwrap()])
        .stdin(Stdio::null()).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot re-enter"),
        "{output:?}"
    );
    assert_eq!(fs::read(source.join("loopflow.db")).unwrap(), sentinel);
    assert!(!clone.join("loopflow.db").exists());
    let manifest: Value =
        serde_json::from_slice(&fs::read(clone.join("clone.json")).unwrap()).unwrap();
    assert_eq!(manifest["ready"], false);
    success(cli(&parent, &["clone", "dispose", clone.to_str().unwrap()]));
    assert!(!clone.exists());
}
