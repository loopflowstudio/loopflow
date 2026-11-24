use crate::test_linear_input::{InventoryFixture, IssueFixture, SeedLinearSqlite};
use loopflow_execution::work::edit::WorkValue;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow_test_support::TestRepo;
use serde_json::Value;

/// The owner's newest unconfirmed action of `kind`.
fn unconfirmed(
    store: &loopflow_storage::sqlite::SqliteStore,
    owner: loopflow_storage::sqlite::tracker_actions::ActionOwner,
    kind: loopflow_storage::sqlite::tracker_actions::ActionKind,
) -> Option<String> {
    use loopflow_storage::sqlite::tracker_actions::ActionPhase;
    store
        .tracker_actions_for(&owner)
        .unwrap()
        .into_iter()
        .rfind(|action| {
            action.kind == kind && matches!(action.phase, ActionPhase::Queued | ActionPhase::Sent)
        })
        .map(|action| action.id)
}

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    for (name, _) in std::env::vars_os() {
        let name_text = name.to_string_lossy();
        if name_text.starts_with("LF_")
            || name_text.starts_with("LOOPFLOW_")
            || name_text.starts_with("LINEAR_")
        {
            command.env_remove(name);
        }
    }
    command
        .current_dir(repo)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", loopflow_test_support::lf_binary())
        .env("LF_USER_NAME", "Fixture Person")
        .args(args);
    if home.join("bin").is_dir() {
        let mut paths = vec![home.join("bin")];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    command
}

#[test]
fn stored_task_publishes_and_completes_after_verified_merge() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "remote".to_string(),
            "set-url".into(),
            "origin".into(),
            "https://github.com/fixture/local.git".into(),
        ],
        vec![
            "config".into(),
            format!("url.{}.insteadOf", repo.bare_path().display()),
            "https://github.com/fixture/local.git".into(),
        ],
    ] {
        assert!(Command::new("git")
            .current_dir(repo.path())
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    std::fs::create_dir(home.path().join("bin")).unwrap();
    let gh = home.path().join("bin/gh");
    std::fs::write(&gh, include_str!("../../../../tests/fixtures/local_gh.sh")).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--title",
            "Deliver locally owned work",
            "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let placed = lf(repo.path(), home.path(), &["checkout", id, "--json"]);
    let worktree = Path::new(placed["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("proof.txt"), "Delivered work\n").unwrap();
    lf(
        worktree,
        home.path(),
        &["commit", "-m", "Deliver locally owned work"],
    );
    lf(
        worktree,
        home.path(),
        &[
            "pr",
            "publish",
            "--title",
            "Deliver locally owned work",
            "--body",
            "Local planning through delivery.",
        ],
    );
    let published = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_ne!(published["execution"]["status"], "done");
    lf(worktree, home.path(), &["land"]);
    let armed = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_ne!(armed["execution"]["status"], "done");
    assert!(home.path().join("gh-armed").exists());
    let head = std::fs::read_to_string(home.path().join("gh-head")).unwrap();
    assert!(Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["update-ref", "refs/heads/main", head.trim()])
        .status()
        .unwrap()
        .success());
    std::fs::write(home.path().join("gh-merged"), "confirmed").unwrap();
    lf(repo.path(), home.path(), &["pr", "reconcile"]);
    let merged = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_ne!(merged["execution"]["status"], "done");
    assert!(
        worktree.exists(),
        "retain checkout until explicit completion"
    );
    lf(repo.path(), home.path(), &["task", "complete", id]);
    let completed = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_eq!(completed["execution"]["status"], "done");
    assert_eq!(completed["execution"]["task_id"], id);
    assert!(completed["task"]["completed_at"].is_string());
    assert!(
        !home.path().join("gh-unexpected").exists(),
        "{}",
        std::fs::read_to_string(home.path().join("gh-unexpected")).unwrap_or_default()
    );
}

fn lf(repo: &Path, home: &Path, args: &[&str]) -> Value {
    let output = command(repo, home, args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?} in {}: {}",
        repo.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| Value::String(String::from_utf8(output.stdout).unwrap()))
}

#[test]
fn concurrent_creations_are_distinct_and_failed_checkout_retains_its_task() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "inbox", "--json"],
    );
    let args = [
        "task",
        "create",
        "--wave",
        "inbox",
        "--title",
        "Concurrent work",
        "--json",
    ];
    let mut ids = std::collections::BTreeSet::new();
    let children: Vec<_> = (0..3)
        .map(|_| {
            command(repo.path(), home.path(), &args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let task: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(ids.insert(task["task"]["id"].as_str().unwrap().to_owned()));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Creation identity"));
    }
    let id = ids.first().unwrap();
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 3);
    let git = Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(git.status.success());
    let git = String::from_utf8(git.stdout).unwrap();
    std::fs::create_dir(home.path().join("bin")).unwrap();
    let shim = home.path().join("bin/git");
    std::fs::write(&shim, format!("#!/bin/sh\nif [ \"$1\" = worktree ] && [ \"$2\" = add ] && [ -f \"$LF_HOME/fail-checkout\" ]; then\n  rm \"$LF_HOME/fail-checkout\"\n  echo 'fixture: checkout interrupted' >&2\n  exit 1\nfi\nexec '{}' \"$@\"\n", git.trim().replace('\'', "'\\''"))).unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(home.path().join("fail-checkout"), "once").unwrap();
    let failed = command(
        repo.path(),
        home.path(),
        &["checkout", id.as_str(), "--json"],
    )
    .output()
    .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("fixture: checkout interrupted"));
    let retained = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", id.as_str(), "--json"],
    );
    assert_eq!(retained["execution"]["task_id"], id.as_str());
    assert!(retained["execution"]["pr"].is_null());
    let placed = lf(
        repo.path(),
        home.path(),
        &["checkout", id.as_str(), "--json"],
    );
    assert_eq!(placed["task_id"], retained["execution"]["task_id"]);
    assert_eq!(placed["worktree"], retained["execution"]["worktree"]);
    assert!(Path::new(placed["worktree"].as_str().unwrap()).is_dir());
    std::fs::remove_dir_all(placed["worktree"].as_str().unwrap()).unwrap();
}

#[test]
fn public_local_plan_matches_desktop() {
    let repo = TestRepo::new();
    repo.create_file(".lf/config.yaml", "default_agent: codex\n");
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--title",
            "Retain quoted input",
            "--notes",
            "Keep the escaped quote",
            "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let comments = lf(
        repo.path(),
        home.path(),
        &["task", "report", id, "Preserve the escaped quote", "--json"],
    );
    assert!(uuid::Uuid::parse_str(comments["comments"][0]["id"].as_str().unwrap()).is_ok());
    assert!(time::OffsetDateTime::parse(
        comments["comments"][0]["created_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339
    )
    .is_ok());
    let roadmap = lf(repo.path(), home.path(), &["desktop", "plan", "--json"]);
    let mut task = roadmap["tasks"]["items"][0].clone();
    assert_eq!(task["task"]["id"], id);
    assert!(task["reference"]["workspace"].is_null());
    assert!(task["run_control"]["unavailable"].is_null());
    assert_eq!(task["condition"]["unresolved_execution"], false);
    assert_eq!(task["actions"]["recommended"], "run");
    task["condition"]["observed_at"] = serde_json::json!("2026-10-08T00:00:00Z");
    task["condition"]["evidence_age_secs"] = serde_json::json!(0);
    task["runtime"]["updated_at"] = serde_json::json!("2026-10-08T00:00:00Z");
    let comment_id = comments["comments"][0]["id"].as_str().unwrap().to_string();
    let comment_date = comments["comments"][0]["created_at"]
        .as_str()
        .unwrap()
        .to_string();
    task["task"]["repo_id"] = serde_json::json!("repo_00000000000040008000000000000001");
    task["task"]["repo"] = serde_json::json!("/repo");
    let proof = serde_json::json!({"task":task,"comments":comments});
    let canonical = serde_json::to_string(&proof)
        .unwrap()
        .replace(id, "task_0123456789ab40008000000000000001")
        .replace(&id.trim_start_matches("task_")[..6], "012345")
        .replace(&comment_id, "00000000-0000-4000-8000-000000000001")
        .replace(&comment_date, "2026-10-08T00:00:00Z");
    let actual: Value = serde_json::from_str(&canonical).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/dto/local_task.json"
    ))
    .unwrap();
    assert_eq!(actual, expected, "actual wire: {actual}");
    serde_json::from_value::<loopflow::lf::commands::waves::RoadmapTask>(actual["task"].clone())
        .unwrap();
    serde_json::from_value::<loopflow_execution::work::task::comment::TaskComments>(
        actual["comments"].clone(),
    )
    .unwrap();
}

#[test]
fn task_unassign_clears_the_assignee() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "Owned", "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    lf(repo.path(), home.path(), &["task", "claim", id, "--json"]);
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", id, "--assignee", "person-id"],
    );
    let unassigned = lf(
        repo.path(),
        home.path(),
        &["task", "unassign", id, "--json"],
    );
    assert!(unassigned["owner"].is_null());
    let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert!(status["task"]["assignee"].is_null());
}

#[test]
fn task_unassign_clears_the_assignee_of_an_unowned_task() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "Assigned", "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", id, "--assignee", "person-id"],
    );
    let unassigned = lf(
        repo.path(),
        home.path(),
        &["task", "unassign", id, "--json"],
    );
    assert!(unassigned["owner"].is_null());
    let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert!(status["task"]["assignee"].is_null());
}

