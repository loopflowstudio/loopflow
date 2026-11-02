//! `lf wave add|remove|rename`: one Wave lifecycle for the CLI and Desktop.

use std::path::Path;
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;
use serde_json::Value;

/// One repository and one `LF_HOME`, so every command shares a planning store.
struct Lab {
    repo: TestRepo,
    home: tempfile::TempDir,
}

impl Lab {
    fn new() -> Self {
        Self {
            repo: TestRepo::new(),
            home: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.repo.path()
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(loopflow_test_support::lf_binary());
        for (name, _) in std::env::vars_os() {
            let name = name.to_string_lossy();
            if name.starts_with("LF_")
                || name.starts_with("LOOPFLOW_")
                || name.starts_with("LINEAR_")
            {
                command.env_remove(name.as_ref());
            }
        }
        command
            .current_dir(cwd)
            .env("HOME", self.home.path())
            .env("LF_HOME", self.home.path())
            .env("LF_BIN", loopflow_test_support::lf_binary())
            .env("LF_USER_NAME", "Fixture Person")
            .stdin(std::process::Stdio::null())
            .args(args);
        command
    }

    /// `lf <args>` in the repository, whatever its exit status.
    fn run(&self, args: &[&str]) -> Output {
        self.command(self.path(), args).output().unwrap()
    }

    /// `lf <args>` in `cwd`, which must succeed; its JSON when it prints any.
    fn lf_in(&self, cwd: &Path, args: &[&str]) -> Value {
        let output = self.command(cwd, args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
    }

    fn lf(&self, args: &[&str]) -> Value {
        self.lf_in(self.path(), args)
    }

    fn task(&self, wave: &str, title: &str) -> String {
        self.lf(&["task", "create", "--wave", wave, "--title", title, "--json"])["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn live_waves_in(&self, cwd: &Path) -> Vec<String> {
        let listing = self.lf_in(cwd, &["wave", "list", "--json"]);
        let mut names: Vec<String> = listing
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|wave| wave["name"].as_str().map(str::to_string))
            .collect();
        names.sort();
        names
    }

    fn live_waves(&self) -> Vec<String> {
        self.live_waves_in(self.path())
    }

    fn wave_of(&self, task: &str) -> Value {
        self.lf(&["desktop", "task", task, "--json"])["task"]["wave_id"].clone()
    }

    fn committed(&self, path: &str) -> bool {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args(["ls-files", "--", path])
            .output()
            .unwrap();
        !output.stdout.is_empty()
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn removing_an_empty_wave_retires_it_and_removes_its_files_on_main() {
    let lab = Lab::new();
    let added = lab.lf(&["wave", "add", "research", "--json"]);
    assert!(lab.committed(".lf/wave/research/GOAL.md"));

    // An edit nobody committed is deleted with the directory; the preview says so.
    std::fs::write(
        lab.path().join(".lf/wave/research/GOAL.md"),
        "## Objective\n\nUnsaved thoughts\n",
    )
    .unwrap();
    let preview = lab.lf(&["wave", "remove", "research", "--preview", "--json"]);
    assert_eq!(preview["summary"][0], "research has no Tasks or Sessions.");
    assert_eq!(preview["uncommitted"][0], ".lf/wave/research/GOAL.md");
    assert_eq!(
        preview["summary"][2],
        ".lf/wave/research is deleted on the main checkout; Git history keeps its committed files."
    );
    assert_eq!(
        preview["summary"][3],
        "Uncommitted edits in .lf/wave/research are deleted with it: .lf/wave/research/GOAL.md."
    );
    assert!(
        lab.path().join(".lf/wave/research").exists(),
        "preview changes nothing"
    );

    let removed = lab.lf(&["wave", "remove", "research", "--json"]);
    assert_eq!(removed["id"], added["id"]);
    assert_eq!(removed["files"]["state"], "committed");
    assert!(!lab.path().join(".lf/wave/research").exists());
    assert!(!lab.committed(".lf/wave/research/GOAL.md"));
    assert!(lab.live_waves().is_empty());
    assert_eq!(
        removed["summary"][1],
        "1 chapter stays readable in history."
    );
    let history = lab.lf(&["wave", "show", "research", "--json"]);
    assert_eq!(
        history["waves"][0]["wave"]["id"], added["id"],
        "its address still shows the history"
    );
    let text = lab.run(&["wave", "show", "research"]);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("removed at "), "{text}");
    assert!(!text.contains("superseded by"), "{text}");

    let again = lab.lf(&["wave", "remove", "research", "--json"]);
    assert_eq!(again["already_removed"], true);
    assert_eq!(again["summary"][0], "research was already removed.");

    let fresh = lab.lf(&["wave", "add", "research", "--json"]);
    assert_eq!(fresh["created"], true);
    assert_ne!(fresh["id"], added["id"], "re-adding mints a fresh Wave");

    // Desktop retries by id; the old id never reaches the new Wave at its address.
    let by_old_id = lab.lf(&["wave", "remove", added["id"].as_str().unwrap(), "--json"]);
    assert_eq!(by_old_id["already_removed"], true);
    assert_eq!(lab.live_waves(), ["research"]);
    assert!(lab.path().join(".lf/wave/research").exists());
}

#[test]
fn removing_a_wave_with_work_moves_open_tasks_and_keeps_history() {
    let lab = Lab::new();
    let desktop = lab.lf(&["wave", "add", "desktop", "--json"]);
    let unstarted = lab.task("desktop", "Backlog idea");
    let started = lab.task("desktop", "Started work");
    let placed = lab.lf(&["checkout", &started, "--json"]);
    let closed = lab.task("desktop", "Closed work");
    lab.lf(&["task", "abandon", &closed, "--json"]);

    let preview = lab.lf(&["wave", "remove", "desktop", "--preview", "--json"]);
    assert_eq!(preview["moved"].as_array().unwrap().len(), 2);
    assert_eq!(preview["completed_tasks"], 1);
    assert_eq!(preview["chapters"], 1);
    let summary = preview["summary"].to_string();
    assert!(
        summary.contains("2 open Tasks move to the repository (1 started keeps its worktree"),
        "{summary}"
    );
    assert!(
        summary.contains("1 completed Task and 1 chapter stay readable"),
        "{summary}"
    );

    // An interrupted removal: one Task already moved by hand.
    lab.lf(&["task", "move", &unstarted, "--no-wave"]);
    let removed = lab.lf(&["wave", "remove", "desktop", "--json"]);
    assert_eq!(removed["moved"].as_array().unwrap().len(), 1);
    assert_eq!(
        lab.wave_of(&closed),
        desktop["id"],
        "closed work keeps its history"
    );
    for task in [&unstarted, &started] {
        assert!(lab.wave_of(task).is_null(), "{task} left the Wave");
    }
    let after = lab.lf(&["desktop", "task", &started, "--json"]);
    assert_eq!(after["execution"]["worktree"], placed["worktree"]);
    assert!(lab.live_waves().is_empty());

    let show = lab.lf(&["wave", "show", removed["id"].as_str().unwrap(), "--json"]);
    assert!(!show.is_null(), "the removed Wave stays readable by id");
}

#[test]
fn removing_a_child_wave_moves_tasks_to_its_parent_and_folds_memory() {
    let lab = Lab::new();
    let product = lab.lf(&["wave", "add", "product", "--json"]);
    lab.lf(&["wave", "add", "product/desktop", "--json"]);
    let child = lab.path().join(".lf/wave/product/desktop");
    std::fs::write(child.join("MEMORY.md"), "Keep sidebars narrow.\n").unwrap();
    std::fs::write(
        child.join("config.yaml"),
        format!(
            "{}graph: auto\n",
            std::fs::read_to_string(child.join("config.yaml")).unwrap()
        ),
    )
    .unwrap();
    let task = lab.task("product/desktop", "Sidebar");

    let removed = lab.lf(&["wave", "remove", "product/desktop", "--json"]);
    assert_eq!(removed["destination"], "product");
    assert_eq!(lab.wave_of(&task), product["id"]);
    let memory = std::fs::read_to_string(lab.path().join(".lf/wave/product/MEMORY.md")).unwrap();
    assert!(
        memory.contains("## From product/desktop (removed "),
        "{memory}"
    );
    assert!(memory.contains("Keep sidebars narrow."), "{memory}");
    let config = std::fs::read_to_string(lab.path().join(".lf/wave/product/config.yaml")).unwrap();
    assert!(config.contains("graph: auto"), "{config}");
    assert!(!child.exists());
    assert_eq!(lab.live_waves(), ["product"]);
}

#[test]
fn delete_backlog_deletes_only_unstarted_tasks() {
    let lab = Lab::new();
    lab.lf(&["wave", "add", "ops", "--json"]);
    let unstarted = lab.task("ops", "Idea");
    let started = lab.task("ops", "Work");
    lab.lf(&["checkout", &started, "--json"]);

    let removed = lab.lf(&["wave", "remove", "ops", "--delete-backlog", "--json"]);
    assert_eq!(removed["deleted"][0]["id"], unstarted.as_str());
    assert_eq!(removed["moved"][0]["id"], started.as_str());
    let listed = lab.lf(&["task", "list", "--json"]).to_string();
    assert!(!listed.contains(&unstarted), "{listed}");
    assert!(listed.contains(&started), "{listed}");
    assert!(lab.wave_of(&started).is_null());
}

#[test]
fn rename_keeps_identity_moves_files_on_main_and_refuses_a_taken_name() {
    let lab = Lab::new();
    let added = lab.lf(&["wave", "add", "product", "--json"]);
    lab.lf(&["wave", "add", "ops", "--json"]);
    std::fs::write(
        lab.path().join(".lf/wave/ops/MEMORY.md"),
        "See .lf/wave/product/GOAL.md; product owns Desktop.\n",
    )
    .unwrap();
    let task = lab.task("product", "Keep me");

    let taken = lab.run(&["wave", "rename", "product", "ops"]);
    assert!(!taken.status.success());
    assert!(stderr(&taken).contains("ops already exists"));
    std::fs::create_dir_all(lab.path().join(".lf/wave/stray")).unwrap();
    let stray = lab.run(&["wave", "rename", "product", "stray"]);
    assert!(stderr(&stray).contains(".lf/wave/stray already exists"));
    assert_eq!(lab.live_waves(), ["ops", "product"], "nothing moved");

    let renamed = lab.lf(&["wave", "rename", "product", "prod", "--json"]);
    assert_eq!(renamed["id"], added["id"]);
    assert_eq!(renamed["files"]["state"], "committed");
    assert!(lab.committed(".lf/wave/prod/GOAL.md"));
    assert!(!lab.path().join(".lf/wave/product").exists());
    assert_eq!(lab.wave_of(&task), added["id"]);
    assert_eq!(
        std::fs::read_to_string(lab.path().join(".lf/wave/ops/MEMORY.md")).unwrap(),
        "See .lf/wave/prod/GOAL.md; product owns Desktop.\n",
        "paths follow; prose stays word for word"
    );
    assert!(renamed["mentions"]
        .to_string()
        .contains(".lf/wave/ops/MEMORY.md:1"));
    assert!(
        renamed["summary"][0]
            .as_str()
            .unwrap()
            .starts_with("Still mentioned in: .lf/wave/ops/MEMORY.md:1"),
        "Desktop shows the same leftovers the CLI prints: {}",
        renamed["summary"]
    );

    let again = lab.lf(&["wave", "rename", "product", "prod", "--json"]);
    assert_eq!(again["renamed"], false);
}

#[test]
fn a_repository_joining_the_portfolio_registers_its_authored_waves_but_not_removed_ones() {
    let lab = Lab::new();
    let archive = lab.lf(&["wave", "add", "archive", "--json"]);
    let clone_root = tempfile::tempdir().unwrap();
    let clone = clone_root.path().join("clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(lab.path())
        .arg(&clone)
        .output()
        .unwrap();
    assert!(cloned.status.success());
    // The clone still has .lf/wave/archive; the store retires that id.
    lab.lf(&["wave", "remove", "archive", "--json"]);
    let research = clone.join(".lf/wave/research");
    std::fs::create_dir_all(&research).unwrap();
    std::fs::write(research.join("GOAL.md"), "## Objective\n\nResearch\n").unwrap();
    assert!(
        lab.live_waves_in(&clone).is_empty(),
        "a read registers nothing"
    );

    lab.lf_in(
        lab.home.path(),
        &["portfolio", "add", clone.to_str().unwrap(), "--json"],
    );
    assert_eq!(lab.live_waves_in(&clone), ["research"]);
    let listing = lab.lf_in(&clone, &["wave", "list", "--json"]);
    assert!(
        !listing
            .to_string()
            .contains(archive["id"].as_str().unwrap()),
        "the removed Wave's directory stays removed"
    );
    assert!(
        !lab.committed(".lf/wave/archive/GOAL.md")
            && clone.join(".lf/wave/archive/GOAL.md").is_file(),
        "only the clone still has the directory"
    );
}
