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
fn context_budget_preview_reads_authored_wave_without_registration() {
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
    fs::write(memory, "Live decision retained.").unwrap();
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
        command(
            home.path(),
            home.path(),
            &["context", "--explain", "--json"],
        )
        .output()
        .unwrap(),
    );
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["machine"]["state"], "unavailable");
    assert_eq!(report["task"]["state"], "unavailable");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn desktop_inspection_rejects_linux_before_machine_routing_or_work_preparation() {
    let home = tempfile::tempdir().unwrap();
    for operation in [
        "inspect", "hide", "restore", "focus", "split", "move", "resize", "zoom",
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
                "--json",
            ],
        )
        .args(match operation {
            "inspect" => vec![],
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