#[test]
fn stored_fields_preserve_order_and_assignment() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let fields: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/dto/planning_fields.json"
    ))
    .unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "First", "--json",
        ],
    );
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "Second", "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "edit",
            id,
            "--title",
            fields["name"].as_str().unwrap(),
            "--notes",
            fields["description"].as_str().unwrap(),
            "--rank",
            "0",
            "--assignee",
            fields["assignee"].as_str().unwrap(),
        ],
    );
    let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    for field in ["name", "description", "rank", "assignee"] {
        assert_eq!(
            status["task"][if field == "name" { "title" } else { field }],
            fields[field]
        );
    }
    lf(repo.path(), home.path(), &["task", "unassign", id]);
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", id, "--notes", ""],
    );
    let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert!(status["task"]["assignee"].is_null());
    assert_eq!(status["task"]["description"], "");
    assert_eq!(status["task"]["rank"], 0);
    let wave = lf(
        repo.path(),
        home.path(),
        &["desktop", "wave", "inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(wave["tasks"]["items"][0]["task"]["id"], id);
    assert_eq!(wave["tasks"]["items"][1]["task"]["rank"], 1);
}

#[test]
fn nested_waves_create_files_and_their_tasks_follow_renames_and_moves() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "tools/parser",
            "--title",
            "Nested work",
            "--json",
        ],
    )["task"]
        .clone();
    let status = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", created["id"].as_str().unwrap(), "--json"],
    );
    let again = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "tools/parser",
            "--title",
            "Another task",
            "--json",
        ],
    )["task"]
        .clone();
    let other = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", again["id"].as_str().unwrap(), "--json"],
    );
    // Both are in the Wave's one Project, made with the Wave.
    assert_eq!(status["project"]["name"], "parser");
    assert_eq!(status["project"]["id"], other["project"]["id"]);
    lf(
        repo.path(),
        home.path(),
        &["wave", "rename", "tools", "instruments", "--json"],
    );
    let renamed = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "instruments/parser", "--json"],
    );
    assert_eq!(renamed["tasks"]["items"][0]["task"]["id"], created["id"]);
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "instruments/lexer", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "move",
            created["id"].as_str().unwrap(),
            "--wave",
            "instruments/lexer",
        ],
    );
    let lexer = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "instruments/lexer", "--json"],
    );
    assert_eq!(lexer["tasks"]["items"][0]["task"]["id"], created["id"]);
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "instruments/parser", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 1);
    let wave_id = wave["wave"]["id"].as_str().unwrap();
    let by_id = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            wave_id,
            "--title",
            "Stable scope",
            "--json",
        ],
    )["task"]
        .clone();
    assert!(by_id["id"].as_str().unwrap().starts_with("task_"));
    assert!(repo.path().join(".lf/wave").exists());
    assert!(!repo.path().join(".lf/config.yaml").exists());
}

#[test]
fn public_task_prefixes_resolve_and_report_ambiguity() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let first = "task_abcd1234400080000000000000000001";
    let second = "task_abcd1235400080000000000000000002";
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "inbox", "--json"],
    );
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let wave: loopflow_model::id::WaveId = conn
        .query_row("SELECT id FROM waves WHERE name='inbox'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let project = store.seed_project(&wave).unwrap().id;
    for (id, title) in [(first, "First"), (second, "Second")] {
        store
            .create_task(&loopflow_execution::work::task::NewTask {
                parent_task_id: None,
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    &store,
                    repo.path().canonicalize().unwrap().to_str().unwrap(),
                )
                .unwrap(),
                due_date: None,
                id: loopflow_execution::durable::TaskId::parse(id).unwrap(),
                project_id: Some(project.clone()),
                title: title.into(),
                description: String::new(),
            })
            .unwrap();
    }
    for selector in ["abcd1234", "ABCD1234", "lf-abcd1234", "task_abcd1234"] {
        let status = lf(
            repo.path(),
            home.path(),
            &["desktop", "task", selector, "--json"],
        );
        assert_eq!(status["execution"]["task_id"], first);
        assert_eq!(status["task"]["identifier"], "abcd1234");
    }
    let failed = command(
        repo.path(),
        home.path(),
        &["task", "edit", "abcd", "--title", "Wrong target"],
    )
    .output()
    .unwrap();
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains(first));
    assert!(error.contains(second));
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", "abcd1234", "--title", "Chosen target"],
    );
    let status = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", first, "--json"],
    );
    assert_eq!(status["task"]["title"], "Chosen target");
    let other = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", second, "--json"],
    );
    assert_eq!(other["task"]["title"], "Second");
}

#[test]
fn public_local_planning_survives_restart_with_generated_identity() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let args = [
        "task",
        "create",
        "--wave",
        "inbox",
        "--title",
        "Fix parser",
        "--notes",
        "Keep quoted input",
        "--json",
    ];
    let created = lf(repo.path(), home.path(), &args)["task"].clone();
    let identity = created["id"].as_str().unwrap().to_owned();
    loopflow_execution::durable::TaskId::parse(&identity).unwrap();
    assert!(created["tracker_id"].is_null());
    assert_eq!(created["title"], "Fix parser");
    assert_eq!(created["description"], "Keep quoted input");
    let other = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Fix parser",
            "--notes",
            "Keep quoted input",
            "--json",
        ],
    )["task"]
        .clone();
    assert_ne!(created["id"], other["id"]);
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "edit",
            identity.as_str(),
            "--title",
            "Retain quoted input",
        ],
    );
    let comments = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "report",
            identity.as_str(),
            "Preserve the escaped quote",
            "--json",
        ],
    );
    assert_eq!(comments["comments"].as_array().unwrap().len(), 1);
    let status = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", identity.as_str(), "--json"],
    );
    assert_eq!(status["task"]["title"], "Retain quoted input");
    assert_eq!(status["execution"]["task_id"], identity.as_str());
    assert!(status["execution"]["issue_id"].is_null());
    assert!(status["execution"]["worktree"].is_null());
    assert!(status["execution"]["pr"].is_null());
    assert!(status["plan_error"].is_null());
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(wave["project"]["value"]["name"], "inbox");
    let goal = repo.path().join(".lf/wave/inbox/GOAL.md");
    let memory = repo.path().join(".lf/wave/inbox/MEMORY.md");
    std::fs::write(&goal, "## Objective\n\nKeep private parser work local.\n").unwrap();
    std::fs::write(&memory, "Retain the escaped quote.").unwrap();
    let context = lf(
        repo.path(),
        home.path(),
        &["context", "--wave", "inbox", "--json"],
    );
    let memory_usage = context["context"]["usage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["source"] == ".lf/wave/inbox/MEMORY.md")
        .unwrap();
    assert_eq!(memory_usage["bytes"], 25);
    let edited_wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert!(edited_wave
        .to_string()
        .contains("Keep private parser work local"));
    let read_comments = lf(
        repo.path(),
        home.path(),
        &["task", "show", identity.as_str(), "--json"],
    )["comments"]
        .clone();
    assert_eq!(read_comments, comments);
    assert_eq!(
        read_comments["comments"][0]["author"]["name"],
        "Fixture Person"
    );
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "complete",
            identity.as_str(),
            "--reason",
            "No implementation needed",
        ],
    );
    let completed = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", identity.as_str(), "--json"],
    );
    assert_eq!(completed["execution"]["status"], "done");
    assert!(completed["task"]["completed_at"].is_string());
    assert!(completed["execution"]["worktree"].is_null());
    let other_id = other["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["task", "abandon", other_id, "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["task", "abandon", other_id, "--json"],
    );
    let canceled = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", other_id, "--json"],
    );
    assert_eq!(canceled["execution"]["status"], "abandoned");
    assert_eq!(canceled["task"]["state"], "canceled");
    for _ in 0..2 {
        lf(repo.path(), home.path(), &["task", "delete", other_id]);
    }
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert!(wave["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["task"]["id"] != other_id));
    let retained = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", other_id, "--json"],
    );
    assert_eq!(retained["execution"]["status"], "abandoned");
    assert_eq!(retained["tracker_state"]["state"], "removed");
    assert_eq!(
        retained["execution"]["actions"]["reason"],
        "Task was deleted; retained history is read-only"
    );
    let rejected = command(
        repo.path(),
        home.path(),
        &["task", "edit", other_id, "--title", "Late edit"],
    )
    .output()
    .unwrap();
    assert!(!rejected.status.success());
    assert!(repo.path().join(".lf/wave").exists());
    assert!(!repo.path().join(".lf/config.yaml").exists());
    let changed = Command::new("git")
        .current_dir(repo.path())
        .args(["diff", "--name-only"])
        .output()
        .unwrap();
    assert!(changed.status.success());
    // Task creation committed the new Wave's files; only this test's own edits remain.
    assert_eq!(
        String::from_utf8_lossy(&changed.stdout),
        ".lf/wave/inbox/GOAL.md\n.lf/wave/inbox/MEMORY.md\n"
    );
}

#[test]
fn public_local_task_places_and_runs_without_a_planning_provider() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        ".lf/graphs/proof.yaml",
        "- cmd: graph show proof\n- interactive: demo\n- cmd: graph show proof\n",
    );
    let definition = repo.path().join(".lf/graphs/proof.yaml");
    repo.stage_all();
    repo.commit("Fixture graph");
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["remote", "remove", "origin"])
        .status()
        .unwrap()
        .success());
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Local execution",
            "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["--wave", "inbox", "set", "graph", "proof"],
    );
    let catalog = lf(repo.path(), home.path(), &["graph", "list", "--json"]);
    let committed = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "proof")
        .unwrap();
    assert_eq!(committed["source"], ".lf/graphs/proof.yaml");
    assert!(committed["unavailable"].is_null());
    let source = lf(repo.path(), home.path(), &["graph", "customize", "proof"]);
    assert_eq!(
        Path::new(source.as_str().unwrap().trim())
            .canonicalize()
            .unwrap(),
        definition.canonicalize().unwrap()
    );
    let placed = lf(
        repo.path(),
        home.path(),
        &["task", "checkout", id, "--json"],
    );
    let worktree = placed["worktree"].as_str().unwrap();
    assert_eq!(placed["task_id"], id);
    assert!(placed["branch"]
        .as_str()
        .unwrap()
        .starts_with(&format!("jack/{}-", &id.trim_start_matches("task_")[..6])));
    // A bare run returns as its Graph registers. Arrival at the interactive
    // step precedes its Graph run's end; wait for both before reading its state.
    lf(repo.path(), home.path(), &["task", "run", id]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
        let work = &status["execution"]["work"];
        if work["graph"]["position"]["kind"] == "at"
            && work["graph_processes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|run| !run["ended_at"].is_null())
        {
            break status;
        }
        assert!(std::time::Instant::now() < deadline, "{status}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(
        status["execution"]["work"]["graph"]["position"]["step"],
        "demo"
    );
    assert_eq!(status["execution"]["status"], "active");
    assert_eq!(status["execution"]["task_id"], id);
    assert!(status["plan_error"].is_null());
    let landing = command(Path::new(worktree), home.path(), &["--task", id, "land"])
        .output()
        .unwrap();
    assert!(!landing.status.success());
    let error = String::from_utf8_lossy(&landing.stderr);
    assert!(error.contains("remote"), "{error}");
    let retained = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_eq!(retained["execution"]["status"], "active");
    assert!(retained["execution"]["pr"].is_null());
    let backlog = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Later work",
            "--json",
        ],
    )["task"]
        .clone();
    let retained = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", backlog["id"].as_str().unwrap(), "--json"],
    );
    assert_eq!(retained["execution"]["wave"], "inbox");
    assert_eq!(retained["project"]["name"], "inbox");
    let retry = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Local execution",
            "--json",
        ],
    )["task"]
        .clone();
    assert_ne!(retry["id"], id);
    lf(
        repo.path(),
        home.path(),
        &["task", "complete", id, "--reason", "Inspection completed"],
    );
    let completed = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_eq!(completed["execution"]["status"], "done");
    if Path::new(worktree).exists() {
        std::fs::remove_dir_all(worktree).unwrap();
    }
}

