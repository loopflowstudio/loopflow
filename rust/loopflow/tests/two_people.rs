//! LOO-555 acceptance: Maya and Lee, each on their own Home, share one
//! repository's Git planning. Built by `lf-world build two-people`; every step
//! runs the real `lf` as one of them, with the World's `gh` and `launchctl`.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use loopflow::ops::cron::Launchctl;
use loopflow::ops::{CronHost, CronSpec, OpsResult};
use serde_json::Value;

struct Pair {
    root: tempfile::TempDir,
    world: PathBuf,
}

#[derive(Clone, Copy)]
enum Person {
    Maya,
    Lee,
}

impl Pair {
    fn build() -> Self {
        let root = tempfile::tempdir().unwrap();
        // Each clone names its person; nothing falls back to a global identity.
        std::fs::write(root.path().join(".gitconfig"), "").unwrap();
        let world = root.path().join("world");
        let output = Self::clean(env!("CARGO_BIN_EXE_lf-world"), root.path())
            .args(["build", "two-people", world.to_str().unwrap()])
            .output()
            .unwrap();
        success(&output);
        // Each person's HOME holds their own LaunchAgents and Git config.
        for home in [world.clone(), world.join("lee")] {
            std::fs::write(home.join(".gitconfig"), "").unwrap();
        }
        Self { root, world }
    }

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

    fn home(&self, person: Person) -> PathBuf {
        match person {
            Person::Maya => self.world.clone(),
            Person::Lee => self.world.join("lee"),
        }
    }

    fn repo(&self, person: Person) -> PathBuf {
        self.home(person).join("repo")
    }

