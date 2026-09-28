//! Chat connection, startup recovery, and placement through real CLI/daemon binaries.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use futures_util::future::join_all;
use loopflow::child::ObservationRecipient;
use loopflow::planning::{LinearProjectId, ProjectPlan};
use loopflow::store::StorageConfig;
use loopflow::work::project::{Project, ProjectEventKind, ProjectId};
use loopflow::work::wave::WaveLocator;
use time::OffsetDateTime;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

struct HomeDaemon {
    sessions: PathBuf,
}

impl HomeDaemon {
    fn stop(&self) {
        let Ok(entries) = std::fs::read_dir(&self.sessions) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|ext| ext == "pid") {
                if let Ok(pid) = std::fs::read_to_string(entry.path()) {
                    let _ = std::process::Command::new("kill")
                        .args(["-TERM", pid.trim()])
                        .status();
                }
            }
        }
    }
}

impl Drop for HomeDaemon {
    fn drop(&mut self) {
        self.stop();
    }
}

fn git(repo: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn write_fake_tmux(directory: &Path) -> PathBuf {
    let bin = directory.join("bin");
    let pid_dir = directory.join("sessions");
    std::fs::create_dir_all(&bin).expect("create fake binary directory");
    std::fs::create_dir_all(&pid_dir).expect("create fake session directory");
    let tmux = bin.join("tmux");
    std::fs::write(
        &tmux,
        r#"#!/bin/sh
if [ "$1" != "new-session" ]; then
  exit 0
fi
session="$4"
cwd="$6"
command="$9"
printf '%s\n' "$command" > "$FAKE_TMUX_PID_DIR/$session.command"
(
  cd "$cwd" || exit 1
  exec /bin/sh -c "$command"
) </dev/null >> "$FAKE_TMUX_PID_DIR/$session.log" 2>&1 &
printf '%s\n' "$!" > "$FAKE_TMUX_PID_DIR/$session.pid"
"#,
    )
    .expect("write fake tmux");
    let mut permissions = std::fs::metadata(&tmux)
        .expect("read fake tmux metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&tmux, permissions).expect("make fake tmux executable");
    // Prevent any configured provider from being reached by fixture observations.
    for provider in ["codex", "claude", "opencode"] {
        let path = bin.join(provider);
        std::fs::write(&path, "#!/bin/sh\nexit 79\n").expect("write fixture provider");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make fixture provider executable");
    }
    bin
}

fn lf_command(repo: &Path, home: &Path, fake_bin: &Path, args: &[&str]) -> tokio::process::Command {
    let path = std::env::var("PATH").unwrap_or_default();
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CARGO_BIN_EXE_lf", env!("CARGO_BIN_EXE_lf"))
        .env("CARGO_BIN_EXE_lfd", env!("CARGO_BIN_EXE_lfd"))
        .env("FAKE_TMUX_PID_DIR", home.join("sessions"))
        .env("PATH", format!("{}:{path}", fake_bin.display()))
        .env_remove("LF_CONTROL_BIN")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_RUN_ID")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

async fn run_lf(repo: &Path, home: &Path, fake_bin: &Path, args: &[&str]) -> std::process::Output {
    tokio::time::timeout(
        COMMAND_TIMEOUT,
        lf_command(repo, home, fake_bin, args).output(),
    )
    .await
    .unwrap_or_else(|_| panic!("lf {args:?} exceeded {COMMAND_TIMEOUT:?}"))
    .expect("run lf")
}

fn live_snapshot(output: &std::process::Output, name: &str) -> serde_json::Value {
    assert!(
        output.status.success(),
        "chat connection for {name} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse connection JSON");
    let snapshot = &body[0];
    assert_eq!(snapshot["name"], name);
    assert_eq!(snapshot["live"], true);
    assert!(snapshot["endpoint"].as_str().is_some());
    snapshot.clone()
}

#[tokio::test]
async fn chat_connects_and_preserves_failed_siblings_and_remote_placement() {
    let temporary = tempfile::tempdir().expect("create temp directory");
    let repo = temporary.path().join("repo");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(repo.join("wave/good")).expect("create good Wave");
    std::fs::create_dir_all(repo.join("wave/broken")).expect("create broken Wave");
    std::fs::write(repo.join("wave/good/GOAL.md"), "A local test Wave.\n")
        .expect("write good goal");
    std::fs::write(
        repo.join("wave/broken/GOAL.md"),
        "---\nchat:\n  provider: [\n---\nBroken on purpose.\n",
    )
    .expect("write broken goal");
    git(&repo, &["init", "-q", "-b", "main", "."]);
    git(&repo, &["config", "user.name", "wave-start-test"]);
    git(&repo, &["config", "user.email", "wave-start@test.invalid"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "seed"]);
    let fake_bin = write_fake_tmux(&home);
    let _daemon = HomeDaemon {
        sessions: home.join("sessions"),
    };

    // A fresh Home starts its exact lfd/lf control pair. The valid sibling
    // stays live even though malformed Wave policy fails preflight, and the
    // failed first registration is removed.
    let mixed = run_lf(
        &repo,
        &home,
        &fake_bin,
        &["__chat-connect", "good", "broken", "--json"],
    )
    .await;
    assert!(!mixed.status.success());
    let failure = String::from_utf8_lossy(&mixed.stderr);
    assert!(failure.contains("broken"), "unexpected failure: {failure}");
    assert!(
        failure.contains("failed preflight"),
        "unexpected failure: {failure}"
    );

    let store =
        loopflow::store::open_ephemeral_store(&StorageConfig::sqlite(home.join("loopflow.db")))
            .await
            .expect("open fresh Home registry");
    let local = store.local_home().await.expect("read Home");
    let good_locator = WaveLocator::discover(&repo, "good").expect("locate good Wave");
    let good = store
        .get_wave_at(&good_locator)
        .await
        .expect("read good Wave")
        .expect("good Wave is registered");
    let broken_locator = WaveLocator::discover(&repo, "broken").expect("locate broken Wave");
    assert!(store
        .get_wave_at(&broken_locator)
        .await
        .expect("read broken Wave")
        .is_none());
    assert!(!repo.join("wave/broken/.wave-endpoint").exists());

    let endpoint_path = home
        .join("lfd")
        .join(format!("{}.endpoint", local.id.as_str()));
    assert!(endpoint_path.exists(), "fresh Home published lfd endpoint");
    let session = format!("lfd-{}", local.id.as_str());
    let launch = std::fs::read_to_string(home.join("sessions").join(format!("{session}.command")))
        .expect("read lfd launch command");
    assert!(launch.contains(env!("CARGO_BIN_EXE_lfd")));
    assert!(launch.contains(env!("CARGO_BIN_EXE_lf")));
    let receipts = std::fs::read_dir(home.join("lfd/startup"))
        .expect("read startup receipts")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        })
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 1);
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(receipts[0].path()).expect("read startup receipt"))
            .expect("parse startup receipt");
    assert_eq!(receipt["state"], "live");

    // An installed/live lfd handles an already-registered Wave without
    // launching a replacement daemon.
    let existing = run_lf(
        &repo,
        &home,
        &fake_bin,
        &["__chat-connect", "good", "--json"],
    )
    .await;
    let first = live_snapshot(&existing, "good");
    let receipt_count = std::fs::read_dir(home.join("lfd/startup"))
        .expect("read startup receipts")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        })
        .count();
    assert_eq!(receipt_count, 1, "live lfd was not replaced");

    // Public chat connects a newly authored Wave without a service command.
    std::fs::create_dir_all(repo.join("wave/chat")).unwrap();
    std::fs::write(repo.join("wave/chat/GOAL.md"), "A chat test Wave.\n").unwrap();
    let chat = run_lf(&repo, &home, &fake_bin, &["chat", "--follow", "-w", "chat"]).await;
    assert!(
        chat.status.success(),
        "{}",
        String::from_utf8_lossy(&chat.stderr)
    );
    assert!(String::from_utf8_lossy(&chat.stdout).contains("chat: chat @ "));
    let locator = WaveLocator::discover(&repo, "chat").unwrap();
    let chat_wave = store.get_wave_at(&locator).await.unwrap().unwrap();
    let work = loopflow::durable::WorkRef::Wave(chat_wave.id().clone());
    assert_eq!(store.placement(&work).await.unwrap().home_id, local.id);
    let remote = loopflow::durable::HomeId::new();
    store
        .observe_home(&remote, "ssh://fixture@mini")
        .await
        .unwrap();
    let placed = run_lf(
        &repo,
        &home,
        &fake_bin,
        &["wave", "place", "chat", remote.as_str(), "--json"],
    )
    .await;
    assert!(
        placed.status.success(),
        "{}",
        String::from_utf8_lossy(&placed.stderr)
    );
    let chat = run_lf(&repo, &home, &fake_bin, &["chat", "--follow", "-w", "chat"]).await;
    assert!(!chat.status.success());
    assert!(String::from_utf8_lossy(&chat.stderr).contains("belongs to Home"));
    assert_eq!(store.placement(&work).await.unwrap().home_id, remote);

    // Connecting chat drains durable child observations before returning.
    let now = OffsetDateTime::now_utc();
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("wave-start-observation-proof")
                .expect("valid Linear project id"),
            slug: "observation-proof".to_owned(),
            name: "Observation proof".to_owned(),
            prompt_context: "Prove wake drains durable child observations.".to_owned(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: good.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .create_project(&project)
        .await
        .expect("create child Project");
    store
        .append_project_event(
            &project.id,
            &ProjectEventKind::Failed {
                error: "durable wake proof".to_owned(),
                resumable: true,
            },
        )
        .await
        .expect("append observable Project event");
    let recipient = ObservationRecipient::Wave {
        wave_id: good.id().clone(),
    };
    assert_eq!(
        store
            .pending_observations(&recipient)
            .await
            .expect("read pending observations")
            .len(),
        1
    );

    // Concurrent wake callers share the listener's startup event. All return
    // the same truthful live endpoint within the command bound.
    let outputs = join_all((0..2).map(|_| {
        run_lf(
            &repo,
            &home,
            &fake_bin,
            &["__chat-connect", "good", "--json"],
        )
    }))
    .await;
    let snapshots = outputs
        .iter()
        .map(|output| live_snapshot(output, "good"))
        .collect::<Vec<_>>();
    let endpoint = snapshots[0]["endpoint"].clone();
    for snapshot in snapshots {
        assert_eq!(snapshot["id"], first["id"]);
        assert_eq!(snapshot["endpoint"], endpoint);
    }
    assert_eq!(
        std::fs::read_to_string(repo.join("wave/good/GOAL.md")).expect("read restarted Wave goal"),
        "A local test Wave.\n"
    );

    assert!(
        store
            .pending_observations(&recipient)
            .await
            .expect("read drained observations")
            .is_empty(),
        "start returned before the durable observation was drained"
    );
}