#[test]
fn a_wave_is_made_with_one_project_and_every_task_joins_it() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let added = lf(
            repo.path(),
            home.path(),
            &["wave", "add", "product", "--json"],
        );
        assert_eq!(added["created"], true);
        // Linear arrives after the Wave, so its Project is not in Linear yet.
        if connected {
            repo.create_file(
                ".lf/config.yaml",
                "planning:\n  provider: linear\n  team: fixture-team\n",
            );
        }
        let store = loopflow_execution::store::open_sqlite(&path).unwrap();
        let wave = loopflow_model::id::WaveId::parse(added["id"].as_str().unwrap()).unwrap();
        let project = store.project_of_wave(&wave).unwrap().unwrap();
        assert_eq!(project.name, "product");
        let plan = lf(
            repo.path(),
            home.path(),
            &["wave", "plan", "product", "--json"],
        );
        assert_eq!(plan["project"]["value"]["name"], "product");

        let create = |title: &str| {
            let output = command(
                repo.path(),
                home.path(),
                &["task", "create", "--wave", "product", "--title", title],
            )
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            (
                String::from_utf8(output.stdout).unwrap(),
                String::from_utf8(output.stderr).unwrap(),
            )
        };
        let (first, warning) = create("First");
        assert!(!first.contains("in Project"), "{first}");
        assert_eq!(
            warning.contains("pending Linear sync"),
            connected,
            "{warning}"
        );
        let tasks = store.list_tasks(Some(&wave)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].project_id.as_ref(), Some(&project.id));
        assert_eq!(
            unconfirmed(
                &store,
                loopflow_storage::sqlite::tracker_actions::ActionOwner::Task(tasks[0].id.clone()),
                loopflow_storage::sqlite::tracker_actions::ActionKind::Create,
            )
            .is_some(),
            connected
        );

        // It ends in the tracker; it is still the Wave's, and the next Task joins it.
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE projects SET status='completed' WHERE id=?1",
                [project.id.as_str()],
            )
            .unwrap();
        create("Second");
        for task in store.list_tasks(Some(&wave)).unwrap() {
            assert_eq!(task.project_id.as_ref(), Some(&project.id));
        }
        assert_eq!(store.list_projects().unwrap().len(), 1);
        let status = String::from_utf8(
            command(repo.path(), home.path(), &["wave", "status", "product"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        assert!(status.contains("· completed"), "{status}");
        assert_eq!(
            status.contains("not in Linear yet; `lf wave sync product` makes it"),
            connected,
            "{status}"
        );
    }
}

#[test]
fn project_reads_keep_canonical_identity_after_edits() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        if connected {
            std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "planning:\n  provider: linear\n  team: team\n",
            )
            .unwrap();
        }
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
        let canonical = loopflow_execution::repository::discover(repo.path()).unwrap();
        let wave = loopflow_execution::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        // The Wave's one Project, bound to the tracker's.
        let observed = serde_json::from_value(serde_json::json!({
            "id": "linear-first", "slug": "first", "name": "first",
            "summary": "", "status": "started", "team_ids": ["team"]
        }))
        .unwrap();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                synced_at: 17,
                snapshot: loopflow_storage::planning::ProjectInventory {
                    project: observed,
                    items: vec![serde_json::from_value(serde_json::json!({
                    "id": "linear-task", "identifier": "FIX-1", "name": "Shared title",
                    "description": "Saved prose", "rank": 4, "project_id": "linear-first",
                    "project": "first", "team_id": "team", "completed": false,
                    "completed_at": null, "state": "unstarted", "due_date": "2026-11-01",
                    "assignee": "Fixture author", "branch_name": null, "url": null, "revision": null
                }))
                .unwrap()],
                },
            })
            .unwrap();
        let project = store.project_of_wave(wave.id()).unwrap().unwrap();
        assert_eq!(project.name, "first");
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        conn.execute(
            "UPDATE projects SET updated_at=1 WHERE id=?1",
            [project.id.as_str()],
        )
        .unwrap();
        let saved = store.task_by_issue("FIX-1").unwrap().unwrap();
        for edited in [false, true] {
            if edited {
                store
                    .edit_project_fields(
                        &project.id,
                        &[WorkValue::ProjectName(Some("Edited".into()))],
                    )
                    .unwrap();
            }
            if edited {
                lf(
                    repo.path(),
                    home.path(),
                    &[
                        "task",
                        "edit",
                        "FIX-1",
                        "--title",
                        "Local title",
                        "--notes",
                        "Local prose",
                    ],
                );
                // Missing provider inventory is not a missing saved due date or Task.
                conn.execute("DELETE FROM tracker_items", []).unwrap();
            }
            let status = lf(
                repo.path(),
                home.path(),
                &["desktop", "wave", "product", "--json"],
            );
            {
                let rows = &status["tasks"]["items"];
                let rows = rows.as_array().unwrap();
                assert_eq!(rows.len(), 1);
                let task = &rows[0]["task"];
                assert_eq!(task["id"], saved.id.as_str());
                assert_eq!(task["tracker_id"], "linear-task");
                assert_eq!(
                    task["name"],
                    if edited {
                        "Local title"
                    } else {
                        "Shared title"
                    }
                );
                assert_eq!(
                    task["description"],
                    if edited { "Local prose" } else { "Saved prose" }
                );
                assert_eq!(task["due_date"], "2026-11-01");
                assert_eq!(task["assignee"], "Fixture author");
                assert!(task["sync"].is_object());
                assert_eq!(rows[0]["runtime"]["work_id"], saved.id.as_str());
            }
            let retained = store.task_by_issue("FIX-1").unwrap().unwrap();
            assert_eq!(retained.worktree, saved.worktree);
            assert_eq!(retained.branch, saved.branch);
            assert_eq!(retained.base_commit, saved.base_commit);
            assert_eq!(retained.tracker_synced_at, saved.tracker_synced_at);
            {
                let row = &status["project"]["value"];
                assert_eq!(row["id"], project.id.as_str());
                assert_eq!(
                    row["tracker_id"],
                    project.tracker_id.as_ref().unwrap().as_str()
                );
                assert!(row.get("work_id").is_none());
                assert!(row["sync"].is_object());
                assert_eq!(row["name"], if edited { "Edited" } else { "first" });
            }
        }
    }
}

