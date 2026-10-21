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

    fn gh(&self, args: &[&str]) -> Output {
        clean(
            self.world.join("bin/gh").to_str().unwrap(),
            self.root.path(),
        )
        .args(args)
        .output()
        .unwrap()
    }

    fn wave_id(&self, name: &str) -> String {
        self.lf(&["wave", "list", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .find(|wave| wave["name"] == name)
            .unwrap_or_else(|| panic!("no Wave {name}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn wave_tasks(&self, name: &str) -> Vec<Value> {
        let id = self.wave_id(name);
        self.lf(&["wave", "show", name, "--json"])["tasks"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["task"]["wave_id"] == id.as_str())
            .cloned()
            .collect()
    }
}

/// Builds every listed recipe once, checks that `lf` reads it, then checks
/// what the recipe's demo depends on.
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
        let built = Built::new(recipe);
        let waves = built.lf(&["wave", "list", "--json"]);
        assert!(
            !waves.as_array().unwrap().is_empty(),
            "{recipe} has no Waves"
        );
        let sessions = built.lf(&["session", "list", "--json", "--all"]);
        assert!(
            !sessions.as_array().unwrap().is_empty(),
            "{recipe} has no Sessions"
        );
        match recipe {
            "wave-backlog" => wave_backlog(&built, &sessions),
            "task-review" => task_review(&built),
            "wave-lifecycle" => wave_lifecycle(&built),
            _ => {}
        }
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

fn task_named<'a>(tasks: &'a [Value], name: &str) -> &'a Value {
    tasks
        .iter()
        .find(|item| item["task"]["name"] == name)
        .unwrap_or_else(|| panic!("no Task {name}"))
}

/// Backlog, open work and the Wave's conversation together.
fn wave_backlog(built: &Built, sessions: &Value) {
    let tasks = built.wave_tasks("desktop");
    let started = |item: &&Value| item["runtime"]["started"] == true;
    let unstarted = tasks.iter().filter(|item| !started(item)).count();
    let open = tasks
        .iter()
        .filter(started)
        .filter(|item| item["task"]["completed"] == false)
        .count();
    assert!(unstarted >= 7, "{unstarted} unstarted Tasks");
    assert!(open >= 4, "{open} open Tasks");
    let merged = task_named(&tasks, "Show PR state on Task rows");
    assert_eq!(merged["pr"]["phase"], "merged");
    assert_eq!(merged["task"]["completed"], false);

    let desktop = built.wave_id("desktop");
    let primaries = sessions
        .as_array()
        .unwrap()
        .iter()
        .filter(|session| {
            session["primary_scope"] == "wave" && session["wave_id"] == desktop.as_str()
        })
        .count();
    assert_eq!(primaries, 1);
}

/// PRs match their Tasks' changes and the World's GitHub answers for them.
fn task_review(built: &Built) {
    let tasks = built.wave_tasks("desktop");
    let review = task_named(&tasks, "Rank search results by recency");
    let url = review["pr"]["publication"]["github"]["url"]
        .as_str()
        .expect("PR #12 is published");
    let published: Value = serde_json::from_str(&success(built.gh(&[
        "pr",
        "view",
        url,
        "--json",
        "number,url,title,body,state,isDraft,additions,deletions,changedFiles,headRefOid,baseRefOid,headRefName,baseRefName,reviewDecision,reviewRequests,statusCheckRollup",
    ])))
    .unwrap();
    let files: Value = serde_json::from_str(&success(built.gh(&[
        "api",
        "--hostname",
        "github.com",
        "--paginate",
        "--slurp",
        "repos/demo/project/pulls/12/files?per_page=100",
    ])))
    .unwrap();
    let files = files[0].as_array().unwrap();
    let additions: u64 = files
        .iter()
        .map(|file| file["additions"].as_u64().unwrap())
        .sum();
    assert_eq!(published["additions"].as_u64(), Some(additions));

    // The PR is the Task's committed range; the local edit stays unpublished.
    let identifier = review["task"]["identifier"].as_str().unwrap();
    let changes = built.lf(&["task", "diff", identifier, "--files", "--json"]);
    assert_eq!(published["baseRefOid"], changes["base_commit"]);
    assert_eq!(published["headRefOid"], changes["head_commit"]);
    let changed = changes["files"].as_array().unwrap();
    let committed: Vec<&str> = changed
        .iter()
        .filter(|file| file["committed"] == true)
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    let mut filenames: Vec<&str> = files
        .iter()
        .map(|file| file["filename"].as_str().unwrap())
        .collect();
    filenames.sort_unstable();
    assert_eq!(filenames, committed);
    assert!(changed.iter().any(|file| file["unstaged"] == true));

    let unavailable = built.gh(&[
        "pr",
        "view",
        "https://github.com/demo/project/pull/14",
        "--json",
        "number",
    ]);
    assert!(!unavailable.status.success());

    let merged = task_named(&tasks, "Rebuild the index on schema change");
    assert!(merged["pr"]["merge_commit"].is_string());
    assert_eq!(merged["task"]["completed"], false);
}

/// An empty Wave and one whose work is all completed.
fn wave_lifecycle(built: &Built) {
    assert!(built.wave_tasks("research").is_empty());
    let archive = built.wave_tasks("archive");
    assert!(!archive.is_empty());
    assert!(archive.iter().all(|item| item["task"]["completed"] == true));
}
