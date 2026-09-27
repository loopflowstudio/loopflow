#![cfg(target_os = "macos")]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use loopflow::lf::commands::runs::{ActiveRunsSnapshot, DiscoveryState};

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn reader(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["runs", "--active", "--watch", "--json"])
        .env("LF_HOME", home)
        .env("LF_CONTROL_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("LF_CONTROL_DB_PATH", home.join("loopflow.db"))
        .env_remove("LF_RUN_ID")
        .env_remove("LF_RUN_DIR")
        .env_remove("LF_WAVE_ID")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command
}

fn next_ready(frames: &Receiver<ActiveRunsSnapshot>, count: usize) -> ActiveRunsSnapshot {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let snapshot = frames
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        assert_ne!(
            snapshot.discovery,
            DiscoveryState::Unavailable,
            "{snapshot:?}"
        );
        if snapshot.discovery == DiscoveryState::Ready && snapshot.runs.len() == count {
            assert!(snapshot.gaps.is_empty(), "{snapshot:?}");
            return snapshot;
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
fn watch_updates_and_releases_only_its_reader_on_eof_or_closed_stdout() {
    let home = tempfile::tempdir().unwrap();
    let mut client = Owned(
        Command::new("/bin/cat")
            .env_remove("LF_RUN_DIR")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let id = "run_00000000000000000000000000000001";
    let dir = home.path().join("runs/00").join(id);
    fs::create_dir_all(dir.join("provider-clients")).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "run_id": id, "parent_run_id": null,
        "created_at": "2020-01-01T00:00:00Z", "harness": "cat", "model": null,
        "surface": "tui", "cwd": home.path(), "repo": null, "worktree": null,
        "skill": null, "subjects": [], "launch": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "fixture", "boot_id": null,
    });
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let receipt = dir
        .join("provider-clients")
        .join(format!("{}.json", client.0.id()));
    let native = serde_json::json!({
        "schema_version": 1, "pid": client.0.id(), "terminal_id": null,
        "started_at": time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
    });
    fs::write(&receipt, serde_json::to_vec(&native).unwrap()).unwrap();
    fs::create_dir_all(home.path().join("runtime")).unwrap();
    let registry_lock =
        fs::File::create(home.path().join("runtime/opencode-servers.json.lock")).unwrap();
    fs2::FileExt::lock_exclusive(&registry_lock).unwrap();
    let mut watch = Owned(reader(home.path()).spawn().unwrap());
    let stdout = watch.0.stdout.take().unwrap();
    let (sender, frames) = mpsc::channel();
    let output = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let snapshot = serde_json::from_str::<ActiveRunsSnapshot>(&line.unwrap()).unwrap();
            if sender.send(snapshot).is_err() {
                break;
            }
        }
    });
    // The existing registry lock holds the real cold read. Progress frames must
    // continue while no completed observation can be produced.
    for _ in 0..2 {
        let scanning = frames.recv_timeout(Duration::from_secs(8)).unwrap();
        assert_eq!(scanning.discovery, DiscoveryState::Scanning);
    }
    fs2::FileExt::unlock(&registry_lock).unwrap();
    let snapshot = next_ready(&frames, 1);
    assert_eq!(snapshot.runs[0].id.as_str(), id);
    fs::remove_file(&receipt).unwrap();
    // The ordinary cadence must observe exit/removal without a UI request.
    next_ready(&frames, 0);
    watch
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"{\"action\":\"refresh\"}\n")
        .unwrap();
    next_ready(&frames, 0);
    watch
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"{\"action\":\"rescan\"}\n")
        .unwrap();
    next_ready(&frames, 0);
    drop(watch.0.stdin.take());
    exited(&mut watch.0);
    output.join().unwrap();
    assert!(client.0.try_wait().unwrap().is_none());

    let mut broken = Owned(reader(home.path()).spawn().unwrap());
    drop(broken.0.stdout.take());
    exited(&mut broken.0);
    assert!(client.0.try_wait().unwrap().is_none());
}