#[test]
fn task_creation_and_edits_save_offline_in_both_connection_modes() {
    use loopflow_execution::durable::TaskId;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "planning:\n  provider: linear\n  team: fixture-team\n",
            )
            .unwrap();
        }
        let path = home.path().join("loopflow.db");
        let store = loopflow_execution::store::open_sqlite(&path).unwrap();
        let canonical = loopflow_execution::repository::discover(repo.path()).unwrap();
        let wave = loopflow_execution::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        let project = loopflow_storage::planning::TrackerProject {
            description: None,
            id: "provider-project".into(),
            revision: Some("2026-10-08T10:00:00Z".into()),
            slug: "product".into(),
            name: "Product".into(),
            summary: "".into(),
            status: loopflow_execution::work::project::ProjectStatus::Started,
            url: None,
            team_ids: vec!["fixture-team".into()],
        };
        loopflow_execution::store::sqlite::linear_events::accept_linear_project_in(
            &store,
            wave.id(),
            &project,
            17,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
        let project_id = store.project_of_wave(wave.id()).unwrap().unwrap().id;
        let conn = rusqlite::Connection::open(&path).unwrap();
        if !mapped {
            conn.execute(
                "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                [project_id.as_str()],
            )
            .unwrap();
        }
        let create = || {
            let output = command(
                repo.path(),
                home.path(),
                &[
                    "task",
                    "create",
                    "--wave",
                    "product",
                    "--title",
                    "Independent same title",
                    "--notes",
                    "Original notes",
                    "--json",
                ],
            )
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stderr).contains("pending Linear sync"),
                connected
            );
            let created: Value = serde_json::from_slice(&output.stdout).unwrap();
            let created = &created["task"];
            let id = TaskId::parse(created["id"].as_str().unwrap()).unwrap();
            let saved = store.task(&id).unwrap().unwrap();
            assert_eq!(*created, serde_json::to_value(saved).unwrap());
            assert_eq!(created["title"], "Independent same title");
            assert_eq!(created["description"], "Original notes");
            assert_eq!(created["project_id"], project_id.as_str());
            assert!(created.get("name").is_none());
            assert!(created["worktree"].is_null());
            created.clone()
        };
        let first = TaskId::parse(create()["id"].as_str().unwrap()).unwrap();
        let second = TaskId::parse(create()["id"].as_str().unwrap()).unwrap();
        assert_ne!(first, second);
        assert_eq!(store.list_tasks(None).unwrap().len(), 2);
        let mut snapshot = loopflow_storage::planning::ProjectInventory {
            project: project.clone(),
            items: vec![],
        };
        if mapped {
            for (id, alias) in [(&first, "FIX-1"), (&second, "FIX-2")] {
                conn.execute(
                    "UPDATE tasks SET external_issue_id=?2,issue_identifier=?2 WHERE id=?1",
                    rusqlite::params![id.as_str(), alias],
                )
                .unwrap();
                let mut item = observed_issue(
                    alias,
                    &project.id,
                    "Independent same title",
                    "Original notes",
                );
                item.identifier = alias.into();
                item.revision = Some("2026-10-08T10:00:00Z".into());
                item.team_id = Some("fixture-team".into());
                snapshot.items.push(item);
            }
            store
                .seed_inventory(&InventoryFixture {
                    wave_id: wave.id().clone(),
                    synced_at: 18,
                    snapshot: snapshot.clone(),
                })
                .unwrap();
        }
        let selector = if mapped { "FIX-2" } else { second.as_str() };
        let edit = [
            "task",
            "edit",
            selector,
            "--title",
            "Saved offline",
            "--notes",
            "",
            "--rank",
            "0",
            "--assignee",
            "person-id",
        ];
        let output = command(repo.path(), home.path(), &edit).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains(" · queued"),
            connected
        );
        // Edits are queued once the Task has a tracker object or a queued
        // creation; otherwise its creation will carry them.
        let pending = store.pending_task_changes(&second).unwrap();
        assert_eq!(pending.len(), if mapped || connected { 3 } else { 0 });
        assert!(store.pending_task_changes(&first).unwrap().is_empty());
        assert_eq!(
            unconfirmed(
                &store,
                loopflow_storage::sqlite::tracker_actions::ActionOwner::Project(
                    store.task(&second).unwrap().unwrap().project_id.unwrap()
                ),
                loopflow_storage::sqlite::tracker_actions::ActionKind::Order,
            )
            .is_some(),
            // A Project not in the tracker yet queues its order when bound (below).
            mapped
        );
        let edited = store.task(&second).unwrap().unwrap();
        let revision = || {
            conn.query_row(
                "SELECT revision FROM store_revisions WHERE domain='planning'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        let before_failure = revision();
        conn.execute_batch("CREATE TRIGGER fail_task_save BEFORE UPDATE ON tasks BEGIN SELECT RAISE(ABORT,'injected task save failure'); END;").unwrap();
        let failed = command(
            repo.path(),
            home.path(),
            &[
                "task",
                "edit",
                selector,
                "--title",
                "Must roll back",
                "--rank",
                "1",
            ],
        )
        .output()
        .unwrap();
        assert!(!failed.status.success());
        conn.execute_batch("DROP TRIGGER fail_task_save").unwrap();
        assert_eq!(store.task(&second).unwrap().unwrap(), edited);
        assert_eq!(store.pending_task_changes(&second).unwrap(), pending);
        assert_eq!(revision(), before_failure);
        lf(repo.path(), home.path(), &edit);
        assert_eq!(store.pending_task_changes(&second).unwrap(), pending);
        assert_eq!(
            store.task(&second).unwrap().unwrap().revision,
            edited.revision
        );
        assert!(store
            .edit_task(
                &second,
                0,
                &loopflow_execution::work::task::TaskUpdate {
                    title: Some("Stale".into()),
                    ..Default::default()
                }
            )
            .is_err());
        let status = lf(
            repo.path(),
            home.path(),
            &["desktop", "task", selector, "--json"],
        );
        assert_eq!(status["task"]["title"], "Saved offline");
        assert_eq!(status["task"]["description"], "");
        assert_eq!(status["task"]["rank"], 0);
        assert_eq!(status["task"]["assignee"], "person-id");
        let sync = loopflow_execution::store::Store {
            sqlite: store.clone(),
        }
        .task_tracker_sync(&second)
        .unwrap();
        assert_eq!(status["sync"], serde_json::to_value(&sync).unwrap());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            sync.lines()
                .into_iter()
                .map(|line| format!("{line}\n"))
                .collect::<String>()
        );
        assert_eq!(sync.actions.is_empty(), !connected);
        if connected {
            assert_eq!(sync.actions.iter().any(|a| a.change == "order"), mapped);
            assert_eq!(sync.actions.iter().any(|a| a.change == "create"), !mapped);
            assert!(sync.actions.iter().any(|a| a.change == "title"));
        }
        let desktop = lf(
            repo.path(),
            home.path(),
            &["desktop", "wave", "product", "--json"],
        );
        let task = desktop["tasks"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["task"]["name"] == "Saved offline")
            .unwrap();
        assert_eq!(task["task"]["sync"], status["sync"]);
        assert!(task["reference"]["workspace"].is_null());
        if connected && !mapped {
            // `lf wave sync` makes the Project and binds it: the reorder made
            // meanwhile is sent then, behind the Tasks' creates.
            loopflow_execution::store::sqlite::linear_events::accept_linear_project_in(
                &store,
                wave.id(),
                &project,
                21,
                loopflow_storage::planning_sync::TrackerKind::Linear,
            )
            .unwrap();
            let order = unconfirmed(
                &store,
                loopflow_storage::sqlite::tracker_actions::ActionOwner::Project(project_id.clone()),
                loopflow_storage::sqlite::tracker_actions::ActionKind::Order,
            )
            .unwrap();
            assert_eq!(
                store.tracker_action(&order).unwrap().unwrap().value,
                serde_json::json!([second.as_str(), first.as_str()])
            );
        }

        if mapped {
            let mut incoming = snapshot.items[1].clone();
            incoming.name = "Concurrent title".into();
            incoming.assignee = Some("remote-person".into());
            incoming.state = Some("completed".into());
            incoming.completed = true;
            incoming.revision = Some("2026-10-08T11:00:00Z".into());
            store
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &IssueFixture {
                        observed_at: 19,
                        project: Some(project.clone()),
                        item: incoming.clone(),
                    },
                )
                .unwrap();
            // Unsent title and assignee saves are sent last and win; status
            // follows Linear.
            let sync = loopflow_execution::store::Store {
                sqlite: store.clone(),
            }
            .task_tracker_sync(&second)
            .unwrap();
            assert!(sync.actions.iter().all(|a| a.phase
                != loopflow_execution::store::sqlite::tracker_actions::ActionPhase::Conflicted));
            let retained = store.task(&second).unwrap().unwrap();
            assert_eq!(retained.title, "Saved offline");
            assert_eq!(retained.description, "");
            assert_eq!(retained.rank, 0);
            assert_eq!(retained.assignee.as_deref(), Some("person-id"));
            assert_eq!(retained.state.as_deref(), Some("completed"));
            let changes = store.pending_task_changes(&second).unwrap();
            for field in ["task_title", "task_assignee"] {
                assert!(changes.iter().any(|change| change.value.field() == field));
            }
            let conflict: Option<String> = conn
                .query_row(
                    "SELECT conflict_json FROM tracker_actions WHERE id=?1",
                    [&pending[0].id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(conflict.is_none());
            incoming.name = "Saved offline".into();
            incoming.assignee = Some("person-id".into());
            incoming.description.clear();
            incoming.revision = Some("2026-10-08T12:00:00Z".into());
            store
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &IssueFixture {
                        observed_at: 20,
                        project: Some(project.clone()),
                        item: incoming,
                    },
                )
                .unwrap();
            assert_eq!(store.pending_task_changes(&second).unwrap(), changes);
        }
        lf(repo.path(), home.path(), &["task", "unassign", selector]);
        let reopened = loopflow_execution::store::open_sqlite(&path).unwrap();
        assert!(reopened.task(&second).unwrap().unwrap().assignee.is_none());
        assert_eq!(reopened.task(&second).unwrap().unwrap().id, second);
        assert!(reopened
            .list_tasks(None)
            .unwrap()
            .iter()
            .all(|task| task.worktree.is_none()));
        let (sessions, graphs): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM agent_sessions),(SELECT count(*) FROM task_graphs)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((sessions, graphs), (0, 0));
        assert!(!repo.path().join(".lf/wave").exists());
    }
}

