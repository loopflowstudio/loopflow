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

fn lf(root: &Path, world: &Path, env: &Value, args: &[&str]) -> Value {
    let mut command = clean(env!("CARGO_BIN_EXE_lf"), root);
    for (name, value) in env.as_object().unwrap() {
        command.env(name, value.as_str().unwrap());
    }
    serde_json::from_str(&success(
        command
            .current_dir(world.join("repo"))
            .args(args)
            .output()
            .unwrap(),
    ))
    .unwrap()
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
fn lifecycle_world_reports_each_state() {
    let (root, world, env) = build("lifecycle");
    let shown = lf(
        root.path(),
        &world,
        &env,
        &["wave", "show", "--all", "--json"],
    );
    let tasks = shown["tasks"]["items"].as_array().unwrap();
    let task = |name: &str| {
        tasks
            .iter()
            .find(|task| task["task"]["name"] == name)
            .unwrap_or_else(|| panic!("{name} is missing"))
    };
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    for (name, days) in [
        ("Shipped yesterday", 1),
        ("Shipped this week", 5),
        ("Shipped last fortnight", 12),
        ("Shipped last quarter", 40),
    ] {
        let completed_at = time::OffsetDateTime::parse(
            task(name)["task"]["completed_at"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
        .unix_timestamp();
        assert!(
            (now - days * 86_400 - completed_at).abs() < 3_600,
            "{name} completed at {completed_at}"
        );
    }
    for name in [
        "Shipped yesterday",
        "Shipped this week",
        "Shipped last fortnight",
        "Shipped last quarter",
        "Abandoned attempt",
    ] {
        assert_eq!(task(name)["condition"]["state"], "clear", "{name}");
    }
    assert_eq!(task("Paused mid-flight")["runtime"]["status"], "paused");
    assert_eq!(task("Abandoned attempt")["runtime"]["status"], "abandoned");
    let canceled = task("Canceled upstream");
    assert_eq!(canceled["task"]["state"], "canceled");
    assert_eq!(canceled["runtime"]["status"], "not_ready");
    // Started local work survives an upstream cancel until someone disposes of it.
    assert_eq!(canceled["condition"]["unresolved_execution"], true);
    let lost = task("Lost checkout");
    assert_eq!(lost["condition"]["local_progress"]["state"], "missing");
    assert_eq!(lost["condition"]["state"], "blocked");
    let mut standalone: Vec<_> = tasks
        .iter()
        .filter(|task| task["task"]["wave_id"].is_null())
        .map(|task| task["task"]["name"].as_str().unwrap())
        .collect();
    standalone.sort_unstable();
    assert_eq!(standalone, ["Standalone fix", "Standalone idea"]);
}
