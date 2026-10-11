#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

fn command(home: &Path, repo: &Path, bin: &Path) -> Command {
    let mut command = Command::new(bin.join("lf"));
    command
        .process_group(0)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", bin.join("lf"))
        .env("CLAUDE_CONFIG_DIR", home.join("claude"))
        .env("CODEX_HOME", home.join("codex"))
        .env("RUST_LOG", "off")
        .current_dir(repo);
    command
}

fn launch(home: &Path, repo: &Path, bin: &Path, measurements: &Path, args: &[&str]) -> Duration {
    fs::create_dir(measurements).unwrap();
    fs::write(home.join("provider-launches"), "").unwrap();
    let start = Instant::now();
    let mut child = command(home, repo, bin)
        .args(args)
        .env("LF_PERF_OUTPUT", measurements)
        .env("GIT_TRACE2_EVENT", measurements.join("git.jsonl"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(
            fs::File::create(measurements.join("stderr")).unwrap(),
        ))
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == "LF_STARTUP_READY" {
                send.send(start.elapsed()).unwrap();
                return;
            }
        }
    });
    // The budget detects hangs and whole-second regressions, not runner jitter.
    let ready = receive.recv_timeout(Duration::from_secs(30));
    if ready.is_ok() {
        child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    } else {
        // SAFETY: this is the new process group created for this fixture;
        // no copied or ambient process identity can be signaled here.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    }
    let finish = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if finish.elapsed() > Duration::from_secs(10) {
            // SAFETY: only this fixture's newly created process group is owned.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            child.wait().unwrap();
            panic!("lf did not finish after the stand-in provider exited");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    reader.join().unwrap();
    let elapsed = ready.unwrap_or_else(|error| {
        panic!(
            "provider never became ready: {error}; {}",
            fs::read_to_string(measurements.join("stderr")).unwrap()
        )
    });
    assert!(
        status.success(),
        "{}",
        fs::read_to_string(measurements.join("stderr")).unwrap()
    );

    assert_eq!(
        fs::read_to_string(home.join("provider-launches")).unwrap(),
        "launch\n",
        "startup executed the provider more than once before readiness"
    );

    let git_count = fs::read_to_string(measurements.join("git.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|event| event["event"] == "start")
        .count();
    assert!(git_count <= 25, "launch spawned {git_count} Git processes");

    let mut rows = 0;
    let mut statements = 0;
    let mut endings = 0;
    for entry in fs::read_dir(measurements).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_name().to_string_lossy().starts_with("lf-") {
            continue;
        }
        for line in fs::read_to_string(entry.path()).unwrap().lines() {
            let event: Value = serde_json::from_str(line).unwrap();
            if event["event"] == "end" {
                endings += 1;
                rows += event["rows"].as_u64().unwrap();
                statements += event["statements"].as_u64().unwrap();
            }
        }
    }
    assert!(endings > 0, "missing completed performance receipt");
    assert!(rows < 25_000, "launch read {rows} SQLite rows");
    assert!(
        statements < 2_000,
        "launch executed {statements} statements"
    );
    eprintln!(
        "{args:?}: {elapsed:?}, {git_count} Git children, {statements} statements, {rows} rows"
    );
    elapsed
}

#[test]
fn bare_and_connect_reach_provider_with_bounded_work_without_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    for path in [&home, &repo, &bin] {
        fs::create_dir(path).unwrap();
    }
    symlink(env!("CARGO_BIN_EXE_lf"), bin.join("lf")).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
printf 'launch\n' >> "$LF_HOME/provider-launches"
if [ "$1" = --version ]; then exit 0; fi
printf '%s\n' "$@" > "$LF_HOME/provider-args"
printf 'LF_STARTUP_READY\n'
read -r finish
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(home.join("config.yaml"), "agent: claude\n").unwrap();
    fs::write(repo.join("AGENTS.md"), "Keep the startup context marker.\n").unwrap();
    for args in [
        &["init", "--quiet", "-b", "fixture"][..],
        &["add", "."],
        &["commit", "--quiet", "-m", "fixture"],
    ] {
        assert!(Command::new("git")
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid"
            ])
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    // Measure launches against an existing Machine; schema creation is setup.
    let initialized = command(&home, &repo, &bin)
        .args(["machine", "id"])
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let nested = repo.join("nested");
    fs::create_dir(&nested).unwrap();
    launch(&home, &nested, &bin, &temp.path().join("bare"), &[]);
    let args = fs::read_to_string(home.join("provider-args")).unwrap();
    let context_file = args
        .lines()
        .skip_while(|arg| *arg != "--append-system-prompt-file")
        .nth(1)
        .unwrap();
    let context = fs::read_to_string(context_file).unwrap();
    assert!(!context.contains("<lf:skill:default>"));
    assert!(args.contains("<lf:skill:default>"));
    let native = args
        .lines()
        .skip_while(|arg| *arg != "--session-id")
        .nth(1)
        .unwrap();
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let session: String = db
        .query_row("SELECT id FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        Path::new(&cwd).canonicalize().unwrap(),
        repo.canonicalize().unwrap()
    );
    let captured: i64 = db
        .query_row(
            "SELECT current_capture FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    // Irrelevant history must not become a scan in the launch path.
    db.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
         WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<30000)
         SELECT ?1,'observed','startup-noise:'||x,1,'{}',?2 FROM n",
        rusqlite::params![session, captured],
    )
    .unwrap();
    launch(
        &home,
        &repo,
        &bin,
        &temp.path().join("connect"),
        &["session", "connect", &session],
    );
    let resumed = fs::read_to_string(home.join("provider-args")).unwrap();
    assert!(
        resumed.contains(&format!("--resume\n{native}\n")),
        "native identity changed"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM session_events WHERE receipt_key LIKE 'startup-noise:%'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        30000
    );
}
