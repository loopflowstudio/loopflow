use std::process::Command;

#[test]
fn cli_volume_records_real_reads_without_contents() {
    let home = tempfile::tempdir().unwrap();
    let receipts = tempfile::tempdir().unwrap();
    for measured in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") {
                command.env_remove(key);
            }
        }
        command
            .args(["session", "list", "--json", "--all"])
            .current_dir(home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"));
        if measured {
            command.env("LF_PERF_OUTPUT", receipts.path());
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!([])
        );
        let files: Vec<_> = std::fs::read_dir(receipts.path()).unwrap().collect();
        assert_eq!(files.len(), usize::from(measured));
    }
    let file = std::fs::read_dir(receipts.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["event"], "start");
    let end = events.last().unwrap();
    assert_eq!(end["event"], "end");
    assert!(end["connections"].as_u64().unwrap() > 0);
    assert!(end["statements"].as_u64().unwrap() > 0);
    assert!(end["rows"].as_u64().unwrap() > 0);
    assert!(!text.contains(home.path().to_str().unwrap()));
    assert!(!text.contains("SELECT"));
    assert!(!text.contains("session"));
}

/// Statements one `lf wave show --all` reading runs, and what it printed.
fn planning_statements(home: &std::path::Path, repo: &std::path::Path) -> (u64, String) {
    let receipts = tempfile::tempdir().unwrap();
    let output = lf(home, repo, &["wave", "show", "--all", "--json"])
        .env("LF_PERF_OUTPUT", receipts.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let file = std::fs::read_dir(receipts.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    let end: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(end["event"], "end");
    (
        end["statements"].as_u64().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

fn lf(home: &std::path::Path, repo: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .stdin(std::process::Stdio::null());
    command
}

#[test]
fn planning_statements_do_not_grow_with_its_tasks() {
    let home = tempfile::tempdir().unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let checkouts = tempfile::tempdir().unwrap();
    // Every other Task has a checkout where a Graph ran; Graph execution is
    // read with the rest of the reading, not once per Graph.
    let create = |count: usize| {
        for index in 0..count {
            let title = format!("Task {index}");
            let output = lf(
                home.path(),
                repo.path(),
                &["task", "create", "--title", &title, "--json"],
            )
            .output()
            .unwrap();
            assert!(output.status.success(), "{output:?}");
            if index % 2 == 1 {
                continue;
            }
            let created: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let checkout = checkouts.path().join(created["id"].as_str().unwrap());
            std::fs::create_dir_all(&checkout).unwrap();
            rusqlite::Connection::open(home.path().join("loopflow.db"))
                .unwrap()
                .execute(
                    "UPDATE tasks SET worktree=?2,
                         checkout_machine_id=(SELECT id FROM machines WHERE route='local')
                     WHERE id=?1",
                    rusqlite::params![created["id"].as_str(), checkout.to_string_lossy()],
                )
                .unwrap();
            crate::support::record_graph(home.path(), &checkout, "code", "implement", "failed");
        }
    };
    create(2);
    let (fewer, _) = planning_statements(home.path(), repo.path());
    create(4);
    let (statements, plan) = planning_statements(home.path(), repo.path());
    assert_eq!(statements, fewer);
    assert_eq!(
        plan.matches(r#""execution":{"state":"blocked""#).count(),
        3,
        "{plan}"
    );
}
