//! `lf wave add|remove|rename`: one Wave lifecycle for the CLI and Desktop.

use std::os::unix::fs::{symlink, PermissionsExt};
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
        self.lf(&["task", "create", "--wave", wave, "--title", title, "--json"])["task"]["id"]
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
        !git(self, &["ls-files", "--", path]).is_empty()
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
    git(&lab, &["add", ".lf/wave/research/GOAL.md"]);
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
    assert!(git(&lab, &["diff", "--cached", "--name-only"]).is_empty());
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
    lab.lf(&["wave", "add", "product/cli", "--json"]);
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
    // Folding into product must not commit its other documents or sibling Waves.
    let unrelated = [".lf/wave/product/GOAL.md", ".lf/wave/product/cli/MEMORY.md"];
    for path in unrelated {
        std::fs::write(lab.path().join(path), "An unrelated edit.\n").unwrap();
    }

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
    assert_eq!(lab.live_waves(), ["product", "product/cli"]);
    assert_eq!(
        git(&lab, &["diff", "HEAD", "--name-only"]),
        unrelated.join("\n") + "\n"
    );
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
    assert_eq!(
        renamed["summary"],
        serde_json::json!([]),
        "ordinary prose needs no cleanup receipt"
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

fn git(lab: &Lab, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(lab.path())
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {}", stderr(&output));
    String::from_utf8(output.stdout).unwrap()
}

fn bind_initiative(lab: &Lab, wave: &str, initiative: &str) {
    let path = lab.path().join(format!(".lf/wave/{wave}/config.yaml"));
    let config = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        format!("{config}linear:\n  initiative: {initiative}\n"),
    )
    .unwrap();
}

fn fail_index_reset(lab: &Lab, args: &[&str]) {
    let shim = tempfile::tempdir().unwrap();
    let path = std::env::var_os("PATH").unwrap();
    let real_git = std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .unwrap();
    let script = shim.path().join("git");
    std::fs::write(
        &script,
        "#!/bin/sh\nfor arg do\n  if [ \"$arg\" = reset ]; then\n    echo 'injected index reset failure' >&2\n    exit 1\n  fi\ndone\nexec \"$TEST_REAL_GIT\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let search = std::iter::once(shim.path().to_path_buf()).chain(std::env::split_paths(&path));
    let output = lab
        .command(lab.path(), args)
        .env("PATH", std::env::join_paths(search).unwrap())
        .env("TEST_REAL_GIT", real_git)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["files"]["state"], "uncommitted", "{receipt}");
    assert!(receipt["files"]["reason"]
        .as_str()
        .unwrap()
        .contains("injected index reset failure"));
}

#[test]
fn lifecycle_retry_reconciles_index_after_commit_preserving_later_edits() {
    for args in [
        vec!["wave", "remove", "research", "--json"],
        vec!["wave", "rename", "research", "ops", "--json"],
    ] {
        let lab = Lab::new();
        lab.lf(&["wave", "add", "research", "--json"]);
        lab.lf(&["wave", "add", "notes", "--json"]);
        let memory = ".lf/wave/notes/MEMORY.md";
        std::fs::write(lab.path().join(memory), "See .lf/wave/research/GOAL.md\n").unwrap();
        std::fs::write(lab.path().join("unrelated.txt"), "Keep staged.\n").unwrap();
        git(&lab, &["add", "unrelated.txt"]);

        fail_index_reset(&lab, &args);
        let committed = git(&lab, &["rev-parse", "HEAD"]);
        assert!(git(&lab, &["ls-tree", "HEAD", ".lf/wave/research/"]).is_empty());
        assert_eq!(
            git(&lab, &["status", "--porcelain", "--", ".lf/wave/research/"])
                .lines()
                .filter(|line| line.starts_with("AD "))
                .count(),
            3
        );
        // Even a second reset failure must leave the operation retryable.
        fail_index_reset(&lab, &args);
        std::fs::write(lab.path().join(memory), "Later staged thought.\n").unwrap();
        git(&lab, &["add", memory]);
        std::fs::write(lab.path().join(memory), "Later working thought.\n").unwrap();

        let retry = lab.lf(&args);
        assert_eq!(retry["files"]["state"], "unchanged", "{retry}");
        assert_eq!(git(&lab, &["rev-parse", "HEAD"]), committed);
        assert!(!lab.path().join(".lf/wave/research").exists());
        assert_eq!(
            git(&lab, &["diff", "--cached", "--name-only"]),
            format!("{memory}\nunrelated.txt\n")
        );
        assert_eq!(
            git(&lab, &["show", &format!(":{memory}")]),
            "Later staged thought.\n"
        );
        assert_eq!(
            std::fs::read_to_string(lab.path().join(memory)).unwrap(),
            "Later working thought.\n"
        );
        assert_eq!(lab.lf(&args)["files"]["state"], "unchanged");
    }
}

