//! Explicit experiments preserve one Home across real CLI processes.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;
use rusqlite::Connection;

fn command(home: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .current_dir(cwd)
        .args(args);
    command
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn custom_home_initializes_once_and_refuses_schema_repair() {
    let home = tempfile::tempdir().unwrap();
    success(
        command(home.path(), home.path(), &["wave", "list", "--json"])
            .output()
            .unwrap(),
    );
    success(
        command(home.path(), home.path(), &["wave", "list", "--json"])
            .output()
            .unwrap(),
    );
    let database = home.path().join("loopflow.db");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE waves ADD COLUMN experiment TEXT; UPDATE waves SET experiment='retained';",
        )
        .unwrap();
    let before: String = connection
        .query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let error = loopflow::store::sqlite::SqliteStore::new(&database)
        .unwrap_err()
        .to_string();
    assert!(error.contains("disposable"), "{error}");
    let after: String = connection
        .query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    assert!(!home.path().join("backups").exists());
}

#[test]
fn flow_steps_keep_the_explicit_home_despite_stale_pins_and_path() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let decoy = tempfile::tempdir().unwrap();
    fs::write(decoy.path().join("loopflow.db"), b"never open this store").unwrap();
    repo.create_file(".lf/flows/home-proof.yaml", "- cmd: task sync --plan\n");
    let output = command(
        home.path(),
        repo.path(),
        &["flow", "home-proof", "--mode", "batch"],
    )
    .env("LF_CONTROL_HOME", decoy.path())
    .env("LF_CONTROL_DB_PATH", decoy.path().join("loopflow.db"))
    .env("LF_CONTROL_BIN", "/missing/historical/lf")
    .output()
    .unwrap();
    success(output);
    let connection = Connection::open(home.path().join("loopflow.db")).unwrap();
    let child_commands: i64 = connection.query_row(
        "SELECT count(*) FROM flow_events f JOIN execs e ON e.id=f.exec_id WHERE f.kind='operation_started' AND e.exit_code=0",
        [], |row| row.get(0)
    ).unwrap();
    assert_eq!(child_commands, 1);
    assert_eq!(
        fs::read(decoy.path().join("loopflow.db")).unwrap(),
        b"never open this store"
    );
    assert_eq!(fs::read_dir(decoy.path()).unwrap().count(), 1);
}

#[test]
fn local_promotion_and_retained_home_commands_are_removed() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["install", "local-preflight", "--store", "/unused", "--json"],
        vec![
            "install",
            "promote",
            "--from-build",
            "/unused",
            "--cli-target",
            "/unused",
        ],
        vec!["install", "promote", "--fresh", "--cli-target", "/unused"],
        vec![
            "install",
            "promote",
            "--reuse-home",
            "old",
            "--cli-target",
            "/unused",
        ],
    ] {
        let output = command(home.path(), home.path(), &args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn default_and_nested_commands_use_the_installed_cli_and_main_home() {
    use std::os::unix::fs::PermissionsExt;

    use loopflow::machine_install::{
        self, ActiveInstall, ArtifactIdentity, ArtifactRole, ArtifactSet, InstallSelection,
        InstallSource,
    };
    use sha2::{Digest, Sha256};

    assert!(Path::new("/.dockerenv").is_file());
    let account = machine_install::account_home().unwrap();
    assert_eq!(account, Path::new("/home/lf-task-proof"));
    let root = machine_install::root().unwrap();
    assert!(!root.exists());
    let files = tempfile::tempdir().unwrap();
    let cli = files.path().join("lf");
    // Simulate only the installed boundary; both incoming processes are the real source CLI.
    fs::write(&cli, "#!/bin/sh\nif [ \"$1\" = home ]; then exec \"$SOURCE_CLI\" monitor list --json; fi\nprintf '%s\\n' \"$LF_HOME\" \"$LF_DB_PATH\" \"$LF_BIN\" \"$*\"\n").unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let artifact = ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap();
    let set = ArtifactSet {
        id: "main-home-proof".into(),
        source: InstallSource::Published,
        source_revision: "fixture".into(),
        source_identity: "fixture".into(),
        content_sha256: hex::encode(Sha256::digest(artifact.sha256.as_bytes())),
        artifacts: vec![artifact],
    };
    let main = account.join(".lf");
    machine_install::write_active(
        &root,
        &ActiveInstall {
            schema_version: 1,
            selection: InstallSelection {
                installation_id: "main-home-proof".into(),
                source: InstallSource::Published,
                artifact_set: set.clone(),
                store: main.join("loopflow.db"),
            },
            published_fallback: set.clone(),
            retained_published_sets: vec![set],
        },
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &account)
        .env("SOURCE_CLI", env!("CARGO_BIN_EXE_lf"))
        .env("LF_CONTROL_HOME", "/retired/side-home")
        .env("LF_CONTROL_DB_PATH", "/retired/side-home/loopflow.db")
        .env("LF_CONTROL_BIN", "/retired/lf")
        .env("LF_DB_PATH", "/retired/side-home/loopflow.db")
        .env("LF_BIN", "/retired/lf")
        .args(["home", "id", "--json"])
        .output()
        .unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}\n{}\n{}\nmonitor list --json\n",
            main.display(),
            main.join("loopflow.db").display(),
            cli.display()
        )
    );
    assert!(!account.join(".lf-dev").exists());
}