#[test]
fn moving_saves_membership_offline_and_retains_inbound_changes() {
    use loopflow_execution::durable::TaskId;
    use loopflow_execution::work::task::NewTask;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = loopflow_execution::store::open_sqlite(&path).unwrap();
        let canonical = loopflow_execution::repository::discover(repo.path()).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        let mut projects = Vec::new();
        let mut observations = Vec::new();
        for name in ["source", "destination", "competing"] {
            let wave = loopflow_execution::work::wave::Wave::new(
                loopflow_model::id::WaveId::new(),
                name.into(),
                canonical.to_string(),
            );
            loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
            lf(repo.path(), home.path(), &["wave", "add", name, "--json"]);
            let mut project = store.seed_project(wave.id()).unwrap();
            if mapped {
                // Bind through the writer so foreground Git exchange sees the same
                // identity as SQLite, rather than restoring a pre-binding journal.
                project.tracker_id =
                    Some(loopflow_execution::linear::LinearProjectId::new(name).unwrap());
                store.insert_project(&project).unwrap();
                let observed = loopflow_storage::planning::TrackerProject {
                    id: name.into(),
                    slug: project.slug.clone(),
                    name: project.name.clone(),
                    summary: project.summary.clone(),
                    description: Some(project.description.clone()),
                    status: project.status,
                    url: None,
                    team_ids: vec!["fixture-team".into()],
                    revision: None,
                };
                loopflow_execution::store::sqlite::linear_events::accept_linear_project_in(
                    &store,
                    wave.id(),
                    &observed,
                    1,
                    loopflow_storage::planning_sync::TrackerKind::Linear,
                )
                .unwrap();
                observations.push(observed);
            }
            projects.push(project);
        }
        // Linear arrives after the Waves: an unmapped one's Project is not in Linear.
        if connected {
            repo.create_file(
                ".lf/config.yaml",
                "planning:\n  provider: linear\n  team: fixture-team\n",
            );
        }
        let task = store
            .create_task(&NewTask {
                parent_task_id: None,
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    &store,
                    repo.path().canonicalize().unwrap().to_str().unwrap(),
                )
                .unwrap(),
                due_date: None,
                id: TaskId::new(),
                project_id: Some(projects[0].id.clone()),
                title: "Refile me".into(),
                description: "Original".into(),
            })
            .unwrap();
        if mapped {
            db.execute("UPDATE tasks SET external_issue_id='provider-issue',issue_identifier='FIX-1',planning_team_id='fixture-team' WHERE id=?1", [task.id.as_str()]).unwrap();
        }
        let original = crate::test_linear_input::IssueFixture {
            item: observed_issue("provider-issue", "source", "Refile me", "Original"),
            project: observations.first().cloned(),
            observed_at: 1,
        };
        if mapped {
            store
                .seed_issue(&canonical.to_string(), "linear", &original)
                .unwrap();
        }
        let args = ["task", "move", task.id.as_str(), "--wave", "destination"];
        let revision = || {
            db.query_row(
                "SELECT revision FROM store_revisions WHERE domain='planning'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        let before = revision();
        db.execute_batch("CREATE TRIGGER fail_refile BEFORE INSERT ON plan_changes BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;").unwrap();
        assert!(!command(repo.path(), home.path(), &args)
            .output()
            .unwrap()
            .status
            .success());
        db.execute_batch("DROP TRIGGER fail_refile").unwrap();
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().project_id,
            Some(projects[0].id.clone())
        );
        assert!(store.pending_task_changes(&task.id).unwrap().is_empty());
        assert_eq!(revision(), before);
        let output = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains(" · queued"),
            connected
        );
        let moved = store.task(&task.id).unwrap().unwrap();
        assert_eq!(moved.project_id, Some(projects[1].id.clone()));
        assert_eq!(moved.wave_id, Some(projects[1].wave_id.clone()));
        assert_eq!(moved.revision, task.revision + 1);
        assert!(moved.worktree.is_none());
        // An unmapped Task has no tracker object to edit: its creation carries
        // the new membership.
        let changes = store.pending_task_changes(&task.id).unwrap();
        assert_eq!(changes.len(), usize::from(mapped));
        if !mapped {
            continue;
        }
        assert_eq!(changes[0].value.field(), "task_project");
        assert_eq!(
            changes[0].value,
            WorkValue::TaskProject(Some(projects[1].id.to_string()))
        );
        // A repeated command may refresh exchange status, but cannot edit the Task
        // or enqueue another membership effect.
        let before = store.task(&task.id).unwrap().unwrap();
        assert!(command(repo.path(), home.path(), &args)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(store.task(&task.id).unwrap().unwrap(), before);
        assert_eq!(store.pending_task_changes(&task.id).unwrap(), changes);
        if mapped {
            let mut incoming = original.clone();
            incoming.item.description = "Incoming after refiling".into();
            incoming.item.revision = Some("2026-10-08T14:00:00Z".into());
            incoming.observed_at += 1;
            store
                .seed_issue(&canonical.to_string(), "linear", &incoming)
                .unwrap();
            let retained = store.task(&task.id).unwrap().unwrap();
            assert_eq!(retained.project_id, Some(projects[1].id.clone()));
            assert_eq!(retained.description, "Incoming after refiling");
            assert_eq!(store.pending_task_changes(&task.id).unwrap(), changes);
            let mut other = incoming.project.clone().unwrap();
            // Preserve the duplicate slug: exact provider IDs must beat names.
            other.id = "competing".into();
            other.revision = Some("2026-10-08T14:01:00Z".into());
            incoming.project = Some(other);
            incoming.item.project_id = Some("competing".into());
            incoming.observed_at += 1;
            incoming.item.revision = Some("2026-10-08T14:01:00Z".into());
            store
                .seed_issue(&canonical.to_string(), "linear", &incoming)
                .unwrap();
            // The unsent refiling is sent after Linear's move and wins, rebased
            // onto the move it replaces.
            let rebased = store.pending_task_changes(&task.id).unwrap();
            assert_eq!(rebased.len(), 1);
            assert_eq!(
                (&rebased[0].id, &rebased[0].value),
                (&changes[0].id, &changes[0].value)
            );
            assert_eq!(
                store.task(&task.id).unwrap().unwrap().project_id,
                Some(projects[1].id.clone())
            );
            assert_eq!(
                store.task_tracker_state(&task.id).unwrap().state,
                loopflow_execution::store::PlanningState::Available
            );
        }
        db.execute(
            "UPDATE projects SET created_at=created_at+10 WHERE id=?1",
            [projects[2].id.as_str()],
        )
        .unwrap();
        let reopened = loopflow_execution::store::open_sqlite(&path).unwrap();
        assert_eq!(
            reopened.task(&task.id).unwrap().unwrap().project_id,
            Some(projects[1].id.clone())
        );
        assert_eq!(
            reopened.pending_task_changes(&task.id).unwrap(),
            store.pending_task_changes(&task.id).unwrap()
        );
        // Membership moves retain the original start, even without a checkout.
        db.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',17)", [task.id.as_str()]).unwrap();
        db.execute(
            "INSERT INTO task_ownership(id,task_id,kind,at,ordered_at,writer_machine_id,written_here) VALUES('own_'||lower(hex(randomblob(16))),?1,'recorded',17,17000,(SELECT id FROM machines WHERE is_local=1),1)",
            [task.id.as_str()],
        )
        .unwrap();
        let moved = command(
            repo.path(),
            home.path(),
            &["task", "move", task.id.as_str(), "--wave", "source"],
        )
        .output()
        .unwrap();
        assert!(
            moved.status.success(),
            "{}",
            String::from_utf8_lossy(&moved.stderr)
        );
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().project_id,
            Some(projects[0].id.clone())
        );
        assert_eq!(
            db.query_row(
                "SELECT max(at) FROM task_ownership WHERE task_id=?1",
                [task.id.as_str()],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            17
        );
        assert!(repo.path().join(".lf/wave").exists());
    }
}

#[test]
fn wave_documents_and_graphs_follow_files() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let wave_id = loopflow_model::id::WaveId::new();
        let root = repo.path().join(".lf/wave/tools");
        std::fs::create_dir_all(root.join("parser")).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        let goal = format!("---\nid: {wave_id}\n---\n## Objective\n\nKeep parsing.\n");
        std::fs::write(root.join("GOAL.md"), &goal).unwrap();
        std::fs::write(root.join("MEMORY.md"), "Inherited decisions.\n").unwrap();
        std::fs::write(
            root.join("parser/GOAL.md"),
            "## Objective\n\nPreserve tokens.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("parser/README.md"),
            "Additional authored context.\n",
        )
        .unwrap();
        let graph = "- cmd: help\n";
        std::fs::write(repo.path().join(".lf/graphs/finish.yaml"), graph).unwrap();
        let project = lf(
            repo.path(),
            home.path(),
            &["wave", "add", "tools/parser", "--json"],
        );
        assert!(project["id"].is_string());
        if connected {
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "planning:\n  provider: linear\n  team: fixture-team\n",
            )
            .unwrap();
        }
        let task = lf(
            repo.path(),
            home.path(),
            &[
                "task",
                "create",
                "--wave",
                "tools/parser",
                "--title",
                "Retain definitions",
                "--json",
            ],
        )["task"]
            .clone();
        let task_id = task["id"].as_str().unwrap();
        lf(
            repo.path(),
            home.path(),
            &["--wave", "tools/parser", "set", "graph", "finish"],
        );
        std::fs::write(root.join("parser/MEMORY.md"), "Edited child memory λ.\n").unwrap();
        let context = lf(
            repo.path(),
            home.path(),
            &["context", "--wave", "tools/parser", "--json"],
        );
        let text = context.to_string();
        assert!(text.contains(".lf/wave/tools/MEMORY.md"), "{text}");
        assert!(text.contains(".lf/wave/tools/parser/MEMORY.md"), "{text}");
        assert_eq!(std::fs::read_to_string(root.join("GOAL.md")).unwrap(), goal);
        std::fs::rename(
            repo.path().join(".lf/wave"),
            repo.path().join("authored-wave"),
        )
        .unwrap();
        std::fs::remove_file(repo.path().join(".lf/graphs/finish.yaml")).unwrap();
        let absent = lf(
            repo.path(),
            home.path(),
            &["context", "--wave", "tools/parser", "--json"],
        );
        assert!(!absent
            .to_string()
            .contains(".lf/wave/tools/parser/MEMORY.md"));
        // A Wave keeps only the name; its definition follows the file.
        let missing = command(
            repo.path(),
            home.path(),
            &["graph", "show", "finish", "--json"],
        )
        .output()
        .unwrap();
        assert!(!missing.status.success());
        std::fs::write(repo.path().join(".lf/graphs/finish.yaml"), graph).unwrap();
        let shown = lf(
            repo.path(),
            home.path(),
            &["graph", "show", "finish", "--json"],
        );
        assert_eq!(shown["source"], ".lf/graphs/finish.yaml");
        std::fs::write(repo.path().join(".lf/graphs/unimported.yaml"), graph).unwrap();
        std::fs::write(repo.path().join(".lf/graphs/finish.yaml"), "invalid: true").unwrap();
        let catalog = lf(repo.path(), home.path(), &["graph", "list", "--json"]);
        let entries = catalog.as_array().unwrap();
        let edited = entries
            .iter()
            .find(|entry| entry["name"] == "finish")
            .unwrap();
        assert_eq!(edited["source"], ".lf/graphs/finish.yaml");
        assert!(edited["unavailable"].is_string());
        let unimported = entries
            .iter()
            .find(|entry| entry["name"] == "unimported")
            .unwrap();
        assert!(unimported["compiled"].is_object());
        assert!(unimported["unavailable"].is_null());
        lf(
            repo.path(),
            home.path(),
            &["wave", "rename", "tools", "instruments", "--json"],
        );
        let status = lf(
            repo.path(),
            home.path(),
            &["wave", "status", "instruments/parser", "--json"],
        );
        assert_eq!(status["wave"]["goal"], "");
        assert_eq!(status["tasks"]["items"][0]["task"]["id"], task_id);
        assert!(!repo.path().join(".lf/wave").exists());
        assert_eq!(
            std::fs::read_to_string(repo.path().join("authored-wave/tools/GOAL.md")).unwrap(),
            goal
        );
    }
}