    fn env(&self, person: Person) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.home(person).join("env.json")).unwrap())
            .unwrap()
    }

    fn run(&self, person: Person, args: &[&str]) -> Output {
        let mut command = Self::clean(env!("CARGO_BIN_EXE_lf"), &self.home(person));
        for (name, value) in self.env(person).as_object().unwrap() {
            command.env(name, value.as_str().unwrap());
        }
        command
            .current_dir(self.repo(person))
            .args(args)
            .output()
            .unwrap()
    }

    fn ok(&self, person: Person, args: &[&str]) -> String {
        success(&self.run(person, args))
    }

    fn json(&self, person: Person, args: &[&str]) -> Value {
        serde_json::from_str(&self.ok(person, args)).unwrap()
    }

    fn git(&self, cwd: &Path, args: &[&str]) {
        let output = Self::clean("git", self.root.path())
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap();
        success(&output);
    }

    fn steers(&self, person: Person) -> Vec<String> {
        let db = rusqlite::Connection::open(self.home(person).join("home/loopflow.db")).unwrap();
        let mut query = db.prepare("SELECT text FROM steers").unwrap();
        query
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn create_task(&self, person: Person, title: &str) -> String {
        let task = self.json(
            person,
            &[
                "task", "create", "--wave", "infra", "--title", title, "--json",
            ],
        );
        task["identifier"].as_str().unwrap().to_string()
    }

    fn branch(&self, person: Person, task: &str) -> String {
        self.json(person, &["task", "status", task, "--json"])["task"]["branch"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn launch_agents(&self, person: Person) -> PathBuf {
        self.home(person).join("Library/LaunchAgents")
    }

    /// Install a private cron the way `lf cron add` does. The command itself
    /// refuses builds that are not releases, so the test writes the job.
    fn install_cron(&self, person: Person, name: &str, command: &[&str]) {
        let db = rusqlite::Connection::open(self.home(person).join("home/loopflow.db")).unwrap();
        let machine: String = db
            .query_row("SELECT id FROM machines WHERE route='local'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let env = self.env(person);
        let spec = CronSpec {
            wave: "infra".into(),
            graph: name.into(),
            command: command.iter().map(|arg| arg.to_string()).collect(),
            schedule: loopflow::ops::parse_schedule("daily").unwrap(),
            working_directory: self.repo(person),
            lf_path: env!("CARGO_BIN_EXE_lf").into(),
            host: CronHost {
                machine_id: machine.parse().unwrap(),
                lf_home: env["LF_HOME"].as_str().unwrap().into(),
                path_env: env["PATH"].as_str().unwrap().into(),
            },
            published: false,
        };
        loopflow::ops::add_cron(&self.launch_agents(person), &spec, &Unloaded).unwrap();
    }
}

/// Writes the job without loading it into launchd.
struct Unloaded;

impl Launchctl for Unloaded {
    fn load(&self, _: &Path) -> OpsResult<()> {
        Ok(())
    }
    fn unload(&self, _: &Path) -> OpsResult<()> {
        Ok(())
    }
    fn is_loaded(&self, _: &str) -> OpsResult<bool> {
        Ok(false)
    }
    fn trigger(&self, _: &str) -> OpsResult<()> {
        Ok(())
    }
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn two_people_share_one_repository() {
    use Person::{Lee, Maya};
    let pair = Pair::build();
    // A takeover Workflow whose only Graph runs no agent.
    let maya = pair.repo(Maya);
    std::fs::create_dir_all(maya.join(".lf/graphs")).unwrap();
    std::fs::write(
        maya.join(".lf/graphs/takeover.yaml"),
        "- cmd: task status\n- interactive: {name: demo, id: review}\n",
    )
    .unwrap();
    pair.git(&maya, &["add", ".lf"]);
    pair.git(&maya, &["commit", "-m", "Add the takeover Workflow"]);
    pair.git(&maya, &["push", "origin", "main"]);
    pair.git(&pair.repo(Lee), &["pull", "--ff-only"]);
    let task = pair.create_task(Maya, "GPU fallback");

    // 5. Lee's refresh imports Maya's new Task.
    pair.ok(Lee, &["refresh"]);
    assert!(pair.ok(Lee, &["task", "list"]).contains("GPU fallback"));

    // 2. Maya's first checkout records the branch in planning; Lee gets it.
    pair.ok(Maya, &["task", "checkout", &task]);
    let branch = pair.branch(Maya, &task);
    assert!(branch.starts_with("maya/"), "{branch}");
    // Maya pushes work and opens PR #1 on the World's GitHub.
    let status = pair.json(Maya, &["task", "status", &task, "--json"]);
    let worktree = PathBuf::from(status["execution"]["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("gpu.txt"), "fallback\n").unwrap();
    pair.git(&worktree, &["add", "gpu.txt"]);
    pair.git(&worktree, &["commit", "-m", "Fall back to CPU"]);
    pair.git(&worktree, &["push", "origin", &branch]);
    let pulls = pair.world.join("github/pulls");
    std::fs::create_dir_all(&pulls).unwrap();
    std::fs::write(
        pulls.join("1.json"),
        serde_json::json!({
            "number": 1,
            "url": "https://github.com/demo/project/pull/1",
            "state": "OPEN",
            "isDraft": false,
            "headRefName": branch,
        })
        .to_string(),
    )
    .unwrap();
    pair.ok(Maya, &["refresh"]);
    pair.ok(Lee, &["refresh"]);

    // 1. Lee's comment, synced through Git, steers Maya's Task.
    pair.ok(Lee, &["comment", &task, "keep CPU path"]);
    pair.ok(Maya, &["refresh"]);
    assert!(
        pair.steers(Maya)
            .iter()
            .any(|text| text.contains("by Lee") && text.contains("keep CPU path")),
        "{:?}",
        pair.steers(Maya)
    );

    // 4. Owners appear in every Task list, and the log reads as dates and names.
    let show = pair.ok(Lee, &["wave", "show", "infra"]);
    assert!(show.contains("Maya"), "{show}");
    assert!(pair.ok(Lee, &["task", "list"]).contains("Maya"));
    let status = pair.ok(Lee, &["task", "status", &task]);
    // Maya's Machine is private, so Lee sees only its id.
    let claim = status
        .lines()
        .find(|line| line.contains("claim  Maya on home_"))
        .unwrap_or_else(|| panic!("{status}"));
    assert!(
        !claim
            .split_whitespace()
            .any(|word| word.len() == 10 && word.bytes().all(|b| b.is_ascii_digit())),
        "{claim}"
    );
    // 3. Maya publishes a cron. Lee sees it without installing anything.
    pair.install_cron(Maya, "nightly", &["--wave", "infra", "graph", "look"]);
    let published = || {
        pair.ok(Maya, &["cron", "publish", "nightly"]);
        let listing = pair.json(Lee, &["cron", "list", "--json"]);
        assert_eq!(listing["installed"], serde_json::json!([]), "{listing}");
        let [cron] = listing["published"].as_array().unwrap().as_slice() else {
            panic!("{listing}");
        };
        assert_eq!(cron["name"], "nightly");
        assert_eq!(cron["person"], "Maya");
        assert_eq!(cron["person_key"], "maya@example.com");
        assert!(cron["machine_id"].as_str().unwrap().starts_with("home_"));
        cron["machine_label"].clone()
    };
    // A private Machine publishes only its id.
    assert_eq!(published(), Value::Null);
    // A public Machine's later entries carry the name Maya chose.
    pair.ok(Maya, &["machine", "public", "maya-mbp"]);
    assert_eq!(published(), "maya-mbp");
    // Going private again republishes without waiting for a cron command.
    pair.ok(Maya, &["machine", "public", "off"]);
    let listing = pair.json(Lee, &["cron", "list", "--json"]);
    assert_eq!(
        listing["published"][0]["machine_label"],
        Value::Null,
        "{listing}"
    );
    pair.ok(Maya, &["machine", "public", "maya-mbp"]);
    assert!(!pair.launch_agents(Lee).exists());

    // 6. Without names, claims still belong to the person who made them.
    for person in [Maya, Lee] {
        pair.git(&pair.repo(person), &["config", "--unset", "user.name"]);
        let path = pair.home(person).join("home/config.yaml");
        let mut config: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        config.as_mapping_mut().unwrap().remove("user");
        std::fs::write(&path, serde_yaml_ng::to_string(&config).unwrap()).unwrap();
    }
    let nameless = pair.create_task(Maya, "Nameless");
    pair.ok(Maya, &["task", "claim", &nameless]);
    pair.ok(Maya, &["refresh"]);
    pair.ok(Lee, &["refresh"]);
    // An unnamed owner reads as their key, so Lee can tell whose claim it is.
    let named = pair.ok(Lee, &["task", "status", &nameless]);
    assert!(
        named.contains("claim  maya@example.com on maya-mbp"),
        "{named}"
    );
    let show = pair.ok(Lee, &["wave", "show", "infra"]);
    let row = show
        .lines()
        .find(|line| line.contains("Nameless"))
        .unwrap_or_else(|| panic!("{show}"));
    assert!(row.contains("maya@example.com"), "{row}");
    let refused = pair.run(Lee, &["task", "unclaim", &nameless]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("claimed by someone else"));
    pair.ok(Maya, &["task", "unclaim", &nameless]);

    // 2. Lee takes the Task over on Maya's branch, so it keeps one PR.
    pair.ok(Lee, &["task", "claim", &task]);
    pair.ok(Lee, &["-b", "task", "run", &task, "takeover"]);
    let taken = pair.json(Lee, &["task", "status", &task, "--json"]);
    assert_eq!(taken["execution"]["branch"], branch, "{taken}");
    assert_eq!(
        taken["execution"]["pr"]["publication"]["github"]["number"], 1,
        "{taken}"
    );

    // 7. Workflow definitions are committed files; a Wave selects a name.
    let rejected = pair.run(
        Maya,
        &["wave", "workflow", "set", "review", "infra", "--file", "x"],
    );
    assert_eq!(rejected.status.code(), Some(2));
    std::fs::write(
        maya.join(".lf/graphs/review.yaml"),
        "- implement\n- interactive: {name: demo, id: review}\n",
    )
    .unwrap();
    pair.git(&maya, &["add", ".lf/graphs"]);
    pair.git(
        &maya,
        &[
            "-c",
            "user.name=Maya",
            "commit",
            "-m",
            "Add the review Workflow",
        ],
    );
    pair.git(&maya, &["push", "origin", "main"]);
    pair.ok(Maya, &["wave", "workflow", "set", "review", "infra"]);
    pair.ok(Maya, &["refresh"]);
    pair.git(&pair.repo(Lee), &["pull", "--ff-only"]);
    pair.ok(Lee, &["refresh"]);
    let fresh = pair.create_task(Lee, "Review me");
    let preview = pair.json(Lee, &["task", "run", &fresh, "--explain", "--json"]);
    assert_eq!(preview["action"]["workflow"], "review", "{preview}");
}
