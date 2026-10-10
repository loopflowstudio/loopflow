//! An installed repository declaration invokes the real, finite CLI collector.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use loopflow::ops::cron::{
    add_cron, parse_schedule, repository_cron_key, run_cron, CronHost, CronSource, CronSpec,
    CronTargetKind, Launchctl,
};
use loopflow::ops::OpsResult;
use loopflow_test_support::TestRepo;

struct HeadlessLaunchctl;

struct CheckoutUser(Child);
impl Drop for CheckoutUser {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Launchctl for HeadlessLaunchctl {
    fn load(&self, _: &Path) -> OpsResult<()> {
        Ok(())
    }
    fn unload(&self, _: &Path) -> OpsResult<()> {
        Ok(())
    }
    fn is_loaded(&self, _: &str) -> OpsResult<bool> {
        Ok(true)
    }
    fn trigger(&self, _: &str) -> OpsResult<()> {
        Ok(())
    }
}

#[test]
fn scheduled_cleanup_retries_a_busy_checkout_after_it_becomes_idle() {
    scheduled_cleanup(false);
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn experimental_cleanup_discovers_release_checkout_admission() {
    assert!(Path::new("/.dockerenv").is_file());
    assert_eq!(
        loopflow::installation::account_home().unwrap(),
        Path::new("/home/lf-task-proof")
    );
    scheduled_cleanup(true);
}

fn scheduled_cleanup(release_admission: bool) {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_lf"));
    let initialized = Command::new(binary)
        .args(["wave", "list", "--json"])
        .env_clear()
        .env("LF_HOME", home.path())
        .env("HOME", home.path())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let path = repo.create_named_worktree("landed").canonicalize().unwrap();
    let neighbor = repo.create_named_worktree("unfinished");
    fs::write(neighbor.join("notes"), "unfinished work").unwrap();
    let head = loopflow::engine::git::rev_parse(&path, "HEAD").unwrap();
    let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    connection.execute("INSERT INTO pr_landings(id,repo,pr_number,worktree,branch,requested_head_sha,observed_head_sha,merge_commit,state,generation,created_at,updated_at) VALUES('00000000-0000-0000-0000-000000000003','example/cleanup',1,?1,'landed',?2,?2,?2,'merged',1,1,1)", rusqlite::params![path.to_str().unwrap(), head]).unwrap();
    connection
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('00000000-0000-0000-0000-000000000001','cleanup',?1,1)",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
    connection.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('proj_00000000000000000000000000000001','00000000-0000-0000-0000-000000000001','project',1)", []).unwrap();
    for (id, branch, checkout, done) in [
        (
            "task_00000000000000000000000000000001",
            "landed",
            &path,
            true,
        ),
        (
            "task_00000000000000000000000000000002",
            "unfinished",
            &neighbor,
            false,
        ),
    ] {
        connection.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,branch,base_commit,created_at,updated_at,issue_title,issue_description,pm_snapshot_synced_at,planning_completed) VALUES(?1,'proj_00000000000000000000000000000001',?1,?1,?2,?3,?3,?4,1,1,'Cleanup','',1,?5)", rusqlite::params![id, checkout.to_str().unwrap(), branch, head, done]).unwrap();
    }
    connection.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,github_number,github_url,github_head_sha,merge_commit,created_at,updated_at) VALUES('pr_00000000000000000000000000000001','task_00000000000000000000000000000001',1,'landed','landed',?1,1,1,'https://github.com/example/cleanup/pull/1',?1,?1,1,1)", [&head]).unwrap();
    let mut checkout_user = CheckoutUser(
        Command::new("/bin/sh")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .current_dir(&path)
            .spawn()
            .unwrap(),
    );
    let started_at = time::OffsetDateTime::now_utc().unix_timestamp();
    connection.execute("INSERT INTO processes(lfid,trace_id,pid,cwd,started_at) VALUES('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002',?1,?2,?3)", rusqlite::params![checkout_user.0.id(), path.to_str().unwrap(), started_at]).unwrap();
    let receipts = home.path().join("runtime/exec-processes");
    fs::create_dir_all(&receipts).unwrap();
    fs::write(receipts.join("checkout-user.json"), serde_json::to_vec(&serde_json::json!({
        "schema_version":1, "exec_id":"00000000-0000-0000-0000-000000000001",
        "trace_id":"00000000-0000-0000-0000-000000000002", "pid":checkout_user.0.id(), "started_at":started_at
    })).unwrap()).unwrap();
    // No pending PRs means reconciliation needs no provider. Use a valid repo
    // identity, without asking a network service to decide checkout disposition.
    assert!(Command::new("git")
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/example/cleanup.git"
        ])
        .current_dir(repo.path())
        .status()
        .unwrap()
        .success());
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let lsof = bin.join("lsof");
    fs::write(&lsof, "#!/bin/sh\nprintf 'p1\\nn/\\n'\n").unwrap();
    fs::set_permissions(&lsof, fs::Permissions::from_mode(0o755)).unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    store
        .finish_follow_through(
            &loopflow::durable::TaskId::parse("task_00000000000000000000000000000001").unwrap(),
            "No remaining scope",
            true,
        )
        .unwrap();
    let machine = store.local_machine().unwrap().id;
    let spec = CronSpec {
        wave: String::new(),
        flow: repository_cron_key(repo.path(), &machine),
        target_kind: CronTargetKind::Repository,
        schedule: parse_schedule("every-minute").unwrap(),
        working_directory: repo.path().to_path_buf(),
        lf_path: binary.to_path_buf(),
        host: CronHost {
            machine_id: machine.clone(),
            lf_home: home.path().to_path_buf(),
            path_env: format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
        },
    };
    let agents = home.path().join("agents");
    let installed = add_cron(&agents, &spec, &HeadlessLaunchctl).unwrap();
    let definition = fs::read_to_string(installed.path).unwrap();
    assert!(definition.contains("--scheduled"));
    let run = || {
        run_cron(
            &agents,
            "",
            &spec.flow,
            &machine,
            &machine,
            CronSource::Scheduled,
        )
    };
    let first = run().unwrap();
    assert!(path.exists());
    let first_log = fs::read_to_string(first.log_path).unwrap();
    assert!(first_log.contains("unknown execution"), "{first_log}");
    // No row is marked complete: the next tick must observe the real PID's exit.
    drop(checkout_user.0.stdin.take());
    checkout_user.0.wait().unwrap();
    let release_lock = release_admission.then(|| {
        use fs2::FileExt;
        use sha2::{Digest, Sha256};

        let release = loopflow::installation::account_home().unwrap().join(".lf");
        fs::create_dir_all(&release).unwrap();
        let database = release.join("loopflow.db");
        assert!(
            !database.exists(),
            "proof requires an unused disposable release registry"
        );
        // An independent registry is opened by the CLI through getpwuid, not LF_HOME.
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(database.clone()),
            ))
            .unwrap();
        let root = database.with_extension("admission");
        fs::create_dir_all(&root).unwrap();
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(hex::encode(Sha256::digest(
                path.as_os_str().as_encoded_bytes(),
            ))))
            .unwrap();
        lock.lock_exclusive().unwrap();
        lock
    });
    if release_lock.is_some() {
        let held = run().unwrap();
        let log = fs::read_to_string(held.log_path).unwrap();
        assert!(path.exists(), "{log}");
        assert!(log.contains("checkout admission unavailable"), "{log}");
    }
    drop(release_lock);
    let second = run().unwrap();
    let log = fs::read_to_string(second.log_path).unwrap();
    assert!(!path.exists(), "{log}");
    assert_eq!(
        fs::read_to_string(neighbor.join("notes")).unwrap(),
        "unfinished work"
    );
    run().unwrap();
    if release_admission {
        let home = loopflow::installation::account_home().unwrap().join(".lf");
        for name in ["loopflow.db", "loopflow.db-wal", "loopflow.db-shm"] {
            let path = home.join(name);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        fs::remove_dir_all(home.join("loopflow.admission")).unwrap();
    }
}
