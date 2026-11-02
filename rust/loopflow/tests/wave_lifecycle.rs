//! `lf wave add|remove|rename`: one Wave lifecycle for the CLI and Desktop.

use std::path::Path;
use std::process::Command;

use loopflow_test_support::TestRepo;
use serde_json::Value;

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") || name.starts_with("LINEAR_") {
            command.env_remove(name.as_ref());
        }
    }
    command
        .current_dir(repo)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", loopflow_test_support::lf_binary())
        .env("LF_USER_NAME", "Fixture Person")
        .stdin(std::process::Stdio::null())
        .args(args);
    command
}

fn lf(repo: &Path, home: &Path, args: &[&str]) -> Value {
    let output = command(repo, home, args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
}

fn create_task(repo: &Path, home: &Path, wave: &str, title: &str) -> String {
    lf(
        repo,
        home,
        &["task", "create", "--wave", wave, "--title", title, "--json"],
    )["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn live_waves(repo: &Path, home: &Path) -> Vec<String> {
    let listing = lf(repo, home, &["wave", "list", "--json"]);
    let mut names: Vec<String> = listing
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|wave| wave["name"].as_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

fn wave_of(repo: &Path, home: &Path, task: &str) -> Value {
    lf(repo, home, &["desktop", "task", task, "--json"])["task"]["wave_id"].clone()
}

fn committed(repo: &TestRepo, path: &str) -> bool {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["ls-files", "--", path])
        .output()
        .unwrap();
    !output.stdout.is_empty()
}

#[test]
fn removing_an_empty_wave_retires_it_and_removes_its_files_on_main() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let added = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "research", "--json"],
    );
    assert!(committed(&repo, ".lf/wave/research/GOAL.md"));

    // An edit nobody committed is deleted with the directory; the preview says so.
    std::fs::write(
        repo.path().join(".lf/wave/research/GOAL.md"),
        "## Objective\n\nUnsaved thoughts\n",
    )
    .unwrap();
    let preview = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "research", "--preview", "--json"],
    );
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
        repo.path().join(".lf/wave/research").exists(),
        "preview changes nothing"
    );

    let removed = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "research", "--json"],
    );
    assert_eq!(removed["id"], added["id"]);
    assert_eq!(removed["files"]["state"], "committed");
    assert!(!repo.path().join(".lf/wave/research").exists());
    assert!(!committed(&repo, ".lf/wave/research/GOAL.md"));
    assert!(live_waves(repo.path(), home.path()).is_empty());
    assert_eq!(
        removed["summary"][1],
        "1 chapter stays readable in history."
    );
    let history = lf(
        repo.path(),
        home.path(),
        &["wave", "show", "research", "--json"],
    );
    assert_eq!(
        history["waves"][0]["wave"]["id"], added["id"],
        "its address still shows the history"
    );
    let text = command(repo.path(), home.path(), &["wave", "show", "research"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("removed at "), "{text}");
    assert!(!text.contains("superseded by"), "{text}");

    let again = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "research", "--json"],
    );
    assert_eq!(again["already_removed"], true);
    assert_eq!(again["summary"][0], "research was already removed.");

    let fresh = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "research", "--json"],
    );
    assert_eq!(fresh["created"], true);
    assert_ne!(fresh["id"], added["id"], "re-adding mints a fresh Wave");

    // Desktop retries by id; the old id never reaches the new Wave at its address.
    let by_old_id = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", added["id"].as_str().unwrap(), "--json"],
    );
    assert_eq!(by_old_id["already_removed"], true);
    assert_eq!(live_waves(repo.path(), home.path()), ["research"]);
    assert!(repo.path().join(".lf/wave/research").exists());
}

#[test]
fn removing_a_wave_with_work_moves_open_tasks_and_keeps_history() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let desktop = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "desktop", "--json"],
    );
    let unstarted = create_task(repo.path(), home.path(), "desktop", "Backlog idea");
    let started = create_task(repo.path(), home.path(), "desktop", "Started work");
    let placed = lf(repo.path(), home.path(), &["checkout", &started, "--json"]);
    let closed = create_task(repo.path(), home.path(), "desktop", "Closed work");
    lf(
        repo.path(),
        home.path(),
        &["task", "abandon", &closed, "--json"],
    );

    let preview = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "desktop", "--preview", "--json"],
    );
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
    lf(
        repo.path(),
        home.path(),
        &["task", "move", &unstarted, "--no-wave"],
    );
    let removed = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "desktop", "--json"],
    );
    assert_eq!(removed["moved"].as_array().unwrap().len(), 1);
    assert_eq!(
        wave_of(repo.path(), home.path(), &closed),
        desktop["id"],
        "closed work keeps its history"
    );
    for task in [&unstarted, &started] {
        assert!(
            wave_of(repo.path(), home.path(), task).is_null(),
            "{task} left the Wave"
        );
    }
    let after = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", &started, "--json"],
    );
    assert_eq!(after["execution"]["worktree"], placed["worktree"]);
    assert!(live_waves(repo.path(), home.path()).is_empty());

    let show = lf(
        repo.path(),
        home.path(),
        &["wave", "show", removed["id"].as_str().unwrap(), "--json"],
    );
    assert!(!show.is_null(), "the removed Wave stays readable by id");
}

