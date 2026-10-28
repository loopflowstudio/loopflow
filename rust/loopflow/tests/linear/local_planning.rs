use crate::test_linear_input::{InventoryFixture, IssueFixture, SeedLinearSqlite};
use crate::work::edit::WorkValue;
use crate::work::project::ProjectStatus;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow_test_support::TestRepo;
use serde_json::Value;

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
    );
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
    let published = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_ne!(published["execution"]["status"], "done");
    lf(worktree, home.path(), &["land"]);
    let armed = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
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
    let merged = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_ne!(merged["execution"]["status"], "done");
    assert!(
        worktree.exists(),
        "retain checkout until explicit completion"
    );
    lf(repo.path(), home.path(), &["task", "complete", id]);
    let completed = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
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
        &["wave", "ensure", "inbox", "--json"],
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
        assert!(ids.insert(task["id"].as_str().unwrap().to_owned()));
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
        &["task", "status", id.as_str(), "--json"],
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
fn stored_rotation_commits_all_waves_and_serializes_new_work() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    let mut predecessors = Vec::new();
    let successor = uuid::Uuid::new_v4().to_string();
    for name in ["first", "second"] {
        lf(
            repo.path(),
            home.path(),
            &["wave", "ensure", name, "--json"],
        );
        let wave = lf(
            repo.path(),
            home.path(),
            &["wave", "status", name, "--json"],
        );
        predecessors.push(
            wave["projects"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|project| project["current"] == true)
                .unwrap()
                .clone(),
        );
        entries.push(serde_json::json!({"wave_id":wave["wave"]["id"],"successor_id":successor,"create":true,"project_name":"Next","metric_targets":[],"krs":[{"text":"Keep private work","holds":false}]}));
    }
    let path = home.path().join("rotation.json");
    let args = [
        "wave",
        "new-chapter",
        "--all",
        "Next",
        "--plan",
        path.to_str().unwrap(),
        "--json",
    ];
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"name":"Next","waves":entries})).unwrap(),
    )
    .unwrap();
    let failed = command(repo.path(), home.path(), &args).output().unwrap();
    assert!(!failed.status.success());
    for (i, name) in ["first", "second"].iter().enumerate() {
        let wave = lf(
            repo.path(),
            home.path(),
            &["wave", "status", name, "--json"],
        );
        assert!(predecessors[i].is_object(), "{wave}");
        assert_eq!(
            wave["projects"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|project| project["current"] == true)
                .unwrap(),
            &predecessors[i]
        );
    }
    entries[1]["successor_id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"name":"Next","waves":entries})).unwrap(),
    )
    .unwrap();
    let children: Vec<_> = (0..3)
        .map(|_| {
            command(
                repo.path(),
                home.path(),
                &[
                    "task",
                    "create",
                    "--wave",
                    "first",
                    "--title",
                    "Concurrent work",
                    "--json",
                ],
            )
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
        })
        .collect();
    lf(repo.path(), home.path(), &args);
    let current = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "first", "--json"],
    );
    let destination = &current["projects"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|project| project["current"] == true)
        .unwrap()["id"];
    assert_ne!(destination, &predecessors[0]["id"]);
    let mut identities = std::collections::HashSet::new();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let created: Value = serde_json::from_slice(&output.stdout).unwrap();
        let id = created["id"].as_str().unwrap();
        assert!(identities.insert(id.to_string()));
        let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
        let project = &status["project"]["id"];
        assert!(project == destination || project == &predecessors[0]["id"]);
        assert!(status["execution"]["worktree"].is_null());
    }
    assert!(repo.path().join(".lf/wave").exists());
}

#[test]
fn public_local_plan_matches_desktop() {
    let repo = TestRepo::new();
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
    );
    let id = created["id"].as_str().unwrap();
    let comments = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "comment",
            id,
            "Preserve the escaped quote",
            "--json",
        ],
    );
    assert!(uuid::Uuid::parse_str(comments["comments"][0]["id"].as_str().unwrap()).is_ok());
    assert!(time::OffsetDateTime::parse(
        comments["comments"][0]["created_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339
    )
    .is_ok());
    let roadmap = lf(repo.path(), home.path(), &["wave", "show", "--json"]);
    let mut task = roadmap["tasks"]["items"][0].clone();
    assert_eq!(task["task"]["id"], id);
    assert!(task["reference"]["workspace"].is_null());
    assert!(task["run_control"]["unavailable"].is_null());
    assert_eq!(task["condition"]["unresolved_execution"], false);
    assert_eq!(task["actions"]["recommended"], "resume");
    task["condition"]["observed_at"] = serde_json::json!("2026-10-08T00:00:00Z");
    task["condition"]["evidence_age_secs"] = serde_json::json!(0);
    task["runtime"]["updated_at"] = serde_json::json!("2026-10-08T00:00:00Z");
    let comment_id = comments["comments"][0]["id"].as_str().unwrap().to_string();
    let comment_date = comments["comments"][0]["created_at"]
        .as_str()
        .unwrap()
        .to_string();
    task["task"]["repository_id"] = serde_json::json!("repo_00000000000040008000000000000001");
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
    serde_json::from_value::<loopflow::work::task::comment::TaskComments>(
        actual["comments"].clone(),
    )
    .unwrap();
}

#[test]
fn stored_fields_preserve_order_assignment_and_project_summary() {
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
    );
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
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    for field in ["name", "description", "rank", "assignee"] {
        assert_eq!(
            status["task"][if field == "name" { "title" } else { field }],
            fields[field]
        );
    }
    let project = status["project"]["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "wave",
            "edit-plan",
            "--project",
            project,
            "--name",
            "Current",
            "--summary",
            fields["summary"].as_str().unwrap(),
        ],
    );
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(status["project"]["summary"], fields["summary"]);
    assert_eq!(status["project"]["name"], "Current");
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", id, "--unassign", "--notes", ""],
    );
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert!(status["task"]["assignee"].is_null());
    assert_eq!(status["task"]["description"], "");
    assert_eq!(status["task"]["rank"], 0);
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(wave["tasks"]["items"][0]["task"]["id"], id);
    assert_eq!(wave["tasks"]["items"][1]["task"]["rank"], 1);
}

