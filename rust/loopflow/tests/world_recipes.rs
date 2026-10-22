use std::path::{Path, PathBuf};
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

/// A built World and the commands that read it.
struct Built {
    root: tempfile::TempDir,
    world: PathBuf,
    env: Value,
}

impl Built {
    fn new(recipe: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".gitconfig"),
            "[user]\n\tname = Demo\n\temail = demo@example.com\n",
        )
        .unwrap();
        let world = root.path().join("world");
        let env = serde_json::from_str(&success(
            clean(env!("CARGO_BIN_EXE_lf-world"), root.path())
                .args(["build", recipe, world.to_str().unwrap()])
                .output()
                .unwrap(),
        ))
        .unwrap();
        Self { root, world, env }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = clean(env!("CARGO_BIN_EXE_lf"), self.root.path());
        for (name, value) in self.env.as_object().unwrap() {
            command.env(name, value.as_str().unwrap());
        }
        command.current_dir(self.world.join("repo")).args(args);
        command
    }

    fn lf(&self, args: &[&str]) -> Value {
        serde_json::from_str(&success(self.command(args).output().unwrap())).unwrap()
    }

    fn issue_named(&self, name: &str) -> String {
        self.lf(&["task", "list", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| &entry["task"])
            .find(|task| task["name"] == name)
            .unwrap_or_else(|| panic!("no Task {name:?}"))["identifier"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn workflow(&self, issue: &str) -> Value {
        self.lf(&["task", "status", issue, "--json"])["execution"]["work"]["workflow"].clone()
    }

    /// The steps of the Task's latest Flow `name`.
    fn flow_steps(&self, issue: &str, name: &str) -> Vec<Value> {
        let flows = self.lf(&["flow", "list", "--processes", "--for-task", issue, "--json"]);
        let id = flows["entries"]
            .as_array()
            .unwrap()
            .iter()
            .rfind(|flow| flow["name"] == name)
            .unwrap_or_else(|| panic!("{issue} ran no {name}"))["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.lf(&["flow", "show", "--processes", &id, "--json"])["steps"]
            .as_array()
            .unwrap()
            .clone()
    }
}

#[test]
fn lifecycle_world_reports_each_state() {
    let built = Built::new("lifecycle");
    let shown = built.lf(&["wave", "show", "--all", "--json"]);
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

#[test]
fn world_sessions_resume_live_through_the_stub_provider() {
    let built = Built::new("one-task");
    let sessions = built.lf(&["session", "list", "--json", "--all"]);
    let id = sessions[0]["id"].as_str().unwrap();
    let mut connect = built
        .command(&["session", "connect", id])
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

#[test]
fn one_task_waits_at_demo_after_an_eight_step_pursue() {
    let built = Built::new("one-task");
    let issue = built.issue_named("Search Session transcripts");
    let workflow = built.workflow(&issue);
    assert_eq!(workflow["name"], "feature");
    assert_eq!(workflow["position"]["node"], "demo");
    let moves: Vec<&str> = workflow["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["kind"].as_str().unwrap())
        .collect();
    assert_eq!(moves, ["took_up", "chose", "arrived", "chose", "arrived"]);

    let steps = built.flow_steps(&issue, "pursue");
    let nodes: Vec<(u64, &str)> = steps
        .iter()
        .map(|step| {
            (
                step["key"].as_u64().unwrap(),
                step["label"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        nodes,
        [
            (0, "implement"),
            (1, "compress"),
            (2, "sync"),
            (3, "realign"),
            (4, "loop-or-next"),
            (5, "concept-review"),
            (6, "loop-or-next"),
            (7, "pr-publish"),
        ]
    );
}

#[test]
fn portfolio_has_a_stopped_edge_and_a_hand_moved_workflow() {
    let built = Built::new("portfolio");

    let stopped = built.issue_named("Record a reader floor per migration");
    let workflow = built.workflow(&stopped);
    assert_eq!(workflow["position"]["kind"], "edge");
    assert_eq!(workflow["position"]["running"], false);
    let steps = built.flow_steps(&stopped, "pursue");
    let outcomes: Vec<&str> = steps
        .iter()
        .map(|step| step["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["succeeded", "succeeded", "failed"]);
    assert_eq!(steps[2]["label"], "sync");

    let workflow = built.workflow(&built.issue_named("Open conversations faster"));
    assert_eq!(workflow["position"]["node"], "demo");
    let last = workflow["history"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "set");
    assert_eq!(last["note"], "Built in the Session");
}