#[test]
fn removing_a_child_wave_moves_tasks_to_its_parent_and_folds_memory() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let product = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product/desktop", "--json"],
    );
    let child = repo.path().join(".lf/wave/product/desktop");
    std::fs::write(child.join("MEMORY.md"), "Keep sidebars narrow.\n").unwrap();
    std::fs::write(
        child.join("config.yaml"),
        format!(
            "{}graph: auto\n",
            std::fs::read_to_string(child.join("config.yaml")).unwrap()
        ),
    )
    .unwrap();
    let task = create_task(repo.path(), home.path(), "product/desktop", "Sidebar");

    let removed = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "product/desktop", "--json"],
    );
    assert_eq!(removed["destination"], "product");
    assert_eq!(wave_of(repo.path(), home.path(), &task), product["id"]);
    let memory = std::fs::read_to_string(repo.path().join(".lf/wave/product/MEMORY.md")).unwrap();
    assert!(
        memory.contains("## From product/desktop (removed "),
        "{memory}"
    );
    assert!(memory.contains("Keep sidebars narrow."), "{memory}");
    let config = std::fs::read_to_string(repo.path().join(".lf/wave/product/config.yaml")).unwrap();
    assert!(config.contains("graph: auto"), "{config}");
    assert!(!child.exists());
    assert_eq!(live_waves(repo.path(), home.path()), ["product"]);
}

#[test]
fn delete_backlog_deletes_only_unstarted_tasks() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    lf(repo.path(), home.path(), &["wave", "add", "ops", "--json"]);
    let unstarted = create_task(repo.path(), home.path(), "ops", "Idea");
    let started = create_task(repo.path(), home.path(), "ops", "Work");
    lf(repo.path(), home.path(), &["checkout", &started, "--json"]);

    let removed = lf(
        repo.path(),
        home.path(),
        &["wave", "remove", "ops", "--delete-backlog", "--json"],
    );
    assert_eq!(removed["deleted"][0]["id"], unstarted.as_str());
    assert_eq!(removed["moved"][0]["id"], started.as_str());
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]).to_string();
    assert!(!listed.contains(&unstarted), "{listed}");
    assert!(listed.contains(&started), "{listed}");
    assert!(wave_of(repo.path(), home.path(), &started).is_null());
}

#[test]
fn rename_keeps_identity_moves_files_on_main_and_refuses_a_taken_name() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let added = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product", "--json"],
    );
    lf(repo.path(), home.path(), &["wave", "add", "ops", "--json"]);
    std::fs::write(
        repo.path().join(".lf/wave/ops/MEMORY.md"),
        "See .lf/wave/product/GOAL.md; product owns Desktop.\n",
    )
    .unwrap();
    let task = create_task(repo.path(), home.path(), "product", "Keep me");

    let taken = command(
        repo.path(),
        home.path(),
        &["wave", "rename", "product", "ops"],
    )
    .output()
    .unwrap();
    assert!(!taken.status.success());
    assert!(String::from_utf8_lossy(&taken.stderr).contains("ops already exists"));
    std::fs::create_dir_all(repo.path().join(".lf/wave/stray")).unwrap();
    let stray = command(
        repo.path(),
        home.path(),
        &["wave", "rename", "product", "stray"],
    )
    .output()
    .unwrap();
    assert!(String::from_utf8_lossy(&stray.stderr).contains(".lf/wave/stray already exists"));
    assert_eq!(
        live_waves(repo.path(), home.path()),
        ["ops", "product"],
        "nothing moved"
    );

    let renamed = lf(
        repo.path(),
        home.path(),
        &["wave", "rename", "product", "prod", "--json"],
    );
    assert_eq!(renamed["id"], added["id"]);
    assert_eq!(renamed["files"]["state"], "committed");
    assert!(committed(&repo, ".lf/wave/prod/GOAL.md"));
    assert!(!repo.path().join(".lf/wave/product").exists());
    assert_eq!(wave_of(repo.path(), home.path(), &task), added["id"]);
    assert_eq!(
        std::fs::read_to_string(repo.path().join(".lf/wave/ops/MEMORY.md")).unwrap(),
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

    let again = lf(
        repo.path(),
        home.path(),
        &["wave", "rename", "product", "prod", "--json"],
    );
    assert_eq!(again["renamed"], false);
}

#[test]
fn a_repository_joining_the_portfolio_registers_its_authored_waves_but_not_removed_ones() {
    let origin = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let archive = lf(
        origin.path(),
        home.path(),
        &["wave", "add", "archive", "--json"],
    );
    let clone_root = tempfile::tempdir().unwrap();
    let clone = clone_root.path().join("clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(origin.path())
        .arg(&clone)
        .output()
        .unwrap();
    assert!(cloned.status.success());
    // The clone still has .lf/wave/archive; the store retires that id.
    lf(
        origin.path(),
        home.path(),
        &["wave", "remove", "archive", "--json"],
    );
    let research = clone.join(".lf/wave/research");
    std::fs::create_dir_all(&research).unwrap();
    std::fs::write(research.join("GOAL.md"), "## Objective\n\nResearch\n").unwrap();
    assert!(
        live_waves(&clone, home.path()).is_empty(),
        "a read registers nothing"
    );

    lf(
        home.path(),
        home.path(),
        &["portfolio", "add", clone.to_str().unwrap(), "--json"],
    );
    assert_eq!(live_waves(&clone, home.path()), ["research"]);
    let listing = lf(&clone, home.path(), &["wave", "list", "--json"]);
    assert!(
        !listing
            .to_string()
            .contains(archive["id"].as_str().unwrap()),
        "the removed Wave's directory stays removed"
    );
    assert!(
        !committed(&origin, ".lf/wave/archive/GOAL.md")
            && clone.join(".lf/wave/archive/GOAL.md").is_file(),
        "only the clone still has the directory"
    );
}
