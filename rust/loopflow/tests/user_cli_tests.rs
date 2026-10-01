use std::path::Path;
use std::process::Command;

fn user_name_command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["home", "user", "--json"])
        .current_dir(home)
        .env("LF_HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("LF_USER_NAME");
    command
}

fn read_name(command: &mut Command) -> Option<String> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn user_name_uses_git_until_loopflow_overrides_it() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("gitconfig"), "[user]\nname = Jack Heart\n").unwrap();
    assert_eq!(
        read_name(&mut user_name_command(home.path())).as_deref(),
        Some("Jack Heart")
    );

    for args in [
        vec!["init", "-q"],
        vec!["config", "user.name", "Jack at Work"],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(home.path())
            .status()
            .unwrap()
            .success());
    }
    assert_eq!(
        read_name(&mut user_name_command(home.path())).as_deref(),
        Some("Jack at Work")
    );

    for (config, expected) in [
        ("user:\n  name: ' Jack '\n", "Jack"),
        ("user:\n  name: '  '\n", "Jack at Work"),
        ("user:\n  name: null\n", "Jack at Work"),
        ("agent: codex\n", "Jack at Work"),
    ] {
        std::fs::write(home.path().join("config.yaml"), config).unwrap();
        assert_eq!(
            read_name(&mut user_name_command(home.path())).as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn user_name_is_optional_when_git_has_no_name_or_is_unavailable() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(read_name(&mut user_name_command(home.path())), None);
    assert_eq!(
        read_name(user_name_command(home.path()).env("PATH", home.path())),
        None
    );
    std::fs::write(home.path().join("config.yaml"), "user:\n  name: Jack\n").unwrap();
    assert_eq!(
        read_name(user_name_command(home.path()).env("PATH", home.path())).as_deref(),
        Some("Jack")
    );
}