#[test]
fn deletion_saves_offline_and_retains_execution_and_retry_identity() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "planning:\n  provider: linear\n  team: fixture-team\n",
            )
            .unwrap();
        }
        let task = lf(
            repo.path(),
            home.path(),
            &["task", "create", "--title", "Remove planning", "--json"],
        )["task"]
            .clone();
        let id = task["id"].as_str().unwrap();
        let store =
            loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
        let task_id = loopflow_execution::durable::TaskId::parse(id).unwrap();
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if connected {
            db.execute("UPDATE tasks SET external_issue_id='retained-issue',issue_identifier='FIX-1' WHERE id=?1", [id]).unwrap();
        }
        let before = store.task(&task_id).unwrap().unwrap();
        let work = loopflow_execution::durable::WorkRef::Task(task_id.clone());
        let execution = store.work_status(&work).unwrap();
        // The removal and its action are one save: a failed action leaves the
        // Task as it was. A Task with no tracker object queues no action.
        if connected {
            db.execute_batch("CREATE TRIGGER fail_removal BEFORE INSERT ON tracker_actions WHEN NEW.kind='delete' BEGIN SELECT RAISE(ABORT,'receipt failed'); END;").unwrap();
            assert!(!command(repo.path(), home.path(), &["task", "delete", id])
                .output()
                .unwrap()
                .status
                .success());
            assert_eq!(store.task(&task_id).unwrap().unwrap(), before);
            db.execute_batch("DROP TRIGGER fail_removal;").unwrap();
        }
        let output = lf(repo.path(), home.path(), &["task", "delete", id]);
        if connected {
            assert!(output.as_str().unwrap().contains("Pending Linear sync"));
        }
        let changes = unconfirmed(
            &store,
            loopflow_storage::sqlite::tracker_actions::ActionOwner::Task(task_id.clone()),
            loopflow_storage::sqlite::tracker_actions::ActionKind::Delete,
        );
        assert_eq!(changes.is_some(), connected);
        lf(repo.path(), home.path(), &["task", "delete", id]);
        assert_eq!(
            unconfirmed(
                &store,
                loopflow_storage::sqlite::tracker_actions::ActionOwner::Task(task_id.clone()),
                loopflow_storage::sqlite::tracker_actions::ActionKind::Delete,
            ),
            changes
        );
        let removed = store.task(&task_id).unwrap().unwrap();
        assert_eq!(store.work_status(&work).unwrap(), execution);
        assert_eq!(removed.worktree, before.worktree);
        assert_eq!(removed.tracker_id, before.tracker_id);
        assert_eq!(removed.revision, before.revision + 1);
        let status = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
        assert_eq!(status["tracker_state"]["state"], "removed");
        assert!(store.list_tasks(None).unwrap().is_empty());
        drop(store);
        let reopened =
            loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
        assert_eq!(
            unconfirmed(
                &reopened,
                loopflow_storage::sqlite::tracker_actions::ActionOwner::Task(task_id.clone()),
                loopflow_storage::sqlite::tracker_actions::ActionKind::Delete,
            ),
            changes
        );
    }
}

#[test]
fn task_selectors_use_repository_scope_and_preserve_exact_identity_through_aliases() {
    let first = TestRepo::new();
    let second = TestRepo::new();
    // Keep exchange from overwriting the synthetic duplicate identifiers below.
    for repo in [&first, &second] {
        std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
        std::fs::write(
            repo.path().join(".lf/config.yaml"),
            "planning:\n  provider: git\n  remote: unavailable-fixture-remote\n",
        )
        .unwrap();
    }
    let home = tempfile::tempdir().unwrap();
    let tasks: Vec<_> = [&first, &second]
        .into_iter()
        .map(|repo| {
            lf(
                repo.path(),
                home.path(),
                &["task", "create", "--title", "Original", "--json"],
            )["task"]
                .clone()
        })
        .collect();
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute("UPDATE tasks SET issue_identifier='DUP-1'", [])
        .unwrap();
    let alias = home.path().join("first-alias");
    std::os::unix::fs::symlink(first.path(), &alias).unwrap();
    for (repo, task) in [
        (first.path(), &tasks[0]),
        (alias.as_path(), &tasks[0]),
        (second.path(), &tasks[1]),
    ] {
        for selector in ["DUP-1", "dup-1", "Dup-1"] {
            let status = lf(repo, home.path(), &["desktop", "task", selector, "--json"]);
            assert_eq!(status["task"]["id"], task["id"]);
            assert_eq!(status["task"]["identifier"], "DUP-1");
        }
    }
    let ambiguous = command(home.path(), home.path(), &["--task", "dup-1", "--explain"])
        .output()
        .unwrap();
    assert!(ambiguous.status.success());
    let error = String::from_utf8_lossy(&ambiguous.stdout);
    for task in &tasks {
        assert!(error.contains(task["id"].as_str().unwrap()), "{error}");
    }
    // Exact identity remains constrained to the selected repository.
    let foreign = tasks[1]["id"].as_str().unwrap();
    let foreign_read = command(
        first.path(),
        home.path(),
        &["task", "show", foreign, "--json"],
    )
    .output()
    .unwrap();
    assert!(!foreign_read.status.success());
    assert!(String::from_utf8_lossy(&foreign_read.stderr).contains("is not registered"));
    let rejected = command(
        first.path(),
        home.path(),
        &["task", "edit", foreign, "--title", "Wrong owner"],
    )
    .output()
    .unwrap();
    assert!(!rejected.status.success());
    lf(
        &alias,
        home.path(),
        &["task", "edit", "dup-1", "--title", "First only"],
    );
    let status = lf(
        second.path(),
        home.path(),
        &["desktop", "task", "DUP-1", "--json"],
    );
    assert_eq!(status["task"]["title"], "Original");
    let status = lf(
        first.path(),
        home.path(),
        &["desktop", "task", "DUP-1", "--json"],
    );
    assert_eq!(status["task"]["title"], "First only");
    let missing = command(
        first.path(),
        home.path(),
        &["task", "show", "MISSING-1", "--json"],
    )
    .output()
    .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("is not registered"));
}

#[test]
fn ordinary_task_creation_returns_resolvable_identifier_without_a_public_retry_token() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let help = command(repo.path(), home.path(), &["task", "create", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(!String::from_utf8_lossy(&help.stdout).contains("creation-id"));
    let removed = command(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--title",
            "Saved task",
            "--creation-id",
            "task_0123456789ab40008000000000000001",
        ],
    )
    .output()
    .unwrap();
    assert!(!removed.status.success());
    assert!(
        String::from_utf8_lossy(&removed.stderr).contains("unexpected argument '--creation-id'")
    );
    let output = command(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Saved task"],
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    let identifier = output.split_whitespace().last().unwrap();
    let status = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", identifier, "--json"],
    );
    assert_eq!(status["task"]["identifier"], identifier);
    assert_eq!(status["task"]["title"], "Saved task");
    let id = status["task"]["id"].as_str().unwrap();
    loopflow_execution::durable::TaskId::parse(id).unwrap();
    assert_eq!(status["execution"]["task_id"], id);
    assert!(status["execution"]["worktree"].is_null());
    assert!(status["execution"]["pr"].is_null());
}

#[test]
fn wave_sync_offline_fails_with_the_tracker_error_and_queues_no_project() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "inbox", "--json"],
    );
    repo.create_file(
        ".lf/config.yaml",
        "planning:\n  provider: linear\n  team: fixture-team\n",
    );
    let output = command(repo.path(), home.path(), &["wave", "sync", "inbox"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No Linear credential found"), "{stderr}");
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let queued: i64 = db
        .query_row(
            "SELECT count(*) FROM tracker_actions WHERE project_id IS NOT NULL AND phase!='retired'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(queued, 0);
    let status = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert!(
        status["project"]["value"]["tracker_id"].is_null(),
        "{status}"
    );
}

#[test]
fn wave_sync_makes_only_its_own_project() {
    use crate::lf::commands::write::Write;
    use loopflow_execution::test_support::planning_tasks::with_planning_task;
    use loopflow_execution::work::wave::Wave;
    use loopflow_model::id::WaveId;

    with_planning_task(|runtime, fixture, repo, _, state| {
        let wave =
            |name: String| Wave::new(WaveId::new(), name, repo.to_string_lossy().into_owned());
        for n in 0..3 {
            runtime
                .block_on(fixture.store.create_wave(&wave(format!("older-{n}"))))
                .unwrap();
        }
        let billing = wave("billing".into());
        runtime
            .block_on(fixture.store.create_wave(&billing))
            .unwrap();
        let sync = || {
            Write::WaveSync {
                directory: repo.into(),
                wave: "billing".into(),
            }
            .run()
            .map(|status| serde_json::to_value(status).unwrap())
        };
        runtime.block_on(state.lock()).field_outage = true;
        assert!(sync().is_err());
        {
            let mut linear = runtime.block_on(state.lock());
            linear.field_outage = false;
            linear.export_mode = true;
        }
        let id = runtime
            .block_on(fixture.store.project_of_wave(billing.id()))
            .unwrap()
            .unwrap()
            .id;
        let external = uuid::Uuid::parse_str(id.as_str().strip_prefix("proj_").unwrap())
            .unwrap()
            .to_string();
        for _ in 0..2 {
            let status = sync().unwrap();
            assert_eq!(status["project"]["value"]["id"], id.as_str());
            assert_eq!(status["project"]["value"]["tracker_id"], external);
            assert!(fixture.wave_refresh_at(&billing).is_some());
        }
        let linear = runtime.block_on(state.lock());
        let names: Vec<_> = linear
            .extra_projects
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["billing"]);
    });
}

#[test]
fn wave_sync_without_a_tracker_sends_the_plan_by_git() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| loopflow_execution::git::git_stdout(repo.path(), args).unwrap();
    git(&["remote", "remove", "origin"]);
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "billing", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "billing",
            "--title",
            "Saved offline",
            "--json",
        ],
    );
    git(&[
        "remote",
        "add",
        "origin",
        repo.bare_path().to_str().unwrap(),
    ]);
    let synced = lf(
        repo.path(),
        home.path(),
        &["wave", "sync", "billing", "--json"],
    );
    assert_eq!(synced["project"]["value"]["name"], "billing");
    assert!(!git(&["ls-remote", "origin", "refs/loopflow/plan/*"])
        .trim()
        .is_empty());
}