#[test]
fn lifecycle_retry_reconciles_index_without_touching_replacement_waves() {
    for args in [
        vec!["wave", "remove", "research", "--json"],
        vec!["wave", "rename", "research", "ops", "--json"],
    ] {
        let lab = Lab::new();
        let original = lab.lf(&["wave", "add", "research", "--json"]);
        lab.lf(&["wave", "add", "notes", "--json"]);
        let memory = ".lf/wave/notes/MEMORY.md";
        std::fs::write(lab.path().join(memory), "See .lf/wave/research/GOAL.md\n").unwrap();
        fail_index_reset(&lab, &args);

        let replacement = lab.lf(&[
            "wave",
            "add",
            "research",
            "--objective",
            "Replacement",
            "--json",
        ]);
        assert_ne!(original["id"], replacement["id"]);
        let goal = ".lf/wave/research/GOAL.md";
        std::fs::write(lab.path().join(goal), "Replacement staged thought.\n").unwrap();
        git(&lab, &["add", goal]);
        let replacement_index = git(&lab, &["ls-files", "--stage", "--", ".lf/wave/research/"]);
        let committed = git(&lab, &["rev-parse", "HEAD"]);

        let retry = lab.lf(&args);
        assert_eq!(retry["id"], original["id"]);
        assert_eq!(retry["files"]["state"], "unchanged", "{retry}");
        assert_eq!(git(&lab, &["rev-parse", "HEAD"]), committed);
        assert_eq!(
            git(&lab, &["ls-files", "--stage", "--", ".lf/wave/research/"]),
            replacement_index
        );
        assert_eq!(
            git(&lab, &["diff", "--cached", "--name-only"]),
            format!("{goal}\n")
        );
        assert_eq!(
            std::fs::read_to_string(lab.path().join(goal)).unwrap(),
            "Replacement staged thought.\n"
        );
        assert_eq!(
            lab.lf(&["wave", "show", "research", "--json"])["waves"][0]["wave"]["id"],
            replacement["id"]
        );
    }
}

#[test]
fn removal_retry_retains_descendant_archive_bindings_after_retirement_and_replacement() {
    let lab = Lab::new();
    let parent = lab.lf(&["wave", "add", "parent", "--json"]);
    lab.lf(&["wave", "add", "parent/child", "--json"]);
    bind_initiative(&lab, "parent/child", "original-child-initiative");
    for _ in 0..2 {
        let removed = lab.lf(&["wave", "remove", "parent", "--json"]);
        assert_eq!(
            removed["waves"],
            serde_json::json!(["parent", "parent/child"])
        );
        assert_eq!(
            removed["initiatives"][0]["initiative"],
            "original-child-initiative"
        );
        assert!(removed["initiatives"][0]["error"].is_string());
        assert_eq!(removed["files"]["state"], "uncommitted");
        assert!(!removed["summary"]
            .to_string()
            .contains("removed on the main checkout"));
        assert!(lab
            .path()
            .join(".lf/wave/parent/child/config.yaml")
            .exists());
    }
    let replacement = lab.lf(&["wave", "add", "parent", "--json"]);
    assert_ne!(replacement["id"], parent["id"]);
    bind_initiative(&lab, "parent", "replacement-initiative");
    let config = std::fs::read(lab.path().join(".lf/wave/parent/config.yaml")).unwrap();
    let retried = lab.lf(&["wave", "remove", "parent", "--json"]);
    assert_eq!(retried["id"], parent["id"]);
    assert_eq!(retried["initiatives"].as_array().unwrap().len(), 1);
    assert_eq!(
        retried["initiatives"][0]["initiative"],
        "original-child-initiative"
    );
    assert_eq!(
        std::fs::read(lab.path().join(".lf/wave/parent/config.yaml")).unwrap(),
        config
    );
    assert_eq!(lab.live_waves(), ["parent"]);
}

