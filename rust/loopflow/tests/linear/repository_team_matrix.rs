//! Repository Team bindings preserve independent Wave and Project ownership.

use crate::test_linear_input::{Inventory, InventoryFixture, SeedLinearSqlite};
use std::path::Path;
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::ops::linear::{canonical_wave_title_path, list_local_waves};
use loopflow::store::sqlite::SqliteStore;
use loopflow::work::wave::{Wave, WaveLocator};

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_goal(repo: &Path, wave: &str, initiative: &str) {
    let directory = repo.join(".lf/wave").join(wave);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("GOAL.md"),
        format!("---\nlinear:\n  initiative: {initiative}\n---\n\n## Objective\n\nTest {wave}.\n"),
    )
    .unwrap();
}

fn snapshot(
    initiative: &str,
    project_id: &str,
    project_slug: &str,
    project_name: &str,
    issue_id: &str,
    identifier: &str,
    completed: bool,
) -> Inventory {
    serde_json::from_value(serde_json::json!({
        "projects": [{
            "id": project_id,
            "slug": project_slug,
            "name": project_name,
            "summary": "Fixture project",
            "metric_targets": [],
            "status": "started",
            "krs": [{ "text": "Ownership is deterministic", "holds": true }],
            "initiative_ids": [initiative],
            "team_ids": ["team-loo"]
        }],
        "items": [{
            "id": issue_id,
            "identifier": identifier,
            "url": null,
            "name": format!("Task {identifier}"),
            "description": "",
            "rank": 0,
            "completed": completed,
            "project_id": project_id,
            "project": project_slug,
            "team_id": "team-loo",
            "assignee": null
        }]
    }))
    .unwrap()
}

fn put_snapshot(
    store: &SqliteStore,
    repo: &Path,
    wave: &str,
    initiative: &str,
    snapshot: Inventory,
) {
    let registered = store
        .get_wave_at(&WaveLocator::discover(repo, wave).unwrap())
        .unwrap()
        .expect("registered Wave");
    let goal = std::fs::read_to_string(repo.join(".lf/wave").join(wave).join("GOAL.md"))
        .unwrap_or_else(|_| {
            format!("---\nlinear:\n  initiative: {initiative}\n---\nFixture Wave.\n")
        });
    std::fs::create_dir_all(repo.join(".lf/wave").join(wave)).unwrap();
    std::fs::write(repo.join(".lf/wave").join(wave).join("GOAL.md"), goal).unwrap();
    store
        .seed_inventory(&InventoryFixture {
            wave_id: registered.id().clone(),
            provider: "linear".to_string(),
            initiative: initiative.to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            snapshot,
        })
        .unwrap();
}

fn lf_command(home: &Path, repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("HOME", home)
        .env_remove("LF_WAVE_ID");
    command
}

fn run_lf(home: &Path, repo: &Path, args: &[&str]) -> Output {
    lf_command(home, repo, args).output().expect("run lf")
}

fn assert_success(output: &Output, command: &str) -> String {
    assert!(
        output.status.success(),
        "{command} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn seed_repository(repo: &Path) {
    std::fs::create_dir_all(repo.join(".lf")).unwrap();
    std::fs::write(
        repo.join(".lf/config.yaml"),
        "planning:\n  provider: linear\n  team: team-loo\n",
    )
    .unwrap();
    write_goal(repo, "survival", "initiative-survival");
    write_goal(repo, "survival/infrastructure", "initiative-infrastructure");
    write_goal(repo, "intelligence", "initiative-intelligence");
    git(repo, &["init", "-b", "main"]);
    git(repo, &["config", "user.email", "matrix@loopflow.test"]);
    git(repo, &["config", "user.name", "Repository Team Matrix"]);
    git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/loopflowstudio/fixture.git",
        ],
    );
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "seed repository team fixture"]);
}