#[test]
fn nested_waves_create_files_and_store_project_identity() {
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
    );
    let status = lf(
        repo.path(),
        home.path(),
        &["task", "status", created["id"].as_str().unwrap(), "--json"],
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
    );
    let other = lf(
        repo.path(),
        home.path(),
        &["task", "status", again["id"].as_str().unwrap(), "--json"],
    );
    assert_eq!(status["project"]["id"], other["project"]["id"]);
    lf(
        repo.path(),
        home.path(),
        &["wave", "rename", "tools", "--name", "instruments", "--json"],
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
        &["wave", "ensure", "instruments/lexer", "--json"],
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
    let moved = lf(
        repo.path(),
        home.path(),
        &["task", "status", created["id"].as_str().unwrap(), "--json"],
    );
    assert_ne!(moved["project"]["id"], status["project"]["id"]);
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "instruments/parser", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 1);
    let wave_id = wave["wave"]["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["wave", "ensure", wave_id, "--json"],
    );
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
    );
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
        &["wave", "ensure", "inbox", "--json"],
    );
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let project: String = conn
        .query_row(
            "SELECT current_project_id FROM waves WHERE name='inbox'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for (id, title) in [(first, "First"), (second, "Second")] {
        store
            .create_task(&loopflow::work::task::NewTask {
                repository_id: store
                    .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
                    .unwrap(),
                due_date: None,
                id: loopflow::durable::TaskId::parse(id).unwrap(),
                project_id: Some(loopflow::durable::ProjectId::parse(&project).unwrap()),
                title: title.into(),
                description: String::new(),
            })
            .unwrap();
    }
    for selector in ["abcd1234", "ABCD1234", "lf-abcd1234", "task_abcd1234"] {
        let status = lf(
            repo.path(),
            home.path(),
            &["task", "status", selector, "--json"],
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
        &["task", "status", first, "--json"],
    );
    assert_eq!(status["task"]["title"], "Chosen target");
    let other = lf(
        repo.path(),
        home.path(),
        &["task", "status", second, "--json"],
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
    let created = lf(repo.path(), home.path(), &args);
    let identity = created["id"].as_str().unwrap().to_owned();
    loopflow::durable::TaskId::parse(&identity).unwrap();
    assert!(created["linear_id"].is_null());
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
    );
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
            "comment",
            identity.as_str(),
            "Preserve the escaped quote",
            "--json",
        ],
    );
    assert_eq!(comments["comments"].as_array().unwrap().len(), 1);
    let status = lf(
        repo.path(),
        home.path(),
        &["task", "status", identity.as_str(), "--json"],
    );
    assert_eq!(status["task"]["title"], "Retain quoted input");
    assert_eq!(status["execution"]["task_id"], identity.as_str());
    assert!(status["execution"]["issue_id"].is_null());
    assert!(status["execution"]["worktree"].is_null());
    assert!(status["execution"]["pr"].is_null());
    assert!(status["planning_error"].is_null());
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(wave["project_readiness"]["state"], "ready");
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
    let plan_file = home.path().join("plan.json");
    std::fs::write(
        &plan_file,
        r#"{"metric_targets":[],"krs":[{"text":"Quoted input survives","holds":false}]}"#,
    )
    .unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "wave",
            "update-plan",
            "-w",
            "inbox",
            "--plan",
            plan_file.to_str().unwrap(),
        ],
    );
    let project = lf(
        repo.path(),
        home.path(),
        &["task", "status", identity.as_str(), "--json"],
    );
    assert_eq!(
        project["project"]["krs"][0]["text"],
        "Quoted input survives"
    );
    let read_comments = lf(
        repo.path(),
        home.path(),
        &["task", "comment", identity.as_str(), "--json"],
    );
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
        &["task", "status", identity.as_str(), "--json"],
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
        &["task", "status", other_id, "--json"],
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
        &["task", "status", other_id, "--json"],
    );
    assert_eq!(retained["execution"]["status"], "abandoned");
    assert_eq!(retained["planning_state"], "removed");
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
    assert!(changed.stdout.is_empty());
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
    );
    let id = created["id"].as_str().unwrap();
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    let project_id = status["project"]["id"].as_str().unwrap();
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
    // A bare run may return as its Graph registers; it stops before the
    // interactive step, where a person takes part.
    lf(repo.path(), home.path(), &["task", "run", id]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
        if status["execution"]["work"]["graph"]["position"]["kind"] == "at" {
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
    assert!(status["planning_error"].is_null());
    let landing = command(Path::new(worktree), home.path(), &["--task", id, "land"])
        .output()
        .unwrap();
    assert!(!landing.status.success());
    let error = String::from_utf8_lossy(&landing.stderr);
    assert!(error.contains("remote"), "{error}");
    let retained = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
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
    );
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    let successor = uuid::Uuid::new_v4();
    let plan_file = home.path().join("rotation.json");
    std::fs::write(&plan_file,serde_json::to_vec(&serde_json::json!({"name":"Next", "waves":[{"wave_id":wave["wave"]["id"],"successor_id":successor.to_string(),"create":true,"project_name":"Next","metric_targets":[],"krs":[{"text":"Retain active work","holds":false}]}]})).unwrap()).unwrap();
    let rotate = [
        "wave",
        "new-chapter",
        "inbox",
        "Next",
        "--plan",
        plan_file.to_str().unwrap(),
        "--json",
    ];
    let rotation = lf(repo.path(), home.path(), &rotate);
    assert_eq!(rotation["waves"][0]["successor"]["status"], "started");
    lf(repo.path(), home.path(), &rotate);
    let original_plan = std::fs::read_to_string(&plan_file).unwrap();
    std::fs::write(
        &plan_file,
        original_plan.replace("Retain active work", "Conflicting retry"),
    )
    .unwrap();
    let conflict = command(repo.path(), home.path(), &rotate).output().unwrap();
    assert!(!conflict.status.success());
    std::fs::write(&plan_file, original_plan).unwrap();
    let moved = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(
        moved["project"]["id"],
        format!("proj_{}", successor.simple())
    );
    assert_eq!(
        moved["execution"]["worktree"],
        status["execution"]["worktree"]
    );
    assert_eq!(
        moved["execution"]["work"]["graph"],
        status["execution"]["work"]["graph"]
    );
    let retained = lf(
        repo.path(),
        home.path(),
        &["task", "status", backlog["id"].as_str().unwrap(), "--json"],
    );
    assert_eq!(retained["project"]["id"], project_id);
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
    );
    assert_ne!(retry["id"], id);
    lf(
        repo.path(),
        home.path(),
        &["task", "complete", id, "--reason", "Inspection completed"],
    );
    let completed = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(completed["execution"]["status"], "done");
    if Path::new(worktree).exists() {
        std::fs::remove_dir_all(worktree).unwrap();
    }
}

