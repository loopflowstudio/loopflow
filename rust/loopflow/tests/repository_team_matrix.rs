//! PRD-43: one repository Team, stable Project ownership, and a fail-closed migration.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow::id::WaveId;
use loopflow::ops::pm::{canonical_wave_title_path, list_local_waves};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::PmSnapshotRow;
use loopflow::wave::Wave;

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
) -> String {
    serde_json::json!({
        "projects": [{
            "id": project_id,
            "slug": project_slug,
            "name": project_name,
            "summary": "Fixture project",
            "definition": "A measured bet.",
            "flows": { "first": null, "loop": null, "finally": null },
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
    })
    .to_string()
}

fn put_snapshot(store: &SqliteStore, repo: &Path, wave: &str, initiative: &str, payload: String) {
    store
        .put_pm_snapshot(&PmSnapshotRow {
            repo: std::fs::canonicalize(repo).unwrap().display().to_string(),
            wave: wave.to_string(),
            provider: "linear".to_string(),
            initiative: initiative.to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            payload,
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
        .env_remove("LF_DB_PATH")
        .env_remove("LF_WAVE_ID");
    command
}

fn run_lf(home: &Path, repo: &Path, args: &[&str]) -> Output {
    lf_command(home, repo, args).output().expect("run lf")
}

fn run_project_control(home: &Path, repo: &Path, project: &str) -> Output {
    lf_command(home, repo, &["project", "run", project])
        .env("LF_BIN", repo.join("missing-lf"))
        .env_remove("LF_CONTROL_BIN")
        .output()
        .expect("run project control")
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
    let survival = Wave::new(
        WaveId::new(),
        "survival".to_string(),
        repo.display().to_string(),
    );
    let infrastructure = Wave::new(
        WaveId::new(),
        "survival/infrastructure".to_string(),
        repo.display().to_string(),
    )
    .with_parent(survival.id().clone());
    store.create_wave(&survival).unwrap();
    store.create_wave(&infrastructure).unwrap();
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
    drop(store);

    // Recursive discovery and durable ancestry make nested titles legible.
    assert_eq!(
        list_local_waves(&repo).unwrap(),
        ["survival", "survival/infrastructure"]
    );
    let old_home = std::env::var_os("LF_HOME");
    let old_db = std::env::var_os("LF_DB_PATH");
    // SAFETY: this integration binary contains one test; no sibling thread can
    // observe the temporary storage selection.
    unsafe {
        std::env::set_var("LF_HOME", &home);
        std::env::remove_var("LF_DB_PATH");
    }
    assert_eq!(
        canonical_wave_title_path(&repo, "survival/infrastructure").unwrap(),
        "Survival / Infrastructure"
    );
    // SAFETY: restore the process environment before exercising subprocesses.
    unsafe {
        match old_home {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
        match old_db {
            Some(value) => std::env::set_var("LF_DB_PATH", value),
            None => std::env::remove_var("LF_DB_PATH"),
        }
    }

    for (wave, issue) in [("survival", "LOO-1"), ("survival/infrastructure", "LOO-2")] {
        let show = run_lf(
            &home,
            &repo,
            &["pm", "show", "--wave", wave, "--no-sync", "--json"],
        );
        let stdout = assert_success(&show, "pm show");
        assert!(stdout.contains(issue), "{wave} snapshot lost {issue}");

        let run = run_lf(&home, &repo, &["task", "run", issue]);
        let error = String::from_utf8_lossy(&run.stderr);
        assert!(!run.status.success());
        assert!(
            error.contains("already complete"),
            "unexpected task result: {error}"
        );
    }
    let status = assert_success(&run_lf(&home, &repo, &["pm", "status"]), "pm status");
    assert!(status.contains("survival"));
    assert!(status.contains("survival/infrastructure"));
    let roadmap = assert_success(&run_lf(&home, &repo, &["roadmap", "--json"]), "roadmap");
    assert!(roadmap.contains("LOO-1"));
    assert!(roadmap.contains("LOO-2"));

    // Project controls resolve each stable Project id to its own Wave before
    // the deliberately missing worker binary stops the fixture from launching.
    let control_home = fixture.path().join("project-control-home");
    std::fs::create_dir_all(&control_home).unwrap();
    let control_store = SqliteStore::new(&control_home.join("loopflow.db")).unwrap();
    let control_survival = Wave::new(
        WaveId::new(),
        "survival".to_string(),
        repo.display().to_string(),
    );
    let control_infrastructure = Wave::new(
        WaveId::new(),
        "survival/infrastructure".to_string(),
        repo.display().to_string(),
    )
    .with_parent(control_survival.id().clone());
    control_store.create_wave(&control_survival).unwrap();
    control_store.create_wave(&control_infrastructure).unwrap();
    put_snapshot(
        &control_store,
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
        &control_store,
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
    drop(control_store);
    for (project_id, wave_id) in [
        ("project-survival", control_survival.id()),
        ("project-gmail", control_infrastructure.id()),
    ] {
        let output = run_project_control(&control_home, &repo, project_id);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            error.contains("cannot resolve current lf binary"),
            "{error}"
        );
        let control_store = SqliteStore::new(&control_home.join("loopflow.db")).unwrap();
        let project = control_store
            .project_by_project(project_id)
            .unwrap()
            .expect("Project control reserved the resolved Project");
        assert_eq!(&project.wave_id, wave_id);
    }

    // Reopening the store preserves the one Team and both stable associations.
    let reopened = SqliteStore::new(&database).unwrap();
    assert_eq!(reopened.list_waves(None).unwrap().len(), 2);

    // A duplicated Project/Issue association fails before Work or worktree creation.
    put_snapshot(
        &reopened,
        &repo,
        "survival/infrastructure",
        "initiative-infrastructure",
        snapshot(
            "initiative-infrastructure",
            "project-survival",
            "a-real-task",
            "A real task reaches done",
            "issue-survival",
            "LOO-1",
            false,
        ),
    );
    drop(reopened);
    let duplicate = run_lf(&home, &repo, &["task", "run", "LOO-1"]);
    let error = String::from_utf8_lossy(&duplicate.stderr);
    assert!(error.contains("belongs to both"), "{error}");
    for args in [
        &["pm", "status"][..],
        &["status", "survival", "--json"][..],
        &["roadmap", "--json"][..],
        &["roadmap", "--wave", "survival", "--json"][..],
        &["project", "run", "project-survival"][..],
    ] {
        let output = run_lf(&home, &repo, args);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{} accepted ambiguous ownership",
            args.join(" ")
        );
        assert!(
            error.contains("belongs to both") || error.contains("belongs to 2"),
            "{} returned an unrelated error: {error}",
            args.join(" ")
        );
    }
    assert!(!fixture
        .path()
        .join("fixture.a-real-task-reaches-done")
        .exists());

    // The capable PR leaves a legacy repository readable but blocks mutations
    // with the PRD-44 handoff before any provider call.
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
    legacy_store
        .create_wave(&Wave::new(
            WaveId::new(),
            "product".to_string(),
            legacy_repo.display().to_string(),
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
    drop(legacy_store);
    assert_success(
        &run_lf(
            &home,
            &legacy_repo,
            &["pm", "show", "--wave", "product", "--no-sync", "--json"],
        ),
        "legacy cached read",
    );
    let blocked = run_lf(
        &home,
        &legacy_repo,
        &[
            "pm",
            "project",
            "create",
            "--wave",
            "product",
            "--title",
            "Blocked",
            "--definition",
            "No side effects",
            "--kr",
            "Nothing changed",
        ],
    );
    let error = String::from_utf8_lossy(&blocked.stderr);
    assert!(!blocked.status.success());
    assert!(error.contains("lf pm reteam --apply"), "{error}");
    assert!(error.contains("PRD-44"), "{error}");

    // PRD-43 must preserve Loopflow's checked-in legacy bindings verbatim.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = std::fs::read_to_string(source.join(".lf/config.yaml")).unwrap();
    assert!(config.contains("linear:\n  team:"));
    assert!(!config.contains("linear_team:"));
    let product = std::fs::read_to_string(source.join("wave/product/GOAL.md")).unwrap();
    assert!(product.contains("linear_team:"));

    println!("one Team team-loo: Survival — A real task reaches done; Survival / Infrastructure — Gmail; LOO-1 and LOO-2 resolve independently");
}
