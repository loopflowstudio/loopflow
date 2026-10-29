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
            .find(|task| task["title"] == name)
            .unwrap_or_else(|| panic!("no Task {name:?}"))["identifier"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn task_graph(&self, issue: &str) -> Value {
        self.lf(&["desktop", "task", issue, "--json"])["execution"]["work"]["graph"].clone()
    }

    /// The steps of the Task's latest Graph `name`.
    fn graph_steps(&self, issue: &str, name: &str) -> Vec<Value> {
        let graphs = self.lf(&[
            "graph",
            "list",
            "--processes",
            "--for-task",
            issue,
            "--json",
        ]);
        let id = graphs["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|graph| graph["name"] == name)
            .max_by_key(|graph| graph["updated_at"].as_i64())
            .unwrap_or_else(|| panic!("{issue} ran no {name}"))["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.lf(&["graph", "show", "--processes", &id, "--json"])["steps"]
            .as_array()
            .unwrap()
            .clone()
    }
}

#[test]
fn lifecycle_world_reports_each_state() {
    let built = Built::new("lifecycle");
    let shown = built.lf(&["desktop", "plan", "--all", "--json"]);
    let tasks = shown["tasks"]["items"].as_array().unwrap();
    // The plan holds open Tasks; closed ones page in from the history read.
    let closed_page = built.lf(&["desktop", "done", "--limit", "3", "--json"]);
    let mut closed = closed_page["tasks"].as_array().unwrap().clone();
    let after = closed_page["next"].as_str().unwrap();
    let rest = built.lf(&["desktop", "done", "--after", after, "--json"]);
    assert!(rest["next"].is_null());
    closed.extend(rest["tasks"].as_array().unwrap().iter().cloned());
    let closed_names: Vec<&str> = closed
        .iter()
        .map(|task| task["task"]["name"].as_str().unwrap())
        .collect();
    // Most recently closed first.
    assert_eq!(
        closed_names
            .iter()
            .filter(|name| name.starts_with("Shipped"))
            .copied()
            .collect::<Vec<_>>(),
        [
            "Shipped yesterday",
            "Shipped this week",
            "Shipped last fortnight",
            "Shipped last quarter"
        ]
    );
    // A standup's Done column: one bounded read.
    let week = built.lf(&["task", "list", "--done", "--since", "7d", "--json"]);
    let shipped: Vec<&str> = week["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["title"].as_str().unwrap())
        .filter(|name| name.starts_with("Shipped"))
        .collect();
    assert_eq!(shipped, ["Shipped yesterday", "Shipped this week"]);
    for name in &closed_names {
        assert!(
            !tasks.iter().any(|task| task["task"]["name"] == *name),
            "closed {name} is in the plan"
        );
    }
    let task = |name: &str| {
        tasks
            .iter()
            .chain(&closed)
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
    let unassigned = task("Unassigned mid-flight");
    assert_eq!(unassigned["runtime"]["status"], "idle");
    assert!(unassigned["runtime"]["owner"].is_null());
    assert_eq!(task("Abandoned attempt")["runtime"]["status"], "abandoned");
    let canceled = task("Canceled upstream");
    assert_eq!(canceled["task"]["state"], "canceled");
    // Its `pursue` run ran into `end`; with that run finished and nothing
    // restarting it, it reads idle.
    assert_eq!(canceled["runtime"]["status"], "idle");
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
fn task_list_query_finds_open_tasks_only() {
    let built = Built::new("lifecycle");
    let search = |args: &[&str]| {
        let found = built.lf(&[&["task", "list"], args, &["--json"]].concat());
        found.as_array().unwrap().to_vec()
    };
    let names = |args: &[&str]| {
        let mut names: Vec<String> = search(args)
            .iter()
            .map(|task| task["title"].as_str().unwrap().to_owned())
            .collect();
        names.sort_unstable();
        names
    };
    // Every "Shipped" Task is done, so the query leaves it to `--done`.
    assert!(names(&["shipped"]).is_empty());
    assert_eq!(
        names(&["STANDALONE"]),
        ["Standalone fix", "Standalone idea"]
    );
    let unassigned = &search(&["unassigned mid"])[0];
    assert_eq!(unassigned["state"], "idle");
    assert!(unassigned["reason"]
        .as_str()
        .unwrap()
        .starts_with("Unassigned"));
    let identifier = unassigned["identifier"].as_str().unwrap().to_lowercase();
    assert_eq!(names(&[&identifier]), ["Unassigned mid-flight"]);
}

#[test]
fn task_list_watch_sends_a_frame_only_when_the_list_changes() {
    let built = Built::new("lifecycle");
    let issue = built.issue_named("Standalone fix");
    let mut watch = built
        .command(&["task", "list", "--watch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = watch.stdout.take().unwrap();
    let (sender, frames) = std::sync::mpsc::channel::<Value>();
    std::thread::spawn(move || {
        for line in std::io::BufRead::lines(std::io::BufReader::new(stdout)) {
            if sender
                .send(serde_json::from_str(&line.unwrap()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let next = || {
        frames
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("a frame")
    };
    let state = |frame: &Value| {
        frame["body"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["identifier"] == issue.as_str())
            .map(|row| row["state"].clone())
    };
    let first = next();
    assert!(first["answers"].is_null(), "{first}");
    assert_ne!(state(&first), None);

    // An unchanged store sends nothing.
    assert!(frames
        .recv_timeout(std::time::Duration::from_secs(3))
        .is_err());

    success(
        built
            .command(&["task", "edit", &issue, "--title", "Standalone fix, renamed"])
            .output()
            .unwrap(),
    );
    let renamed = next();
    let title = renamed["body"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["identifier"] == issue.as_str())
        .map(|row| row["title"].clone());
    assert_eq!(title, Some("Standalone fix, renamed".into()), "{renamed}");

    // A request replaces the arguments and is always answered.
    let stdin = watch.stdin.as_mut().unwrap();
    std::io::Write::write_all(stdin, b"{\"id\":1,\"args\":{\"query\":\"standalone\"}}\n").unwrap();
    let narrowed = next();
    assert_eq!(narrowed["answers"], 1);
    assert_eq!(narrowed["body"].as_array().unwrap().len(), 2, "{narrowed}");

    drop(watch.stdin.take());
    assert!(watch.wait().unwrap().success());
}

#[test]
fn session_list_task_status_and_top_watch_answer_requests() {
    let built = Built::new("one-task");
    let issue = built.issue_named("Search Session transcripts");
    for args in [
        vec!["session", "list", "--all", "--watch"],
        vec!["task", "status", issue.as_str(), "--watch"],
        vec!["monitor", "ps", "--json", "--watch"],
    ] {
        let mut watch = built
            .command(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::io::Write::write_all(watch.stdin.as_mut().unwrap(), b"{\"id\":7}\n").unwrap();
        drop(watch.stdin.take());
        let output = success(watch.wait_with_output().unwrap());
        let frames: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let last = frames.last().unwrap();
        assert!(last["unavailable"].is_null(), "{args:?}: {last}");
        assert!(!last["body"].is_null(), "{args:?}: {last}");
    }
}

#[test]
fn world_sessions_resume_live_through_the_stub_provider() {
    let built = Built::new("one-task");
    let sessions = built.lf(&["session", "list", "--json", "--all"]);
    let id = sessions[0]["id"].as_str().unwrap();
    let mut connect = built
        .command(&["session", "resume", id])
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
    let graph = built.task_graph(&issue);
    assert_eq!(graph["name"], "feature");
    assert_eq!(
        graph["position"],
        serde_json::json!({"kind": "at", "step": "demo"})
    );
    // `lf task status` says each fact once: the row, then what belongs to it.
    let status = built.lf(&["task", "status", &issue, "--json"]);
    let mut keys: Vec<_> = status.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["comments", "graph_processes", "sessions", "task"]);
    assert_eq!(status["task"]["state"], "active");
    assert_eq!(
        status["task"]["graph"],
        serde_json::json!({"name": "feature", "step": "demo"})
    );
    assert_eq!(status["task"]["next"], format!("lf task run {issue}"));

    let steps = built.graph_steps(&issue, "feature");
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
fn portfolio_has_a_stopped_run_and_a_task_waiting_at_demo() {
    let built = Built::new("portfolio");

    let stopped = built.issue_named("Record a reader floor per migration");
    let graph = built.task_graph(&stopped);
    assert_eq!(graph["position"]["kind"], "run");
    assert_eq!(graph["position"]["running"], false);
    let steps = built.graph_steps(&stopped, "code");
    let outcomes: Vec<&str> = steps
        .iter()
        .map(|step| step["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["succeeded", "succeeded", "failed"]);
    assert_eq!(steps[2]["label"], "sync");

    let graph = built.task_graph(&built.issue_named("Open conversations faster"));
    assert_eq!(
        graph["position"],
        serde_json::json!({"kind": "at", "step": "demo"})
    );
}
