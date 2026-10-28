//! Public commands must acquire only the repository context they actually use.

mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::store::sqlite::SqliteStore;
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;

fn command(home: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home.join(".lf"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("NO_COLOR", "1")
        .current_dir(cwd)
        .args(args);
    // In-process fixtures claim Tasks as this test process's person; `lf` acts
    // as the same person, whatever Git identity the test repository carries.
    if let Some(person) = loopflow::config::participant_name().unwrap() {
        command.env("LF_USER_NAME", person);
    }
    command
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn context_targets_measure_checkout_wave_and_refresh_local_edits() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    fs::create_dir_all(home.path().join(".lf")).unwrap();
    fs::create_dir_all(repo.path().join(".lf")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/wave/local")).unwrap();
    fs::create_dir_all(repo.path().join("scratch")).unwrap();
    fs::write(
        home.path().join(".lf/config.yaml"),
        "context_budgets:\n  memory_tokens: 500\n  scratch_tokens: 600\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "agent: not-a-launchable-harness\ndocs: [missing.md]\ncontext_budgets:\n  memory_tokens: 700\n  scratch_bytes: 10000\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/wave/local/GOAL.md"),
        "---\ncontext_budgets:\n  memory_tokens: 400\n---\nLocal objective.\n",
    )
    .unwrap();
    let memory = repo.path().join(".lf/wave/local/MEMORY.md");
    let scratch = repo.path().join("scratch/plan.md");
    fs::write(
        &memory,
        "Live decision and unresolved evidence. ".repeat(500),
    )
    .unwrap();
    fs::write(&scratch, "Pending work. ".repeat(500)).unwrap();
    SqliteStore::new(&home.path().join(".lf/loopflow.db"))
        .unwrap()
        .ensure_wave(
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            "local",
        )
        .unwrap();
    let query = || -> serde_json::Value {
        serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &["context", "--wave", "local", "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap()
    };
    let report = query();
    assert_eq!(report["wave"], "local");
    assert!(report.get("goal_status").is_none());
    let budgets = &report["context"]["budgets"];
    for (key, value, source) in [
        (
            "memory_tokens",
            400,
            repo.path().join(".lf/wave/local/GOAL.md"),
        ),
        ("scratch_tokens", 600, home.path().join(".lf/config.yaml")),
        ("scratch_bytes", 10000, repo.path().join(".lf/config.yaml")),
    ] {
        assert_eq!(budgets[key]["value"], value);
        assert_eq!(
            Path::new(budgets[key]["source"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            source.canonicalize().unwrap()
        );
    }
    let usage = report["context"]["usage"].as_array().unwrap();
    for source in &usage[..2] {
        let limit = source["token_limit"].as_u64().unwrap();
        assert!(source["tokens"].as_u64().unwrap() > limit);
    }
    assert!(budgets.get("input_tokens").is_none());
    fs::write(memory, "Live decision retained.").unwrap();
    fs::write(scratch, "Pending work retained.").unwrap();
    let refreshed = query();
    for source in &refreshed["context"]["usage"].as_array().unwrap()[..2] {
        assert!(source["tokens"].as_u64().unwrap() < 100);
    }
}

#[test]
fn explicit_home_ignores_retired_control_home_pins() {
    let home = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let store_path = home.path().join(".lf/loopflow.db");
    let store = SqliteStore::new(&store_path).unwrap();
    let source_db = source.path().join("loopflow.db");
    fs::write(&source_db, b"source must not be opened").unwrap();
    let marker = loopflow::durable::MachineId::new();
    store
        .add_machine(&marker, "ssh://proof@example.invalid", "proof")
        .unwrap();
    for args in [
        vec!["machine", "rename", "proof", "renamed"],
        vec!["monitor", "ps", "--json"],
    ] {
        let output = command(home.path(), home.path(), &args)
            .env("LF_RUN_DIR", source.path().join("runs/parent"))
            .env("LF_CAPTURE_KEY", "run_parent")
            .output()
            .unwrap();
        success(output);
    }
    let connection = rusqlite::Connection::open(&store_path).unwrap();
    let label: String = connection
        .query_row(
            "SELECT label FROM machines WHERE id=?1",
            [marker.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(label, "renamed");
    assert_eq!(fs::read(source_db).unwrap(), b"source must not be opened");
    assert_eq!(fs::read_dir(source.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn installation_uses_candidate_authority_from_any_checkout() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    repo.create_branch("install-task");
    let task = support::register_task_with_pr(
        &home.path().join(".lf"),
        repo.path(),
        "install-task",
        &repo.head_sha(),
    );
    for (cwd, declaration) in [
        (repo.path(), None),
        (home.path(), Some(format!("task:{}", task.task.id))),
        (home.path(), None),
    ] {
        let mut cmd = command(
            home.path(),
            cwd,
            &["self", "install", "promote", "--cli-target", "/unused/lf"],
        );
        if let Some(value) = declaration {
            cmd.env("LF_AS", value);
        }
        let output = cmd.output().unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("only a published release may advance"),
            "{error}"
        );
    }
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn installation_reaches_candidate_verdict_with_an_unreadable_task_registry() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let database = home.path().join(".lf/loopflow.db");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    fs::write(&database, b"not a compatible Task registry").unwrap();
    let target = home.path().join("lf");
    let output = command(
        home.path(),
        repo.path(),
        &[
            "self",
            "install",
            "promote",
            "--cli-target",
            target.to_str().unwrap(),
        ],
    )
    .output()
    .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("every target is unchanged"), "{error}");
    assert_eq!(
        fs::read(database).unwrap(),
        b"not a compatible Task registry"
    );
    assert!(!target.exists());
}

#[test]
fn machine_commands_and_catalog_work_without_git_or_a_repository() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let no_tools = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/bin/ps", no_tools.path().join("ps")).unwrap();
    for args in [
        vec!["list"],
        vec!["graph", "show", "pursue"],
        vec!["help", "graph", "pursue"],
        vec!["account", "--cached"],
        vec!["account", "--cached", "--details"],
        vec!["account", "route"],
        vec!["account", "route", "--repo", "example/project"],
        vec!["wave", "list", "--json"],
        vec!["monitor", "ps", "--json"],
    ] {
        let output = command(home.path(), cwd.path(), &args)
            .env("PATH", no_tools.path())
            .output()
            .unwrap();
        let stdout = success(output);
        if args == ["list"] {
            assert!(stdout.contains("debug"));
            assert!(stdout.contains("unbreak"));
        }
        if args.get(1) == Some(&"route") {
            assert!(stdout.contains("claude"));
        }
        assert!(
            !cwd.path().join(".lf").exists(),
            "{args:?} wrote repository state"
        );
    }
}

#[test]
fn repository_errors_do_not_prevent_home_command_admission() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".lf")).unwrap();
    SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    let database = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
    let parent = loopflow_test_support::RunningParent::insert(&home.path().join(".lf/loopflow.db"));
    let child = |args: &[&str]| {
        parent
            .adopt(&mut command(home.path(), cwd.path(), args))
            .output()
            .unwrap()
    };
    let output = child(&["sync", "--plan"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Run lf sync from a Git repository"));
    let identity = success(child(&["machine", "id"]));
    assert!(!identity.trim().is_empty());
    let commands: i64 = database
        .query_row(
            "SELECT count(*) FROM processes WHERE parent_lf_process_id=?1",
            [&parent.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        commands, 2,
        "the failed sync and Machine read each own a process"
    );
    let output = command(
        home.path(),
        cwd.path(),
        &["set", "account.claude", "person@example.com"],
    )
    .output()
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--global"));
    assert!(!cwd.path().join(".lf").exists());
}

#[test]
fn global_wave_listing_and_repository_catalog_use_real_checkout_scope() {
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let other = TestRepo::new();
    let linked = repo.create_named_worktree("list");
    let nested = repo.path().join("nested");
    fs::create_dir_all(&nested).unwrap();
    for root in [repo.path(), linked.as_path()] {
        fs::create_dir_all(root.join(".lf/skills")).unwrap();
        fs::write(
            root.join(".lf/skills/checkout-marker.md"),
            "A repository-specific skill.",
        )
        .unwrap();
    }
    fs::create_dir_all(home.path().join(".lf")).unwrap();
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    for (name, root) in [("first", repo.path()), ("second", other.path())] {
        store
            .create_wave(&Wave::new(
                WaveId::new(),
                name.into(),
                root.display().to_string(),
            ))
            .unwrap();
    }
    for (cwd, count, local_catalog) in [
        (outside.path(), 2, false),
        (repo.path(), 1, true),
        (nested.as_path(), 1, true),
        (linked.as_path(), 1, true),
    ] {
        let output = success(
            command(home.path(), cwd, &["wave", "list", "--json"])
                .output()
                .unwrap(),
        );
        let waves: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            waves.as_array().unwrap().len(),
            count,
            "cwd: {}",
            cwd.display()
        );
        let catalog = success(command(home.path(), cwd, &["list"]).output().unwrap());
        assert_eq!(catalog.contains("checkout-marker"), local_catalog);
    }
    assert!(!outside.path().join(".lf").exists());
}

#[test]
fn broken_git_metadata_is_not_treated_as_an_ordinary_folder() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    fs::write(
        cwd.path().join(".git"),
        "gitdir: /nonexistent/loopflow-test-repository",
    )
    .unwrap();
    let output = command(home.path(), cwd.path(), &["list"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot discover Git repository"));
}

#[cfg(target_os = "macos")]
#[test]
fn scheduled_install_is_independent_of_the_invoking_checkout_and_reusable() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    symlink("/usr/bin/id", bin.join("id")).unwrap();
    fs::write(bin.join("launchctl"), "#!/bin/sh\ncase $1 in\nprint) test -f \"$HOME/loaded\";;\nbootstrap) : > \"$HOME/loaded\";;\nbootout) /bin/rm \"$HOME/loaded\";;\nesac\n").unwrap();
    fs::set_permissions(bin.join("launchctl"), fs::Permissions::from_mode(0o755)).unwrap();
    let run = |args: &[&str]| {
        success(
            command(home.path(), cwd.path(), &["self", "install", "schedule"])
                .args(args)
                .env("PATH", &bin)
                .env("LF_INSTALL_DIR", home.path().join("installed & current"))
                .output()
                .unwrap(),
        )
    };
    assert!(run(&[]).contains("weekly"));
    let path = home
        .path()
        .join("Library/LaunchAgents/com.loopflow.refresh.plist");
    let read_plist = || -> serde_json::Value {
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&path)
            .output()
            .unwrap();
        serde_json::from_str(&success(output)).unwrap()
    };
    let plist = read_plist();
    assert_eq!(plist["ProgramArguments"][1], "install");
    assert_eq!(plist["ProgramArguments"].as_array().unwrap().len(), 2);
    assert_eq!(
        plist["EnvironmentVariables"]["LF_INSTALL_DIR"],
        home.path().join("installed & current").to_str().unwrap()
    );
    assert!(plist.get("WorkingDirectory").is_none());
    assert_eq!(plist["RunAtLoad"], true);
    assert_eq!(
        plist["StartCalendarInterval"],
        serde_json::json!({"Weekday": 1, "Hour": 9, "Minute": 0})
    );
    assert!(home.path().join("loaded").exists());
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    run(&[]);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    fs::remove_file(home.path().join("loaded")).unwrap();
    run(&[]);
    assert!(home.path().join("loaded").exists());

    for (frequency, expected) in [
        ("daily", serde_json::json!({"Hour": 9, "Minute": 0})),
        ("hourly", serde_json::json!({"Minute": 0})),
        (
            "5min",
            serde_json::json!([
                {"Minute": 0}, {"Minute": 5}, {"Minute": 10}, {"Minute": 15},
                {"Minute": 20}, {"Minute": 25}, {"Minute": 30}, {"Minute": 35},
                {"Minute": 40}, {"Minute": 45}, {"Minute": 50}, {"Minute": 55}
            ]),
        ),
        (
            "weekly",
            serde_json::json!({"Weekday": 1, "Hour": 9, "Minute": 0}),
        ),
    ] {
        run(&[frequency]);
        assert_eq!(read_plist()["StartCalendarInterval"], expected);
        assert!(home.path().join("loaded").exists());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        run(&[frequency]);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    }
    let before = fs::read(&path).unwrap();
    let output = command(
        home.path(),
        cwd.path(),
        &["self", "install", "schedule", "monthly"],
    )
    .env("PATH", &bin)
    .output()
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn context_explanation_works_outside_git_and_preserves_unavailable_registry() {
    let home = tempfile::tempdir().unwrap();
    let output = success(
        command(home.path(), home.path(), &["--explain", "--json"])
            .output()
            .unwrap(),
    );
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["machine"]["state"], "unavailable");
    assert_eq!(report["task"]["state"], "unavailable");
    assert!(!home.path().join(".lf").exists());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_rejects_linux_before_machine_routing_or_work_preparation() {
    let home = tempfile::tempdir().unwrap();
    for operation in [
        "state", "ls", "actions", "events", "wait", "back", "hide", "show", "focus", "move",
        "resize", "zoom", "run", "capture", "send", "send-key",
    ] {
        let output = command(
            home.path(),
            home.path(),
            &[
                "--machine",
                "unregistered",
                "--task",
                "absent",
                "desktop",
                operation,
            ],
        )
        .args(match operation {
            "state" | "ls" | "actions" | "events" | "back" => vec!["--json"],
            "zoom" => vec!["-p", "absent", "--on"],
            "send" => vec!["-p", "absent", "literal"],
            "send-key" => vec!["-p", "absent", "enter"],
            "move" => vec![
                "-p",
                "absent",
                "--destination",
                "absent",
                "--axis",
                "horizontal",
            ],
            "resize" => vec!["-p", "absent", "--toward", "absent", "--ratio", "0.6"],
            _ => vec!["-p", "absent"],
        })
        .output()
        .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("require macOS"), "{error}");
        assert!(error.contains("lf task status"), "{error}");
        assert!(!home.path().join(".lf/loopflow.db").exists());
    }
}

#[test]
fn repo_selection_uses_machine_root_or_explicit_checkout_without_registration() {
    use std::os::unix::fs::symlink;
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    fs::create_dir_all(home.path().join("src")).unwrap();
    symlink(repo.path(), home.path().join("src/example")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::write(
        repo.path().join(".lf/skills/selected-marker.md"),
        "Selected checkout",
    )
    .unwrap();
    // A repository cannot change the root that selected it.
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "repo_root: /unavailable\n",
    )
    .unwrap();
    for (cwd, selector) in [
        (outside.path(), "example"),
        (outside.path(), repo.path().to_str().unwrap()),
        (home.path(), "./src/example"),
        (outside.path(), "~/src/example"),
    ] {
        let output = success(
            command(home.path(), cwd, &["--repo", selector, "list"])
                .output()
                .unwrap(),
        );
        assert!(output.contains("selected-marker"), "{selector}: {output}");
    }
    assert!(!home.path().join(".lf/loopflow.db").exists());
    fs::create_dir_all(home.path().join(".lf")).unwrap();
    fs::create_dir_all(home.path().join("custom")).unwrap();
    symlink(repo.path(), home.path().join("custom/override")).unwrap();
    fs::write(home.path().join(".lf/config.yaml"), "repo_root: ~/custom\n").unwrap();
    let output = success(
        command(home.path(), outside.path(), &["--repo", "override", "list"])
            .output()
            .unwrap(),
    );
    assert!(output.contains("selected-marker"));
    assert!(!home.path().join(".lf/loopflow.db").exists());
}

#[test]
fn repo_selection_reports_resolved_missing_or_non_git_path_before_effects() {
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join("src/empty")).unwrap();
    for name in ["absent", "empty"] {
        let output = command(
            home.path(),
            outside.path(),
            &[
                "--repo",
                name,
                "task",
                "create",
                "--title",
                "Must not create",
            ],
        )
        .output()
        .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(home.path().join("src").join(name).to_str().unwrap()));
        assert!(!home.path().join(".lf/loopflow.db").exists());
    }
}

#[test]
fn repo_selection_rejects_another_tasks_identity_before_edit_or_checkout() {
    let home = tempfile::tempdir().unwrap();
    let selected = TestRepo::new();
    let other = TestRepo::new();
    let created: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            other.path(),
            &["task", "create", "--title", "Keep original", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = created["id"].as_str().unwrap();
    for args in [
        vec![
            "--repo",
            selected.path().to_str().unwrap(),
            "task",
            "edit",
            id,
            "--title",
            "Wrong repository",
        ],
        vec![
            "--repo",
            selected.path().to_str().unwrap(),
            "task",
            "checkout",
            id,
        ],
        vec![
            "--repo",
            selected.path().to_str().unwrap(),
            "--task",
            id,
            ":",
            "Must not launch",
        ],
    ] {
        let output = command(home.path(), other.path(), &args).output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("does not belong to selected repository"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let status = success(
        command(home.path(), other.path(), &["task", "status", id, "--json"])
            .output()
            .unwrap(),
    );
    assert!(status.contains("Keep original"));
    assert!(!status.contains("Wrong repository"));
}

#[test]
fn repo_selection_carries_scoped_task_prefix_through_reads_and_writes() {
    let home = tempfile::tempdir().unwrap();
    let selected = TestRepo::new();
    let other = TestRepo::new();
    let first = "task_abcd1234400080000000000000000001";
    let second = "task_abcd1235400080000000000000000002";
    for (repo, id, title) in [
        (selected.path(), first, "Selected"),
        (other.path(), second, "Other"),
    ] {
        success(
            command(home.path(), repo, &["wave", "ensure", "inbox", "--json"])
                .output()
                .unwrap(),
        );
        let path = home.path().join(".lf/loopflow.db");
        let store = SqliteStore::new(&path).unwrap();
        let conn = rusqlite::Connection::open(path).unwrap();
        let project: String = conn
            .query_row(
                "SELECT current_project_id FROM waves WHERE repo=?1 AND name='inbox'",
                [repo.canonicalize().unwrap().to_str().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        store
            .create_task(&loopflow::work::task::NewTask {
                repository_id: store
                    .ensure_repository(repo.canonicalize().unwrap().to_str().unwrap())
                    .unwrap(),
                due_date: None,
                id: loopflow::durable::TaskId::parse(id).unwrap(),
                project_id: Some(loopflow::durable::ProjectId::parse(&project).unwrap()),
                title: title.into(),
                description: String::new(),
            })
            .unwrap();
    }
    // Checkout inference and explicit repository selection resolve the same Task.
    let inferred: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            selected.path(),
            &["task", "status", "abcd", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(inferred["execution"]["task_id"], first);
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    assert!(store
        .task_by_issue("abcd")
        .unwrap_err()
        .to_string()
        .contains("multiple stable Tasks"));
    let repository = store
        .ensure_repository(selected.path().canonicalize().unwrap().to_str().unwrap())
        .unwrap();
    for (flag, value) in [
        ("--repo", selected.path().to_str().unwrap()),
        ("--repository", repository.as_str()),
    ] {
        let status: serde_json::Value = serde_json::from_str(&success(
            command(
                home.path(),
                other.path(),
                &[flag, value, "task", "status", "abcd", "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(status["execution"]["task_id"], first);
        success(
            command(
                home.path(),
                other.path(),
                &[
                    flag,
                    value,
                    "task",
                    "edit",
                    "abcd",
                    "--title",
                    "Changed selected",
                ],
            )
            .output()
            .unwrap(),
        );
        // The launch front door must resolve the same ID before discovering the
        // missing checkout, rather than failing global prefix resolution.
        let launch = command(
            home.path(),
            other.path(),
            &[flag, value, "--task", "abcd", ":", "Do not launch"],
        )
        .output()
        .unwrap();
        assert!(!launch.status.success());
        let error = String::from_utf8_lossy(&launch.stderr);
        assert!(error.contains("has no checkout"), "{error}");
        assert!(!error.contains("multiple stable Tasks"), "{error}");
    }
    assert_eq!(
        store.task_by_issue(first).unwrap().unwrap().title,
        "Changed selected"
    );
    let untouched = store.task_by_issue(second).unwrap().unwrap();
    assert_eq!(untouched.title, "Other");
    assert!(untouched.worktree.is_none());
    success(
        command(
            home.path(),
            other.path(),
            &[
                "--repo",
                selected.path().to_str().unwrap(),
                "task",
                "checkout",
                "abcd",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let before = fs::read(&path).unwrap();
    let output = success(
        command(
            home.path(),
            other.path(),
            &[
                "--repo",
                selected.path().to_str().unwrap(),
                "--task",
                "abcd",
                "--context",
                "--explain",
                "--json",
                ":",
                "Preview seed only",
            ],
        )
        .output()
        .unwrap(),
    );
    let preview: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(preview["resolution"]["task"]["value"], first);
    assert!(preview["input"]["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["content"]
            .as_str()
            .unwrap()
            .contains("Changed selected")));
    assert!(
        fs::read(path).unwrap() == before,
        "preview changed database bytes"
    );
}

#[test]
fn invocation_previews_preserve_absent_storage_and_reject_unsupported_commands() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    repo.create_file(".lf/skills/probe.md", "Inspect preview marker.");
    repo.create_file(
        ".lf/config.yaml",
        "diff: false\ndiff_files: false\npaste: false\ncontext_budgets:\n  memory_tokens: 300\n",
    );
    let message = "Long supplied direction. ".repeat(500);
    let output = success(
        command(
            home.path(),
            repo.path(),
            &["skill", "probe", &message, "--context", "--json"],
        )
        .output()
        .unwrap(),
    );
    let input: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(input["task_prompt"]
        .as_str()
        .unwrap()
        .contains("Inspect preview marker."));
    assert!(input["task_prompt"].as_str().unwrap().contains(&message));
    assert!(!repo.path().join(".lf/prompts").exists());
    for args in [
        vec!["probe", &message, "--context", "--json"],
        vec!["probe:", &message, "--context", "--json"],
        vec!["skill", "probe:", &message, "--context", "--json"],
    ] {
        let equivalent: serde_json::Value = serde_json::from_str(&success(
            command(home.path(), repo.path(), &args).output().unwrap(),
        ))
        .unwrap();
        for field in [
            "system_prompt",
            "task_prompt",
            "skill_invocation",
            "references",
        ] {
            assert_eq!(input[field], equivalent[field], "{args:?}: {field}");
        }
    }
    assert!(!repo.path().join(".lf/tmp").exists());
    assert!(!home.path().join(".lf").exists());
    for args in [
        vec!["--context", "task", "create", "--title", "Never created"],
        vec!["--explain", "skill", ":"],
        vec!["--explain", "commit", "-m", "Never committed"],
        vec![
            "--machine",
            "unconfigured",
            "--explain",
            "task",
            "run",
            "abcd",
        ],
    ] {
        let result = command(home.path(), repo.path(), &args).output().unwrap();
        assert!(!result.status.success(), "{args:?}");
    }
    assert!(!home.path().join(".lf").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn non_agent_context_previews_reject_before_reading_stdin() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let input = tempfile::NamedTempFile::new().unwrap();
    fs::write(input.path(), [0xff]).unwrap();
    for args in [
        vec!["task", "create", "--title", "Never created"],
        vec!["task", "save", "unknown", "note.md", "--revision", "old"],
        vec!["task", "run", "unknown"],
        vec!["open"],
        vec!["session", "connect", "unknown"],
    ] {
        let output = command(home.path(), repo.path(), &args)
            .arg("--context")
            .stdin(fs::File::open(input.path()).unwrap())
            .output()
            .unwrap();
        assert!(!output.status.success(), "{args:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("--context requires"), "{args:?}: {error}");
    }
    assert!(!home.path().join(".lf").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn task_run_explain_reads_unstarted_work_without_preparing_it() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let created: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "create",
                "--title",
                "Explain without starting",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = created["id"].as_str().unwrap();
    let path = home.path().join(".lf/loopflow.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(conn);
    let before = fs::read(&path).unwrap();
    let output = success(
        command(
            home.path(),
            repo.path(),
            &["task", "run", id, "--explain", "--json"],
        )
        .output()
        .unwrap(),
    );
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["resolution"]["task"]["value"], id);
    assert_eq!(
        report["resolution"]["execution_machine"]["source"],
        "owning_machine:this_machine"
    );
    assert!(report["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("Future checkout")));
    let inherited: serde_json::Value = serde_json::from_str(&success(
        command(home.path(), repo.path(), &["--explain", "--json"])
            .env("LF_AS", format!("task:{id}"))
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(inherited["task"]["value"], id);
    assert_eq!(inherited["task"]["source"], "inherited_declaration");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    assert!(
        fs::read(path).unwrap() == before,
        "preview changed database bytes"
    );
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    assert!(store.task_by_issue(id).unwrap().unwrap().worktree.is_none());
}

#[test]
fn open_explain_reads_a_plain_repository_without_initializing_or_launching() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let run = |args: &[&str]| command(home.path(), repo.path(), args).output().unwrap();
    let plain: serde_json::Value =
        serde_json::from_str(&success(run(&["open", "--explain", "--json"]))).unwrap();
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    assert_eq!(plain["links"][0]["url"], format!("loopflow://{folder}"));
    assert!(plain["links"][0]["target"]["repository_id"].is_null());
    let invalid = run(&["open", "--diff", "--explain", "--json"]);
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("--diff needs a Task"));
    assert!(success(run(&["open", "--explain"])).contains("Nothing was executed"));
    assert!(!home.path().join(".lf").exists());
    assert!(!repo.path().join(".lf/tmp").exists());

    // An unreadable registry is not an unregistered repository to initialize.
    fs::create_dir(home.path().join(".lf")).unwrap();
    let database = home.path().join(".lf/loopflow.db");
    fs::write(&database, "not SQLite").unwrap();
    assert!(!run(&["open", "--explain", "--json"]).status.success());
    assert_eq!(fs::read_to_string(database).unwrap(), "not SQLite");
}

#[test]
fn open_explain_preserves_state_and_matches_explicit_and_inferred_work() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("open-explain");
    support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "open-explain",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let run = |args: &[&str]| command(home.path(), repo.path(), args).output().unwrap();
    let url = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&success(run(args))).unwrap()["links"][0]["url"]
            .clone()
    };
    let explicit = url(&["--task", "INF-123", "open", "--diff", "--explain", "--json"]);
    let inferred = url(&["open", "--diff", "--explain", "--json"]);
    assert_eq!(explicit, inferred);
    let folder = repo.path().file_name().unwrap().to_str().unwrap();
    assert_eq!(
        explicit,
        format!("loopflow://{folder}/task/INF-123?diff=true")
    );
    assert!(
        !run(&["open", "--session", "missing", "--explain", "--json"])
            .status
            .success()
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(
        fs::read(path).unwrap(),
        before,
        "explanation changed the registry"
    );
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn open_delivers_exact_task_to_desktop_without_preparing_its_execution() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let created: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "create", "--title", "Open only", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = created["id"].as_str().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let received = home.path().join("opening");
    fs::write(
        bin.join("open"),
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HOME/opening\"\n",
    )
    .unwrap();
    fs::set_permissions(bin.join("open"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = |args: &[&str]| {
        let output = command(home.path(), repo.path(), args)
            .env("PATH", &path)
            .output()
            .unwrap();
        serde_json::from_str::<serde_json::Value>(&success(output)).unwrap()
    };
    let preview = run(&["--task", id, "open", "--explain", "--json"]);
    assert!(!received.exists(), "preview launched the app");
    let report = run(&["--task", id, "open", "--host", "desktop", "--json"]);
    let receipt = &report["receipts"][0];
    assert_eq!(receipt["status"], "opening");
    assert_eq!(receipt["host"], "desktop");
    assert_eq!(preview["links"][0]["url"], receipt["url"]);
    let url = receipt["url"].as_str().unwrap();
    assert!(url.ends_with(&format!(
        "/task/{}",
        created["identifier"].as_str().unwrap()
    )));
    assert!(fs::read_to_string(received)
        .unwrap()
        .ends_with(&format!("{url}\n")));
    let conn = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
    let worktree: Option<String> = conn
        .query_row("SELECT worktree FROM tasks WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(worktree.is_none());
}

#[test]
fn task_run_explain_explicit_and_checkout_inferred_actions_preserve_all_state() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("explain-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "explain-proof",
        &repo.head_sha(),
    );
    // A captured graph wins over the Project's feature selection.
    let graph = serde_json::json!({"name": "captured", "steps": [{"Skill":{"skill":{"name":"demo"},"id":"review","interactive":true,"returns":null,"sources":[]}}]});
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'review',1)",
        rusqlite::params![registered.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    support::record_graph(
        &home.path().join(".lf"),
        repo.path(),
        "prior",
        "prior-step",
        "succeeded",
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let explicit = read(&["task", "run", "INF-123", "--explain", "--json"]);
    let inferred = read(&["task", "run", "--explain", "--json"]);
    let scoped = read(&["--task", "INF-123", "task", "run", "--explain", "--json"]);
    assert_eq!(
        explicit["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert_eq!(inferred["resolution"]["task"]["source"], "checkout");
    assert_eq!(explicit["resolution"]["task"]["source"], "explicit");
    for other in [&inferred, &scoped] {
        assert_eq!(explicit["action"], other["action"]);
        assert_eq!(explicit["impediments"], other["impediments"]);
        assert_eq!(
            explicit["resolution"]["execution_machine"],
            other["resolution"]["execution_machine"]
        );
    }
    assert_eq!(
        explicit["action"],
        serde_json::json!({"kind":"end", "graph":"captured", "take_up":false})
    );
    assert_eq!(
        explicit["resolution"]["execution_machine"]["source"],
        "owner"
    );
    let invalid = read(&[
        "task",
        "run",
        "INF-123",
        "nonexistent",
        "--explain",
        "--json",
    ]);
    assert!(invalid["action"].is_null());
    assert_eq!(invalid["resolution"]["wave"]["state"], "bound", "{invalid}");
    assert_eq!(
        invalid["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert!(
        invalid["impediments"][0]
            .as_str()
            .unwrap()
            .contains("graph \"nonexistent\" is unavailable"),
        "{invalid}"
    );
    let text = success(
        command(home.path(), repo.path(), &["task", "run", "--explain"])
            .output()
            .unwrap(),
    );
    assert!(
        text.contains("graph captured: nothing remains; reach end"),
        "{text}"
    );
    assert!(text.contains("Nothing was executed"));
    let options = [
        ("--directive", " ", "directive cannot be empty"),
        ("--directive", "new direction", "already exists"),
        ("--name", "other-workspace", "already uses workspace name"),
        ("--name", "invalid/name", "workspace name"),
    ];
    let mut impediments = Vec::new();
    for (flag, value, expected) in options {
        let report = read(&["task", "run", "INF-123", flag, value, "--explain", "--json"]);
        let reason = report["impediments"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|reason| reason.as_str())
            .find(|reason| reason.contains(expected))
            .unwrap_or_else(|| panic!("missing {expected}: {report}"));
        impediments.push(reason.to_owned());
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(path).unwrap() == before,
        "preview changed database bytes"
    );
    // Launch rejects the same options before checkout/graph preparation.
    // Its ordinary Process observation is allowed, unlike the previews above.
    for ((flag, value, _), reason) in options.into_iter().zip(impediments) {
        let output = command(
            home.path(),
            repo.path(),
            &["task", "run", "INF-123", flag, value],
        )
        .output()
        .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(&reason),
            "preview: {reason}; launch: {error}"
        );
    }
}

#[test]
fn task_run_explain_preserves_absent_and_unreadable_registries() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let path = home.path().join(".lf/loopflow.db");
    for corrupt in [false, true] {
        if corrupt {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "not a database").unwrap();
        }
        let report: serde_json::Value = serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &["task", "run", "UNKNOWN-1", "--explain", "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap();
        assert!(report["action"].is_null());
        assert_eq!(report["resolution"]["task"]["state"], "unavailable");
        assert!(!report["unavailable"].as_array().unwrap().is_empty());
        if corrupt {
            assert_eq!(fs::read(&path).unwrap(), b"not a database");
        } else {
            assert!(!path.exists());
        }
    }
}

fn connection_session(home: &Path, cwd: &Path) -> loopflow::session::LfSession {
    let store = SqliteStore::new(&home.join(".lf/loopflow.db")).unwrap();
    store
        .create_session(
            loopflow::session::LfSession {
                captured: None,
                caller_artifact_key: None,
                task_id: None,
                wave_id: None,
                graph_lf_process_id: None,
                work_source: None,
                bound_at: None,
                id: "connection-proof".into(),
                artifact_key: Some(uuid::Uuid::new_v4().simple().to_string()),
                input_published: true,
                cwd: cwd.to_path_buf(),
                skill: None,
                provider: Some("opencode".into()),
                model: None,
                node: None,
                iterations: None,
                interactive: true,
                repo: None,
                title: "Connection proof".into(),
                title_source: loopflow::session::TitleSource::Human,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
        )
        .unwrap()
}

fn connection_history(db: &rusqlite::Connection, session: &loopflow::session::LfSession) {
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'observed',?2,1,?3)",
        rusqlite::params![session.id, format!("{}:provider-session:proof", session.artifact_key.as_ref().unwrap()),
        serde_json::json!({"source":"provider-session:proof","evidence":{"schema_version":1,"provider_session_id":"native-proof","account_id":null}}).to_string()]).unwrap();
}

#[test]
fn session_connect_explain_shares_explicit_native_and_inferred_selection_without_effects() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let session = connection_session(home.path(), repo.path());
    let db_path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    connection_history(&db, &session);
    db.execute(
        "UPDATE agent_sessions SET primary_scope='repository' WHERE id=?1",
        [&session.id],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&db_path).unwrap();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let explicit = read(&[
        "session",
        "connect",
        &session.id,
        "--explain",
        "--prepare",
        "--json",
    ]);
    let native = read(&[
        "session",
        "connect",
        "native-proof",
        "--explain",
        "--prepare",
        "--json",
    ]);
    let inferred = read(&["--json", "session", "resume", "--explain"]);
    assert_eq!(
        explicit["resolution"]["session"]["value"],
        session.id.as_str()
    );
    assert_eq!(
        explicit["resolution"]["session"],
        native["resolution"]["session"]
    );
    assert_eq!(
        inferred["resolution"]["session"]["value"],
        session.id.as_str()
    );
    assert_eq!(
        inferred["resolution"]["session"]["source"],
        "contextual_session"
    );
    assert_eq!(explicit["action"]["intent"], "resume");
    assert_eq!(explicit["action"]["prepare_only"], true);
    assert_eq!(inferred["action"]["prepare_only"], false);
    assert!(explicit["impediments"].as_array().unwrap().is_empty());
    // Explaining client state does not prepare launch argv or require a usable
    // child executable. Actual opening still owns that launch validation.
    let missing_bin: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &[
                "session",
                "connect",
                &session.id,
                "--explain",
                "--prepare",
                "--json",
            ],
        )
        .env("LF_BIN", home.path().join("missing-lf"))
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(missing_bin["state"], explicit["state"]);
    assert_eq!(missing_bin["actions"], explicit["actions"]);
    assert_eq!(missing_bin["unavailable"], explicit["unavailable"]);
    let replace = read(&[
        "session",
        "connect",
        &session.id,
        "--takeover",
        "--explain",
        "--prepare",
        "--json",
    ]);
    let try_open = read(&[
        "session",
        "connect",
        &session.id,
        "--try",
        "--explain",
        "--prepare",
        "--json",
    ]);
    assert_eq!(replace["action"]["mode"], "takeover");
    assert_eq!(try_open["action"]["mode"], "try");
    let text = success(
        command(
            home.path(),
            repo.path(),
            &["session", "connect", &session.id, "--takeover", "--explain"],
        )
        .output()
        .unwrap(),
    );
    assert!(text.contains("Resume saved provider history"), "{text}");
    assert!(text.contains("unsent text there is lost"));
    assert!(text.contains("Nothing was executed"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(before, fs::read(db_path).unwrap());
    assert!(!home.path().join(".lf/runs").exists());
    assert!(!home.path().join(".lf/human-sessions").exists());
    let prepared = read(&["session", "connect", "native-proof", "--prepare", "--json"]);
    assert_eq!(prepared["id"], explicit["resolution"]["session"]["value"]);
    assert_eq!(prepared["state"], explicit["state"]);
    assert_eq!(prepared["actions"], explicit["actions"]);
}

#[test]
fn session_connect_explain_preserves_missing_and_stale_evidence() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let read = |id: &str| -> serde_json::Value {
        serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &["session", "connect", id, "--explain", "--prepare", "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap()
    };
    assert!(read("missing")["action"].is_null());
    assert!(!home.path().join(".lf/loopflow.db").exists());
    let session = connection_session(home.path(), repo.path());
    let db_path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&db_path).unwrap();
    let missing = read("unrecorded-native-conversation");
    assert!(missing["action"].is_null());
    assert_eq!(missing["resolution"]["session"]["state"], "unavailable");
    assert!(missing["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("admission"));
    assert_eq!(read(&session.id)["action"]["intent"], "start");
    let clients = home
        .path()
        .join(".lf/runs")
        .join(&session.artifact_key.as_deref().unwrap()[..2])
        .join(session.artifact_key.as_ref().unwrap())
        .join("provider-clients");
    fs::create_dir_all(&clients).unwrap();
    let receipt = clients.join("stale.json");
    fs::write(&receipt, serde_json::json!({"schema_version":1,"pid":std::process::id(),"terminal_id":"stale","started_at":"2000-01-01T00:00:00Z"}).to_string()).unwrap();
    let stale = read(&session.id);
    assert_eq!(stale["state"], "unknown");
    assert!(stale["actions"][0]["unavailable_reason"].is_null());
    fs::write(&receipt, "unreadable receipt").unwrap();
    let invalid = read(&session.id);
    // Native receipt corruption does not replace recorded Process evidence.
    assert_eq!(invalid["state"], "unknown");
    assert_eq!(invalid["actions"], stale["actions"]);
    assert_eq!(fs::read_to_string(&receipt).unwrap(), "unreadable receipt");
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(before, fs::read(db_path).unwrap());
}

#[test]
fn session_connect_explain_observes_takeover_without_signaling_owned_client() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let session = connection_session(home.path(), repo.path());
    let script = home.path().join("opencode");
    fs::write(&script, "#!/bin/sh\nread line\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new(&script)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let clients = home
            .path()
            .join(".lf/runs")
            .join(&session.artifact_key.as_deref().unwrap()[..2])
            .join(session.artifact_key.as_ref().unwrap())
            .join("provider-clients");
        fs::create_dir_all(&clients).unwrap();
        let receipt = clients.join("client.json");
        let bytes = serde_json::json!({"schema_version":1,"pid":child.id(),"terminal_id":"owned","started_at":time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap()}).to_string();
        fs::write(&receipt, &bytes).unwrap();
        let db_path = home.path().join(".lf/loopflow.db");
        let db = rusqlite::Connection::open(&db_path).unwrap();
        connection_history(&db, &session);
        db.execute("INSERT INTO processes(id,kind,trace_id,agent_session_id,pid,os_started_at,started_at) VALUES('00000000000040008000000000000001','agent','fixture',?1,?2,?3,?3)", rusqlite::params![session.id,child.id(),time::OffsetDateTime::now_utc().unix_timestamp()]).unwrap();
        db.execute(
            "UPDATE agent_sessions SET agent_process_id='00000000000040008000000000000001' WHERE id=?1",
            [&session.id],
        )
        .unwrap();
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = fs::read(&db_path).unwrap();
        for flag in [None, Some("--takeover"), Some("--try")] {
            let mut args = vec![
                "session",
                "connect",
                &session.id,
                "--explain",
                "--prepare",
                "--json",
            ];
            args.extend(flag);
            let report: serde_json::Value = serde_json::from_str(&success(
                command(home.path(), repo.path(), &args).output().unwrap(),
            ))
            .unwrap();
            assert_eq!(report["state"], "active", "{report}");
            assert!(
                report["impediments"].as_array().unwrap().is_empty(),
                "{report}"
            );
            assert_eq!(report["actions"][1]["kind"], "move_here");
            assert!(child.try_wait().unwrap().is_none());
            assert_eq!(fs::read_to_string(&receipt).unwrap(), bytes);
        }
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        assert_eq!(before, fs::read(db_path).unwrap());
        assert_eq!(fs::read_dir(clients.parent().unwrap()).unwrap().count(), 1);
    }));
    let _ = child.kill();
    let _ = child.wait();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn session_connect_explain_never_probes_or_claims_a_live_endpoint() {
    use std::os::unix::net::UnixListener;
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let session = connection_session(home.path(), repo.path());
    let db_path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    connection_history(&db, &session);
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let endpoint = socket_dir.path().join("live.sock");
    let listener = UnixListener::bind(&endpoint).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = Command::new("cat")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Owned endpoint with a retained driver: no provider or account is started.
    db.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES('00000000000040008000000000000003','fixture',1)",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO processes(id,kind,trace_id,parent_lf_process_id,agent_session_id,attached_lf_process_id,attachment_token,endpoint,started_at) VALUES('00000000000040008000000000000002','agent','fixture','00000000000040008000000000000003',?1,'00000000000040008000000000000003','claim',?2,1)",
        rusqlite::params![session.id, endpoint.to_str().unwrap()]).unwrap();
    db.execute("UPDATE agent_sessions SET provider='codex',agent_process_id='00000000000040008000000000000002',transport_lf_process_id='00000000000040008000000000000003',provider_thread='native-proof' WHERE id=?1", [&session.id]).unwrap();
    db.execute(
        "UPDATE processes SET pid=?1,os_started_at=?2 WHERE id='00000000000040008000000000000002'",
        rusqlite::params![child.id(), time::OffsetDateTime::now_utc().unix_timestamp()],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&db_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["--json", "session", "connect", &session.id, "--explain"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(report["action"]["intent"], "connect_or_resume");
    assert_eq!(report["action"]["prepare_only"], false);
    assert!(report["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().contains("not probed")));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(before, fs::read(db_path).unwrap());
    assert!(!home.path().join(".lf/runs").exists());
    assert!(!home.path().join(".lf/human-sessions").exists());
    drop(child.stdin.take());
    child.wait().unwrap();
}

#[test]
fn repository_move_keeps_identity_work_and_sibling_checkouts() {
    let source = TestRepo::new();
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let old = root.join("before");
    success(
        Command::new("git")
            .args([
                "clone",
                source.path().to_str().unwrap(),
                old.to_str().unwrap(),
            ])
            .output()
            .unwrap(),
    );
    let sibling = root.join("before.work");
    success(
        Command::new("git")
            .current_dir(&old)
            .args(["worktree", "add", "-b", "work", sibling.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    fs::write(sibling.join("unfinished.txt"), "preserve me").unwrap();
    let nested = old.join("nested");
    success(
        Command::new("git")
            .current_dir(&old)
            .args(["worktree", "add", "-b", "nested", nested.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    fs::write(nested.join("unfinished.txt"), "nested edits").unwrap();
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    let wave = store
        .ensure_wave(old.to_str().unwrap(), "retained")
        .unwrap();
    let identity = |path: &Path| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), path, &["config", "repository", "--json"])
                .output()
                .unwrap(),
        ))
        .unwrap()
    };
    let before = identity(&old);
    let session: serde_json::Value = serde_json::from_str(&success(
        command(home.path(), &old, &["connect", "--prepare", "--json"])
            .output()
            .unwrap(),
    ))
    .unwrap();
    let moved = root.join("after");
    fs::rename(&old, &moved).unwrap();
    let after = identity(&moved);
    assert_eq!(before, after);
    assert_eq!(
        store.get_wave(&wave).unwrap().unwrap().repo(),
        moved.to_str().unwrap()
    );
    assert_eq!(identity(&sibling), after);
    assert_eq!(identity(&moved.join("nested")), after);
    assert_eq!(
        fs::read_to_string(moved.join("nested/unfinished.txt")).unwrap(),
        "nested edits"
    );
    assert_eq!(identity(&moved), after);
    let resumed: serde_json::Value = serde_json::from_str(&success(
        command(home.path(), &moved, &["connect", "--prepare", "--json"])
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(session["id"], resumed["id"]);
    assert!(session["id"].as_str().is_some());
    assert_eq!(resumed["cwd"], session["cwd"]);
    assert_eq!(
        fs::read_to_string(sibling.join("unfinished.txt")).unwrap(),
        "preserve me"
    );

    // Reusing the old name and remote creates a distinct repository.
    success(
        Command::new("git")
            .args([
                "clone",
                source.path().to_str().unwrap(),
                old.to_str().unwrap(),
            ])
            .output()
            .unwrap(),
    );
    assert_ne!(identity(&old)["id"], after["id"]);
    assert_eq!(identity(&moved), after);
    assert_eq!(
        store.list_waves(Some(old.to_str().unwrap())).unwrap().len(),
        0
    );
}

#[test]
fn repository_recovery_refuses_copies_and_reuses_verified_legacy_identity() {
    let original = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    let old = original.path().canonicalize().unwrap();
    let id = store.ensure_repository(old.to_str().unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let copy = root.path().join("copy");
    success(
        Command::new("cp")
            .args(["-R", old.to_str().unwrap(), copy.to_str().unwrap()])
            .output()
            .unwrap(),
    );
    let rejected = command(home.path(), &copy, &["config", "repository", "--json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(store.repository_path(&id).unwrap().as_deref(), old.to_str());

    // A legacy registration has no physical locator. Recovery is explicit;
    // basename or remote equality cannot silently merge it with another clone.
    fs::remove_file(
        old.join(".git/loopflow")
            .join(store.local_machine().unwrap().id.as_str())
            .join("loopflow-repository"),
    )
    .unwrap();
    let moved = root.path().join("moved");
    fs::rename(&old, &moved).unwrap();
    let recovered: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            &moved,
            &["config", "repository", "--relocate", id.as_str(), "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(recovered["id"], id.as_str());
    let repeated = success(
        command(
            home.path(),
            &moved,
            &["config", "repository", "--relocate", id.as_str()],
        )
        .output()
        .unwrap(),
    );
    assert_eq!(repeated.trim(), id.as_str());
}

#[test]
fn repository_identity_binding_is_explicit_and_does_not_select_planning() {
    let source = TestRepo::new();
    let target = TestRepo::new();
    let source_home = tempfile::tempdir().unwrap();
    let target_home = tempfile::tempdir().unwrap();
    for home in [source_home.path(), target_home.path()] {
        fs::create_dir_all(home.join(".lf")).unwrap();
        SqliteStore::new(&home.join(".lf/loopflow.db")).unwrap();
    }
    let identity: loopflow::durable::RepositoryIdentity = serde_json::from_str(&success(
        command(
            source_home.path(),
            source.path(),
            &["config", "repository", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = identity.id.to_string();
    assert_eq!(identity.locators, vec![identity.id.clone()]);
    let bound: loopflow::durable::RepositoryIdentity = serde_json::from_str(&success(
        command(
            target_home.path(),
            target.path(),
            &["config", "repository", "--bind", &id, "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(bound.id, identity.id);
    assert_eq!(bound.locators, vec![identity.id]);
    let store = SqliteStore::new(&target_home.path().join(".lf/loopflow.db")).unwrap();
    let repo = target
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(store.peer_planning_status(&repo, None).unwrap().is_empty());
    assert!(store.list_waves(None).unwrap().is_empty());
    let task: serde_json::Value = serde_json::from_str(&success(
        command(
            target_home.path(),
            target.path(),
            &[
                "task",
                "create",
                "--title",
                "Retain through association",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let planning_before =
        serde_json::to_value(store.peer_planning_status(&repo, None).unwrap()).unwrap();
    let other = loopflow::durable::RepositoryId::new();
    let rebound = command(
        target_home.path(),
        target.path(),
        &["config", "repository", "--bind", other.as_str()],
    )
    .output()
    .unwrap();
    assert_eq!(success(rebound).trim(), other.as_str());
    assert_eq!(store.repository_id(&repo).unwrap(), Some(other));
    let prior = loopflow::durable::RepositoryId::parse(&id).unwrap();
    assert_eq!(store.repository_path(&prior).unwrap(), Some(repo.clone()));
    let through_prior: loopflow::durable::RepositoryIdentity = serde_json::from_str(&success(
        command(
            target_home.path(),
            target.path(),
            &["--repository", &id, "config", "repository", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        through_prior.id.as_str(),
        store.repository_id(&repo).unwrap().unwrap().as_str()
    );
    assert!(through_prior.locators.contains(&prior));
    assert!(through_prior.locators.contains(&through_prior.id));
    assert_eq!(through_prior.locators.len(), 2);
    let explained = success(
        command(
            target_home.path(),
            source.path(),
            &[
                "--repository",
                &id,
                "task",
                "run",
                task["id"].as_str().unwrap(),
                "--explain",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    );
    assert!(explained.contains(task["id"].as_str().unwrap()));
    assert!(explained.contains(through_prior.id.as_str()));
    assert_eq!(
        serde_json::to_value(store.peer_planning_status(&repo, None).unwrap()).unwrap(),
        planning_before
    );
}

#[test]
fn task_run_from_explain_reads_the_captured_graph_without_moving_the_task() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("from-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "from-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    let graph = serde_json::json!({"name": "captured", "steps": [
        {"Skill":{"skill":{"name":"implement"},"id":null,"interactive":false,"returns":null,"sources":[]}},
        {"Skill":{"skill":{"name":"demo"},"id":"review","interactive":true,"returns":null,"sources":[]}}
    ]});
    db.execute(
        "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'end',1)",
        rusqlite::params![registered.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    // An interactive step: the Task would wait there, running nothing.
    let review = read(&[
        "task",
        "run",
        "INF-123",
        "--from",
        "review",
        "--explain",
        "--json",
    ]);
    assert_eq!(
        review["action"],
        serde_json::json!({"kind":"wait","graph":"captured","take_up":false,"step":"review"})
    );
    assert_eq!(review["impediments"], serde_json::json!([]));
    let scoped = read(&[
        "--task",
        registered.task.id.as_str(),
        "task",
        "run",
        "--from",
        "review",
        "--explain",
        "--json",
    ]);
    assert_eq!(scoped["action"], review["action"]);
    // A step that runs: the run would stop before review; `-b` runs past it.
    let implement = read(&[
        "task",
        "run",
        "INF-123",
        "--from",
        "implement",
        "--explain",
        "--json",
    ]);
    assert_eq!(
        implement["action"],
        serde_json::json!({"kind":"run","graph":"captured","take_up":false,"from":"implement","stops_at":"review"})
    );
    let through = read(&[
        "-b",
        "task",
        "run",
        "INF-123",
        "--from",
        "review",
        "--explain",
        "--json",
    ]);
    assert_eq!(through["action"]["kind"], "run");
    assert!(through["action"]["stops_at"].is_null(), "{through}");
    // At end a bare run has nothing to do; it points to `--from`.
    let ended = read(&["task", "run", "INF-123", "--explain", "--json"]);
    assert!(ended["action"].is_null());
    assert!(
        ended["impediments"][0]
            .as_str()
            .unwrap()
            .contains("--from <step>"),
        "{ended}"
    );
    let missing = read(&[
        "task",
        "run",
        "INF-123",
        "--from",
        "missing",
        "--explain",
        "--json",
    ]);
    assert!(missing["action"].is_null());
    let reason = missing["impediments"][0].as_str().unwrap().to_owned();
    assert!(reason.contains("has no step \"missing\""), "{missing}");
    let text = success(
        command(
            home.path(),
            repo.path(),
            &["task", "run", "INF-123", "--from", "review", "--explain"],
        )
        .output()
        .unwrap(),
    );
    assert!(
        text.contains("graph captured: wait at review; run nothing"),
        "{text}"
    );
    assert!(text.contains("Nothing was executed"));
    let output = command(
        home.path(),
        repo.path(),
        &["--task", "UNKNOWN-1", "--explain", "task", "run", "INF-123"],
    )
    .output()
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicting Task selections"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed store bytes"
    );
    let output = command(
        home.path(),
        repo.path(),
        &["task", "run", "INF-123", "--from", "missing"],
    )
    .output()
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(&reason));
    // Waiting at an interactive step runs nothing and moves the Task there.
    success(
        command(
            home.path(),
            repo.path(),
            &["task", "run", "INF-123", "--from", "review"],
        )
        .output()
        .unwrap(),
    );
    let step: String = db
        .query_row(
            "SELECT step FROM task_graphs WHERE task_id=?1",
            [registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(step, "review");
}

#[test]
fn task_planning_explain_validates_mutations_without_changing_checkpointed_storage() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("planning-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "planning-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let edit = read(&[
        "task",
        "edit",
        "INF-123",
        "--title",
        "New title",
        "--unassign",
        "--rank",
        "2",
        "--explain",
        "--json",
    ]);
    assert_eq!(
        edit["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert_eq!(edit["action"]["kind"], "edit");
    assert_eq!(
        edit["action"]["fields"],
        serde_json::json!(["title", "rank", "assignee"])
    );
    assert_eq!(edit["impediments"], serde_json::json!([]));
    let text = success(
        command(
            home.path(),
            repo.path(),
            &["task", "edit", "INF-123", "--notes", "", "--explain"],
        )
        .output()
        .unwrap(),
    );
    assert!(text.contains("Intended action: edit description"));
    assert!(text.contains("Effect: Save changed fields"));
    for (args, expected) in [
        (
            vec!["task", "edit", "INF-123", "--explain", "--json"],
            "requires --title",
        ),
        (
            vec![
                "task",
                "edit",
                "INF-123",
                "--title",
                "  ",
                "--explain",
                "--json",
            ],
            "title cannot be empty",
        ),
        (
            vec!["task", "comment", "INF-123", "  ", "--explain", "--json"],
            "comment cannot be empty",
        ),
    ] {
        let refused = read(&args);
        assert!(refused["action"].is_null());
        assert!(
            refused["impediments"][0]
                .as_str()
                .unwrap()
                .contains(expected),
            "{refused}"
        );
    }
    let comment = read(&[
        "task",
        "comment",
        "INF-123",
        "  Keep the draft  ",
        "--steer",
        "--explain",
        "--json",
    ]);
    assert_eq!(
        comment["action"],
        serde_json::json!({"kind":"comment", "message":"Keep the draft", "steer":true, "refresh":false})
    );
    let thread = read(&["task", "comment", "INF-123", "--explain", "--json"]);
    assert!(thread["action"]["message"].is_null());
    assert_eq!(thread["action"]["refresh"], true);
    assert!(thread["effects"][0]
        .as_str()
        .unwrap()
        .contains("Refresh Linear"));
    let created = read(&[
        "task",
        "create",
        "--wave",
        "task-pr-tests",
        "--title",
        "  A new Task  ",
        "--explain",
        "--json",
    ]);
    assert_eq!(created["action"]["kind"], "create", "{created}");
    assert_eq!(created["action"]["title"], "A new Task");
    assert_eq!(created["action"]["description"], "A new Task");
    assert_eq!(
        created["action"]["project"],
        registered.task.project_id.as_ref().unwrap().as_str()
    );
    assert_eq!(created["resolution"]["task"]["state"], "unbound");
    assert_eq!(created["impediments"], serde_json::json!([]));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn task_planning_explain_retains_deletion_and_missing_evidence() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let absent = read(&["task", "create", "--title", "New", "--explain", "--json"]);
    assert!(absent["action"].is_null());
    assert!(absent["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("absent"));
    assert!(!home.path().join(".lf/loopflow.db").exists());
    support::bind_task_planning(&repo);
    repo.create_branch("planning-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "planning-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "UPDATE tasks SET planning_deleted_at=1 WHERE id=?1",
        [registered.task.id.as_str()],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    for args in [
        vec![
            "task",
            "edit",
            "INF-123",
            "--title",
            "Changed",
            "--explain",
            "--json",
        ],
        vec!["task", "comment", "INF-123", "Hello", "--explain", "--json"],
    ] {
        let refused = read(&args);
        assert!(refused["action"].is_null());
        assert!(
            refused["impediments"][0]
                .as_str()
                .unwrap()
                .contains("was deleted"),
            "{refused}"
        );
    }
    let missing = read(&[
        "task",
        "edit",
        "INF-999",
        "--title",
        "Changed",
        "--explain",
        "--json",
    ]);
    assert!(missing["action"].is_null());
    assert!(missing["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("no planning acquisition"));
    let unregistered = read(&[
        "task",
        "create",
        "--wave",
        "not-registered",
        "--title",
        "New",
        "--explain",
        "--json",
    ]);
    assert_eq!(unregistered["resolution"]["wave"]["state"], "unavailable");
    assert!(unregistered["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("registration/acquisition was not performed"));
    let report: loopflow::ops::task::TaskPlanningExplanation =
        serde_json::from_value(unregistered).unwrap();
    assert!(report.render().contains("Project selection unavailable"));
    let standalone = read(&[
        "task",
        "create",
        "--title",
        "Standalone Task",
        "--explain",
        "--json",
    ]);
    assert_eq!(standalone["action"]["kind"], "create");
    assert!(standalone["action"]["project"].is_null());
    assert_eq!(standalone["resolution"]["wave"]["state"], "unbound");
    assert_eq!(standalone["impediments"], serde_json::json!([]));
    let invalid = read(&[
        "task",
        "create",
        "--wave",
        "task-pr-tests",
        "--explain",
        "--json",
    ]);
    assert!(invalid["action"].is_null());
    // Invalid creation input must not inherit the deleted Task in this checkout.
    assert_eq!(invalid["resolution"]["wave"]["state"], "unavailable");
    assert!(invalid["resolution"]["wave"]["reason"]
        .as_str()
        .unwrap()
        .contains("not been validated"));
    assert_eq!(invalid["resolution"]["task"]["state"], "unbound");
    assert!(invalid["impediments"][0]
        .as_str()
        .unwrap()
        .contains("title or piped report"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
}

#[test]
fn task_planning_explain_create_uses_piped_input_and_current_project_validation() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("creation-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "creation-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let notes = tempfile::NamedTempFile::new().unwrap();
    fs::write(notes.path(), "  First line\n\nA useful description.\n").unwrap();
    let mut invocation = command(
        home.path(),
        repo.path(),
        &[
            "task",
            "create",
            "--wave",
            "task-pr-tests",
            "--explain",
            "--json",
        ],
    );
    invocation.stdin(fs::File::open(notes.path()).unwrap());
    let preview: serde_json::Value =
        serde_json::from_str(&success(invocation.output().unwrap())).unwrap();
    assert_eq!(preview["action"]["title"], "First line");
    assert_eq!(
        preview["action"]["description"],
        "First line\n\nA useful description."
    );
    assert_eq!(preview["resolution"]["checkout"]["state"], "unbound");
    assert_eq!(
        preview["resolution"]["execution_machine"]["state"],
        "unbound"
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);

    db.execute(
        "UPDATE projects SET status='completed' WHERE id=?1",
        [registered.task.project_id.as_ref().unwrap().as_str()],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let preview: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "create",
                "--wave",
                "task-pr-tests",
                "--title",
                "Not admitted",
                "--explain",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert!(
        preview["impediments"][0]
            .as_str()
            .unwrap()
            .contains("terminal"),
        "{preview}"
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
    // Execution uses the same current-Project refusal, rather than creating in history.
    let refused = command(
        home.path(),
        repo.path(),
        &[
            "task",
            "create",
            "--wave",
            "task-pr-tests",
            "--title",
            "Not admitted",
        ],
    )
    .output()
    .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("terminal"));
    assert_eq!(
        db.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn task_planning_explain_creation_provenance_follows_resolved_selection() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let path = home.path().join(".lf/loopflow.db");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let store = SqliteStore::new(&path).unwrap();
    let canonical = repo.path().canonicalize().unwrap();
    let selected = store
        .ensure_wave_project(canonical.to_str().unwrap(), "selected")
        .unwrap();
    store
        .ensure_wave_project(canonical.to_str().unwrap(), "inbox")
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    for (explicit, no_wave, declaration) in [
        (true, false, "unknown"),
        (false, false, selected.wave_id.as_str()),
        (false, false, "unknown"),
        (false, false, ""),
        (false, false, "  "),
        (false, true, selected.wave_id.as_str()),
        (false, true, "unknown"),
    ] {
        let mut invocation = command(
            home.path(),
            repo.path(),
            &["task", "create", "--title", "New", "--explain", "--json"],
        );
        invocation.env("LF_WAVE_ID", declaration);
        if !declaration.trim().is_empty() {
            invocation.env("LF_AS", format!("wave:{declaration}"));
        }
        if explicit {
            invocation.args(["--wave", "selected"]);
        }
        if no_wave {
            invocation.arg("--no-wave");
        }
        let preview: serde_json::Value =
            serde_json::from_str(&success(invocation.output().unwrap())).unwrap();
        if explicit {
            assert_eq!(
                preview["resolution"]["wave"]["value"],
                selected.wave_id.as_str()
            );
            assert_eq!(preview["resolution"]["wave"]["source"], "explicit");
            assert_eq!(preview["action"]["project"], selected.id.as_str());
        } else {
            assert_eq!(preview["resolution"]["wave"]["state"], "unbound");
            assert!(preview["action"]["project"].is_null());
            let report: loopflow::ops::task::TaskPlanningExplanation =
                serde_json::from_value(preview.clone()).unwrap();
            assert!(report
                .render()
                .contains("create Task \"New\" without a Wave"));
        }
        assert_eq!(preview["resolution"]["task"]["state"], "unbound");
        assert_eq!(preview["impediments"], serde_json::json!([]));
        assert_eq!(
            preview["unavailable"],
            serde_json::json!(["Provider freshness, synchronization delivery and concurrent saves are not observed or reserved"])
        );
        assert_eq!(preview["resolution"]["repository"]["state"], "bound");
        assert_eq!(
            preview["resolution"]["repository_path"]["value"],
            canonical.to_str().unwrap()
        );
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
}

#[test]
fn task_planning_explain_move_resolves_destination_and_retains_recorded_work() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    for wave in ["source", "destination"] {
        success(
            command(
                home.path(),
                repo.path(),
                &["wave", "ensure", wave, "--json"],
            )
            .output()
            .unwrap(),
        );
    }
    // Setup owns these locks; preview must not recreate them.
    fs::remove_dir_all(home.path().join(".lf/chapter-locks")).unwrap();
    let path = home.path().join(".lf/loopflow.db");
    let store = SqliteStore::new(&path).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let project = |wave: &str| -> String {
        db.query_row(
            "SELECT current_project_id FROM waves WHERE name=?1",
            [wave],
            |row| row.get(0),
        )
        .unwrap()
    };
    let source = project("source");
    let destination = project("destination");
    let task = store
        .create_task(&loopflow::work::task::NewTask {
            repository_id: store
                .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
                .unwrap(),
            due_date: None,
            id: loopflow::durable::TaskId::new(),
            project_id: Some(loopflow::durable::ProjectId::parse(&source).unwrap()),
            title: "Unallocated work".into(),
            description: String::new(),
        })
        .unwrap();
    let read = |wave: &str| -> serde_json::Value {
        serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &[
                    "task",
                    "move",
                    task.id.as_str(),
                    "--wave",
                    wave,
                    "--explain",
                    "--json",
                ],
            )
            .output()
            .unwrap(),
        ))
        .unwrap()
    };
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let report = read("destination");
    assert_eq!(report["action"]["kind"], "move", "{report}");
    assert_eq!(report["action"]["project"], destination);
    assert_eq!(report["action"]["previous_project"], source);
    assert_eq!(report["impediments"], serde_json::json!([]), "{report}");
    let destination_wave: String = db
        .query_row("SELECT id FROM waves WHERE name='destination'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(report["action"]["wave"], destination_wave);
    assert_eq!(read(&destination_wave)["action"], report["action"]);
    let absent = read("unregistered");
    assert!(absent["action"].is_null());
    assert!(absent["unavailable"].to_string().contains("not registered"));
    let text = success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "move",
                task.id.as_str(),
                "--wave",
                "destination",
                "--explain",
            ],
        )
        .output()
        .unwrap(),
    );
    assert!(text.contains("Intended action: move Task to Wave"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    // Execution addresses the same durable destination, not a new Wave named after its ID.
    let second = store
        .create_task(&loopflow::work::task::NewTask {
            repository_id: store
                .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
                .unwrap(),
            due_date: None,
            id: loopflow::durable::TaskId::new(),
            project_id: task.project_id.clone(),
            title: "Move by ID".into(),
            description: String::new(),
        })
        .unwrap();
    success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "move",
                second.id.as_str(),
                "--wave",
                &destination_wave,
            ],
        )
        .output()
        .unwrap(),
    );
    let actual: String = db
        .query_row(
            "SELECT project_id FROM tasks WHERE id=?1",
            [second.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(actual, destination);
    let count: i64 = db
        .query_row("SELECT count(*) FROM waves", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);
    fs::remove_dir_all(home.path().join(".lf/chapter-locks")).unwrap();
    // Recorded work stays put while its planning membership can change.
    db.execute(
        "UPDATE tasks SET worktree=?2,checkout_machine_id=?3 WHERE id=?1",
        rusqlite::params![
            task.id.as_str(),
            repo.path().to_str().unwrap(),
            store.local_machine().unwrap().id.as_str()
        ],
    )
    .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let retained = read("destination");
    assert_eq!(retained["action"], report["action"]);
    assert_eq!(retained["impediments"], serde_json::json!([]));
    assert_eq!(read("source")["impediments"], serde_json::json!([]));
    let detached: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "move",
                task.id.as_str(),
                "--no-wave",
                "--explain",
                "--json",
            ],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(detached["action"]["kind"], "move");
    assert_eq!(detached["action"]["previous_project"], source);
    assert!(detached["action"]["project"].is_null());
    assert!(detached["action"]["wave"].is_null());
    assert_eq!(detached["impediments"], serde_json::json!([]));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    let remote = loopflow::durable::MachineId::new();
    store
        .add_machine(&remote, "must-not-contact.invalid", "fixture-peer")
        .unwrap();
    store
        .set_machine_default(
            &loopflow::durable::DefaultScope::Wave(
                loopflow::id::WaveId::parse(&destination_wave).unwrap(),
            ),
            loopflow::durable::MachineSetting::Tasks,
            &remote,
        )
        .unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let remote = read("destination");
    assert_eq!(remote["action"], report["action"]);
    assert_eq!(remote["impediments"], serde_json::json!([]));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
    assert!(!home.path().join(".lf/chapter-locks").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn task_planning_explain_save_validates_draft_without_changing_files_or_storage() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("save-proof");
    let registered = support::register_task_with_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "save-proof",
        &repo.head_sha(),
    );
    fs::write(repo.path().join("note.md"), "Original\r\n").unwrap();
    fs::write(repo.path().join("binary"), [0, 255]).unwrap();
    std::os::unix::fs::symlink("note.md", repo.path().join("link.md")).unwrap();
    let snapshot: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "file", "INF-123", "note.md", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let revision = snapshot["revision"].as_str().unwrap();
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let draft = tempfile::NamedTempFile::new().unwrap();
    let read = |file: &str, revision: &str, content: &str, json: bool| -> String {
        fs::write(draft.path(), content).unwrap();
        let mut args = vec![
            "task",
            "save",
            "INF-123",
            file,
            "--revision",
            revision,
            "--explain",
        ];
        if json {
            args.push("--json");
        }
        success(
            command(home.path(), repo.path(), &args)
                .stdin(fs::File::open(draft.path()).unwrap())
                .output()
                .unwrap(),
        )
    };
    let report: serde_json::Value =
        serde_json::from_str(&read("./note.md", revision, "New 🦀\r\n", true)).unwrap();
    assert_eq!(
        report["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert_eq!(
        report["action"],
        serde_json::json!({"kind":"save", "path":"note.md", "revision":revision, "draft_bytes":10})
    );
    assert_eq!(report["impediments"], serde_json::json!([]), "{report}");
    assert!(read("note.md", revision, "Draft", false).contains("Intended action: save \"note.md\""));
    for (file, rev, content) in [
        ("../escape", revision, "Draft"),
        (".git/config", revision, "Draft"),
        ("note.md", "stale", "Draft"),
        ("note.md", revision, "Bad\0draft"),
        ("link.md", revision, "Draft"),
        ("binary", revision, "Draft"),
        ("missing", revision, "Draft"),
    ] {
        let refused: serde_json::Value =
            serde_json::from_str(&read(file, rev, content, true)).unwrap();
        assert!(
            !refused["impediments"].as_array().unwrap().is_empty(),
            "{refused}"
        );
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    assert_eq!(
        fs::read(repo.path().join("note.md")).unwrap(),
        b"Original\r\n"
    );
    assert_eq!(fs::read(repo.path().join("binary")).unwrap(), [0, 255]);
    assert!(!repo.path().join(".git/loopflow-file-recovery").exists());
    assert!(!repo.path().join(".lf/tmp").exists());
}

#[test]
fn task_checkout_explain_reuses_explicit_and_inferred_work_without_effects() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("checkout-proof");
    let registered = support::register_task_without_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "checkout-proof",
        &repo.head_sha(),
    );
    repo.create_file("draft.txt", "unfinished draft");
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let git_before = checkout_snapshot(&repo.path().join(".git"));
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let explicit = read(&["task", "checkout", "INF-123", "--explain", "--json"]);
    assert_eq!(
        explicit["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert_eq!(explicit["action"]["behavior"], "reuse");
    assert_eq!(explicit["impediments"], serde_json::json!([]));
    for args in [
        vec!["task", "checkout", "--explain", "--json"],
        vec![
            "--task",
            "INF-123",
            "task",
            "checkout",
            "--explain",
            "--json",
        ],
    ] {
        let report = read(&args);
        assert_eq!(report["action"], explicit["action"]);
        assert_eq!(report["impediments"], explicit["impediments"]);
        assert_eq!(
            report["resolution"]["task"]["value"],
            explicit["resolution"]["task"]["value"]
        );
    }
    let text = success(
        command(home.path(), repo.path(), &["task", "checkout", "--explain"])
            .output()
            .unwrap(),
    );
    assert!(text.contains("reuse recorded"), "{text}");
    for (flag, value, reason) in [
        ("--name", "other-workspace", "already uses workspace name"),
        ("--directive", "new direction", "already exists"),
        ("--stack-on", "UNKNOWN-1", "has no Task"),
    ] {
        let report = read(&[
            "task",
            "checkout",
            "INF-123",
            flag,
            value,
            "--explain",
            "--json",
        ]);
        assert!(
            report["impediments"].to_string().contains(reason),
            "{report}"
        );
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(checkout_snapshot(&repo.path().join(".git")), git_before);
    assert_eq!(
        fs::read_to_string(repo.path().join("draft.txt")).unwrap(),
        "unfinished draft"
    );
    // Execution consumes the same inferred target, without launching a Graph.
    for args in [
        vec!["task", "checkout", "--json"],
        vec!["--task", "INF-123", "task", "checkout", "--json"],
    ] {
        let checkout = read(&args);
        assert_eq!(
            checkout["task_id"],
            registered.task.id.as_str(),
            "{checkout}"
        );
    }
}

#[test]
fn task_checkout_explain_reads_restoration_and_refusals_without_restoring() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let registered = support::register_task_without_pr(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "restore-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    let checkout = repo.path().join("missing-checkout");
    db.execute("UPDATE tasks SET worktree=?1", [checkout.to_str().unwrap()])
        .unwrap();
    let store = SqliteStore::new(&path).unwrap();
    let local = store.local_machine().unwrap().id;
    let peer = loopflow::durable::MachineId::new();
    // A handoff is append-only, so the remote owner comes last.
    for condition in ["absent", "occupied", "terminal", "remote"] {
        db.execute(
            "UPDATE tasks SET planning_state=NULL,checkout_machine_id=?1",
            [local.as_str()],
        )
        .unwrap();
        match condition {
            "occupied" => fs::create_dir(&checkout).unwrap(),
            "terminal" => {
                db.execute("UPDATE tasks SET planning_state='canceled'", [])
                    .unwrap();
            }
            "remote" => {
                store
                    .add_machine(&peer, "must-not-contact.invalid", "peer")
                    .unwrap();
                store.handoff_task(&registered.task.id, &peer).unwrap();
            }
            _ => {}
        }
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = fs::read(&path).unwrap();
        let git_before = checkout_snapshot(&repo.path().join(".git"));
        let report: serde_json::Value = serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &[
                    "task",
                    "checkout",
                    registered.task.id.as_str(),
                    "--explain",
                    "--json",
                ],
            )
            .output()
            .unwrap(),
        ))
        .unwrap();
        match condition {
            "absent" => {
                assert_eq!(report["action"]["behavior"], "restore");
                assert_eq!(report["impediments"], serde_json::json!([]));
                assert!(
                    report["unavailable"].to_string().contains("must fetch"),
                    "{report}"
                );
                assert!(!checkout.exists());
            }
            "occupied" => assert!(
                report["impediments"].to_string().contains("occupied"),
                "{report}"
            ),
            "terminal" => assert!(
                report["impediments"]
                    .to_string()
                    .contains("terminal planning"),
                "{report}"
            ),
            "remote" => {
                assert_eq!(
                    report["resolution"]["execution_machine"]["value"],
                    peer.as_str(),
                    "{report}"
                );
                assert!(
                    report["unavailable"]
                        .to_string()
                        .contains("no remote preparation was performed"),
                    "{report}"
                );
            }
            _ => unreachable!(),
        }
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(checkout_snapshot(&repo.path().join(".git")), git_before);
    }
}

#[test]
fn task_checkout_explain_proposes_unallocated_work_and_preserves_missing_storage() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let path = home.path().join(".lf/loopflow.db");
    let absent: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "checkout", "UNKNOWN-1", "--explain", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert!(absent["action"].is_null());
    assert!(!path.exists());
    let created: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "create", "--title", "Proposed checkout", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = created["id"].as_str().unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let git_before = checkout_snapshot(&repo.path().join(".git"));
    let report: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "checkout", id, "--explain", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(report["action"]["behavior"], "prepare", "{report}");
    assert_eq!(report["impediments"], serde_json::json!([]));
    assert!(!Path::new(report["action"]["path"].as_str().unwrap()).exists());
    assert!(report["unavailable"]
        .to_string()
        .contains("local Git facts"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(checkout_snapshot(&repo.path().join(".git")), git_before);
    assert!(SqliteStore::new(&path)
        .unwrap()
        .task_by_issue(id)
        .unwrap()
        .unwrap()
        .worktree
        .is_none());
}

fn checkout_snapshot(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(checkout_snapshot(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}

#[test]
fn scoped_first_task_run_prepares_with_its_own_options() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    fs::write(
        repo.path().join(".lf/graphs/first-start.yaml"),
        "- cmd: list\n",
    )
    .unwrap();
    repo.stage_all();
    repo.commit("Fixture Graph");
    repo.push();
    let created: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["task", "create", "--title", "First run", "--json"],
        )
        .output()
        .unwrap(),
    ))
    .unwrap();
    let id = created["id"].as_str().unwrap();
    success(
        command(
            home.path(),
            repo.path(),
            &[
                "-b",
                "--task",
                id,
                "task",
                "run",
                id,
                "first-start",
                "--name",
                "selected-first-checkout",
                "-a",
                "codex",
            ],
        )
        .output()
        .unwrap(),
    );
    let db = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    let task = db.task_by_issue(id).unwrap().unwrap();
    assert_eq!(task.workspace_slug, "selected-first-checkout");
    assert_eq!(task.agent.as_deref(), Some("codex"));
    assert!(task.worktree.unwrap().join(".git").exists());
}

#[test]
fn repository_upgrade_enrolls_retained_linked_work_without_manual_binding() {
    for condition in ["retained", "alias", "branch-changed", "offline", "reused"] {
        let enrolls = matches!(condition, "retained" | "alias");
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let main = repo.path().canonicalize().unwrap();
        let checkout = root.path().canonicalize().unwrap().join("task");
        success(
            Command::new("git")
                .current_dir(&main)
                .args(["worktree", "add", "-b", "continuity"])
                .arg(&checkout)
                .output()
                .unwrap(),
        );
        let registered = support::register_task_with_pr(
            &home.path().join(".lf"),
            &main,
            "continuity",
            &repo.head_sha(),
        );
        let db = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
        db.execute(
            "UPDATE tasks SET worktree=?1 WHERE id=?2",
            rusqlite::params![checkout.to_str().unwrap(), registered.task.id.as_str()],
        )
        .unwrap();
        let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
        let id = store
            .repository_id(main.to_str().unwrap())
            .unwrap()
            .unwrap();
        let locator = main
            .join(".git/loopflow")
            .join(store.local_machine().unwrap().id.as_str())
            .join("loopflow-repository");
        fs::remove_file(&locator).unwrap();
        fs::write(checkout.join("unpublished.txt"), "keep me").unwrap();
        match condition {
            "alias" => {
                let alias = root.path().join("alias");
                std::os::unix::fs::symlink(&checkout, &alias).unwrap();
                db.execute(
                    "UPDATE tasks SET worktree=?1 WHERE id=?2",
                    rusqlite::params![alias.to_str().unwrap(), registered.task.id.as_str()],
                )
                .unwrap();
            }
            "branch-changed" => {
                success(
                    Command::new("git")
                        .current_dir(&checkout)
                        .args(["checkout", "-b", "unrelated"])
                        .output()
                        .unwrap(),
                );
            }
            "offline" => {
                fs::rename(&checkout, root.path().join("offline")).unwrap();
            }
            "reused" => {
                fs::rename(&main, root.path().join("original")).unwrap();
                success(
                    Command::new("git")
                        .args(["init", "-q"])
                        .arg(&main)
                        .output()
                        .unwrap(),
                );
            }
            _ => {}
        }
        let reads: Vec<_> = (0..3)
            .map(|_| {
                command(home.path(), &main, &["session", "list", "--json"])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for read in reads {
            let result = read.wait_with_output().unwrap();
            if enrolls {
                success(result);
            } else {
                assert!(!result.status.success());
                assert!(String::from_utf8_lossy(&result.stderr).contains("--bind"));
            }
        }
        assert_eq!(locator.exists(), enrolls);
        assert_eq!(
            store.repository_id(main.to_str().unwrap()).unwrap(),
            Some(id.clone())
        );
        if enrolls {
            assert_eq!(fs::read_to_string(&locator).unwrap().trim(), id.as_str());
            assert_eq!(
                fs::read_to_string(checkout.join("unpublished.txt")).unwrap(),
                "keep me"
            );
            assert_eq!(
                success(
                    command(home.path(), &checkout, &["config", "repository"])
                        .output()
                        .unwrap()
                )
                .trim(),
                id.as_str()
            );
        }
    }
}

#[test]
fn repository_reuse_before_reopening_cannot_steal_work() {
    let original = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let old = original.path().canonicalize().unwrap();
    let before = success(
        command(home.path(), &old, &["config", "repository"])
            .output()
            .unwrap(),
    );
    let root = tempfile::tempdir().unwrap();
    let moved = root.path().join("moved");
    fs::rename(&old, &moved).unwrap();
    fs::create_dir(&old).unwrap();
    success(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&old)
            .output()
            .unwrap(),
    );
    let rejected = command(home.path(), &old, &["config", "repository"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--relocate"));
    let read = command(home.path(), &old, &["session", "list", "--json"])
        .output()
        .unwrap();
    assert!(!read.status.success());
    assert!(String::from_utf8_lossy(&read.stderr).contains("--relocate"));
    // Explicit recovery resolves the ambiguous reuse without enrolling it.
    assert_eq!(
        success(
            command(
                home.path(),
                &moved,
                &["config", "repository", "--relocate", before.trim()]
            )
            .output()
            .unwrap()
        ),
        before
    );
    assert_ne!(
        success(
            command(home.path(), &old, &["config", "repository"])
                .output()
                .unwrap()
        ),
        before
    );
}

#[test]
fn repository_locators_isolate_machines_and_collapse_symlink_aliases() {
    let repo = TestRepo::new();
    let production = tempfile::tempdir().unwrap();
    let experiment = tempfile::tempdir().unwrap();
    let identify = |home: &Path, cwd: &Path| {
        success(
            command(home, cwd, &["config", "repository"])
                .output()
                .unwrap(),
        )
    };
    let original = identify(production.path(), repo.path());
    let experimental = identify(experiment.path(), repo.path());
    assert_ne!(original, experimental);
    let root = tempfile::tempdir().unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(repo.path(), &alias).unwrap();
    assert_eq!(identify(production.path(), &alias), original);
    let moved = root.path().join("moved");
    fs::rename(repo.path(), &moved).unwrap();
    assert_eq!(identify(experiment.path(), &moved), experimental);
    assert_eq!(identify(production.path(), &moved), original);
}

#[test]
fn repository_concurrent_reconciliation_retains_one_identity() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let original = success(
        command(home.path(), repo.path(), &["config", "repository"])
            .output()
            .unwrap(),
    );
    let root = tempfile::tempdir().unwrap();
    let moved = root.path().join("moved");
    fs::rename(repo.path(), &moved).unwrap();
    let children: Vec<_> = (0..4)
        .map(|_| {
            command(home.path(), &moved, &["config", "repository"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        assert_eq!(success(child.wait_with_output().unwrap()), original);
    }
}

#[test]
fn repository_move_with_a_separate_git_directory_keeps_work_readable() {
    let root = tempfile::tempdir().unwrap();
    let before = root.path().join("before");
    let git = root.path().join("git");
    success(
        Command::new("git")
            .args(["init", "-q", "--separate-git-dir"])
            .arg(&git)
            .arg(&before)
            .output()
            .unwrap(),
    );
    let home = tempfile::tempdir().unwrap();
    let identity = success(
        command(home.path(), &before, &["config", "repository"])
            .output()
            .unwrap(),
    );
    let after = root.path().join("after");
    fs::rename(&before, &after).unwrap();
    success(
        command(home.path(), &after, &["session", "list", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(
            command(home.path(), &after, &["config", "repository"])
                .output()
                .unwrap()
        ),
        identity
    );
}

#[test]
fn a_hook_left_by_an_older_lf_does_not_fail_the_provider_turn() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    // `__planning-check` was the Stop hook before 0.14.11 renamed it.
    let output = command(home.path(), repo.path(), &["__planning-check"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("restart the Session"));
    // A misspelled skill still fails.
    let output = command(home.path(), repo.path(), &["no-such-skill"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
}
