//! An installed repository declaration invokes the real, finite CLI collector.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::ops::cron::{
    add_cron, parse_schedule, repository_cron_key, run_cron, CronHost, CronSource, CronSpec,
    CronTargetKind, Launchctl,
};
use loopflow::ops::OpsResult;
use loopflow_test_support::TestRepo;

struct HeadlessLaunchctl;
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
    connection.execute("INSERT INTO processes(lfid,trace_id,cwd,started_at) VALUES('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002',?1,unixepoch())", [path.to_str().unwrap()]).unwrap();
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
    connection.execute("UPDATE processes SET completed_at=2,outcome='succeeded',exit_code=0 WHERE lfid='00000000-0000-0000-0000-000000000001'", []).unwrap();
    let second = run().unwrap();
    let log = fs::read_to_string(second.log_path).unwrap();
    assert!(!path.exists(), "{log}");
    assert_eq!(
        fs::read_to_string(neighbor.join("notes")).unwrap(),
        "unfinished work"
    );
    run().unwrap();
}

#[test]
fn cleanup_report_json_preserves_deferred_reasons_and_unknown_sizes() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/repository_reconciliation.json"
    ))
    .unwrap();
    let report: loopflow::ops::pr_landing::DeliveryCheck =
        serde_json::from_value(value.clone()).unwrap();
    let cleanup = report.cleanup.as_ref().unwrap();
    assert_eq!(cleanup.deferred[0].estimated_bytes, None);
    assert!(matches!(&cleanup.deferred[0].action,
        loopflow::ops::wt::cleanup::CleanupAction::Retain(reason) if reason == "running external process"));
    assert_eq!(serde_json::to_value(report).unwrap(), value);
}
