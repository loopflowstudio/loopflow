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
fn context_budget_preview_reads_saved_wave_and_refreshes_local_edits() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    fs::create_dir_all(home.path().join(".lf")).unwrap();
    fs::create_dir_all(repo.path().join(".lf")).unwrap();
    fs::create_dir_all(repo.path().join("wave/local")).unwrap();
    fs::create_dir_all(repo.path().join("scratch")).unwrap();
    fs::write(
        home.path().join(".lf/config.yaml"),
        "context_budgets:\n  memory_tokens: 500\n  scratch_tokens: 600\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "context_budgets:\n  memory_tokens: 700\n  input_tokens: 100\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("wave/local/GOAL.md"),
        "---\ncontext_budgets:\n  memory_tokens: 400\n---\nLocal objective.\n",
    )
    .unwrap();
    let memory = repo.path().join("wave/local/MEMORY.md");
    let scratch = repo.path().join("scratch/plan.md");
    fs::write(
        &memory,
        "Live decision and unresolved evidence. ".repeat(500),
    )
    .unwrap();
    fs::write(&scratch, "Pending work. ".repeat(500)).unwrap();
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    let wave = store
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
    let budgets = &report["context"]["budgets"];
    for (key, value, source) in [
        ("memory_tokens", 400, repo.path().join("wave/local/GOAL.md")),
        ("scratch_tokens", 600, home.path().join(".lf/config.yaml")),
        ("input_tokens", 100, repo.path().join(".lf/config.yaml")),
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
        assert!(source["original_tokens"].as_u64().unwrap() > limit);
        assert!(source["submitted_tokens"].as_u64().unwrap() <= limit);
    }
    assert!(usage.last().unwrap()["submitted_tokens"].as_u64().unwrap() > 100);
    store
        .update_wave_document(&wave, "MEMORY.md", "Live decision retained.")
        .unwrap();
    fs::write(scratch, "Pending work retained.").unwrap();
    let refreshed = query();
    for source in &refreshed["context"]["usage"].as_array().unwrap()[..2] {
        assert_eq!(source["original_tokens"], source["submitted_tokens"]);
        assert!(source["original_tokens"].as_u64().unwrap() < 100);
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
        .add_machine(&marker, "ssh://proof@example.invalid", "proof", "~/project")
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
    let task = support::register_unrun_task(
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
        assert!(error.contains("only a published candidate"), "{error}");
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
    assert!(error.contains("only a published candidate"), "{error}");
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
        vec!["flow", "show", "pursue"],
        vec!["help", "flow", "pursue"],
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
    let output = command(home.path(), cwd.path(), &["sync", "--plan"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Run lf sync from a Git repository"));
    let output = command(home.path(), cwd.path(), &["machine", "id"])
        .output()
        .unwrap();
    let identity = success(output);
    assert!(!identity.trim().is_empty());
    let database = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
    let commands: i64 = database
        .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        commands, 2,
        "the failed sync and Machine read each own a process"
    );
    let output = command(
        home.path(),
        cwd.path(),
        &["account", "route", "set", "claude", "person@example.com"],
    )
    .output()
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--repo owner/name"));
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
        "open", "list", "hide", "restore", "focus", "split", "move", "resize", "zoom", "shell",
        "files", "flow-log", "read", "text", "key",
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
            "open" => vec!["--session", "absent", "--diff", "--json"],
            "list" => vec!["--json"],
            "text" => vec!["--target", "invalid-json", "--surface", "stale", "literal"],
            "key" => vec!["--target", "invalid-json", "--surface", "stale", "enter"],
            "read" => vec![
                "--target",
                "invalid-json",
                "--surface",
                "stale",
                "--region",
                "screen",
            ],
            "files" | "flow-log" => vec!["--target", "invalid-json", "--task", "absent"],
            "split" => vec!["--target", "invalid-json", "--axis", "vertical"],
            "move" => vec![
                "--target",
                "invalid-json",
                "--destination",
                "invalid-json",
                "--axis",
                "horizontal",
            ],
            "resize" => vec![
                "--target",
                "invalid-json",
                "--toward",
                "invalid-json",
                "--ratio",
                "0.6",
            ],
            _ => vec!["--target", "invalid-json"],
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
            .create_task(&loopflow::planning::NewTask {
                id: loopflow::durable::TaskId::parse(id).unwrap(),
                project_id: loopflow::durable::ProjectId::parse(&project).unwrap(),
                title: title.into(),
                description: String::new(),
            })
            .unwrap();
    }
    let unscoped = command(
        home.path(),
        selected.path(),
        &["task", "status", "abcd", "--json"],
    )
    .output()
    .unwrap();
    assert!(!unscoped.status.success());
    assert!(String::from_utf8_lossy(&unscoped.stderr).contains("multiple stable Tasks"));
    let store = SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
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
        // missing checkout/PR, rather than failing global prefix resolution.
        let launch = command(
            home.path(),
            other.path(),
            &[flag, value, "--task", "abcd", ":", "Do not launch"],
        )
        .output()
        .unwrap();
        assert!(!launch.status.success());
        let error = String::from_utf8_lossy(&launch.stderr);
        assert!(error.contains("no recorded PR"), "{error}");
        assert!(!error.contains("multiple stable Tasks"), "{error}");
    }
    assert_eq!(
        store.task_by_issue(first).unwrap().unwrap().plan.title,
        "Changed selected"
    );
    let untouched = store.task_by_issue(second).unwrap().unwrap();
    assert_eq!(untouched.plan.title, "Other");
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
    assert!(preview["input"]["system_prompt"]
        .as_str()
        .unwrap()
        .contains("Changed selected"));
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
        "diff: false\ndiff_files: false\npaste: false\ncontext_budgets:\n  goal_tokens: 300\n",
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
    assert!(input["system_prompt"]
        .as_str()
        .unwrap()
        .contains("Inspect preview marker."));
    assert_eq!(input["unwritten_sources"][0]["content"], message);
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
            "unwritten_sources",
        ] {
            assert_eq!(input[field], equivalent[field], "{args:?}: {field}");
        }
    }
    assert!(!repo.path().join(".lf/tmp").exists());
    assert!(!home.path().join(".lf").exists());
    for args in [
        vec!["--context", "task", "create", "--title", "Never created"],
        vec!["--context", "flow", "pursue"],
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
        "effective_delegation"
    );
    assert!(report["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("first-start")));
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
fn desktop_open_explain_checks_the_opening_without_initializing_or_launching() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let plain = read(&["desktop", "open", "--explain", "--json"]);
    let url = reqwest::Url::parse(plain["url"].as_str().unwrap()).unwrap();
    assert_eq!(url.host_str(), Some("open"));
    assert_eq!(plain["resolution"]["repository"]["state"], "unbound");
    assert!(!plain["unavailable"].as_array().unwrap().is_empty());
    if cfg!(target_os = "macos") {
        assert!(plain["impediments"].as_array().unwrap().is_empty());
    } else {
        assert!(plain["impediments"][0]
            .as_str()
            .unwrap()
            .contains("require macOS"));
        assert!(plain["impediments"][0]
            .as_str()
            .unwrap()
            .contains("terminal instead"));
    }
    let invalid = read(&["desktop", "open", "--diff", "--explain", "--json"]);
    assert!(invalid["url"].is_null());
    assert!(invalid["impediments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("--diff needs a Task")));
    let text = success(
        command(home.path(), repo.path(), &["desktop", "open", "--explain"])
            .output()
            .unwrap(),
    );
    assert!(text.contains("Open on this Machine"));
    assert!(text.contains("Nothing was executed"));
    assert!(!home.path().join(".lf").exists());
    assert!(!repo.path().join(".lf/tmp").exists());

    // An unreadable registry is not an unregistered repository to initialize.
    fs::create_dir(home.path().join(".lf")).unwrap();
    let database = home.path().join(".lf/loopflow.db");
    fs::write(&database, "not SQLite").unwrap();
    let unavailable = read(&["desktop", "open", "--explain", "--json"]);
    assert!(unavailable["url"].is_null());
    assert_eq!(
        unavailable["resolution"]["repository"]["state"],
        "unavailable"
    );
    assert_eq!(fs::read_to_string(database).unwrap(), "not SQLite");
}

#[test]
fn desktop_open_explain_preserves_state_and_matches_explicit_and_inferred_work() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("open-explain");
    let registered = support::register_task(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "open-explain",
        &repo.head_sha(),
    );
    support::record_flow(
        &home.path().join(".lf"),
        repo.path(),
        "prior",
        "prior-step",
        "succeeded",
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
    let explicit = read(&[
        "--task",
        "INF-123",
        "desktop",
        "open",
        "--diff",
        "--explain",
        "--json",
    ]);
    let inferred = read(&["desktop", "open", "--diff", "--explain", "--json"]);
    assert_eq!(explicit["url"], inferred["url"]);
    assert_eq!(explicit["resolution"]["task"]["source"], "explicit");
    assert_eq!(inferred["resolution"]["task"]["source"], "checkout");
    let url = reqwest::Url::parse(explicit["url"].as_str().unwrap()).unwrap();
    assert_eq!(url.path(), format!("/{}", registered.task.id));
    assert!(url
        .query_pairs()
        .any(|(key, value)| key == "diff" && value == "true"));
    let missing = read(&[
        "desktop",
        "open",
        "--session",
        "missing",
        "--explain",
        "--json",
    ]);
    assert!(missing["url"].is_null());
    assert!(!missing["impediments"].as_array().unwrap().is_empty());
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
fn desktop_open_delivers_exact_task_without_preparing_its_execution() {
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
    let preview: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &[
                "--task",
                id,
                "desktop",
                "open",
                "--diff",
                "--explain",
                "--json",
            ],
        )
        .env("PATH", &path)
        .output()
        .unwrap(),
    ))
    .unwrap();
    assert!(!received.exists(), "preview launched the app");
    let output = success(
        command(
            home.path(),
            repo.path(),
            &["--task", id, "desktop", "open", "--diff", "--json"],
        )
        .env("PATH", path)
        .output()
        .unwrap(),
    );
    let opening: loopflow::lf::commands::desktop::DesktopOpening =
        serde_json::from_str(&output).unwrap();
    assert_eq!(
        opening.status,
        loopflow::lf::commands::desktop::DesktopOpeningStatus::Opening
    );
    assert_eq!(preview["url"], opening.url);
    let url = reqwest::Url::parse(&opening.url).unwrap();
    assert_eq!(url.path(), format!("/{id}"));
    assert!(url
        .query_pairs()
        .any(|(key, value)| key == "diff" && value == "true"));
    assert!(fs::read_to_string(received)
        .unwrap()
        .ends_with(&format!("{}\n", opening.url)));
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
    let registered = support::register_task(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "explain-proof",
        &repo.head_sha(),
    );
    // A captured Workflow wins over the Project's feature selection.
    let graph = serde_json::json!({"name": "captured", "nodes": [{"name":"review", "skill":"demo", "description":null}], "edges":[{"from":"review", "to":"end", "flow":null}]});
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,'review',1)",
        rusqlite::params![registered.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    support::record_flow(
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
        serde_json::json!({"kind":"edge", "workflow":"captured", "take_up":false, "from":"review", "to":"end", "flow":null})
    );
    assert_eq!(
        explicit["resolution"]["execution_machine"]["source"],
        "recorded_checkout"
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
    assert!(invalid["impediments"][0]
        .as_str()
        .unwrap()
        .contains("does not leave review"));
    let text = success(
        command(home.path(), repo.path(), &["task", "run", "--explain"])
            .output()
            .unwrap(),
    );
    assert!(
        text.contains("Workflow captured: review → end; run no Flow"),
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
    // Launch rejects the same options before checkout/Workflow preparation.
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
fn task_run_explain_reports_remote_unknown_and_refused_evidence_without_effects() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("explain-proof");
    let registered = support::register_task(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "explain-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    let store = SqliteStore::new(&path).unwrap();
    let machine = store.local_machine().unwrap().id;
    for condition in ["terminal", "remote", "unknown", "missing_checkout"] {
        db.execute(
            "UPDATE tasks SET planning_state=NULL, planning_completed=0, checkout_machine_id=?1",
            [machine.as_str()],
        )
        .unwrap();
        match condition {
            "terminal" => {
                db.execute("UPDATE tasks SET planning_state='canceled'", [])
                    .unwrap();
            }
            "remote" => {
                let remote = loopflow::durable::MachineId::new();
                store
                    .add_machine(
                        &remote,
                        "must-not-contact.invalid",
                        "fixture-peer",
                        "/peer/repo",
                    )
                    .unwrap();
                db.execute("UPDATE tasks SET checkout_machine_id=?1", [remote.as_str()])
                    .unwrap();
            }
            "unknown" => {
                db.execute("UPDATE tasks SET checkout_machine_id=NULL", [])
                    .unwrap();
            }
            "missing_checkout" => {
                db.execute(
                    "UPDATE tasks SET worktree=?1",
                    [repo.path().join("absent").to_str().unwrap()],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = fs::read(&path).unwrap();
        let report: serde_json::Value = serde_json::from_str(&success(
            command(
                home.path(),
                repo.path(),
                &["task", "run", "INF-123", "--explain", "--json"],
            )
            .output()
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(
            report["resolution"]["task"]["value"],
            registered.task.id.as_str()
        );
        match condition {
            "terminal" => assert!(
                report["impediments"]
                    .to_string()
                    .contains("terminal planning"),
                "{report}"
            ),
            "remote" => {
                assert!(report["action"].is_null());
                assert!(report["unavailable"].to_string().contains("no peer read"));
            }
            "unknown" => assert_eq!(
                report["resolution"]["execution_machine"]["state"],
                "unavailable"
            ),
            "missing_checkout" => {
                assert!(report["impediments"].to_string().contains("missing"));
                assert!(report["action"].is_null());
            }
            _ => unreachable!(),
        }
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        assert!(
            fs::read(&path).unwrap() == before,
            "{condition}: preview changed database bytes"
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