#[test]
fn repository_team_matrix() {
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path().join("home");
    let repo = fixture.path().join("fixture");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    seed_repository(&repo);

    let database = home.join("loopflow.db");
    let store = SqliteStore::new(&database).unwrap();
    let repo_locator = WaveLocator::discover(&repo, "survival").unwrap();
    let survival = Wave::new(
        WaveId::new(),
        "survival".to_string(),
        repo_locator.repo().to_string(),
    );
    let infrastructure = Wave::new(
        WaveId::new(),
        "infrastructure".to_string(),
        repo_locator.repo().to_string(),
    )
    .with_parent(survival.id().clone());
    let intelligence = Wave::new(
        WaveId::new(),
        "intelligence".to_string(),
        repo_locator.repo().to_string(),
    );
    store.create_wave(&survival).unwrap();
    store.create_wave(&infrastructure).unwrap();
    store.create_wave(&intelligence).unwrap();
    let foreign_repo = tempfile::tempdir().unwrap();
    let foreign_locator = WaveLocator::discover(foreign_repo.path(), "intelligence").unwrap();
    store
        .create_wave(&Wave::new(
            WaveId::new(),
            "intelligence".to_string(),
            foreign_locator.repo().to_string(),
        ))
        .unwrap();
    let survival_snapshot = snapshot(
        "initiative-survival",
        "project-survival",
        "a-real-task",
        "A real task reaches done",
        "issue-survival",
        "LOO-1",
        true,
    );
    put_snapshot(
        &store,
        &repo,
        "survival",
        "initiative-survival",
        survival_snapshot.clone(),
    );
    put_snapshot(
        &store,
        &repo,
        "survival/infrastructure",
        "initiative-infrastructure",
        snapshot(
            "initiative-infrastructure",
            "project-gmail",
            "gmail",
            "Gmail",
            "issue-gmail",
            "LOO-2",
            true,
        ),
    );
    put_snapshot(
        &store,
        &repo,
        "intelligence",
        "initiative-intelligence",
        snapshot(
            "initiative-intelligence",
            "project-trace",
            "trace",
            "Trace",
            "issue-intelligence",
            "LOO-3",
            true,
        ),
    );
    put_snapshot(
        &store,
        foreign_repo.path(),
        "intelligence",
        "initiative-intelligence",
        snapshot(
            "initiative-intelligence",
            "project-foreign",
            "foreign",
            "Foreign",
            "issue-foreign",
            "OTHER-1",
            false,
        ),
    );
    drop(store);

    let old_home = std::env::var_os("LF_HOME");
    std::env::set_var("LF_HOME", &home);
    // Stored discovery and durable ancestry make nested titles legible.
    assert_eq!(
        list_local_waves(&repo).unwrap(),
        ["intelligence", "survival", "survival/infrastructure"]
    );
    assert_eq!(
        canonical_wave_title_path(&repo, "survival/infrastructure").unwrap(),
        "Survival / Infrastructure"
    );
    assert_eq!(
        canonical_wave_title_path(&repo, "intelligence").unwrap(),
        "Intelligence"
    );
    // SAFETY: restore the process environment before exercising subprocesses.
    unsafe {
        match old_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
    }

    for (wave, issue) in [("survival", "LOO-1"), ("survival/infrastructure", "LOO-2")] {
        // Done Tasks leave the plan; the Wave's closed Tasks list them.
        let show = run_lf(
            &home,
            &repo,
            &["task", "list", "--done", "--wave", wave, "--json"],
        );
        let stdout = assert_success(&show, "Closed Tasks");
        assert!(
            stdout.contains(issue),
            "{wave} snapshot lost {issue}: {stdout}"
        );

        let checkout = run_lf(&home, &repo, &["task", "checkout", issue]);
        let error = String::from_utf8_lossy(&checkout.stderr);
        assert!(!checkout.status.success());
        assert!(
            error.contains("is done; `lf task reopen"),
            "unexpected task result: {error}"
        );
    }
    let status = assert_success(
        &run_lf(&home, &repo, &["wave", "list", "--json"]),
        "Wave list",
    );
    assert!(status.contains("survival"));
    assert!(status.contains("survival/infrastructure"));
    // Another repository's same-named Wave does not hide this repository's work.
    let roadmap = assert_success(
        &run_lf(&home, &repo, &["task", "list", "--done", "--json"]),
        "closed Tasks",
    );
    assert!(roadmap.contains("LOO-1"));
    assert!(roadmap.contains("LOO-2"));

    // Reopening the store preserves the local ancestry and foreign collision.
    let reopened = SqliteStore::new(&database).unwrap();
    assert_eq!(reopened.list_waves(None).unwrap().len(), 4);

    // Reject ambiguous ownership before replacing accepted planning facts.
    let mut ambiguous = snapshot(
        "initiative-infrastructure",
        "project-ambiguous",
        "a-real-task",
        "A real task reaches done",
        "issue-ambiguous",
        "LOO-4",
        false,
    );
    ambiguous.projects[0]
        .initiative_ids
        .push("initiative-survival".into());
    let survival = reopened
        .get_wave_at(&WaveLocator::discover(&repo, "survival").unwrap())
        .unwrap()
        .unwrap();
    let accepted = reopened.list_projects(Some(survival.id())).unwrap();
    let mut planning = InventoryFixture {
        wave_id: survival.id().clone(),
        provider: "linear".into(),
        initiative: "initiative-survival".into(),
        synced_at: 2,
        snapshot: survival_snapshot,
    };
    planning.snapshot.projects.extend(ambiguous.projects);
    planning.snapshot.items.extend(ambiguous.items);
    let error = reopened.seed_inventory(&planning).unwrap_err();
    assert!(
        error.to_string().contains("does not belong to Initiative"),
        "{error}"
    );
    assert_eq!(
        reopened.list_projects(Some(survival.id())).unwrap(),
        accepted
    );
    drop(reopened);
    for args in [
        &["wave", "list", "--json"][..],
        &["wave", "status", "survival", "--json"][..],
        &["wave", "show", "--json"][..],
        &["wave", "show", "survival", "--json"][..],
    ] {
        let output = assert_success(&run_lf(&home, &repo, args), "retained planning");
        assert!(!output.contains("LOO-4"));
    }
    assert!(!fixture
        .path()
        .join("fixture.a-real-task-reaches-done")
        .exists());
}