#[test]
fn wave_planning_distinguishes_missing_registry_from_unreadable_and_removes_old_roots() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let shown = lf(repo.path(), home.path(), &["wave", "show", "--json"]);
    assert_eq!(shown["waves"], serde_json::json!([]));
    assert!(!home.path().join("loopflow.db").exists());
    // Definitions are files: reading them needs no registry and creates none.
    for args in [
        vec!["graph", "show", "code", "--json"],
        vec!["graph", "list", "--json"],
    ] {
        lf(repo.path(), home.path(), &args);
        assert!(!home.path().join("loopflow.db").exists());
    }
    let graph_reads = [vec!["wave", "graph", "show", "missing", "--json"]];
    for args in &graph_reads {
        let output = command(repo.path(), home.path(), args).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("registry is absent"));
        assert!(!home.path().join("loopflow.db").exists());
    }
    std::fs::write(home.path().join("loopflow.db"), "not a database").unwrap();
    for args in &graph_reads {
        let output = command(repo.path(), home.path(), args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            std::fs::read(home.path().join("loopflow.db")).unwrap(),
            b"not a database"
        );
    }
    let output = command(repo.path(), home.path(), &["wave", "show", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    for root in ["roadmap", "project"] {
        let output = command(repo.path(), home.path(), &[root, "--help"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "retired command {root} survived");
    }
}

#[test]
fn wave_add_commits_on_main_and_checkout_edits_reach_status() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let checkout = repo.create_named_worktree("wave-documents");
    let before = repo.head_sha();
    let args = [
        "wave",
        "add",
        "local",
        "--objective",
        "The shared objective.",
        "--json",
    ];
    let added = lf(&checkout, home.path(), &args);
    assert_eq!(added["created"], true);
    assert_eq!(added["files"]["state"], "committed");
    assert_ne!(repo.head_sha(), before, "add commits on the main checkout");
    assert_eq!(
        std::fs::read_to_string(repo.path().join(".lf/wave/local/GOAL.md")).unwrap(),
        "## Objective\n\nThe shared objective.\n"
    );
    assert!(
        !checkout.join(".lf/wave/local").exists(),
        "add must not strand files in the invoking worktree"
    );
    let again = lf(&checkout, home.path(), &args);
    assert_eq!(again["created"], false);
    assert_eq!(again["id"], added["id"]);
    assert_eq!(again["files"]["state"], "unchanged");
    let listing = lf(repo.path(), home.path(), &["wave", "list", "--json"]);
    assert_eq!(listing.as_array().unwrap().len(), 1);

    let goal = checkout.join(".lf/wave/local/GOAL.md");
    let memory = checkout.join(".lf/wave/local/MEMORY.md");
    std::fs::create_dir_all(goal.parent().unwrap()).unwrap();
    std::fs::write(&goal, "## Objective\n\nThe checkout objective.\n").unwrap();
    std::fs::write(&memory, "A freshly edited decision.").unwrap();
    let status = lf(
        &checkout,
        home.path(),
        &["wave", "status", "local", "--json"],
    );
    assert_eq!(status["wave"]["goal"], "The checkout objective.");
    let listing = lf(&checkout, home.path(), &["wave", "list", "--json"]);
    assert_eq!(listing[0]["goal"], "The checkout objective.");
    let main = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "local", "--json"],
    );
    assert_eq!(
        main["wave"]["goal"], "The shared objective.",
        "each checkout reads its own files"
    );
    let context = lf(
        &checkout,
        home.path(),
        &["context", "--wave", "local", "--json"],
    );
    let usage = context["context"]["usage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["source"] == ".lf/wave/local/MEMORY.md")
        .unwrap();
    assert_eq!(usage["bytes"], "A freshly edited decision.".len());
    let removed = command(
        &checkout,
        home.path(),
        &[
            "wave",
            "edit",
            "local",
            "--memory",
            memory.to_str().unwrap(),
        ],
    )
    .output()
    .unwrap();
    assert!(!removed.status.success());
    assert_eq!(
        std::fs::read_to_string(memory).unwrap(),
        "A freshly edited decision."
    );
}

#[test]
fn canonical_status_retains_content_and_execution_across_acquisition_states() {
    use crate::test_linear_input::IssueFixture;
    use loopflow_execution::durable::TaskId;
    use serde_json::json;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "Original", "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let id = TaskId::parse(id).unwrap();
    let path = home.path().join("loopflow.db");
    let store = loopflow_execution::store::open_sqlite(&path).unwrap();
    let task = store.task(&id).unwrap().unwrap();
    // The Wave's Project, bound to the tracker's.
    let project = store
        .project(task.project_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE projects SET external_project_id='provider-project' WHERE id=?1",
            [project.id.as_str()],
        )
        .unwrap();
    let observed: loopflow_storage::planning::TrackerProject = serde_json::from_value(json!({
        "id":"provider-project", "slug":project.slug, "name":project.name,
        "summary":"", "description":"Saved Project prose", "status":"started", "team_ids":["team"], "revision":null
    }))
    .unwrap();
    loopflow_execution::store::sqlite::linear_events::accept_linear_project_in(
        &store,
        task.wave_id.as_ref().unwrap(),
        &observed,
        17,
        loopflow_storage::planning_sync::TrackerKind::Linear,
    )
    .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "UPDATE tasks SET external_issue_id='provider-task',issue_identifier='FIX-1' WHERE id=?1",
        [id.as_str()],
    )
    .unwrap();
    let record: IssueFixture = serde_json::from_value(json!({
        "item":{"id":"provider-task","identifier":"FIX-1","name":"Original", "description":"Provider prose",
            "rank":0,"completed":false,"state":"unstarted","project_id":"provider-project", "project":project.slug,
            "team_id":"team","due_date":"2026-10-11", "assignee":null, "url":null},
        "project":observed,"observed_at":17
    })).unwrap();
    let scope = loopflow_execution::repository::discover(repo.path())
        .unwrap()
        .to_string();
    store.seed_issue(&scope, "linear", &record).unwrap();
    let task = store.task(&id).unwrap().unwrap();
    store
        .edit_task(
            &id,
            task.revision,
            &loopflow_execution::work::task::TaskUpdate {
                title: Some("Saved locally".into()),
                description: Some("Saved Task prose".into()),
                ..Default::default()
            },
        )
        .unwrap();
    // Seed a locally retained date differing from the provider observation, and
    // a captured graph. Reading must not copy either value from another owner.
    db.execute(
        "UPDATE tasks SET planning_due_date='2026-10-20' WHERE id=?1",
        [id.as_str()],
    )
    .unwrap();
    db.execute("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code','/retained/task',2)", []).unwrap();
    db.execute("INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,?2,'review','{\"index\":1}','11111111-1111-4111-8111-111111111111',7)", rusqlite::params![id.as_str(),json!({"name":"review","steps":[{"Skill":{"skill":{"name":"review"},"id":null,"interactive":true,"returns":null,"sources":[]}},{"Skill":{"skill":{"name":"implement"},"id":null,"interactive":false,"returns":null,"sources":[]}}]}).to_string()]).unwrap();
    let saved = store.task(&id).unwrap().unwrap();
    let read = || {
        lf(
            repo.path(),
            home.path(),
            &["desktop", "task", id.as_str(), "--json"],
        )
    };
    let initial = read();
    for (state, statement) in [
        ("available", "SELECT 1"),
        ("invalid", "UPDATE tracker_items SET needs_refresh=1"),
        ("available", "UPDATE tracker_items SET needs_refresh=0"),
        (
            "invalid",
            "UPDATE tracker_items SET project_id='another-project',claimed_project_id='another-project'",
        ),
    ] {
        db.execute_batch(statement).unwrap();
        let status = read();
        assert_eq!(status["tracker_state"]["state"], state);
        assert_eq!(status["tracker_state"]["synced_at"], 17);
        assert_eq!(status["task"]["id"], id.as_str());
        assert_eq!(status["task"]["title"], "Saved locally");
        assert_eq!(status["task"]["description"], "Saved Task prose");
        assert_eq!(status["task"]["due_date"], "2026-10-20");
        assert_eq!(
            status["project"]["id"],
            saved.project_id.as_ref().unwrap().as_str()
        );
        assert_eq!(status["project"]["description"], "Saved Project prose");
        assert_eq!(status["execution"], initial["execution"]);
        let wave = lf(
            repo.path(),
            home.path(),
            &["desktop", "wave", "inbox", "--json"],
        );
        let summary = &wave["tasks"]["items"][0]["task"];
        assert_eq!(
            summary["tracker_state"],
            json!({"state":state,"synced_at":17})
        );
        assert_eq!(summary["due_date"], "2026-10-20");
        if state == "invalid" {
            assert!(wave["tasks"]["items"][0]["run_control"]["unavailable"]
                .as_str()
                .unwrap()
                .contains("planning"));
        }
    }
    // A pending membership save explains the difference without replacing content.
    db.execute("INSERT INTO tracker_actions(id,task_id,subject,kind,value_json,phase,tracker) VALUES('00000000-0000-4000-8000-0000000000aa',?1,'task_project','field',?2,'queued','linear')", rusqlite::params![id.as_str(),serde_json::to_string(&WorkValue::TaskProject(saved.project_id.as_ref().map(ToString::to_string))).unwrap()]).unwrap();
    assert_eq!(read()["tracker_state"]["state"], "available");
    store
        .record_tracker_event("provider-task", None, true)
        .unwrap();
    let removed = read();
    assert_eq!(removed["tracker_state"]["state"], "removed");
    assert_eq!(removed["tracker_state"]["synced_at"], 17);
    assert_eq!(removed["execution"]["work"], initial["execution"]["work"]);
    assert_eq!(
        removed["execution"]["execution"],
        initial["execution"]["execution"]
    );
    assert_eq!(removed["task"]["due_date"], "2026-10-20");
    assert_eq!(removed["task"]["title"], "Saved locally");
    let after_removal = store.task(&id).unwrap().unwrap();
    assert_eq!(after_removal.worktree, saved.worktree);
    assert_eq!(after_removal.branch, saved.branch);
    assert_eq!(after_removal.project_id, saved.project_id);
    let step: String = db
        .query_row(
            "SELECT step FROM task_graphs WHERE task_id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(step, "review");
    let process: (Option<i64>, Option<String>) = db.query_row("SELECT completed_at,outcome FROM processes WHERE id='11111111-1111-4111-8111-111111111111'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(process, (None, None));
}

fn observed_issue(
    id: &str,
    project: &str,
    title: &str,
    description: &str,
) -> loopflow_storage::planning::TrackerItem {
    loopflow_storage::planning::TrackerItem {
        id: id.into(),
        identifier: "FIX-1".into(),
        name: title.into(),
        description: description.into(),
        project_id: Some(project.into()),
        team_id: Some("fixture-team".into()),
        revision: None,
        branch_name: None,
        url: None,
        rank: 0,
        completed: false,
        completed_at: None,
        state: Some("unstarted".into()),
        assignee: None,
        parent_id: None,
        parent_observed: true,
        due_date: None,
    }
}

#[test]
fn readable_checkout_names_preserve_explicit_names_and_reject_occupied_branches() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let title = "Fix login with a very long explanation";
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", title, "--json"],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let label = created["identifier"].as_str().unwrap();
    assert_eq!(label.len(), 6);
    let expected = format!("jack/{label}-fix-login-with");
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["branch", &expected])
        .status()
        .unwrap()
        .success());
    let rejected = command(repo.path(), home.path(), &["checkout", label, "--json"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("already exists"));
    let placed = lf(
        repo.path(),
        home.path(),
        &["checkout", label, "--name", "login-repair", "--json"],
    );
    assert_eq!(placed["branch"], format!("jack/{label}-login-repair"));
    assert!(placed["worktree"]
        .as_str()
        .unwrap()
        .ends_with(".login-repair"));
    let retained = lf(
        repo.path(),
        home.path(),
        &["checkout", id, "--name", "login-repair", "--json"],
    );
    assert_eq!(retained["branch"], placed["branch"]);
    assert_eq!(retained["worktree"], placed["worktree"]);
}