#[test]
fn project_creation_binding_and_activation_save_offline() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            repo.create_file(
                ".lf/config.yaml",
                "planning:\n  provider: linear\n  team: fixture-team\n",
            );
        }
        let path = home.path().join("loopflow.db");
        let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let wave = loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        store.create_wave(&wave).unwrap();
        let args = ["wave", "ensure", "product", "--json"];
        let mut first = command(repo.path(), home.path(), &args)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let second = lf(repo.path(), home.path(), &args);
        assert!(first.wait().unwrap().success());
        let created = lf(repo.path(), home.path(), &args);
        assert_eq!(created, second);
        let projects = store.list_projects(Some(wave.id())).unwrap();
        assert_eq!(projects.len(), 1);
        let id = projects[0].id.clone();
        assert!(projects[0].linear_id.is_none());
        assert!(store.list_tasks(Some(wave.id())).unwrap().is_empty());
        let db = rusqlite::Connection::open(&path).unwrap();
        let intent: String = db
            .query_row(
                "SELECT local_plan_json FROM project_transitions WHERE wave_id=?1",
                [wave.id()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&intent).unwrap()["name"],
            "product"
        );
        // Creation retries keep later authored content; binding accepts either identity.
        lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "edit-plan",
                "--project",
                id.as_str(),
                "--name",
                "Authored name",
                "--summary",
                "Retain summary",
            ],
        );
        for mapped in [false, true] {
            let provider = "11111111-1111-4111-8111-111111111111";
            db.execute(
                "UPDATE projects SET external_project_id=?2,status='backlog',krs=?3 WHERE id=?1",
                rusqlite::params![
                    id.as_str(),
                    mapped.then_some(provider),
                    r#"[{"text":"Preserve Tasks","holds":false}]"#
                ],
            )
            .unwrap();
            db.execute(
                "UPDATE waves SET current_project_id=NULL WHERE id=?1",
                [wave.id()],
            )
            .unwrap();
            let bound = lf(
                repo.path(),
                home.path(),
                &["wave", "bind-project", "product", id.as_str(), "--json"],
            );
            assert_eq!(bound["status"], "backlog");
            if mapped {
                assert_eq!(
                    lf(
                        repo.path(),
                        home.path(),
                        &["wave", "bind-project", "product", provider, "--json"]
                    ),
                    bound
                );
            }
            let active = lf(repo.path(), home.path(), &args);
            assert_eq!(active["status"], "started");
            assert_eq!(active["name"], "Authored name");
            assert_eq!(active["summary"], "Retain summary");
            assert!(active.get("graph").is_none());
            assert_eq!(active["krs"][0]["text"], "Preserve Tasks");
            let changes = store.pending_project_changes(&id).unwrap();
            let state = changes
                .iter()
                .find(|change| change.value.field() == "project_status")
                .unwrap();
            assert_eq!(
                state.value,
                WorkValue::ProjectStatus(ProjectStatus::Started)
            );
            lf(repo.path(), home.path(), &args);
            lf(
                repo.path(),
                home.path(),
                &["wave", "bind-project", "product", id.as_str(), "--json"],
            );
            assert_eq!(store.pending_project_changes(&id).unwrap(), changes);
            if mapped {
                let incoming = serde_json::from_value(serde_json::json!({
                    "id": provider, "slug": active["slug"], "name": "Authored name",
                    "summary": "Retain summary", "krs": active["krs"],
                    "metric_targets": [], "status": "completed", "revision": "2026-10-08T11:00:00Z",
                    "initiative_ids": ["initiative"], "team_ids": ["fixture-team"]
                }))
                .unwrap();
                loopflow::linear::acceptance::accept_linear_project_in(
                    &store,
                    wave.id(),
                    "initiative",
                    &incoming,
                    20,
                )
                .unwrap();
                // The unsent activation is sent last, so it stays pending and
                // replaces Linear's completion.
                assert!(store
                    .pending_project_changes(&id)
                    .unwrap()
                    .iter()
                    .any(|change| change.id == state.id));
                let conflict: Option<String> = db
                    .query_row(
                        "SELECT conflict_json FROM project_changes WHERE id=?1",
                        [&state.id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(conflict.is_none());
                assert_eq!(
                    db.query_row(
                        "SELECT status FROM projects WHERE id=?1",
                        [id.as_str()],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                    "started"
                );
            }
        }
        db.execute(
            "UPDATE projects SET status='completed' WHERE id=?1",
            [id.as_str()],
        )
        .unwrap();
        assert!(!command(repo.path(), home.path(), &args)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(
            db.query_row(
                "SELECT status FROM projects WHERE id=?1",
                [id.as_str()],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "completed"
        );
        db.execute(
            "UPDATE waves SET current_project_id=NULL WHERE id=?1",
            [wave.id()],
        )
        .unwrap();
        for selector in [id.as_str(), "Authored name"] {
            assert!(!command(
                repo.path(),
                home.path(),
                &["wave", "bind-project", "product", selector, "--json"]
            )
            .output()
            .unwrap()
            .status
            .success());
        }
        assert!(db
            .query_row(
                "SELECT current_project_id FROM waves WHERE id=?1",
                [wave.id()],
                |row| row.get::<_, Option<String>>(0)
            )
            .unwrap()
            .is_none());
        assert!(repo.path().join(".lf/wave").exists());
        assert_eq!(store.list_projects(Some(wave.id())).unwrap().len(), 1);
    }
}

#[test]
fn project_reads_keep_canonical_identity_and_authored_order_after_edits() {
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
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let wave = loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        store.create_wave(&wave).unwrap();
        let projects = ["first", "second"]
            .into_iter()
            .map(|name| {
                serde_json::from_value(serde_json::json!({
                    "id": format!("linear-{name}"), "slug": name, "name": name,
                    "summary": "", "revision": null, "status": "started", "krs": [], "metric_targets": [], "initiative_ids": ["initiative"],
                    "team_ids": ["team"]
                }))
                .unwrap()
            })
            .collect();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative".into(),
                synced_at: 17,
                snapshot: crate::test_linear_input::Inventory {
                    projects,
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
        let projects = store.list_projects(Some(wave.id())).unwrap();
        assert_eq!(
            projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
        lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "bind-project",
                "product",
                projects[0].id.as_str(),
                "--json",
            ],
        );
        // Make recency disagree with the shared order before and after an ordinary edit.
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        conn.execute(
            "UPDATE projects SET updated_at=1 WHERE id=?1",
            [projects[0].id.as_str()],
        )
        .unwrap();
        let saved = store.task_by_issue("FIX-1").unwrap().unwrap();
        for edited in [false, true] {
            if edited {
                lf(
                    repo.path(),
                    home.path(),
                    &[
                        "wave",
                        "edit-plan",
                        "--project",
                        projects[1].id.as_str(),
                        "--name",
                        "Edited",
                    ],
                );
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
                conn.execute("DELETE FROM pm_items", []).unwrap();
            }
            let status = lf(
                repo.path(),
                home.path(),
                &["wave", "status", "product", "--json"],
            );
            {
                let rows = &status["tasks"]["items"];
                let rows = rows.as_array().unwrap();
                assert_eq!(rows.len(), 1);
                let task = &rows[0]["task"];
                assert_eq!(task["id"], saved.id.as_str());
                assert_eq!(task["linear_id"], "linear-task");
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
            assert_eq!(retained.pm_snapshot_synced_at, saved.pm_snapshot_synced_at);
            {
                let rows = &status["projects"]["items"];
                let rows = rows.as_array().unwrap();
                assert_eq!(rows.len(), 2);
                for (row, project) in rows.iter().zip(&projects) {
                    assert_eq!(row["id"], project.id.as_str());
                    assert_eq!(
                        row["linear_id"],
                        project.linear_id.as_ref().unwrap().as_str()
                    );
                    assert!(row.get("work_id").is_none());
                    assert!(row["sync"].is_object());
                }
                assert_eq!(rows[0]["current"], true);
                assert_eq!(rows[1]["name"], if edited { "Edited" } else { "second" });
            }
            assert_eq!(
                status["project_readiness"]["project_id"],
                projects[0].id.as_str()
            );
        }
    }
}

#[test]
fn project_edits_save_offline_in_both_connection_modes() {
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
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let wave = loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        store.create_wave(&wave).unwrap();
        let project = loopflow::linear::LinearProject {
            description: None,
            id: "provider-project".into(),
            revision: Some("2026-10-08T10:00:00Z".into()),
            slug: "original".into(),
            name: "Original".into(),
            summary: "Original summary".into(),
            status: loopflow::work::project::ProjectStatus::Started,
            krs: vec![loopflow::work::project::ProjectKR {
                text: "Retain the proof".into(),
                holds: false,
            }],
            metric_targets: Vec::new(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
        };
        loopflow::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &project,
            17,
        )
        .unwrap();
        let id = store.list_projects(Some(wave.id())).unwrap()[0].id.clone();
        // Selection is shared planning too: seed it through the typed writer,
        // not raw SQL that disagrees with the retained Git journal.
        lf(
            repo.path(),
            home.path(),
            &["wave", "bind-project", "product", id.as_str(), "--json"],
        );
        if !mapped {
            rusqlite::Connection::open(home.path().join("loopflow.db"))
                .unwrap()
                .execute(
                    "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                    [id.as_str()],
                )
                .unwrap();
        }
        let mut authored = store.project(&id).unwrap().unwrap();
        authored.description =
            "Authored prose — preserved\n\n```yaml\nworkflow: example\n```\n".into();
        // Exact identities still resolve when another Project's name/slug shadows them.
        let mut shadow = store.project(&id).unwrap().unwrap();
        shadow.id = loopflow::durable::ProjectId::new();
        shadow.linear_id = None;
        shadow.name = id.to_string();
        shadow.slug = project.id.clone();
        store.insert_project(&shadow).unwrap();
        // Both repositories use an ordinary Wave, with optional mapping and no credentials.
        lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "edit-plan",
                "--project",
                id.as_str(),
                "--name",
                "Saved",
                "--summary",
                "",
                "--description",
                &authored.description,
            ],
        );
        // An active writer cannot block inspection of the committed plan.
        let writer = std::fs::File::open(
            home.path()
                .join("chapter-locks")
                .join(format!("{}.lock", wave.id())),
        )
        .unwrap();
        fs2::FileExt::lock_exclusive(&writer).unwrap();
        let saved = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
        drop(writer);
        assert_eq!(saved["project"]["id"], id.as_str());
        assert_eq!(
            saved["project"]["linear_id"],
            if mapped {
                serde_json::json!(project.id)
            } else {
                Value::Null
            }
        );
        assert_eq!(saved["project"]["description"], authored.description.trim());
        assert_eq!(saved["project"]["name"], "Saved");
        assert_eq!(saved["project"]["summary"], "");
        assert_eq!(saved["project"]["krs"][0]["text"], "Retain the proof");
        assert_eq!(saved["sync"]["connected"], connected);
        assert_eq!(
            saved["sync"]["changes"].as_array().unwrap().is_empty(),
            !connected
        );
        if mapped {
            assert_eq!(
                serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()}),
                saved
            );
        }
        assert_eq!(store.project(&shadow.id).unwrap().unwrap(), shadow);
        let changes = store.pending_project_changes(&id).unwrap();
        assert_eq!(changes.len(), 3);
        lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "edit-plan",
                "--project",
                id.as_str(),
                "--name",
                "Saved",
                "--summary",
                "",
                "--description",
                &authored.description,
            ],
        );
        // Repeated commands and reopened stores retain the original delivery identities.
        drop(store);
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        assert_eq!(store.pending_project_changes(&id).unwrap(), changes);
        if mapped {
            let mut incoming = project.clone();
            incoming.name = "Remote name".into();
            incoming.revision = Some("2026-10-08T11:00:00Z".into());
            incoming.krs[0].holds = true;
            loopflow::linear::acceptance::accept_linear_project_in(
                &store,
                wave.id(),
                "initiative",
                &incoming,
                18,
            )
            .unwrap();
            let read = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
            // The unsent rename is sent last and wins; unsaved fields follow Linear.
            assert_eq!(read["project"]["name"], "Saved");
            assert_eq!(read["project"]["krs"][0]["holds"], true);
            assert_eq!(read["project"]["pm_snapshot_synced_at"], 18);
            let revision: String = rusqlite::Connection::open(home.path().join("loopflow.db"))
                .unwrap()
                .query_row(
                    "SELECT planning_provider_revision FROM projects WHERE id=?1",
                    [id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(revision, "2026-10-08T11:00:00Z");
            let pending = store.pending_project_changes(&id).unwrap();
            assert!(pending
                .iter()
                .any(|change| change.value.field() == "project_name"));
            let conflicts = read["sync"]["changes"].as_array().unwrap();
            assert!(conflicts
                .iter()
                .all(|change| change["state"] != "adopted_linear"));
            // A provider edit matching the save neither settles nor duplicates it.
            incoming.name = "Saved".into();
            incoming.revision = Some("2026-10-08T12:00:00Z".into());
            loopflow::linear::acceptance::accept_linear_project_in(
                &store,
                wave.id(),
                "initiative",
                &incoming,
                19,
            )
            .unwrap();
            let read = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
            assert_eq!(store.pending_project_changes(&id).unwrap(), pending);
            assert_eq!(read["sync"]["changes"], Value::Array(conflicts.clone()));
        }
        let plan_file = home.path().join("plan.json");
        let content = serde_json::json!({
            "metric_targets": [],
            "krs": [{"text": "Save the complete plan offline", "holds": false}]
        });
        std::fs::write(&plan_file, serde_json::to_vec(&content).unwrap()).unwrap();
        let args = [
            "wave",
            "update-plan",
            "--wave",
            "product",
            "--plan",
            plan_file.to_str().unwrap(),
        ];
        let output = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains("pending planning sync"),
            connected
        );
        let saved = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
        assert_eq!(saved["project"]["krs"], content["krs"]);
        assert_eq!(saved["project"]["id"], id.as_str());
        assert_eq!(
            saved["project"]["linear_id"],
            if mapped {
                serde_json::json!(project.id)
            } else {
                Value::Null
            }
        );
        assert_eq!(saved["project"]["description"], authored.description.trim());
        assert_eq!(saved["project"]["name"], "Saved");
        assert_eq!(saved["project"]["summary"], "");
        assert_eq!(store.pending_project_changes(&id).unwrap().len(), 4);
        lf(repo.path(), home.path(), &args);
        let repeated = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
        assert_eq!(repeated, saved);
        if mapped {
            let mut incoming = project.clone();
            incoming.revision = Some("2026-10-08T13:00:00Z".into());
            incoming.krs[0].text = "Keep the concurrent provider plan".into();
            loopflow::linear::acceptance::accept_linear_project_in(
                &store,
                wave.id(),
                "initiative",
                &incoming,
                20,
            )
            .unwrap();
            let read = serde_json::json!({"project":store.project(&id).unwrap().unwrap(), "sync":store.project_planning_sync(&id).unwrap()});
            // The unsent plan is sent after the concurrent provider plan and wins.
            assert_eq!(
                read["project"]["krs"][0]["text"],
                "Save the complete plan offline"
            );
            assert!(store
                .pending_project_changes(&id)
                .unwrap()
                .iter()
                .any(|change| change.value.field() == "krs"));
            assert!(read["sync"]["changes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|change| change["state"] != "adopted_linear"));
        }
        assert!(store.list_tasks(None).unwrap().is_empty());
        assert!(!repo.path().join(".lf/wave").exists());
    }
}

#[test]
fn task_creation_and_edits_save_offline_in_both_connection_modes() {
    use loopflow::durable::TaskId;

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
        let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let wave = loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        store.create_wave(&wave).unwrap();
        let project = loopflow::linear::LinearProject {
            description: None,
            id: "provider-project".into(),
            revision: Some("2026-10-08T10:00:00Z".into()),
            slug: "product".into(),
            name: "Product".into(),
            summary: "".into(),
            status: loopflow::work::project::ProjectStatus::Started,
            krs: vec![],
            metric_targets: vec![],
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
        };
        loopflow::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &project,
            17,
        )
        .unwrap();
        let project_id = store.list_projects(Some(wave.id())).unwrap()[0].id.clone();
        let conn = rusqlite::Connection::open(&path).unwrap();
        lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "bind-project",
                "product",
                project_id.as_str(),
                "--json",
            ],
        );
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
            let id = TaskId::parse(created["id"].as_str().unwrap()).unwrap();
            let saved = store.task(&id).unwrap().unwrap();
            assert_eq!(created, serde_json::to_value(saved).unwrap());
            assert_eq!(created["title"], "Independent same title");
            assert_eq!(created["description"], "Original notes");
            assert_eq!(created["project_id"], project_id.as_str());
            assert!(created.get("name").is_none());
            assert!(created["worktree"].is_null());
            created
        };
        let first = TaskId::parse(create()["id"].as_str().unwrap()).unwrap();
        let second = TaskId::parse(create()["id"].as_str().unwrap()).unwrap();
        assert_ne!(first, second);
        assert_eq!(store.list_tasks(None).unwrap().len(), 2);
        let mut snapshot = crate::test_linear_input::Inventory {
            projects: vec![project.clone()],
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
                    provider: "linear".into(),
                    initiative: "initiative".into(),
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
            String::from_utf8_lossy(&output.stderr).contains("pending planning sync"),
            connected
        );
        let pending = store.pending_task_changes(&second).unwrap();
        assert_eq!(pending.len(), 3);
        assert!(store.pending_task_changes(&first).unwrap().is_empty());
        assert!(store
            .project_order_delivery(
                store
                    .task(&second)
                    .unwrap()
                    .unwrap()
                    .project_id
                    .as_ref()
                    .unwrap()
            )
            .unwrap()
            .is_some());
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
                &loopflow::work::task::TaskUpdate {
                    title: Some("Stale".into()),
                    ..Default::default()
                }
            )
            .is_err());
        let status = lf(
            repo.path(),
            home.path(),
            &["task", "status", selector, "--json"],
        );
        assert_eq!(status["task"]["title"], "Saved offline");
        assert_eq!(status["task"]["description"], "");
        assert_eq!(status["task"]["rank"], 0);
        assert_eq!(status["task"]["assignee"], "person-id");
        let sync = store.task_planning_sync(&second).unwrap();
        assert_eq!(sync.connected, connected);
        assert_eq!(status["sync"], serde_json::to_value(&sync).unwrap());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            sync.lines()
                .into_iter()
                .map(|line| format!("{line}\n"))
                .collect::<String>()
        );
        assert_eq!(sync.changes.is_empty(), !connected);
        if connected {
            assert!(sync.changes.iter().any(|c| c.field == "task_order"));
            assert_eq!(sync.changes.iter().any(|c| c.field == "creation"), !mapped);
            assert!(sync.changes.iter().any(|c| c.field == "task_title"));
        }
        let desktop = lf(
            repo.path(),
            home.path(),
            &["wave", "status", "product", "--json"],
        );
        let task = desktop["tasks"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["task"]["name"] == "Saved offline")
            .unwrap();
        assert_eq!(task["task"]["sync"], status["sync"]);
        assert!(task["reference"]["workspace"].is_null());

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
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            // Unsent title and assignee saves are sent last and win; status
            // follows Linear.
            let sync = store.task_planning_sync(&second).unwrap();
            assert!(sync.changes.iter().all(
                |c| c.state != loopflow::store::planning_sync::PlanningSyncState::AdoptedLinear
            ));
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
                    "SELECT conflict_json FROM task_changes WHERE id=?1",
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
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            assert_eq!(store.pending_task_changes(&second).unwrap(), changes);
        }
        lf(
            repo.path(),
            home.path(),
            &["task", "edit", selector, "--unassign"],
        );
        let reopened = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
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
fn rotation_saves_membership_and_pending_effects_offline() {
    use crate::test_linear_input::InventoryFixture;
    use loopflow::durable::TaskId;
    use loopflow::work::task::NewTask;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            repo.create_file(
                ".lf/config.yaml",
                "planning:\n  provider: linear\n  team: fixture-team\n",
            );
        }
        let path = home.path().join("loopflow.db");
        let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let wave = loopflow::work::wave::Wave::new(
            loopflow::id::WaveId::new(),
            "product".into(),
            canonical.to_string(),
        );
        store.create_wave(&wave).unwrap();
        lf(
            repo.path(),
            home.path(),
            &["wave", "ensure", "product", "--json"],
        );
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        let db = rusqlite::Connection::open(&path).unwrap();
        if mapped {
            db.execute("UPDATE projects SET external_project_id='provider-project',planning_teams='[\"fixture-team\"]',planning_initiatives='[\"initiative\"]' WHERE id=?1", [project.id.as_str()]).unwrap();
        }
        let active = store
            .create_task(&NewTask {
                repository_id: store
                    .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
                    .unwrap(),
                due_date: Some("2026-12-31".into()),
                id: TaskId::new(),
                project_id: Some(project.id.clone()),
                title: "Started work".into(),
                description: "Retain description".into(),
            })
            .unwrap();
        let backlog = store
            .create_task(&NewTask {
                repository_id: store
                    .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
                    .unwrap(),
                due_date: None,
                id: TaskId::new(),
                project_id: Some(project.id.clone()),
                title: "Unreviewed backlog".into(),
                description: "".into(),
            })
            .unwrap();
        db.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',17)", [active.id.as_str()]).unwrap();
        db.execute(
            "INSERT INTO task_ownership(id,task_id,kind,at,ordered_at,written_here) VALUES('own_'||lower(hex(randomblob(16))),?1,'recorded',17,17000,1)",
            [active.id.as_str()],
        )
        .unwrap();
        if mapped {
            db.execute("UPDATE tasks SET external_issue_id='provider-issue',issue_identifier='FIX-1',planning_team_id='fixture-team' WHERE id=?1", [active.id.as_str()]).unwrap();
        }
        let observed_project = loopflow::linear::LinearProject {
            id: "provider-project".into(),
            slug: project.slug.clone(),
            name: project.name.clone(),
            summary: project.summary.clone(),
            description: Some(project.description.clone()),
            status: project.status,
            krs: project.krs.clone(),
            metric_targets: project.metric_targets.clone(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
            revision: None,
        };
        let mut snapshot = crate::test_linear_input::Inventory {
            projects: vec![observed_project.clone()],
            items: vec![observed_issue(
                "provider-issue",
                "provider-project",
                "Started work",
                "Retain description",
            )],
        };
        snapshot.items[0].due_date.clone_from(&active.due_date);
        if mapped {
            store
                .seed_inventory(&InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 20,
                    snapshot: snapshot.clone(),
                })
                .unwrap();
        }
        db.execute("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code','/retained/task',2)", []).unwrap();
        db.execute("INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,?2,'review','{\"index\":1}','11111111-1111-4111-8111-111111111111',7)", rusqlite::params![active.id.as_str(),serde_json::json!({"name":"review","steps":[{"Skill":{"skill":{"name":"review"},"id":null,"interactive":true,"returns":null,"sources":[]}},{"Skill":{"skill":{"name":"implement"},"id":null,"interactive":false,"returns":null,"sources":[]}}]}).to_string()]).unwrap();
        let retained_execution = || {
            [
                ("task_graphs", "task_id", active.id.as_str()),
                ("processes", "id", "11111111-1111-4111-8111-111111111111"),
            ]
            .map(|(table, key, id)| {
                let mut query = db
                    .prepare(&format!("SELECT * FROM {table} WHERE {key}=?1"))
                    .unwrap();
                let columns = query.column_count();
                query
                    .query_row([id], |row| {
                        (0..columns)
                            .map(|column| row.get::<_, rusqlite::types::Value>(column))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
            })
        };
        let execution = retained_execution();
        let successor = loopflow::durable::ProjectId::new();
        let file = home.path().join("rotation.json");
        // Backlog may be newer than started work; rotation does not depend on display order.
        db.execute(
            "UPDATE tasks SET updated_at=updated_at+60 WHERE id=?1",
            [backlog.id.as_str()],
        )
        .unwrap();
        let plan = serde_json::json!({"name":"Next", "waves":[{"wave_id":wave.id(),"successor_id":successor,"create":true,"project_name":"Next", "metric_targets":[],"krs":[{"text":"Preserve work","holds":false}]}]});
        std::fs::write(&file, serde_json::to_vec(&plan).unwrap()).unwrap();
        let args = [
            "wave",
            "new-chapter",
            "product",
            "Next",
            "--plan",
            file.to_str().unwrap(),
            "--json",
        ];
        let preview = lf(
            repo.path(),
            home.path(),
            &[
                "wave",
                "new-chapter",
                "product",
                "Next",
                "--plan",
                file.to_str().unwrap(),
                "--dry-run",
                "--json",
            ],
        );
        let preview_decision = preview["waves"][0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|decision| decision["task"]["id"] == active.id.as_str())
            .unwrap();
        assert_eq!(preview_decision["disposition"], "move");
        let expected = store.task(&active.id).unwrap().unwrap();
        let preview_task = &preview_decision["task"];
        assert_eq!(preview_task, &serde_json::to_value(&expected).unwrap());
        assert_eq!(preview_task["id"], active.id.as_str());
        assert_eq!(preview_task["due_date"], "2026-12-31");
        assert_eq!(preview_task["description"], "Retain description");
        assert_eq!(
            store.task(&active.id).unwrap().unwrap().project_id,
            Some(project.id.clone())
        );
        let revision = || {
            db.query_row(
                "SELECT revision FROM store_revisions WHERE domain='planning'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
        };
        let before = revision();
        db.execute_batch("CREATE TRIGGER fail_rotation BEFORE UPDATE OF current_project_id ON waves BEGIN SELECT RAISE(ABORT,'injected rotation failure'); END;").unwrap();
        assert!(!command(repo.path(), home.path(), &args)
            .output()
            .unwrap()
            .status
            .success());
        db.execute_batch("DROP TRIGGER fail_rotation").unwrap();
        assert!(store.project(&successor).unwrap().is_none());
        assert!(store.pending_task_changes(&active.id).unwrap().is_empty());
        assert!(store
            .pending_project_changes(&project.id)
            .unwrap()
            .is_empty());
        assert_eq!(revision(), before);
        let applied = lf(repo.path(), home.path(), &args);
        let moved = store.task(&active.id).unwrap().unwrap();
        let applied_decision = applied["waves"][0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|decision| decision["task"]["id"] == active.id.as_str())
            .unwrap();
        assert_eq!(
            applied_decision["task"],
            serde_json::to_value(&moved).unwrap()
        );
        assert_eq!(moved.project_id, Some(successor.clone()));
        assert_eq!(
            db.query_row(
                "SELECT max(at) FROM task_ownership WHERE task_id=?1",
                [active.id.as_str()],
                |r| r.get::<_, Option<i64>>(0)
            )
            .unwrap(),
            Some(17)
        );
        assert_eq!(
            store.task(&backlog.id).unwrap().unwrap().project_id,
            Some(project.id.clone())
        );
        let changes = store.pending_task_changes(&active.id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].value.field(), "task_project");
        assert_eq!(
            changes[0].value,
            WorkValue::TaskProject(Some(successor.to_string()))
        );
        let status = store.pending_project_changes(&project.id).unwrap();
        assert_eq!(
            status[0].value,
            WorkValue::ProjectStatus(ProjectStatus::Completed)
        );
        // Released receipts nest content and preserve their original JSON bytes/order.
        let retained = format!(
            r#"{{"wave_id":"{}","successor_id":"{}","create":true,"project_name":"Next","content":{{"metric_targets":[],"krs":[{{"text":"Preserve work","holds":false}}]}}}}"#,
            wave.id(),
            successor,
        );
        db.execute("UPDATE project_transitions SET local_plan_json=?1 WHERE wave_id=?2 AND successor_id=?3",
            rusqlite::params![retained, wave.id(), successor.as_str()]).unwrap();
        let settled_revision = revision();
        lf(repo.path(), home.path(), &args);
        let saved: String = db.query_row("SELECT local_plan_json FROM project_transitions WHERE wave_id=?1 AND successor_id=?2",
            rusqlite::params![wave.id(), successor.as_str()], |row| row.get(0)).unwrap();
        assert_eq!(saved, retained);
        let mut conflicting = plan.clone();
        conflicting["waves"][0]["krs"][0]["text"] = serde_json::json!("Changed request");
        std::fs::write(&file, serde_json::to_vec(&conflicting).unwrap()).unwrap();
        let rejected = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(!rejected.status.success());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("original rotation request changed")
        );
        std::fs::write(&file, serde_json::to_vec(&plan).unwrap()).unwrap();
        assert_eq!(revision(), settled_revision);
        assert_eq!(store.pending_task_changes(&active.id).unwrap(), changes);
        assert_eq!(store.pending_project_changes(&project.id).unwrap(), status);
        if mapped {
            snapshot.items[0].description = "Incoming unrelated edit".into();
            snapshot.items[0].revision = Some("2026-10-08T12:00:00Z".into());
            store
                .seed_inventory(&InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 21,
                    snapshot,
                })
                .unwrap();
            let retained = store.task(&active.id).unwrap().unwrap();
            assert_eq!(retained.project_id, Some(successor.clone()));
            assert_eq!(retained.description, "Incoming unrelated edit");
            assert_eq!(store.pending_task_changes(&active.id).unwrap(), changes);
            assert_eq!(
                store.project(&project.id).unwrap().unwrap().status,
                loopflow::work::project::ProjectStatus::Completed
            );
        }
        let reopened = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        assert_eq!(
            reopened.task(&active.id).unwrap().unwrap().project_id,
            Some(successor.clone())
        );
        assert_eq!(reopened.pending_task_changes(&active.id).unwrap(), changes);
        if mapped {
            db.execute(
                "UPDATE projects SET external_project_id='exported-successor' WHERE id=?1",
                [successor.as_str()],
            )
            .unwrap();
            let mut echo: crate::test_linear_input::IssueFixture =
                loopflow_test_support::retained_task_input(
                    &home.path().join("loopflow.db"),
                    &canonical.to_string(),
                    "provider-issue",
                );
            let successor_plan = store.project(&successor).unwrap().unwrap();
            let project = echo.project.as_mut().unwrap();
            project.id = "exported-successor".into();
            project.name = successor_plan.name;
            project.slug = successor_plan.slug;
            project.status = successor_plan.status;
            project.summary = successor_plan.summary;
            project.description = Some(successor_plan.description);
            project.krs = successor_plan.krs;
            project.metric_targets = successor_plan.metric_targets;
            echo.item.project_id = Some(project.id.clone());
            echo.item.revision = Some("2026-10-08T12:30:00Z".into());
            echo.observed_at = 22;
            store
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &echo,
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            assert_eq!(store.pending_task_changes(&active.id).unwrap(), changes);
            assert_eq!(
                store.task(&active.id).unwrap().unwrap().project_id,
                Some(successor.clone())
            );
        }
        // Select an already stored destination using its provider alias when present.
        let mut existing = project.clone();
        existing.id = loopflow::durable::ProjectId::new();
        existing.name = "Future".into();
        existing.slug = "future".into();
        existing.status = loopflow::work::project::ProjectStatus::Planned;
        existing.linear_id =
            mapped.then(|| loopflow::linear::LinearProjectId::new("future-provider").unwrap());
        store.insert_project(&existing).unwrap();
        let mut next = plan.clone();
        next["waves"][0]["successor_id"] = serde_json::json!(existing
            .linear_id
            .as_ref()
            .map(|id| id.as_str())
            .unwrap_or(existing.id.as_str()));
        next["waves"][0]["create"] = serde_json::json!(false);
        std::fs::write(&file, serde_json::to_vec(&next).unwrap()).unwrap();
        lf(repo.path(), home.path(), &args);
        assert_eq!(
            store.task(&active.id).unwrap().unwrap().project_id,
            Some(existing.id.clone())
        );
        assert_eq!(store.project(&existing.id).unwrap().unwrap().name, "Next");
        let pending = store.pending_project_changes(&existing.id).unwrap();
        assert!(pending
            .iter()
            .any(|c| c.value == WorkValue::ProjectStatus(ProjectStatus::Started)));
        assert!(pending.iter().any(|c| c.value.field() == "krs"));
        lf(repo.path(), home.path(), &args);
        assert_eq!(
            store.pending_project_changes(&existing.id).unwrap(),
            pending
        );
        if mapped {
            let mut remote: crate::test_linear_input::IssueFixture =
                loopflow_test_support::retained_task_input(
                    &home.path().join("loopflow.db"),
                    &canonical.to_string(),
                    "provider-issue",
                );
            let project = remote.project.as_mut().unwrap();
            project.id = "third-provider-project".into();
            project.name = "Another plan".into();
            project.slug = "another-plan".into();
            project.initiative_ids = vec!["initiative".into()];
            project.team_ids = vec!["fixture-team".into()];
            remote.item.project_id = Some(project.id.clone());
            remote.item.revision = Some("2026-10-08T13:00:00Z".into());
            remote.observed_at = 22;
            let moved = store.task(&active.id).unwrap().unwrap().project_id;
            store
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &remote,
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            // The unsent move is sent after Linear's and wins.
            assert_eq!(store.task(&active.id).unwrap().unwrap().project_id, moved);
            assert!(!store.pending_task_changes(&active.id).unwrap().is_empty());
            assert_eq!(
                store.task_acquisition(&active.id).unwrap().state,
                loopflow::store::PlanningState::Available
            );
        }
        assert_eq!(retained_execution(), execution);
        assert!(repo.path().join(".lf/wave").exists());
    }
}

