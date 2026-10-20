use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn clean(binary: &str, home: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .stdin(Stdio::null());
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

fn build(recipe: &str) -> (tempfile::TempDir, std::path::PathBuf, Value) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join(".gitconfig"),
        "[user]\n\tname = Demo\n\temail = demo@example.com\n",
    )
    .unwrap();
    let world = root.path().join("world");
    let env: Value = serde_json::from_str(&success(
        clean(env!("CARGO_BIN_EXE_lf-world"), root.path())
            .args(["build", recipe, world.to_str().unwrap()])
            .output()
            .unwrap(),
    ))
    .unwrap();
    (root, world, env)
}

fn world_lf(root: &Path, world: &Path, env: &Value, args: &[&str]) -> Command {
    let mut command = clean(env!("CARGO_BIN_EXE_lf"), root);
    for (name, value) in env.as_object().unwrap() {
        command.env(name, value.as_str().unwrap());
    }
    command.current_dir(world.join("repo")).args(args);
    command
}

fn lf(root: &Path, world: &Path, env: &Value, args: &[&str]) -> Value {
    serde_json::from_str(&success(world_lf(root, world, env, args).output().unwrap())).unwrap()
}

#[test]
fn every_recipe_builds_a_world_lf_reads() {
    let listed = success(
        clean(env!("CARGO_BIN_EXE_lf-world"), Path::new("/"))
            .arg("list")
            .output()
            .unwrap(),
    );
    for recipe in listed
        .lines()
        .filter_map(|line| line.split_whitespace().next())
    {
        let (root, world, env) = build(recipe);
        let waves = lf(root.path(), &world, &env, &["wave", "list", "--json"]);
        assert!(
            !waves.as_array().unwrap().is_empty(),
            "{recipe} has no Waves"
        );
        let sessions = lf(
            root.path(),
            &world,
            &env,
            &["session", "list", "--json", "--all"],
        );
        assert!(
            !sessions.as_array().unwrap().is_empty(),
            "{recipe} has no Sessions"
        );
    }
}

#[test]
fn world_sessions_resume_live_through_the_stub_provider() {
    let (root, world, env) = build("one-task");
    let sessions = lf(
        root.path(),
        &world,
        &env,
        &["session", "list", "--json", "--all"],
    );
    let id = sessions[0]["id"].as_str().unwrap();
    let mut connect = world_lf(root.path(), &world, &env, &["session", "connect", id])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(connect.stdin.as_mut().unwrap(), b"hello world\nquit\n").unwrap();
    let output = success(connect.wait_with_output().unwrap());
    assert!(output.contains("resumed "), "{output}");
    assert!(output.contains(": hello world"), "{output}");
}
