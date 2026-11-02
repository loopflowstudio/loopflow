//! Explicit experiments preserve one Machine across real CLI processes.

#[allow(dead_code)] // Linked for its inherited provider-home scrub.
mod support;

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
fn graph_steps_keep_the_explicit_home() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/graphs/home-proof.yaml", "- cmd: sync --plan\n");
    let output = command(
        home.path(),
        repo.path(),
        &["graph", "home-proof", "--batch"],
    )
    .output()
    .unwrap();
    success(output);
    let connection = Connection::open(home.path().join("loopflow.db")).unwrap();
    let child_commands: i64 = connection.query_row(
        "SELECT count(*) FROM processes e JOIN graph_process_steps s ON s.lf_process_id=e.id WHERE e.exit_code=0",
        [], |row| row.get(0)
    ).unwrap();
    assert_eq!(child_commands, 1);
}

#[test]
fn local_promotion_and_retained_home_commands_are_removed() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "self",
            "install",
            "local-preflight",
            "--store",
            "/unused",
            "--json",
        ],
        vec![
            "self",
            "install",
            "promote",
            "--from-build",
            "/unused",
            "--cli-target",
            "/unused",
        ],
        vec![
            "self",
            "install",
            "promote",
            "--fresh",
            "--cli-target",
            "/unused",
        ],
        vec![
            "self",
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
