//! PRD-43: one repository Team, stable Project ownership, and a fail-closed migration.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::ops::pm::{canonical_wave_title_path, list_local_waves};
use loopflow::pm::PmSnapshot;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::PmSnapshotRow;
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

fn write_goal(repo: &Path, wave: &str, initiative: &str, legacy: bool) {
    let directory = repo.join("wave").join(wave);
    std::fs::create_dir_all(&directory).unwrap();
    let legacy = if legacy {
        "  provider: linear\n  linear_team: team-old\n"
    } else {
        ""
    };
    std::fs::write(
        directory.join("GOAL.md"),
        format!(
            "---\npm:\n{legacy}  linear_initiative: {initiative}\n---\n\n## Objective\n\nTest {wave}.\n"
        ),
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
) -> PmSnapshot {
    serde_json::from_value(serde_json::json!({
        "projects": [{
            "id": project_id,
            "slug": project_slug,
            "name": project_name,
            "summary": "Fixture project",
            "metric_targets": [],
            "workflow": "feature", "status": "started",
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
    snapshot: PmSnapshot,
) {
    let registered = store
        .get_wave_at(&WaveLocator::discover(repo, wave).unwrap())
        .unwrap()
        .expect("registered Wave");
    store
        .put_pm_snapshot(&PmSnapshotRow {
            wave_id: registered.id().clone(),
            provider: "linear".to_string(),
            initiative: initiative.to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            snapshot,
        })
        .unwrap();
}

fn lf_command(home: &Path, repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
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
        "pm:\n  provider: linear\n  linear_team: team-loo\n",
    )
    .unwrap();
    write_goal(repo, "survival", "initiative-survival", false);
    write_goal(
        repo,
        "survival/infrastructure",
        "initiative-infrastructure",
        false,
    );
    write_goal(repo, "intelligence", "initiative-intelligence", false);
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
    put_snapshot(
        &store,
        &repo,
        "survival",
        "initiative-survival",
        snapshot(
            "initiative-survival",
            "project-survival",
            "a-real-task",
            "A real task reaches done",
            "issue-survival",
            "LOO-1",
            true,
        ),
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
        let show = run_lf(&home, &repo, &["wave", "status", wave, "--json"]);
        let stdout = assert_success(&show, "Wave status");
        assert!(stdout.contains(issue), "{wave} snapshot lost {issue}");

        let checkout = run_lf(&home, &repo, &["task", "checkout", issue]);
        let error = String::from_utf8_lossy(&checkout.stderr);
        assert!(!checkout.status.success());
        assert!(
            error.contains("terminal planning state"),
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
        &run_lf(&home, &repo, &["wave", "show", "--json"]),
        "roadmap",
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
    let accepted = reopened.pm_snapshot(survival.id()).unwrap().unwrap();
    let mut planning = accepted.clone();
    planning.snapshot.projects.extend(ambiguous.projects);
    planning.snapshot.items.extend(ambiguous.items);
    let error = reopened.put_pm_snapshot(&planning).unwrap_err();
    assert!(
        error.to_string().contains("does not belong to Initiative"),
        "{error}"
    );
    assert_eq!(
        reopened
            .pm_snapshot(survival.id())
            .unwrap()
            .unwrap()
            .snapshot,
        accepted.snapshot
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

    // Legacy provider configuration cannot prevent a local save. Delivery
    // remains pending until the connection is repaired.
    let legacy_repo = fixture.path().join("legacy");
    std::fs::create_dir_all(legacy_repo.join(".lf")).unwrap();
    std::fs::write(
        legacy_repo.join(".lf/config.yaml"),
        "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
    )
    .unwrap();
    write_goal(&legacy_repo, "product", "initiative-product", true);
    git(&legacy_repo, &["init", "-b", "main"]);
    git(
        &legacy_repo,
        &["config", "user.email", "matrix@loopflow.test"],
    );
    git(
        &legacy_repo,
        &["config", "user.name", "Repository Team Matrix"],
    );
    git(
        &legacy_repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/loopflowstudio/legacy-fixture.git",
        ],
    );
    git(&legacy_repo, &["add", "."]);
    git(&legacy_repo, &["commit", "-m", "seed legacy fixture"]);
    let legacy_store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let legacy_locator = WaveLocator::discover(&legacy_repo, "product").unwrap();
    legacy_store
        .create_wave(&Wave::new(
            WaveId::new(),
            "product".to_string(),
            legacy_locator.repo().to_string(),
        ))
        .unwrap();
    put_snapshot(
        &legacy_store,
        &legacy_repo,
        "product",
        "initiative-product",
        snapshot(
            "initiative-product",
            "project-api",
            "loopflow-api",
            "Loopflow API",
            "issue-legacy",
            "OLD-1",
            true,
        ),
    );
    let legacy_wave = legacy_store.get_wave_at(&legacy_locator).unwrap().unwrap();
    rusqlite::Connection::open(&database).unwrap().execute(
        "UPDATE waves SET current_project_id=(SELECT id FROM projects WHERE wave_id=?1 AND external_project_id='project-api') WHERE id=?1",
        [legacy_wave.id()],
    ).unwrap();
    drop(legacy_store);
    assert_success(
        &run_lf(
            &home,
            &legacy_repo,
            &["wave", "status", "product", "--json"],
        ),
        "legacy cached read",
    );
    let saved = run_lf(
        &home,
        &legacy_repo,
        &[
            "task",
            "create",
            "--wave",
            "product",
            "--title",
            "Retain offline work",
            "--json",
        ],
    );
    let item: serde_json::Value =
        serde_json::from_str(&assert_success(&saved, "save local Task")).unwrap();
    assert_eq!(item["name"], "Retain offline work");
    assert!(String::from_utf8_lossy(&saved.stderr).contains("pending Linear sync"));

    // PRD-44 leaves the repository Team as the sole PM authority after the
    // provider migration verifies successfully.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = std::fs::read_to_string(source.join(".lf/config.yaml")).unwrap();
    assert!(config.contains("linear_team:"));
    assert!(!config.contains("linear:\n  team:"));
    for wave in ["infrastructure", "intelligence", "product"] {
        let goal = std::fs::read_to_string(source.join(format!("wave/{wave}/GOAL.md"))).unwrap();
        assert!(!goal.contains("provider: linear"), "{wave}");
        assert!(!goal.contains("linear_team:"), "{wave}");
    }

    println!("one Team team-loo: Survival — A real task reaches done; Survival / Infrastructure — Gmail; LOO-1 and LOO-2 resolve independently");
}
