#![cfg(target_os = "macos")]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_")
            || key.to_string_lossy().starts_with("LOOPFLOW_")
        {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_HOME", home)
        .env_remove("LF_CAPTURE_KEY")
        .env_remove("LF_RUN_DIR")
        .env_remove("LF_WAVE_ID")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command
}

/// The next frame whose row for `id` is in one of `states`.
fn next_state(frames: &Receiver<serde_json::Value>, id: &str, states: &[&str]) {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let frame = frames
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        assert_eq!(frame["unavailable"], serde_json::Value::Null, "{frame}");
        let state = frame["body"]
            .as_array()
            .unwrap_or_else(|| panic!("{frame}"))
            .iter()
            .find(|row| row["id"] == id)
            .and_then(|row| row["state"].as_str());
        if state.is_some_and(|state| states.contains(&state)) {
            return;
        }
    }
}

fn exited(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "reader did not exit after pipe closure"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn session_rows_follow_their_provider_and_release_only_the_reader_on_eof_or_closed_stdout() {
    let home = tempfile::tempdir().unwrap();
    let mut client = Owned(
        Command::new("/bin/cat")
            .env_remove("LF_RUN_DIR")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client_started = time::OffsetDateTime::now_utc().unix_timestamp();
    let id = "run_00000000000000000000000000000001";
    let dir = home.path().join("runs/00").join(id);
    fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "artifact_key": id, "caller_artifact_key": null,
        "created_at": "2020-01-01T00:00:00Z", "harness": "cat", "model": null,
        "surface": "tui", "cwd": home.path(), "repo": null, "worktree": null,
        "skill": null, "subjects": [], "process": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "fixture", "boot_id": null,
    });
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let initialized = command(home.path(), &["session", "list", "--json"])
        .output()
        .unwrap();
    assert!(initialized.status.success(), "{initialized:?}");
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,provider,provider_thread,cwd,input_published) VALUES(?1,'Watch','generated',1577836800,'cat','fixture-thread',?2,1)", rusqlite::params![id,home.path().to_str().unwrap()]).unwrap();
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'captured',?1,1577836800,?2)", rusqlite::params![id,serde_json::json!({"artifact_key":id,"cwd":home.path(),"provider":"cat"}).to_string()]).unwrap();
    db.execute(
        "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
        rusqlite::params![id, db.last_insert_rowid()],
    )
    .unwrap();
    // Membership comes from the recorded AgentProcess.
    let agent = loopflow::id::LfProcessId::new();
    db.execute(
        "INSERT INTO processes(id,trace_id,kind,agent_session_id,started_at,pid,os_started_at,
            agent_provider,agent_interactive,spawn_state)
         VALUES(?1,?1,'agent',?2,?3,?4,?3,'cat',1,'spawn_requested')",
        rusqlite::params![agent, id, client_started, client.0.id()],
    )
    .unwrap();
    db.execute(
        "UPDATE agent_sessions SET agent_process_id=?2 WHERE id=?1",
        rusqlite::params![id, agent],
    )
    .unwrap();
    let mut watch = Owned(
        command(home.path(), &["session", "list", "--all", "--watch"])
            .spawn()
            .unwrap(),
    );
    let stdout = watch.0.stdout.take().unwrap();
    let (sender, frames) = mpsc::channel();
    let output = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let frame = serde_json::from_str::<serde_json::Value>(&line.unwrap()).unwrap();
            if sender.send(frame).is_err() {
                break;
            }
        }
    });
    // The first read comes from process records, with no registry to wait on.
    next_state(&frames, id, &["active", "detached"]);
    db.execute(
        "UPDATE processes SET completed_at=?2,spawn_state='exited' WHERE id=?1",
        rusqlite::params![agent, client_started],
    )
    .unwrap();
    // The ordinary cadence must observe the recorded exit without a UI request.
    next_state(&frames, id, &["closed"]);
    watch
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"{\"id\":1}\n")
        .unwrap();
    next_state(&frames, id, &["closed"]);
    drop(watch.0.stdin.take());
    exited(&mut watch.0);
    output.join().unwrap();
    assert!(client.0.try_wait().unwrap().is_none());

    let mut broken = Owned(
        command(home.path(), &["session", "list", "--all", "--watch"])
            .spawn()
            .unwrap(),
    );
    drop(broken.0.stdout.take());
    exited(&mut broken.0);
    assert!(client.0.try_wait().unwrap().is_none());
}
