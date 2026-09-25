//! Public commands must acquire only the repository context they actually use.

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
        .env("LF_DB_PATH", home.join(".lf/loopflow.db"))
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
fn machine_commands_and_catalog_work_without_git_or_a_repository() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let no_tools = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/bin/ps", no_tools.path().join("ps")).unwrap();
    for args in [
        vec!["list"],
        vec!["--list"],
        vec!["flow", "show", "code"],
        vec!["flow", "validate", "code"],
        vec!["auth", "status"],
        vec!["profile", "list"],
        vec!["route", "show"],
        vec!["route", "show", "--repo", "example/project"],
        vec!["ls", "--json"],
        vec!["ps", "--json"],
    ] {
        let output = command(home.path(), cwd.path(), &args)
            .env("PATH", no_tools.path())
            .output()
            .unwrap();
        let stdout = success(output);
        if args == ["list"] || args == ["--list"] {
            assert!(stdout.contains("debug"));
        }
        if args[0] == "route" {
            assert!(stdout.contains("claude"));
        }
        assert!(
            !cwd.path().join(".lf").exists(),
            "{args:?} wrote repository state"
        );
    }
}

#[test]
fn missing_repository_and_missing_home_are_distinct() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let output = command(home.path(), cwd.path(), &["rebase", "--plan"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Run lf rebase from a Git repository"));
    let output = command(home.path(), cwd.path(), &["home", "id"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("initialized local store"));
    let output = command(
        home.path(),
        cwd.path(),
        &["route", "set", "claude", "person@example.com"],
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
    let linked = repo.create_named_worktree("catalog");
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
            command(home.path(), cwd, &["ls", "--json"])
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
            command(home.path(), cwd.path(), &["install", "schedule"])
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
    let output = command(home.path(), cwd.path(), &["install", "schedule", "monthly"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
    assert_eq!(fs::read(&path).unwrap(), before);
}