#[test]
fn offline_linear_checkout_uses_known_identifier_and_caps_title() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let title = format!("{} login", "Long".repeat(30));
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", &title, "--json"],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "UPDATE tasks SET issue_identifier='ENG-123' WHERE id=?1",
        [id],
    )
    .unwrap();
    let placed = lf(repo.path(), home.path(), &["checkout", "ENG-123", "--json"]);
    assert_eq!(
        placed["branch"],
        format!("jack/eng-123-{}", "long".repeat(8))
    );
    conn.execute(
        "UPDATE tasks SET issue_identifier='ENG-124',issue_title='Changed title' WHERE id=?1",
        [id],
    )
    .unwrap();
    let retained = lf(repo.path(), home.path(), &["checkout", "ENG-124", "--json"]);
    assert_eq!(retained["branch"], placed["branch"]);
    assert_eq!(retained["worktree"], placed["worktree"]);
}

/// The Task's graph as `lf desktop task --json` carries it.
fn task_graph(repo: &Path, home: &Path, id: &str) -> Value {
    lf(repo, home, &["desktop", "task", id, "--json"])["execution"]["work"]["graph"].clone()
}

/// Run `lf task run …` bare, which may return as its Graph registers, and
/// wait until the Task stands at an interactive step or `end`.
fn run_until_waiting(repo: &Path, home: &Path, args: &[&str], id: &str) -> Value {
    lf(repo, home, args);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let graph = task_graph(repo, home, id);
        if graph["position"]["kind"] == "at" {
            return graph;
        }
        assert!(std::time::Instant::now() < deadline, "{graph}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
fn waveless_task_lifecycle() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(repo.path().join(".lf/config.yaml"), "graph: proof\n").unwrap();
    repo.create_file(
        ".lf/graphs/proof.yaml",
        "- cmd: graph show proof\n- interactive: {name: demo, id: review}\n",
    );
    repo.stage_all();
    repo.commit("Configure the repository graph");
    repo.push();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Standalone proof",
            "--json",
        ],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    assert!(created["repo_id"].is_string());
    assert!(created["project_id"].is_null());
    assert!(created["wave_id"].is_null());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let counts: (u32, u32, u32) = db.query_row("SELECT (SELECT count(*) FROM tasks),(SELECT count(*) FROM waves),(SELECT count(*) FROM projects)", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
    assert_eq!(counts, (1, 0, 0));
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(listed[0]["task"]["id"], id);
    assert_eq!(listed[0]["graph_name"], "proof");
    let inventory = lf(repo.path(), home.path(), &["wave", "show", "--json"]);
    assert_eq!(inventory["tasks"]["items"][0]["task"]["id"], id);
    let placed = lf(repo.path(), home.path(), &["checkout", id, "--json"]);
    let before = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(before["position"]["step"], "review");
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["task", "move", id, "--wave", "product"],
    );
    let attached = lf(repo.path(), home.path(), &["desktop", "task", id, "--json"]);
    assert_eq!(attached["execution"]["worktree"], placed["worktree"]);
    assert_eq!(attached["execution"]["wave"], "product");
    assert_eq!(task_graph(repo.path(), home.path(), id), before);
    lf(repo.path(), home.path(), &["task", "move", id, "--no-wave"]);
    lf(repo.path(), home.path(), &["task", "move", id, "--no-wave"]);
    let detached = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(detached[0]["task"]["id"], id);
    assert!(detached[0]["task"]["wave_id"].is_null());
    lf(repo.path(), home.path(), &["task", "complete", id]);
    assert_eq!(
        lf(repo.path(), home.path(), &["desktop", "task", id, "--json"])["execution"]["status"],
        "done"
    );

    let standalone = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Always standalone", "--json"],
    )["task"]
        .clone();
    let standalone_id = standalone["id"].as_str().unwrap();
    run_until_waiting(
        repo.path(),
        home.path(),
        &["task", "run", standalone_id],
        standalone_id,
    );
    // Nothing runs after review: the next run reaches end and completes.
    lf(repo.path(), home.path(), &["task", "run", standalone_id]);
    let completed = lf(
        repo.path(),
        home.path(),
        &["desktop", "task", standalone_id, "--json"],
    );
    assert_eq!(completed["execution"]["status"], "done");
    assert!(completed["execution"]["wave"].is_null());
    assert!(completed["execution"]["project_id"].is_null());
}

#[test]
fn unconfigured_repository_runs_tasks_on_the_builtin_code_graph() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    // Stand in for the agent steps of plan and build; the Task's graph stays
    // the builtin.
    repo.create_file(".lf/graphs/plan.yaml", "- cmd: graph show plan\n");
    repo.create_file(".lf/graphs/build.yaml", "- cmd: graph show build\n");
    repo.stage_all();
    repo.commit("Stand in for plan and build");
    repo.push();
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "First Task", "--json"],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(listed[0]["graph_name"], "code");
    let shown = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(shown["name"], "code");
    assert_eq!(shown["position"]["step"], "ship-review");
}

#[test]
fn graph_defaults_follow_task_wave_and_repository() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/config.yaml", "graph: repository\n");
    repo.create_file(".lf/graphs/proof.yaml", "- cmd: graph show proof\n");
    for (name, step) in [
        ("repository", "repo-review"),
        ("wave", "wave-review"),
        ("explicit", "task-review"),
        ("code", "default-review"),
    ] {
        repo.create_file(
            &format!(".lf/graphs/{name}.yaml"),
            &format!("- graph: proof\n- interactive: {{name: demo, id: {step}}}\n"),
        );
    }
    repo.stage_all();
    repo.commit("Configure selection proof");
    repo.push();
    lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["--wave", "product", "set", "graph", "wave"],
    );
    let task = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "product",
            "--title",
            "Wave default",
            "--json",
        ],
    )["task"]
        .clone();
    let id = task["id"].as_str().unwrap();
    let captured = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(captured["name"], "wave");
    assert_eq!(captured["position"]["step"], "wave-review");
    lf(
        repo.path(),
        home.path(),
        &["--wave", "product", "set", "graph", "explicit"],
    );
    lf(repo.path(), home.path(), &["task", "move", id, "--no-wave"]);
    repo.create_file(".lf/config.yaml", "graph: ''\n");
    // The Task keeps the graph it took up.
    assert_eq!(task_graph(repo.path(), home.path(), id), captured);
    let standalone = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--no-wave",
            "--title",
            "Fallback default",
            "--json",
        ],
    )["task"]
        .clone();
    let standalone_id = standalone["id"].as_str().unwrap();
    let fallback = run_until_waiting(
        repo.path(),
        home.path(),
        &["task", "run", standalone_id],
        standalone_id,
    );
    assert_eq!(fallback["name"], "code");
    assert_eq!(fallback["position"]["step"], "default-review");
    repo.create_file(".lf/config.yaml", "graph: repository\n");
    let explicit = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "product",
            "--title",
            "Explicit choice",
            "--json",
        ],
    )["task"]
        .clone();
    let explicit_id = explicit["id"].as_str().unwrap();
    let named = run_until_waiting(
        repo.path(),
        home.path(),
        &["task", "run", explicit_id, "repository"],
        explicit_id,
    );
    assert_eq!(named["name"], "repository");
}

#[test]
fn task_creation_defaults_to_standalone_in_bound_contexts() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "add", "product", "--json"],
    );
    let wave_id = wave["id"].as_str().unwrap();
    let parent = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "product", "--title", "Parent", "--json",
        ],
    )["task"]
        .clone();
    let parent_id = parent["id"].as_str().unwrap();
    let placed = lf(repo.path(), home.path(), &["checkout", parent_id, "--json"]);
    let checkout = Path::new(placed["worktree"].as_str().unwrap());
    let task_context = format!("task:{parent_id}");
    let wave_context = format!("wave:{wave_id}");
    for (cwd, declaration, inherited_wave) in [
        (repo.path(), None, ""),
        (repo.path(), Some(wave_context.as_str()), wave_id),
        (repo.path(), Some(task_context.as_str()), wave_id),
        (repo.path(), Some("wave:missing-wave"), "missing-wave"),
        (repo.path(), Some("task:missing-task"), "missing-wave"),
        (checkout, None, ""),
        (checkout, Some(task_context.as_str()), wave_id),
    ] {
        for flags in [&[][..], &["--no-wave"][..], &["--wave", "product"][..]] {
            let mut args = vec!["task", "create", "--title", "Capture", "--json"];
            args.extend_from_slice(flags);
            let mut invocation = command(cwd, home.path(), &args);
            invocation.env("LF_WAVE_ID", inherited_wave);
            if let Some(declaration) = declaration {
                invocation.env("LF_AS", declaration);
            }
            let output = invocation.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let task: Value = serde_json::from_slice(&output.stdout).unwrap();
            let task = &task["task"];
            assert_eq!(task["repo_id"], parent["repo_id"]);
            if flags.first() == Some(&"--wave") {
                assert_eq!(task["wave_id"], parent["wave_id"]);
                assert_eq!(task["project_id"], parent["project_id"]);
            } else {
                assert!(task["wave_id"].is_null(), "{task}");
                assert!(task["project_id"].is_null(), "{task}");
            }
            assert!(task["worktree"].is_null());
        }
    }
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let retained = store
        .task(&loopflow_execution::durable::TaskId::parse(parent_id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retained.wave_id.as_ref().unwrap().as_str(), wave_id);
    assert_eq!(retained.worktree.as_deref(), Some(checkout));
}