#[test]
fn moving_saves_membership_offline_and_retains_inbound_changes() {
    use loopflow::durable::TaskId;
    use loopflow::work::task::NewTask;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            repo.create_file(
                ".lf/config.yaml",
                "planning:\n  provider: linear\n  team: fixture-team\n",
            );
        }
        let path = home.path().join("loopflow.db");
        let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        let mut projects = Vec::new();
        let mut observations = Vec::new();
        for name in ["source", "destination", "competing"] {
            let wave = loopflow::work::wave::Wave::new(
                loopflow::id::WaveId::new(),
                name.into(),
                canonical.to_string(),
            );
            store.create_wave(&wave).unwrap();
            lf(
                repo.path(),
                home.path(),
                &["wave", "ensure", name, "--json"],
            );
            let mut project = store.list_projects(Some(wave.id())).unwrap().remove(0);
            if mapped {
                // Bind through the writer so foreground Git exchange sees the same
                // identity as SQLite, rather than restoring a pre-binding journal.
                project.linear_id = Some(loopflow::linear::LinearProjectId::new(name).unwrap());
                store.insert_project(&project).unwrap();
                let observed = loopflow::linear::LinearProject {
                    id: name.into(),
                    slug: project.slug.clone(),
                    name: project.name.clone(),
                    summary: project.summary.clone(),
                    description: Some(project.description.clone()),
                    status: project.status,
                    krs: project.krs.clone(),
                    metric_targets: project.metric_targets.clone(),
                    initiative_ids: vec![name.into()],
                    team_ids: vec!["fixture-team".into()],
                    revision: None,
                };
                loopflow::linear::acceptance::accept_linear_project_in(
                    &store,
                    wave.id(),
                    name,
                    &observed,
                    1,
                )
                .unwrap();
                observations.push(observed);
            }
            projects.push(project);
        }
        let task = store
            .create_task(&NewTask {
                repository_id: store
                    .ensure_repository(repo.path().canonicalize().unwrap().to_str().unwrap())
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
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &original,
                    Some((&projects[0].wave_id, "source")),
                )
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
        db.execute_batch("CREATE TRIGGER fail_refile BEFORE INSERT ON task_changes BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;").unwrap();
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
            String::from_utf8_lossy(&output.stderr).contains("pending planning sync"),
            connected
        );
        let moved = store.task(&task.id).unwrap().unwrap();
        assert_eq!(moved.project_id, Some(projects[1].id.clone()));
        assert_eq!(moved.wave_id, Some(projects[1].wave_id.clone()));
        assert_eq!(moved.revision, task.revision + 1);
        assert!(moved.worktree.is_none());
        let changes = store.pending_task_changes(&task.id).unwrap();
        assert_eq!(changes.len(), 1);
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
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &incoming,
                    Some((&projects[0].wave_id, "source")),
                )
                .unwrap();
            let retained = store.task(&task.id).unwrap().unwrap();
            assert_eq!(retained.project_id, Some(projects[1].id.clone()));
            assert_eq!(retained.description, "Incoming after refiling");
            assert_eq!(store.pending_task_changes(&task.id).unwrap(), changes);
            let mut other = incoming.project.clone().unwrap();
            // Preserve the duplicate slug: exact provider IDs must beat names.
            other.id = "competing".into();
            other.revision = Some("2026-10-08T14:01:00Z".into());
            other.initiative_ids = vec!["competing".into()];
            incoming.project = Some(other);
            incoming.item.project_id = Some("competing".into());
            incoming.observed_at += 1;
            incoming.item.revision = Some("2026-10-08T14:01:00Z".into());
            store
                .seed_issue(
                    &canonical.to_string(),
                    "linear",
                    &incoming,
                    Some((&projects[2].wave_id, "competing")),
                )
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
                store.task_acquisition(&task.id).unwrap().state,
                loopflow::store::PlanningState::Available
            );
        }
        db.execute(
            "UPDATE projects SET created_at=created_at+10 WHERE id=?1",
            [projects[2].id.as_str()],
        )
        .unwrap();
        let reopened = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
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
            "INSERT INTO task_ownership(id,task_id,kind,at,ordered_at,written_here) VALUES('own_'||lower(hex(randomblob(16))),?1,'recorded',17,17000,1)",
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
        let wave_id = loopflow::id::WaveId::new();
        let root = repo.path().join(".lf/wave/tools");
        std::fs::create_dir_all(root.join("parser")).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        if connected {
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "planning:\n  provider: linear\n  team: fixture-team\n",
            )
            .unwrap();
        }
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
            &["wave", "ensure", "tools/parser", "--json"],
        );
        assert!(project["id"].is_string());
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
        );
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
            &["wave", "rename", "tools", "--name", "instruments", "--json"],
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
        );
        let id = task["id"].as_str().unwrap();
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let task_id = loopflow::durable::TaskId::parse(id).unwrap();
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if connected {
            db.execute("UPDATE tasks SET external_issue_id='retained-issue',issue_identifier='FIX-1' WHERE id=?1", [id]).unwrap();
        }
        let before = store.task(&task_id).unwrap().unwrap();
        let work = loopflow::durable::WorkRef::Task(task_id.clone());
        let execution = store.work_status(&work).unwrap();
        db.execute_batch("CREATE TRIGGER fail_removal BEFORE INSERT ON task_changes WHEN NEW.field='deleted' BEGIN SELECT RAISE(ABORT,'receipt failed'); END;").unwrap();
        assert!(!command(repo.path(), home.path(), &["task", "delete", id])
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(store.task(&task_id).unwrap().unwrap(), before);
        assert!(store.pending_task_changes(&task_id).unwrap().is_empty());
        db.execute_batch("DROP TRIGGER fail_removal;").unwrap();
        let output = lf(repo.path(), home.path(), &["task", "delete", id]);
        if connected {
            assert!(output.as_str().unwrap().contains("Pending Linear sync"));
        }
        let changes = store.pending_task_deletion(&task_id).unwrap();
        assert!(changes.is_some());
        lf(repo.path(), home.path(), &["task", "delete", id]);
        assert_eq!(store.pending_task_deletion(&task_id).unwrap(), changes);
        let removed = store.task(&task_id).unwrap().unwrap();
        assert_eq!(store.work_status(&work).unwrap(), execution);
        assert_eq!(removed.worktree, before.worktree);
        assert_eq!(removed.linear_id, before.linear_id);
        assert_eq!(removed.revision, before.revision + 1);
        let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
        assert_eq!(status["planning_state"], "removed");
        assert!(store.list_tasks(None).unwrap().is_empty());
        drop(store);
        let reopened =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        assert_eq!(reopened.pending_task_deletion(&task_id).unwrap(), changes);
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
            )
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
            let status = lf(repo, home.path(), &["task", "status", selector, "--json"]);
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
    let status = lf(
        first.path(),
        home.path(),
        &["task", "status", foreign, "--json"],
    );
    assert!(status["task"].is_null());
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
        &["task", "status", "DUP-1", "--json"],
    );
    assert_eq!(status["task"]["title"], "Original");
    let status = lf(
        first.path(),
        home.path(),
        &["task", "status", "DUP-1", "--json"],
    );
    assert_eq!(status["task"]["title"], "First only");
    let missing = lf(
        first.path(),
        home.path(),
        &["task", "status", "MISSING-1", "--json"],
    );
    assert!(missing["task"].is_null());
    assert_eq!(missing["planning_state"], "unavailable");
    assert!(missing["planning_error"].is_null());
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
        &["task", "status", identifier, "--json"],
    );
    assert_eq!(status["task"]["identifier"], identifier);
    assert_eq!(status["task"]["title"], "Saved task");
    let id = status["task"]["id"].as_str().unwrap();
    loopflow::durable::TaskId::parse(id).unwrap();
    assert_eq!(status["execution"]["task_id"], id);
    assert!(status["execution"]["worktree"].is_null());
    assert!(status["execution"]["pr"].is_null());
}