#[test]
fn removal_retry_commits_saved_folds_and_references_preserving_later_edits() {
    let lab = Lab::new();
    lab.lf(&["wave", "add", "parent/child", "--json"]);
    lab.lf(&["wave", "add", "notes", "--json"]);
    std::fs::write(
        lab.path().join(".lf/wave/parent/child/MEMORY.md"),
        "Remember this once.\n",
    )
    .unwrap();
    std::fs::write(
        lab.path().join(".lf/wave/notes/MEMORY.md"),
        "See .lf/wave/parent/child/GOAL.md\n",
    )
    .unwrap();
    let branch = git(&lab, &["branch", "--show-current"]);
    git(&lab, &["checkout", "--detach"]);
    let first = lab.lf(&["wave", "remove", "parent/child", "--json"]);
    assert_eq!(first["files"]["state"], "uncommitted");
    let memory_path = ".lf/wave/parent/MEMORY.md";
    let folded = std::fs::read_to_string(lab.path().join(memory_path)).unwrap();
    assert_eq!(folded.matches("Remember this once.").count(), 1);
    std::fs::write(
        lab.path().join(memory_path),
        format!("{folded}\nLater staged thought.\n"),
    )
    .unwrap();
    git(&lab, &["add", memory_path]);
    std::fs::write(lab.path().join("unrelated.txt"), "Leave staged.\n").unwrap();
    git(&lab, &["add", "unrelated.txt"]);
    git(&lab, &["checkout", branch.trim()]);
    let again = lab.lf(&["wave", "remove", "parent/child", "--json"]);
    assert_eq!(again["files"]["state"], "committed", "{again}");
    assert_eq!(git(&lab, &["show", &format!("HEAD:{memory_path}")]), folded);
    assert!(git(&lab, &["diff", "--cached"]).contains("Later staged thought."));
    assert!(git(&lab, &["diff", "--cached", "--name-only"]).contains("unrelated.txt"));
    assert_eq!(
        git(&lab, &["show", "HEAD:.lf/wave/notes/MEMORY.md"]),
        "See .lf/wave/parent/GOAL.md\n"
    );
    assert!(!lab.committed(".lf/wave/parent/child/config.yaml"));
    assert_eq!(
        lab.lf(&["wave", "remove", "parent/child", "--json"])["files"]["state"],
        "unchanged"
    );
}

#[test]
fn rename_retry_finishes_saved_move_and_rejects_an_unrelated_target() {
    let lab = Lab::new();
    let original = lab.lf(&["wave", "add", "research", "--json"]);
    lab.lf(&["wave", "add", "notes", "--json"]);
    std::fs::write(
        lab.path().join(".lf/wave/notes/MEMORY.md"),
        "See .lf/wave/research/GOAL.md\n",
    )
    .unwrap();
    let branch = git(&lab, &["branch", "--show-current"]);
    git(&lab, &["checkout", "--detach"]);
    let first = lab.lf(&["wave", "rename", "research", "ops", "--json"]);
    assert_eq!(first["files"]["state"], "uncommitted");
    let goal = std::fs::read_to_string(lab.path().join(".lf/wave/ops/GOAL.md")).unwrap();
    std::fs::write(
        lab.path().join(".lf/wave/ops/GOAL.md"),
        format!("{goal}\nLater edit.\n"),
    )
    .unwrap();
    git(&lab, &["checkout", branch.trim()]);
    let again = lab.lf(&["wave", "rename", "research", "ops", "--json"]);
    assert_eq!(again["id"], original["id"]);
    assert_eq!(again["files"]["state"], "committed", "{again}");
    assert_eq!(git(&lab, &["show", "HEAD:.lf/wave/ops/GOAL.md"]), goal);
    assert!(git(&lab, &["diff"]).contains("Later edit."));
    assert_eq!(
        git(&lab, &["show", "HEAD:.lf/wave/notes/MEMORY.md"]),
        "See .lf/wave/ops/GOAL.md\n"
    );
    let unrelated = lab.run(&["wave", "rename", "never-existed", "ops", "--json"]);
    assert!(!unrelated.status.success());
    assert!(stderr(&unrelated).contains("no Wave never-existed"));
}

