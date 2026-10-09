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
        vec!["desktop", "open"],
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
    assert_eq!(invalid["resolution"]["wave"]["state"], "unavailable");
    assert_eq!(invalid["resolution"]["task"]["state"], "unbound");
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
        if matches!(condition, "remote" | "unknown") {
            let movement: serde_json::Value = serde_json::from_str(&success(
                command(
                    home.path(),
                    repo.path(),
                    &["task", "move", "INF-123", "end", "--explain", "--json"],
                )
                .output()
                .unwrap(),
            ))
            .unwrap();
            assert!(movement["action"].is_null());
            assert_eq!(movement["unavailable"], report["unavailable"]);
            assert_eq!(
                movement["resolution"]["execution_machine"],
                report["resolution"]["execution_machine"]
            );
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

fn connection_session(home: &Path, cwd: &Path) -> loopflow::session::LfSession {
    let store = SqliteStore::new(&home.join(".lf/loopflow.db")).unwrap();
    store
        .create_session(
            loopflow::session::LfSession {
                captured: None,
                caller_artifact_key: None,
                task_id: None,
                wave_id: None,
                flow_process_lfid: None,
                work_source: None,
                bound_at: None,
                id: "connection-proof".into(),
                artifact_key: uuid::Uuid::new_v4().simple().to_string(),
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
        rusqlite::params![session.id, format!("{}:provider-session:proof", session.artifact_key),
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
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&db_path).unwrap();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let explicit = read(&["session", "connect", &session.id, "--explain", "--json"]);
    let native = read(&["session", "connect", "native-proof", "--explain", "--json"]);
    let inferred = read(&["--json", "session", "resume", "--explain"]);
    assert_eq!(explicit["resolution"]["session"]["value"], session.id);
    assert_eq!(
        explicit["resolution"]["session"],
        native["resolution"]["session"]
    );
    assert_eq!(inferred["resolution"]["session"]["value"], session.id);
    assert_eq!(
        inferred["resolution"]["session"]["source"],
        "latest_interactive_in_checkout"
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
            &["session", "connect", &session.id, "--explain", "--json"],
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
        "--replace",
        "--explain",
        "--json",
    ]);
    let try_open = read(&[
        "session",
        "connect",
        &session.id,
        "--try",
        "--explain",
        "--json",
    ]);
    assert_eq!(replace["action"]["mode"], "replace");
    assert_eq!(try_open["action"]["mode"], "try");
    let text = success(
        command(
            home.path(),
            repo.path(),
            &["session", "connect", &session.id, "--replace", "--explain"],
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
    let prepared = read(&["session", "connect", "native-proof", "--json"]);
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
                &["session", "connect", id, "--explain", "--json"],
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
        .join(&session.artifact_key[..2])
        .join(&session.artifact_key)
        .join("provider-clients");
    fs::create_dir_all(&clients).unwrap();
    let receipt = clients.join("stale.json");
    fs::write(&receipt, serde_json::json!({"schema_version":1,"pid":std::process::id(),"terminal_id":"stale","started_at":"2000-01-01T00:00:00Z"}).to_string()).unwrap();
    let stale = read(&session.id);
    assert_eq!(stale["state"], "unknown");
    assert!(stale["actions"][0]["unavailable_reason"].is_null());
    fs::write(&receipt, "unreadable receipt").unwrap();
    let invalid = read(&session.id);
    assert!(invalid["state"].is_null());
    assert!(invalid["actions"].as_array().unwrap().is_empty());
    assert!(invalid["unavailable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().contains("cannot read provider clients")));
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
            .join(&session.artifact_key[..2])
            .join(&session.artifact_key)
            .join("provider-clients");
        fs::create_dir_all(&clients).unwrap();
        let receipt = clients.join("client.json");
        let bytes = serde_json::json!({"schema_version":1,"pid":child.id(),"terminal_id":"owned","started_at":time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap()}).to_string();
        fs::write(&receipt, &bytes).unwrap();
        let db_path = home.path().join(".lf/loopflow.db");
        let db = rusqlite::Connection::open(&db_path).unwrap();
        connection_history(&db, &session);
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = fs::read(&db_path).unwrap();
        for (flag, blocked) in [
            (None, true),
            (Some("--replace"), false),
            (Some("--try"), false),
        ] {
            let mut args = vec!["session", "connect", &session.id, "--explain", "--json"];
            args.extend(flag);
            let report: serde_json::Value = serde_json::from_str(&success(
                command(home.path(), repo.path(), &args).output().unwrap(),
            ))
            .unwrap();
            assert_eq!(report["state"], "active");
            assert_eq!(
                !report["impediments"].as_array().unwrap().is_empty(),
                blocked,
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
    // Owned endpoint with a retained driver: no provider or account is started.
    db.execute(
        "INSERT INTO processes(lfid,trace_id,started_at) VALUES('connection-driver','fixture',1)",
        [],
    )
    .unwrap();
    db.execute("UPDATE agent_sessions SET provider='codex',driver_process_lfid='connection-driver',driver_generation=1,provider_generation=1,provider_endpoint=?1,provider_thread='native-proof' WHERE id=?2",
        rusqlite::params![endpoint.to_str().unwrap(), session.id]).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&db_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&success(
        command(
            home.path(),
            repo.path(),
            &["--json", "session", "resume", &session.id, "--explain"],
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
            &["repo", "identity", "--json"],
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
            &["repo", "identity", "--bind", &id, "--json"],
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
    assert!(store.peer_planning_status(&repo).unwrap().is_empty());
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
    let other = loopflow::durable::RepositoryId::new();
    let rebound = command(
        target_home.path(),
        target.path(),
        &["repo", "identity", "--bind", other.as_str()],
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
            &["--repository", &id, "repo", "identity", "--json"],
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
    assert!(store.peer_planning_status(&repo).unwrap().is_empty());
}

#[test]
fn task_move_explain_validates_the_captured_graph_without_moving_or_reconciling() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("move-proof");
    let registered = support::register_task(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "move-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    let graph = serde_json::json!({"name":"captured", "nodes":[{"name":"review","skill":"demo","description":null}], "edges":[{"from":"review","to":"end","flow":null}]});
    db.execute(
        "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,'review',1)",
        rusqlite::params![registered.task.id.as_str(), graph.to_string()],
    )
    .unwrap();
    let process = support::record_flow(
        &home.path().join(".lf"),
        repo.path(),
        "prior",
        "step",
        "succeeded",
    );
    db.execute(
        "UPDATE task_workflows SET edge=0,process_lfid=?1 WHERE task_id=?2",
        rusqlite::params![process, registered.task.id.as_str()],
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
    let preview = read(&[
        "task",
        "move",
        "INF-123",
        "start",
        "--reason",
        " Restart Workflow ",
        "--explain",
        "--json",
    ]);
    assert_eq!(
        preview["resolution"]["task"]["value"],
        registered.task.id.as_str()
    );
    assert_eq!(
        preview["resolution"]["execution_machine"]["source"],
        "recorded_checkout"
    );
    assert_eq!(
        preview["action"],
        serde_json::json!({"workflow":"captured","from":{"kind":"edge","edge":0,"process_lfid":process,"running":false},"to":"start","reason":"Restart Workflow","force":false})
    );
    assert_eq!(preview["impediments"], serde_json::json!([]));
    assert_eq!(
        preview["action"],
        read(&[
            "task",
            "workflow",
            "restart",
            "INF-123",
            "--explain",
            "--json"
        ])["action"]
    );
    let scoped = read(&[
        "--task",
        registered.task.id.as_str(),
        "task",
        "move",
        "INF-123",
        "start",
        "--reason",
        "Restart Workflow",
        "--explain",
        "--json",
    ]);
    assert_eq!(scoped["action"], preview["action"]);
    for action in [
        vec!["task", "run", "INF-123"],
        vec!["task", "move", "INF-123", "start"],
        vec!["task", "workflow", "restart", "INF-123"],
        vec!["task", "workflow", "show", "INF-123"],
    ] {
        let mut args = vec!["--task", "UNKNOWN-1", "--explain"];
        args.extend(action);
        let output = command(home.path(), repo.path(), &args).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("conflicting Task selections"));
    }
    let cases = [
        (vec!["task", "move", "INF-123", "missing"], "has no node"),
        (
            vec!["task", "move", "INF-123", "start", "--force"],
            "--force applies only",
        ),
    ];
    let mut reasons = Vec::new();
    for (args, expected) in &cases {
        let mut args = args.clone();
        args.extend(["--explain", "--json"]);
        let report = read(&args);
        assert!(report["action"].is_null());
        let reason = report["impediments"][0].as_str().unwrap();
        assert!(reason.contains(expected), "{report}");
        reasons.push(reason.to_owned());
    }
    let completion = read(&[
        "task",
        "move",
        "INF-123",
        "end",
        "--force",
        "--explain",
        "--json",
    ]);
    assert_eq!(completion["action"]["to"], "end");
    assert_eq!(completion["action"]["force"], true);
    assert!(completion["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("none were performed"));
    let text = success(
        command(
            home.path(),
            repo.path(),
            &["task", "move", "INF-123", "start", "--explain"],
        )
        .output()
        .unwrap(),
    );
    assert!(
        text.contains("put Task at start of Workflow captured; run nothing"),
        "{text}"
    );
    assert!(text.contains("Nothing was executed"));
    let invalid_context = command(
        home.path(),
        repo.path(),
        &["task", "move", "INF-123", "start", "--context"],
    )
    .output()
    .unwrap();
    assert!(!invalid_context.status.success());
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed store bytes"
    );
    for ((args, _), reason) in cases.iter().zip(reasons) {
        let output = command(home.path(), repo.path(), args).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&reason));
    }
    success(
        command(
            home.path(),
            repo.path(),
            &["task", "workflow", "restart", "INF-123"],
        )
        .output()
        .unwrap(),
    );
    let node: String = db
        .query_row(
            "SELECT node FROM task_workflows WHERE task_id=?1",
            [registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(node, "start");
    let note: String = db
        .query_row(
            "SELECT note FROM task_workflow_moves WHERE task_id=?1 ORDER BY seq DESC LIMIT 1",
            [registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(note, "Restart Workflow");
}

#[test]
fn task_move_explain_keeps_missing_workflow_and_registry_explicit() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let read = |args: &[&str]| -> serde_json::Value {
        serde_json::from_str(&success(
            command(home.path(), repo.path(), args).output().unwrap(),
        ))
        .unwrap()
    };
    let absent = read(&["task", "move", "INF-123", "end", "--explain", "--json"]);
    assert!(absent["action"].is_null());
    assert!(absent["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("absent"));
    assert!(!home.path().join(".lf/loopflow.db").exists());
    support::bind_task_planning(&repo);
    repo.create_branch("move-proof");
    support::register_task(
        &home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        "move-proof",
        &repo.head_sha(),
    );
    let path = home.path().join(".lf/loopflow.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let restart = read(&[
        "task",
        "workflow",
        "restart",
        "INF-123",
        "--explain",
        "--json",
    ]);
    assert!(restart["action"].is_null());
    assert!(restart["impediments"][0]
        .as_str()
        .unwrap()
        .contains("has no workflow"));
    let end = read(&["task", "move", "INF-123", "end", "--explain", "--json"]);
    assert!(end["action"]["workflow"].is_null());
    assert!(end["action"]["from"].is_null());
    assert_eq!(end["action"]["to"], "end");
    assert!(!end["unavailable"].as_array().unwrap().is_empty());
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
}

#[test]
fn task_planning_explain_validates_mutations_without_changing_checkpointed_storage() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("planning-proof");
    let registered = support::register_task(
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
        registered.task.project_id.as_str()
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
    let registered = support::register_task(
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
    let inbox = read(&[
        "task",
        "create",
        "--title",
        "Inbox Task",
        "--explain",
        "--json",
    ]);
    assert_eq!(inbox["action"]["kind"], "create");
    assert!(inbox["action"]["project"].is_null());
    assert_eq!(inbox["resolution"]["wave"]["state"], "unbound");
    assert!(inbox["unavailable"][0]
        .as_str()
        .unwrap()
        .contains("initialization"));
    let invalid = read(&[
        "task",
        "create",
        "--wave",
        "task-pr-tests",
        "--explain",
        "--json",
    ]);
    assert!(invalid["action"].is_null());
    assert_eq!(invalid["resolution"]["wave"]["state"], "unavailable");
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
    let registered = support::register_task(
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
        [registered.task.project_id.as_str()],
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
    let inbox = store
        .ensure_wave_project(canonical.to_str().unwrap(), "inbox")
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    for (explicit, declaration, project, source) in [
        (true, "unknown", &selected, "explicit"),
        (
            false,
            selected.wave_id.as_str(),
            &selected,
            "inherited_declaration",
        ),
        (false, "", &inbox, "default_inbox"),
        (false, "  ", &inbox, "default_inbox"),
    ] {
        let mut invocation = command(
            home.path(),
            repo.path(),
            &["task", "create", "--title", "New", "--explain", "--json"],
        );
        invocation.env("LF_WAVE_ID", declaration);
        if explicit {
            invocation.args(["--wave", "selected"]);
        }
        let preview: serde_json::Value =
            serde_json::from_str(&success(invocation.output().unwrap())).unwrap();
        assert_eq!(
            preview["resolution"]["wave"]["value"],
            project.wave_id.as_str()
        );
        assert_eq!(preview["resolution"]["wave"]["source"], source);
        assert_eq!(preview["resolution"]["task"]["state"], "unbound");
        assert_eq!(preview["action"]["project"], project.id.as_str());
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(fs::read(&path).unwrap() == before);
}

#[test]
fn task_planning_explain_refile_resolves_destination_and_retains_recorded_work() {
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
        .create_task(&loopflow::planning::NewTask {
            id: loopflow::durable::TaskId::new(),
            project_id: loopflow::durable::ProjectId::parse(&source).unwrap(),
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
                    "refile",
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
    assert_eq!(report["action"]["kind"], "refile", "{report}");
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
    assert!(absent["unavailable"].to_string().contains("not performed"));
    let text = success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "refile",
                task.id.as_str(),
                "--wave",
                "destination",
                "--explain",
            ],
        )
        .output()
        .unwrap(),
    );
    assert!(text.contains("Intended action: refile Task to Wave"));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    // Execution addresses the same durable destination, not a new Wave named after its ID.
    let second = store
        .create_task(&loopflow::planning::NewTask {
            id: loopflow::durable::TaskId::new(),
            project_id: task.project_id.clone(),
            title: "Refile by ID".into(),
            description: String::new(),
        })
        .unwrap();
    success(
        command(
            home.path(),
            repo.path(),
            &[
                "task",
                "refile",
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
    // A recorded checkout prevents moving ownership even without a live process.
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
    let refused = read("destination");
    assert!(
        refused["impediments"].to_string().contains("recorded work"),
        "{refused}"
    );
    assert_eq!(read("source")["impediments"], serde_json::json!([]));
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert!(
        fs::read(&path).unwrap() == before,
        "preview changed checkpointed storage"
    );
    let remote = loopflow::durable::MachineId::new();
    store
        .add_machine(
            &remote,
            "must-not-contact.invalid",
            "fixture-peer",
            "/peer/repo",
        )
        .unwrap();
    db.execute("INSERT INTO work_placements(wave_id,machine_id,enabled,placed_at,provenance)
        VALUES(?1,?2,1,1,'explicit') ON CONFLICT(wave_id) DO UPDATE SET machine_id=excluded.machine_id",
        rusqlite::params![destination_wave, remote.as_str()]).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
    let remote = read("destination");
    assert!(
        remote["impediments"]
            .to_string()
            .contains("run this command with"),
        "{remote}"
    );
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
    let registered = support::register_task(
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
    let registered = support::register_task(
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
    // Execution consumes the same inferred target, without launching a Flow.
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
    let registered = support::register_task(
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
    for condition in ["absent", "occupied", "terminal", "unknown", "remote"] {
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
            "unknown" => {
                db.execute("UPDATE tasks SET checkout_machine_id=NULL", [])
                    .unwrap();
            }
            "remote" => {
                let peer = loopflow::durable::MachineId::new();
                store
                    .add_machine(&peer, "must-not-contact.invalid", "peer", "/repo")
                    .unwrap();
                db.execute("UPDATE tasks SET checkout_machine_id=?1", [peer.as_str()])
                    .unwrap();
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
            "unknown" => assert_eq!(
                report["resolution"]["execution_machine"]["state"],
                "unavailable"
            ),
            "remote" => assert!(
                report["unavailable"].to_string().contains("no peer read"),
                "{report}"
            ),
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
    // Explicitly selected Git planning still cannot authorize first start.
    db.execute("INSERT INTO planning_destinations(repo,id,endpoint,reference) VALUES(?1,'fixture','file:///unavailable','refs/loopflow/planning/users/fixture')",
        [repo.path().to_str().unwrap()]).unwrap();
    db.execute("INSERT INTO planning_members(kind,object_id,repo,destination) VALUES('task',?1,?2,'fixture')",
        rusqlite::params![id,repo.path().to_str().unwrap()]).unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let before = fs::read(&path).unwrap();
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
    assert!(report["action"].is_null());
    assert!(
        report["impediments"]
            .to_string()
            .contains("first-start admission is unavailable"),
        "{report}"
    );
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(checkout_snapshot(&repo.path().join(".git")), git_before);
    let refused = command(home.path(), repo.path(), &["task", "checkout", id])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("first-start admission is unavailable")
    );
    assert_eq!(checkout_snapshot(&repo.path().join(".git")), git_before);
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