#[test]
fn planning_sync_tracks_creation_errors_uncertainty_conflicts_and_settlement() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\n  team: fixture-team\n",
    )
    .unwrap();
    lf(
        repo.path(),
        home.path(),
        &["wave", "ensure", "inbox", "--json"],
    );
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Saved plan",
            "--json",
        ],
    );
    let id = created["id"].as_str().unwrap();
    let project = created["project_id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["task", "edit", id, "--title", "Later title"],
    );
    let comments = lf(
        repo.path(),
        home.path(),
        &["task", "comment", id, "Local comment", "--json"],
    );
    assert_eq!(comments["pending_sync"].as_array().unwrap().len(), 1);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let status = || lf(repo.path(), home.path(), &["task", "status", id, "--json"])["sync"].clone();
    let initial = status();
    for field in ["creation", "task_title", "comment"] {
        assert!(initial["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["field"] == field && c["state"] == "pending"));
    }
    let read_project = || {
        let snapshot = lf(
            repo.path(),
            home.path(),
            &["wave", "status", "inbox", "--json"],
        );
        snapshot["projects"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == project)
            .unwrap()["sync"]
            .clone()
    };
    assert!(read_project()["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["field"] == "creation"));
    let revision = || {
        conn.query_row(
            "SELECT revision FROM store_revisions WHERE domain='planning'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    let before = revision();
    conn.execute("UPDATE task_creation_intents SET export_attempted=1,export_error='Lost creation reply' WHERE task_id=?1", [id]).unwrap();
    assert!(revision() > before);
    conn.execute("UPDATE project_transitions SET export_attempted=1,export_link_attempted=1,export_error='Lost attachment reply' WHERE successor_id=?1", [project]).unwrap();
    conn.execute(
        "UPDATE task_changes SET attempted=1,error='Lost field reply' WHERE task_id=?1",
        [id],
    )
    .unwrap();
    conn.execute(
        "UPDATE task_comment_deliveries SET error='Lost comment reply' WHERE comment_id=?1",
        [comments["comments"][0]["id"].as_str().unwrap()],
    )
    .unwrap();
    let uncertain = status();
    assert!(uncertain["changes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| (c["state"] == "uncertain" || c["field"] == "comment") && c["error"].is_string()));
    // Planning reads retry delivery and report the current missing mapping,
    // while retaining the uncertain creation and attachment attempts.
    let project_sync = read_project();
    assert_eq!(project_sync["changes"][0]["state"], "uncertain");
    assert_eq!(
        project_sync["changes"][0]["error"],
        "Wave has no Linear Initiative; export remains pending"
    );
    // Model provider confirmation and an observed losing field receipt; projection is read-only.
    conn.execute(
        "UPDATE tasks SET external_issue_id='fixture-issue' WHERE id=?1",
        [id],
    )
    .unwrap();
    conn.execute(
        "UPDATE projects SET external_project_id='fixture-project' WHERE id=?1",
        [project],
    )
    .unwrap();
    conn.execute(r#"UPDATE task_changes SET conflict_json='{"revision":null,"value":{"field":"task_title","value":"Linear title"}}',error=NULL WHERE task_id=?1"#, [id]).unwrap();
    conn.execute(
        "UPDATE task_comment_deliveries SET acknowledged=1,error=NULL",
        [],
    )
    .unwrap();
    let settled = status();
    assert_eq!(settled["changes"].as_array().unwrap().len(), 1);
    assert_eq!(settled["changes"][0]["state"], "adopted_linear");
    assert_eq!(settled["changes"][0]["local_value"], "Later title");
    assert_eq!(settled["changes"][0]["linear_value"], "Linear title");
    assert!(read_project()["changes"].as_array().unwrap().is_empty());
    std::fs::write(repo.path().join(".lf/config.yaml"), "{}\n").unwrap();
    conn.execute(
        "UPDATE task_comment_deliveries SET acknowledged=0,error='Offline'",
        [],
    )
    .unwrap();
    let disconnected = status();
    assert_eq!(disconnected["connected"], false);
    assert_eq!(disconnected["changes"], settled["changes"]);
    assert!(
        lf(repo.path(), home.path(), &["task", "comment", id, "--json"])["pending_sync"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn wave_planning_reads_current_and_historical_chapters_without_starting_work() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "inbox",
            "--title",
            "Unstarted proof",
            "--json",
        ],
    );
    let task_id = loopflow::durable::TaskId::parse(created["id"].as_str().unwrap()).unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let task = store.task(&task_id).unwrap().unwrap();
    let current = store
        .project(task.project_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    let mut historical = current.clone();
    historical.id = loopflow::durable::ProjectId::new();
    historical.slug = "earlier".into();
    historical.name = "Earlier chapter".into();
    historical.status = loopflow::work::project::ProjectStatus::Completed;
    historical.krs = vec![loopflow::work::project::ProjectKR {
        text: "Historical proof remains".into(),
        holds: true,
    }];
    store.insert_project(&historical).unwrap();
    let wave = store.get_wave(&current.wave_id).unwrap().unwrap();
    let plans_before = store.list_projects(Some(wave.id())).unwrap();
    let tasks_before = store.list_tasks(Some(&current.wave_id)).unwrap();
    let database = home.path().join("loopflow.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    let before = std::fs::read(&database).unwrap();
    let shown = lf(
        repo.path(),
        home.path(),
        &["wave", "show", wave.slug(), "--json"],
    );
    let plans = shown["waves"][0]["projects"]["items"].as_array().unwrap();
    assert_eq!(plans.len(), 2);
    assert!(plans
        .iter()
        .any(|p| p["id"] == current.id.as_str() && p["current"] == true));
    let old = plans
        .iter()
        .find(|p| p["id"] == historical.id.as_str())
        .unwrap();
    assert_eq!(old["current"], false);
    assert!(old.get("graph").is_none());
    assert_eq!(old["krs"][0]["text"], "Historical proof remains");
    let text = lf(repo.path(), home.path(), &["wave", "show", wave.slug()]);
    let text = text.as_str().unwrap();
    assert!(text.contains("Earlier chapter"));
    assert!(text.contains("Historical proof remains"));
    assert!(text.contains("Unstarted proof"));
    let selected = lf(
        repo.path(),
        home.path(),
        &["wave", "graph", "show", wave.slug(), "--json"],
    );
    assert_eq!(selected["wave_id"], wave.id().as_str());
    let scoped = lf(
        repo.path(),
        home.path(),
        &[
            "--task",
            task_id.as_str(),
            "wave",
            "graph",
            "show",
            "--json",
        ],
    );
    assert_eq!(scoped["wave_id"], wave.id().as_str());
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    assert_eq!(
        std::fs::read(&database).unwrap(),
        before,
        "planning reads changed the registry"
    );
    assert_eq!(store.list_projects(Some(wave.id())).unwrap(), plans_before);
    assert_eq!(
        store.list_tasks(Some(&current.wave_id)).unwrap(),
        tasks_before
    );
    assert_eq!(store.task(&task_id).unwrap().unwrap(), task);
    // Wave selection uses the same writer; only future take-up changes.
    lf(
        repo.path(),
        home.path(),
        &["--wave", wave.slug(), "set", "graph", "code"],
    );
    let selected = lf(
        repo.path(),
        home.path(),
        &["wave", "graph", "show", wave.slug(), "--json"],
    );
    assert_eq!(selected["graph"], "code");
    assert_eq!(store.task(&task_id).unwrap().unwrap(), task);
    assert_eq!(store.project(&historical.id).unwrap().unwrap(), historical);
    lf(
        repo.path(),
        home.path(),
        &[
            "wave",
            "edit-plan",
            wave.slug(),
            "--name",
            "Current chapter",
        ],
    );
    lf(
        repo.path(),
        home.path(),
        &[
            "wave",
            "edit-plan",
            "--project",
            historical.id.as_str(),
            "--summary",
            "Retained proof",
        ],
    );
    assert_eq!(
        store.project(&current.id).unwrap().unwrap().name,
        "Current chapter"
    );
    assert_eq!(
        store.project(&historical.id).unwrap().unwrap().name,
        historical.name
    );
    assert_eq!(
        store.project(&historical.id).unwrap().unwrap().summary,
        "Retained proof"
    );
    assert_eq!(store.task(&task_id).unwrap().unwrap(), task);
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
fn wave_files_are_checkout_local_and_edits_reach_status_without_ensure() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let checkout = repo.create_named_worktree("wave-documents");
    lf(
        &checkout,
        home.path(),
        &["wave", "ensure", "local", "--json"],
    );
    let goal = checkout.join(".lf/wave/local/GOAL.md");
    let memory = checkout.join(".lf/wave/local/MEMORY.md");
    assert!(goal.is_file());
    assert!(memory.is_file());
    assert!(
        !repo.path().join(".lf/wave/local").exists(),
        "creation must not write main"
    );
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
        main["wave"]["goal"], "",
        "no other checkout or stored fallback"
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
    assert!(String::from_utf8_lossy(&removed.stderr).contains("--memory"));
    assert_eq!(
        std::fs::read_to_string(memory).unwrap(),
        "A freshly edited decision."
    );
}

#[test]
fn canonical_status_retains_content_and_execution_across_acquisition_states() {
    use crate::test_linear_input::IssueFixture;
    use loopflow::durable::TaskId;
    use serde_json::json;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "inbox", "--title", "Original", "--json",
        ],
    );
    let id = created["id"].as_str().unwrap();
    let id = TaskId::parse(id).unwrap();
    let path = home.path().join("loopflow.db");
    let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
    let task = store.task(&id).unwrap().unwrap();
    let mut project = store
        .project(task.project_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    project.linear_id = Some(loopflow::linear::LinearProjectId::new("provider-project").unwrap());
    store.insert_project(&project).unwrap();
    let observed: loopflow::linear::LinearProject = serde_json::from_value(json!({
        "id":"provider-project", "slug":project.slug, "name":project.name,
        "summary":"", "description":"Saved Project prose", "status":"started",
        "krs":[], "metric_targets":[], "initiative_ids":["initiative"], "team_ids":["team"], "revision":null
    })).unwrap();
    loopflow::linear::acceptance::accept_linear_project_in(
        &store,
        task.wave_id.as_ref().unwrap(),
        "initiative",
        &observed,
        17,
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
    let scope = loopflow::repository::CanonicalRepo::discover(repo.path())
        .unwrap()
        .to_string();
    store
        .seed_issue(
            &scope,
            "linear",
            &record,
            Some((task.wave_id.as_ref().unwrap(), "initiative")),
        )
        .unwrap();
    let task = store.task(&id).unwrap().unwrap();
    store
        .edit_task(
            &id,
            task.revision,
            &loopflow::work::task::TaskUpdate {
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
            &["task", "status", id.as_str(), "--json"],
        )
    };
    let initial = read();
    for (state, statement) in [
        ("available", "SELECT 1"),
        ("invalid", "UPDATE pm_items SET needs_refresh=1"),
        ("available", "UPDATE pm_items SET needs_refresh=0"),
        (
            "invalid",
            "UPDATE pm_items SET project_id='another-project',claimed_project_id='another-project'",
        ),
    ] {
        db.execute_batch(statement).unwrap();
        let status = read();
        assert_eq!(status["planning_state"], state);
        assert_eq!(status["observed_at"], 17);
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
            &["wave", "status", "inbox", "--json"],
        );
        let summary = &wave["tasks"]["items"][0]["task"];
        assert_eq!(
            summary["acquisition"],
            json!({"state":state,"observed_at":17})
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
    db.execute("INSERT INTO task_changes(id,task_id,value_json,base_json,attempted) VALUES('pending-membership',?1,?2,NULL,0)", rusqlite::params![id.as_str(),serde_json::to_string(&WorkValue::TaskProject(saved.project_id.as_ref().map(ToString::to_string))).unwrap()]).unwrap();
    assert_eq!(read()["planning_state"], "available");
    store
        .observe_pm_issue_change("provider-task", None, true)
        .unwrap();
    let removed = read();
    assert_eq!(removed["planning_state"], "removed");
    assert_eq!(removed["observed_at"], 17);
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
) -> loopflow::linear::LinearIssue {
    loopflow::linear::LinearIssue {
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
    );
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
    );
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

/// The Task's graph as `lf task status --json` carries it.
fn task_graph(repo: &Path, home: &Path, id: &str) -> Value {
    lf(repo, home, &["task", "status", id, "--json"])["execution"]["work"]["graph"].clone()
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
    );
    let id = created["id"].as_str().unwrap();
    assert!(created["repository_id"].is_string());
    assert!(created["project_id"].is_null());
    assert!(created["wave_id"].is_null());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let counts: (u32, u32, u32) = db.query_row("SELECT (SELECT count(*) FROM tasks),(SELECT count(*) FROM waves),(SELECT count(*) FROM projects)", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
    assert_eq!(counts, (1, 0, 0));
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(listed[0]["task"]["id"], id);
    assert_eq!(listed[0]["graph_name"], "proof");
    assert_eq!(listed[0]["graph_source"], "repo");
    let inventory = lf(repo.path(), home.path(), &["wave", "show", "--json"]);
    assert_eq!(inventory["tasks"]["items"][0]["task"]["id"], id);
    let placed = lf(repo.path(), home.path(), &["checkout", id, "--json"]);
    let before = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(before["position"]["step"], "review");
    lf(
        repo.path(),
        home.path(),
        &["wave", "ensure", "product", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["task", "move", id, "--wave", "product"],
    );
    let attached = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(attached["execution"]["worktree"], placed["worktree"]);
    assert_eq!(attached["execution"]["wave"], "product");
    assert_eq!(task_graph(repo.path(), home.path(), id), before);
    lf(repo.path(), home.path(), &["task", "move", id, "--no-wave"]);
    lf(repo.path(), home.path(), &["task", "move", id, "--no-wave"]);
    let detached = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(detached[0]["task"]["id"], id);
    assert!(detached[0]["task"]["wave_id"].is_null());
    assert_eq!(detached[0]["graph_source"], "task");
    lf(repo.path(), home.path(), &["task", "complete", id]);
    assert_eq!(
        lf(repo.path(), home.path(), &["task", "status", id, "--json"])["execution"]["status"],
        "done"
    );

    let standalone = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Always standalone", "--json"],
    );
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
        &["task", "status", standalone_id, "--json"],
    );
    assert_eq!(completed["execution"]["status"], "done");
    assert!(completed["execution"]["wave"].is_null());
    assert!(completed["execution"]["project_id"].is_null());
}

#[test]
fn unconfigured_repository_runs_tasks_on_the_builtin_code_graph() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    // Stand in for pursue's agent steps, keeping the `implement` that code's
    // loops return to; the Task's graph stays the builtin.
    repo.create_file(".lf/graphs/pursue.yaml", "- graph: implement\n");
    repo.create_file(".lf/graphs/implement.yaml", "- cmd: graph show pursue\n");
    repo.stage_all();
    repo.commit("Stand in for pursue");
    repo.push();
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "First Task", "--json"],
    );
    let id = created["id"].as_str().unwrap();
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    assert_eq!(listed[0]["graph_name"], "code");
    assert_eq!(listed[0]["graph_source"], "default");
    let shown = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(shown["name"], "code");
    assert_eq!(shown["position"]["step"], "demo");
}

#[test]
fn auto_graph_runs_a_task_to_end() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(".lf/config.yaml", "graph: auto\n");
    // Stand in for the agent and delivery steps; the graph stays the builtin.
    repo.create_file(".lf/graphs/pursue.yaml", "- cmd: graph show pursue\n");
    repo.create_file(".lf/graphs/deliver.yaml", "- cmd: graph show deliver\n");
    repo.stage_all();
    repo.commit("Run Tasks on auto");
    repo.push();
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Auto Task", "--json"],
    );
    let id = created["id"].as_str().unwrap();
    let shown = run_until_waiting(repo.path(), home.path(), &["task", "run", id], id);
    assert_eq!(shown["name"], "auto");
    assert_eq!(shown["position"]["step"], "end");
}

#[test]
fn your_graph_for_a_wave_selects_the_builtin_auto() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    lf(
        repo.path(),
        home.path(),
        &["wave", "ensure", "product", "--json"],
    );
    lf(
        repo.path(),
        home.path(),
        &["--wave", "product", "set", "graph", "auto"],
    );
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "product", "--title", "Auto", "--json",
        ],
    );
    let listed = lf(repo.path(), home.path(), &["task", "list", "--json"]);
    let task = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["task"]["id"] == created["id"])
        .unwrap();
    assert_eq!(task["graph_name"], "auto");
    assert_eq!(task["graph_source"], "personal");
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
        &["wave", "ensure", "product", "--json"],
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
    );
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
            "Invalid default",
            "--json",
        ],
    );
    let standalone_id = standalone["id"].as_str().unwrap();
    let refused = command(repo.path(), home.path(), &["task", "run", standalone_id])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("invalid graph name"));
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
    );
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
        &["wave", "ensure", "product", "--json"],
    );
    let wave_id = wave["wave_id"].as_str().unwrap();
    let parent = lf(
        repo.path(),
        home.path(),
        &[
            "task", "create", "--wave", "product", "--title", "Parent", "--json",
        ],
    );
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
            assert_eq!(task["repository_id"], parent["repository_id"]);
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
    let store = crate::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let retained = store
        .task(&crate::durable::TaskId::parse(parent_id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retained.wave_id.as_ref().unwrap().as_str(), wave_id);
    assert_eq!(retained.worktree.as_deref(), Some(checkout));
}