#[test]
fn rename_preserves_authored_bytes_modes_and_symlinks() {
    let lab = Lab::new();
    lab.lf(&["wave", "add", "research", "--json"]);
    let source = lab.path().join(".lf/wave/research");
    // The bracketed filename is literal, not a pattern selecting notes1.md.
    std::fs::write(source.join("notes[1].md"), [0, 255, 10]).unwrap();
    std::fs::write(source.join("notes1.md"), "A different document.\n").unwrap();
    std::fs::write(source.join("measure"), "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(
        source.join("measure"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    symlink("notes[1].md", source.join("latest")).unwrap();
    git(&lab, &["add", ".lf/wave/research"]);
    git(&lab, &["commit", "-m", "Authored Wave files"]);

    let renamed = lab.lf(&["wave", "rename", "research", "ops", "--json"]);
    assert_eq!(renamed["files"]["state"], "committed", "{renamed}");
    let target = lab.path().join(".lf/wave/ops");
    assert_eq!(
        std::fs::read(target.join("notes[1].md")).unwrap(),
        [0, 255, 10]
    );
    assert_eq!(
        std::fs::read_link(target.join("latest")).unwrap(),
        Path::new("notes[1].md")
    );
    assert!(git(&lab, &["ls-tree", "HEAD", ".lf/wave/ops/measure"]).starts_with("100755"));
    assert!(git(&lab, &["ls-tree", "HEAD", ".lf/wave/ops/latest"]).starts_with("120000"));
    assert!(git(&lab, &["status", "--porcelain"]).is_empty());
}

#[test]
fn removal_retry_leaves_a_replacement_at_the_deleted_address_intact() {
    let lab = Lab::new();
    let original = lab.lf(&["wave", "add", "research", "--json"]);
    let branch = git(&lab, &["branch", "--show-current"]);
    git(&lab, &["checkout", "--detach"]);
    assert_eq!(
        lab.lf(&["wave", "remove", "research", "--json"])["files"]["state"],
        "uncommitted"
    );
    let replacement = lab.lf(&[
        "wave",
        "add",
        "research",
        "--objective",
        "Replacement work",
        "--json",
    ]);
    let files = std::fs::read(lab.path().join(".lf/wave/research/GOAL.md")).unwrap();
    git(&lab, &["checkout", branch.trim()]);
    let retry = lab.lf(&["wave", "remove", original["id"].as_str().unwrap(), "--json"]);
    assert_eq!(retry["files"]["state"], "unchanged", "{retry}");
    assert_eq!(
        std::fs::read(lab.path().join(".lf/wave/research/GOAL.md")).unwrap(),
        files
    );
    assert_eq!(lab.live_waves(), ["research"]);
    assert_ne!(original["id"], replacement["id"]);
    assert!(git(&lab, &["diff"]).contains("Replacement work"));
}

#[test]
fn rename_retry_preserves_a_reused_source_and_the_original_provider_binding() {
    let lab = Lab::new();
    let original = lab.lf(&["wave", "add", "research", "--json"]);
    bind_initiative(&lab, "research", "original-initiative");
    let branch = git(&lab, &["branch", "--show-current"]);
    git(&lab, &["checkout", "--detach"]);
    let first = lab.lf(&[
        "wave",
        "rename",
        "research",
        "ops",
        "--title",
        "Operations",
        "--json",
    ]);
    assert_eq!(first["files"]["state"], "uncommitted");
    assert!(first["initiative"]["error"].is_string());
    let replacement = lab.lf(&["wave", "add", "research", "--json"]);
    bind_initiative(&lab, "research", "replacement-initiative");
    let replacement_config =
        std::fs::read(lab.path().join(".lf/wave/research/config.yaml")).unwrap();
    // The current target binding also cannot redirect the outstanding provider effect.
    let target_config = lab.path().join(".lf/wave/ops/config.yaml");
    let changed_binding = std::fs::read_to_string(&target_config)
        .unwrap()
        .replace("original-initiative", "later-initiative");
    std::fs::write(&target_config, &changed_binding).unwrap();
    git(&lab, &["checkout", branch.trim()]);
    let again = lab.lf(&["wave", "rename", "research", "ops", "--json"]);
    assert_eq!(again["id"], original["id"]);
    assert_eq!(again["files"]["state"], "committed", "{again}");
    assert_eq!(again["initiative"]["initiative"], "original-initiative");
    assert_eq!(again["initiative"]["title"], "Operations");
    assert!(again["initiative"]["error"].is_string());
    assert_eq!(
        std::fs::read(&target_config).unwrap(),
        changed_binding.as_bytes()
    );
    assert_eq!(
        std::fs::read(lab.path().join(".lf/wave/research/config.yaml")).unwrap(),
        replacement_config
    );
    assert_ne!(original["id"], replacement["id"]);
    assert_eq!(lab.live_waves(), ["ops", "research"]);
}
