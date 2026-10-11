use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow_test_support::TestRepo;
use serde_json::Value;

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
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
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
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
    std::fs::write(&gh, include_str!("../../../tests/fixtures/local_gh.sh")).unwrap();
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
    assert!(worktree.exists(), "retain checkout until follow-through");
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "follow-up",
            id,
            "--none",
            "No accepted remaining work",
        ],
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
    let args = ["task", "create", "--title", "Concurrent work", "--json"];
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
        entries.push(serde_json::json!({"wave_id":wave["wave"]["id"],"successor_id":successor,"create":true,"project_name":"Next","workflow":"","metric_targets":[],"krs":[{"text":"Keep private work","holds":false}]}));
    }
    let path = home.path().join("rotation.json");
    let args = [
        "repo",
        "new-chapter",
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
    assert!(repo.path().join("wave").exists());
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
    let roadmap = lf(repo.path(), home.path(), &["roadmap", "--json"]);
    let mut task = roadmap["waves"][0]["tasks"]["items"][0].clone();
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
    let proof = serde_json::json!({"task":task,"comments":comments});
    let canonical = serde_json::to_string(&proof)
        .unwrap()
        .replace(id, "task_0123456789ab40008000000000000001")
        .replace(
            &format!("lf-{}", &id.trim_start_matches("task_")[..7]),
            "lf-0123456",
        )
        .replace(&comment_id, "00000000-0000-4000-8000-000000000001")
        .replace(&comment_date, "2026-10-08T00:00:00Z");
    let actual: Value = serde_json::from_str(&canonical).unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/dto/local_task.json")).unwrap();
    assert_eq!(actual, expected, "actual wire: {actual}");
    serde_json::from_value::<loopflow::lf::commands::waves::RoadmapTask>(actual["task"].clone())
        .unwrap();
    serde_json::from_value::<loopflow::ops::pm::TaskComments>(actual["comments"].clone()).unwrap();
}

#[test]
fn stored_fields_preserve_order_assignment_and_project_summary() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let fields: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/planning_fields.json"
    ))
    .unwrap();
    lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "First", "--json"],
    );
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Second", "--json"],
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
            "project",
            "edit",
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
        &[
            "task",
            "refile",
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
    assert!(repo.path().join("wave").exists());
    assert!(!repo.path().join(".lf/config.yaml").exists());
    assert!(!repo.path().join(".lf/workflows").exists());
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
                due_date: None,
                id: loopflow::durable::TaskId::parse(id).unwrap(),
                project_id: loopflow::durable::ProjectId::parse(&project).unwrap(),
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
        assert_eq!(status["task"]["identifier"], "lf-abcd1234");
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
    let goal = repo.path().join("wave/inbox/GOAL.md");
    let memory = repo.path().join("wave/inbox/MEMORY.md");
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
        .find(|entry| entry["source"] == "wave/inbox/MEMORY.md")
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
    std::fs::write(&plan_file,r#"{"workflow":"","metric_targets":[],"krs":[{"text":"Quoted input survives","holds":false}]}"#).unwrap();
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
    let project_id = status["project"]["id"].as_str().unwrap();
    let project = lf(
        repo.path(),
        home.path(),
        &["project", "workflow", "show", project_id, "--json"],
    );
    assert_eq!(project["krs"][0]["text"], "Quoted input survives");
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
            "move",
            identity.as_str(),
            "end",
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
    assert!(repo.path().join("wave").exists());
    assert!(!repo.path().join(".lf/config.yaml").exists());
    assert!(!repo.path().join(".lf/workflows").exists());
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
    repo.create_file(".lf/flows/proof.yaml", "- cmd: flow show proof\n");
    let definition = home.path().join("proof.yaml");
    std::fs::write(&definition, "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: end, flow: proof}\n").unwrap();
    repo.stage_all();
    repo.commit("Fixture workflow");
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["remote", "remove", "origin"])
        .status()
        .unwrap()
        .success());
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Local execution", "--json"],
    );
    let id = created["id"].as_str().unwrap();
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    let project_id = status["project"]["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "project",
            "workflow",
            "set",
            project_id,
            "proof",
            "--file",
            definition.to_str().unwrap(),
        ],
    );
    let catalog = lf(
        repo.path(),
        home.path(),
        &[
            "project",
            "workflow",
            "list",
            "--project",
            project_id,
            "--json",
        ],
    );
    let private = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "proof")
        .unwrap();
    assert_eq!(private["source"], "stored");
    assert!(private["workflow"].is_object());
    let source = lf(
        repo.path(),
        home.path(),
        &["project", "workflow", "source", project_id, "proof"],
    );
    assert_eq!(
        source.as_str().unwrap().trim(),
        std::fs::read_to_string(&definition).unwrap().trim()
    );
    assert!(!repo.path().join(".lf/workflows").exists());
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
        .starts_with(&format!("lf/{}/", id.trim_start_matches("task_"))));
    lf(repo.path(), home.path(), &["-b", "task", "run", id]);
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(status["execution"]["status"], "active");
    assert_eq!(status["execution"]["task_id"], id);
    assert!(status["planning_error"].is_null());
    assert!(status["execution"]["work"]["workflow"].is_object());
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
        &["task", "create", "--title", "Later work", "--json"],
    );
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "inbox", "--json"],
    );
    let successor = uuid::Uuid::new_v4();
    let plan_file = home.path().join("rotation.json");
    std::fs::write(&plan_file,serde_json::to_vec(&serde_json::json!({"name":"Next", "waves":[{"wave_id":wave["wave"]["id"],"successor_id":successor.to_string(),"create":true,"project_name":"Next","workflow":"proof","metric_targets":[],"krs":[{"text":"Retain active work","holds":false}]}]})).unwrap()).unwrap();
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
        moved["execution"]["work"]["workflow"],
        status["execution"]["work"]["workflow"]
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
        &["task", "create", "--title", "Local execution", "--json"],
    );
    assert_ne!(retry["id"], id);
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "move",
            id,
            "end",
            "--reason",
            "Inspection completed",
        ],
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
            repo.create_file(".lf/config.yaml", "pm:\n  linear_team: fixture-team\n");
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
                "project",
                "edit",
                id.as_str(),
                "--name",
                "Authored name",
                "--summary",
                "Retain summary",
            ],
        );
        for mapped in [false, true] {
            let provider = "11111111-1111-4111-8111-111111111111";
            db.execute("UPDATE projects SET external_project_id=?2,status='backlog',krs=?3,workflow='feature' WHERE id=?1", rusqlite::params![id.as_str(),mapped.then_some(provider),r#"[{"text":"Preserve Tasks","holds":false}]"#]).unwrap();
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
            assert_eq!(active["workflow"], "feature");
            assert_eq!(active["krs"][0]["text"], "Preserve Tasks");
            let changes = store.pending_project_changes(&id).unwrap();
            let state = changes
                .iter()
                .find(|change| change.field == "status")
                .unwrap();
            assert_eq!(state.value, "started");
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
                    "summary": "Retain summary", "workflow": "feature", "krs": active["krs"],
                    "metric_targets": [], "status": "completed", "revision": "2026-10-08T11:00:00Z",
                    "initiative_ids": ["initiative"], "team_ids": ["fixture-team"]
                }))
                .unwrap();
                store
                    .put_pm_project(wave.id(), "linear", "initiative", &incoming, 20)
                    .unwrap();
                assert!(store
                    .pending_project_changes(&id)
                    .unwrap()
                    .iter()
                    .all(|change| change.field != "status"));
                let losing: (String, String) = db
                    .query_row(
                        "SELECT value_json,conflict_json FROM project_changes WHERE id=?1",
                        [&state.id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(serde_json::from_str::<Value>(&losing.0).unwrap(), "started");
                assert_eq!(
                    serde_json::from_str::<Value>(&losing.1).unwrap()["value"],
                    "completed"
                );
                assert_eq!(
                    db.query_row(
                        "SELECT status FROM projects WHERE id=?1",
                        [id.as_str()],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                    "completed"
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
        assert!(repo.path().join("wave").exists());
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
                "planning:\n  provider: linear\npm:\n  linear_team: team\n",
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
                    "summary": "", "revision": null, "status": "started", "workflow": "",
                    "krs": [], "metric_targets": [], "initiative_ids": ["initiative"],
                    "team_ids": ["team"]
                }))
                .unwrap()
            })
            .collect();
        store
            .put_pm_snapshot(&loopflow::store::PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: "initiative".into(),
                synced_at: 17,
                snapshot: loopflow::pm::PmSnapshot {
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
                        "project",
                        "edit",
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
            let roadmap = lf(
                repo.path(),
                home.path(),
                &["roadmap", "--wave", "product", "--json"],
            );
            for rows in [
                &status["tasks"]["items"],
                &roadmap["waves"][0]["tasks"]["items"],
            ] {
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
            for rows in [
                &status["projects"]["items"],
                &roadmap["waves"][0]["projects"]["items"],
            ] {
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
                "pm:\n  linear_team: fixture-team\n",
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
        let project = loopflow::pm::linear::LinearProject {
            description: None,
            id: "provider-project".into(),
            revision: Some("2026-10-08T10:00:00Z".into()),
            slug: "original".into(),
            name: "Original".into(),
            summary: "Original summary".into(),
            workflow: "feature".into(),
            status: loopflow::work::project::ProjectStatus::Started,
            krs: vec![loopflow::work::project::ProjectKR {
                text: "Retain the proof".into(),
                holds: false,
            }],
            metric_targets: Vec::new(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
        };
        store
            .put_pm_project(wave.id(), "linear", "initiative", &project, 17)
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
        let source = home.path().join("workflow.yaml");
        std::fs::write(&source, "nodes: {}\nedges: [{from: start, to: end}]\n").unwrap();
        lf(
            repo.path(),
            home.path(),
            &[
                "project",
                "edit",
                id.as_str(),
                "--name",
                "Saved",
                "--summary",
                "",
                "--description",
                &authored.description,
            ],
        );
        let output = command(
            repo.path(),
            home.path(),
            &[
                "project",
                "workflow",
                "set",
                id.as_str(),
                "review",
                "--file",
                source.to_str().unwrap(),
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
        // An active writer cannot block inspection of the committed plan.
        let writer = std::fs::File::open(
            home.path()
                .join("chapter-locks")
                .join(format!("{}.lock", wave.id())),
        )
        .unwrap();
        fs2::FileExt::lock_exclusive(&writer).unwrap();
        let saved = lf(
            repo.path(),
            home.path(),
            &["project", "workflow", "show", id.as_str(), "--json"],
        );
        drop(writer);
        assert_eq!(saved["id"], id.as_str());
        assert_eq!(
            saved["linear_id"],
            if mapped {
                serde_json::json!(project.id)
            } else {
                Value::Null
            }
        );
        assert_eq!(saved["description"], authored.description.trim());
        assert_eq!(saved["name"], "Saved");
        assert_eq!(saved["summary"], "");
        assert_eq!(saved["workflow"], "review");
        assert_eq!(saved["krs"][0]["text"], "Retain the proof");
        assert_eq!(saved["sync"]["connected"], connected);
        assert_eq!(
            saved["sync"]["changes"].as_array().unwrap().is_empty(),
            !connected
        );
        if mapped {
            assert_eq!(
                lf(
                    repo.path(),
                    home.path(),
                    &["project", "workflow", "show", &project.id, "--json"]
                ),
                saved
            );
        }
        assert_eq!(store.project(&shadow.id).unwrap().unwrap(), shadow);
        let changes = store.pending_project_changes(&id).unwrap();
        assert_eq!(changes.len(), 4);
        lf(
            repo.path(),
            home.path(),
            &[
                "project",
                "edit",
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
            incoming.workflow = "research".into();
            incoming.revision = Some("2026-10-08T11:00:00Z".into());
            incoming.krs[0].holds = true;
            store
                .put_pm_project(wave.id(), "linear", "initiative", &incoming, 18)
                .unwrap();
            let read = lf(
                repo.path(),
                home.path(),
                &["project", "workflow", "show", id.as_str(), "--json"],
            );
            assert_eq!(read["name"], "Remote name");
            assert_eq!(read["workflow"], "research");
            assert_eq!(read["krs"][0]["holds"], true);
            assert_eq!(read["pm_snapshot_synced_at"], 18);
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
                .all(|change| change.field != "name" && change.field != "workflow"));
            let conflicts = read["sync"]["changes"].as_array().unwrap();
            for field in ["name", "workflow"] {
                assert!(conflicts
                    .iter()
                    .any(|change| change["field"] == field && change["state"] == "adopted_linear"));
            }
            // Later provider edits advance; superseded intentions never become pending again.
            incoming.name = "Saved".into();
            incoming.workflow = "review".into();
            incoming.revision = Some("2026-10-08T12:00:00Z".into());
            store
                .put_pm_project(wave.id(), "linear", "initiative", &incoming, 19)
                .unwrap();
            let read = lf(
                repo.path(),
                home.path(),
                &["project", "workflow", "show", id.as_str(), "--json"],
            );
            assert_eq!(store.pending_project_changes(&id).unwrap(), pending);
            assert_eq!(read["sync"]["changes"], Value::Array(conflicts.clone()));
        }
        let plan_file = home.path().join("plan.json");
        let content = serde_json::json!({
            "workflow": "review", "metric_targets": [],
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
            String::from_utf8_lossy(&output.stderr).contains("pending Linear sync"),
            connected
        );
        let saved = lf(
            repo.path(),
            home.path(),
            &["project", "workflow", "show", id.as_str(), "--json"],
        );
        assert_eq!(saved["krs"], content["krs"]);
        assert_eq!(saved["workflow"], content["workflow"]);
        assert_eq!(saved["id"], id.as_str());
        assert_eq!(
            saved["linear_id"],
            if mapped {
                serde_json::json!(project.id)
            } else {
                Value::Null
            }
        );
        assert_eq!(saved["description"], authored.description.trim());
        assert_eq!(saved["name"], "Saved");
        assert_eq!(saved["summary"], "");
        assert_eq!(
            store.pending_project_changes(&id).unwrap().len(),
            if mapped { 3 } else { 5 }
        );
        lf(repo.path(), home.path(), &args);
        let repeated = lf(
            repo.path(),
            home.path(),
            &["project", "workflow", "show", id.as_str(), "--json"],
        );
        assert_eq!(repeated, saved);
        if mapped {
            let mut incoming = project.clone();
            incoming.revision = Some("2026-10-08T13:00:00Z".into());
            incoming.krs[0].text = "Keep the concurrent provider plan".into();
            store
                .put_pm_project(wave.id(), "linear", "initiative", &incoming, 20)
                .unwrap();
            let read = lf(
                repo.path(),
                home.path(),
                &["project", "workflow", "show", id.as_str(), "--json"],
            );
            assert_eq!(read["krs"][0]["text"], "Keep the concurrent provider plan");
            assert!(store
                .pending_project_changes(&id)
                .unwrap()
                .iter()
                .all(|change| change.field != "krs"));
            assert!(read["sync"]["changes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|change| change["field"] == "krs" && change["state"] == "adopted_linear"));
        }
        assert!(store.list_tasks(None).unwrap().is_empty());
        assert!(!repo.path().join("wave").exists());
        assert!(!repo.path().join(".lf/workflows").exists());
    }
}

#[test]
fn task_creation_and_edits_save_offline_in_both_connection_modes() {
    use loopflow::durable::TaskId;
    use loopflow::store::{PmSnapshotRow, PmTaskRecord};

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "pm:\n  linear_team: fixture-team\n",
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
        let project = loopflow::pm::linear::LinearProject {
            description: None,
            id: "provider-project".into(),
            revision: Some("2026-10-08T10:00:00Z".into()),
            slug: "product".into(),
            name: "Product".into(),
            summary: "".into(),
            workflow: "".into(),
            status: loopflow::work::project::ProjectStatus::Started,
            krs: vec![],
            metric_targets: vec![],
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
        };
        store
            .put_pm_project(wave.id(), "linear", "initiative", &project, 17)
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
        let mut snapshot = loopflow::pm::PmSnapshot {
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
                .put_pm_snapshot(&PmSnapshotRow {
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
            String::from_utf8_lossy(&output.stderr).contains("pending Linear sync"),
            connected
        );
        let pending = store.pending_task_changes(&second).unwrap();
        assert_eq!(pending.len(), 3);
        assert!(store.pending_task_changes(&first).unwrap().is_empty());
        let ordering = store
            .pending_project_changes(&store.task(&second).unwrap().unwrap().project_id)
            .unwrap();
        assert_eq!(
            ordering
                .iter()
                .filter(|change| change.field == "task_order")
                .count(),
            1
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
                &loopflow::pm::PmItemUpdate {
                    name: Some("Stale".into()),
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
            assert!(sync.changes.iter().any(|c| c.field == "name"));
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
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &PmTaskRecord {
                        observed_at: 19,
                        project: Some(project.clone()),
                        item: incoming.clone(),
                    },
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            let sync = store.task_planning_sync(&second).unwrap();
            let conflict = sync.changes.iter().find(|c| c.field == "name").unwrap();
            assert_eq!(
                conflict.state,
                loopflow::store::planning_sync::PlanningSyncState::AdoptedLinear
            );
            assert_eq!(conflict.local_value, "Saved offline");
            assert_eq!(
                conflict.linear_value,
                Some(serde_json::json!("Concurrent title"))
            );
            let retained = store.task(&second).unwrap().unwrap();
            assert_eq!(retained.title, "Concurrent title");
            assert_eq!(retained.description, "");
            assert_eq!(retained.rank, 0);
            assert_eq!(retained.assignee.as_deref(), Some("remote-person"));
            assert!(retained.completed);
            let changes = store.pending_task_changes(&second).unwrap();
            assert!(changes
                .iter()
                .all(|change| !matches!(change.field.as_str(), "name" | "assignee")));
            let losing: (String, String) = conn
                .query_row(
                    "SELECT value_json,conflict_json FROM task_changes WHERE id=?1",
                    [&pending[0].id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&losing.0).unwrap(),
                "Saved offline"
            );
            assert_eq!(
                serde_json::from_str::<Value>(&losing.1).unwrap()["value"],
                "Concurrent title"
            );
            incoming.name = "Saved offline".into();
            incoming.assignee = Some("person-id".into());
            incoming.description.clear();
            incoming.revision = Some("2026-10-08T12:00:00Z".into());
            store
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &PmTaskRecord {
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
        let (sessions,flows): (i64,i64) = conn.query_row("SELECT (SELECT count(*) FROM agent_sessions),(SELECT count(*) FROM task_workflows)",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!((sessions, flows), (0, 0));
        assert!(!repo.path().join("wave").exists());
    }
}

fn place_wave_on_remote(
    store: &loopflow::store::sqlite::SqliteStore,
    db: &rusqlite::Connection,
    wave: &loopflow::id::WaveId,
) -> loopflow::durable::Placement {
    let machine = loopflow::durable::MachineId::new();
    store
        .add_machine(&machine, machine.as_str(), machine.as_str(), "/remote/repo")
        .unwrap();
    db.execute(
        "INSERT INTO work_placements(wave_id,machine_id,enabled,placed_at) VALUES(?1,?2,1,7)
         ON CONFLICT(wave_id) DO UPDATE SET machine_id=excluded.machine_id,placed_at=excluded.placed_at",
        rusqlite::params![wave.as_str(), machine.as_str()],
    )
    .unwrap();
    store
        .placement(&loopflow::durable::WorkRef::Wave(wave.clone()))
        .unwrap()
}

#[test]
fn rotation_saves_membership_and_pending_effects_offline() {
    use loopflow::durable::TaskId;
    use loopflow::store::PmSnapshotRow;
    use loopflow::work::task::NewTask;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            repo.create_file(".lf/config.yaml", "pm:\n  linear_team: fixture-team\n");
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
                due_date: Some("2026-12-31".into()),
                id: TaskId::new(),
                project_id: project.id.clone(),
                title: "Started work".into(),
                description: "Retain description".into(),
            })
            .unwrap();
        let backlog = store
            .create_task(&NewTask {
                due_date: None,
                id: TaskId::new(),
                project_id: project.id.clone(),
                title: "Unreviewed backlog".into(),
                description: "".into(),
            })
            .unwrap();
        db.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',17)", [active.id.as_str()]).unwrap();
        db.execute(
            "UPDATE tasks SET started_at=17 WHERE id=?1",
            [active.id.as_str()],
        )
        .unwrap();
        if mapped {
            db.execute("UPDATE tasks SET external_issue_id='provider-issue',issue_identifier='FIX-1',planning_team_id='fixture-team' WHERE id=?1", [active.id.as_str()]).unwrap();
        }
        let observed_project = loopflow::pm::linear::LinearProject {
            id: "provider-project".into(),
            slug: project.slug.clone(),
            name: project.name.clone(),
            summary: project.summary.clone(),
            description: Some(project.description.clone()),
            workflow: project.workflow.clone(),
            status: project.status,
            krs: project.krs.clone(),
            metric_targets: project.metric_targets.clone(),
            initiative_ids: vec!["initiative".into()],
            team_ids: vec!["fixture-team".into()],
            revision: None,
        };
        let mut snapshot = loopflow::pm::PmSnapshot {
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
                .put_pm_snapshot(&PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 20,
                    snapshot: snapshot.clone(),
                })
                .unwrap();
        }
        db.execute("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code','/retained/task',2)", []).unwrap();
        db.execute("INSERT INTO task_workflows(task_id,graph,node,edge,lf_process_id,updated_at) VALUES(?1,?2,'review',0,'11111111-1111-4111-8111-111111111111',7)", rusqlite::params![active.id.as_str(),serde_json::json!({"name":"review","nodes":[],"edges":[{"from":"review","to":"end","flow":"implement"}]}).to_string()]).unwrap();
        let retained_execution = || {
            [
                ("task_workflows", "task_id", active.id.as_str()),
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
        let placement = place_wave_on_remote(&store, &db, wave.id());
        let successor = loopflow::durable::ProjectId::new();
        let file = home.path().join("rotation.json");
        let plan = serde_json::json!({"name":"Next", "waves":[{"wave_id":wave.id(),"successor_id":successor,"create":true,"project_name":"Next", "workflow":"","metric_targets":[],"krs":[{"text":"Preserve work","holds":false}]}]});
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
        assert_eq!(preview["waves"][0]["tasks"][0]["disposition"], "move");
        let expected = store.task(&active.id).unwrap().unwrap();
        let preview_task = &preview["waves"][0]["tasks"][0]["task"];
        assert_eq!(preview_task, &serde_json::to_value(&expected).unwrap());
        assert_eq!(preview_task["id"], active.id.as_str());
        assert_eq!(preview_task["due_date"], "2026-12-31");
        assert_eq!(preview_task["description"], "Retain description");
        assert_eq!(
            store.task(&active.id).unwrap().unwrap().project_id,
            project.id
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
        assert_eq!(
            applied["waves"][0]["tasks"][0]["task"],
            serde_json::to_value(&moved).unwrap()
        );
        assert_eq!(moved.project_id, successor);
        assert_eq!(
            db.query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [active.id.as_str()],
                |r| r.get::<_, Option<i64>>(0)
            )
            .unwrap(),
            Some(17)
        );
        assert_eq!(
            store.task(&backlog.id).unwrap().unwrap().project_id,
            project.id
        );
        let changes = store.pending_task_changes(&active.id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "project_id");
        assert_eq!(changes[0].value, successor.as_str());
        let status = store.pending_project_changes(&project.id).unwrap();
        assert_eq!(status[0].value, "completed");
        // Released receipts nest content and preserve their original JSON bytes/order.
        let retained = format!(
            r#"{{"wave_id":"{}","successor_id":"{}","create":true,"project_name":"Next","content":{{"metric_targets":[],"workflow":"","krs":[{{"text":"Preserve work","holds":false}}]}}}}"#,
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
                .put_pm_snapshot(&PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative".into(),
                    synced_at: 21,
                    snapshot,
                })
                .unwrap();
            let retained = store.task(&active.id).unwrap().unwrap();
            assert_eq!(retained.project_id, successor);
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
            successor
        );
        assert_eq!(reopened.pending_task_changes(&active.id).unwrap(), changes);
        if mapped {
            db.execute(
                "UPDATE projects SET external_project_id='exported-successor' WHERE id=?1",
                [successor.as_str()],
            )
            .unwrap();
            let mut echo: loopflow::store::PmTaskRecord =
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
            project.workflow = successor_plan.workflow;
            project.krs = successor_plan.krs;
            project.metric_targets = successor_plan.metric_targets;
            echo.item.project_id = Some(project.id.clone());
            echo.item.project = Some(project.slug.clone());
            echo.item.revision = Some("2026-10-08T12:30:00Z".into());
            echo.observed_at = 22;
            store
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &echo,
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            assert_eq!(store.pending_task_changes(&active.id).unwrap(), changes);
            assert_eq!(
                store.task(&active.id).unwrap().unwrap().project_id,
                successor
            );
        }
        // Select an already stored destination using its provider alias when present.
        let mut existing = project.clone();
        existing.id = loopflow::durable::ProjectId::new();
        existing.name = "Future".into();
        existing.slug = "future".into();
        existing.status = loopflow::work::project::ProjectStatus::Planned;
        existing.linear_id =
            mapped.then(|| loopflow::pm::LinearProjectId::new("future-provider").unwrap());
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
            existing.id
        );
        assert_eq!(store.project(&existing.id).unwrap().unwrap().name, "Next");
        let pending = store.pending_project_changes(&existing.id).unwrap();
        assert!(pending
            .iter()
            .any(|c| c.field == "status" && c.value == "started"));
        assert!(pending.iter().any(|c| c.field == "krs"));
        lf(repo.path(), home.path(), &args);
        assert_eq!(
            store.pending_project_changes(&existing.id).unwrap(),
            pending
        );
        if mapped {
            let mut remote: loopflow::store::PmTaskRecord =
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
            store
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &remote,
                    Some((wave.id(), "initiative")),
                )
                .unwrap();
            assert_eq!(
                store.task(&active.id).unwrap().unwrap().project_id,
                store
                    .project_by_project("third-provider-project")
                    .unwrap()
                    .unwrap()
                    .id
            );
            assert!(store.pending_task_changes(&active.id).unwrap().is_empty());
            assert_eq!(
                store.task_acquisition(&active.id).unwrap().state,
                loopflow::store::PlanningState::Available
            );
        }
        assert_eq!(
            store
                .placement(&loopflow::durable::WorkRef::Wave(wave.id().clone()))
                .unwrap(),
            placement
        );
        assert_eq!(retained_execution(), execution);
        assert!(repo.path().join("wave").exists());
    }
}

#[test]
fn refiling_saves_membership_offline_and_retains_inbound_changes() {
    use loopflow::durable::TaskId;
    use loopflow::work::task::NewTask;

    for (connected, mapped) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        if connected {
            repo.create_file(".lf/config.yaml", "pm:\n  linear_team: fixture-team\n");
        }
        let path = home.path().join("loopflow.db");
        let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
        let canonical = loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        let mut projects = Vec::new();
        let mut observations = Vec::new();
        let mut placements = Vec::new();
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
                project.linear_id = Some(loopflow::pm::LinearProjectId::new(name).unwrap());
                store.insert_project(&project).unwrap();
                let observed = loopflow::pm::linear::LinearProject {
                    id: name.into(),
                    slug: project.slug.clone(),
                    name: project.name.clone(),
                    summary: project.summary.clone(),
                    description: Some(project.description.clone()),
                    workflow: project.workflow.clone(),
                    status: project.status,
                    krs: project.krs.clone(),
                    metric_targets: project.metric_targets.clone(),
                    initiative_ids: vec![name.into()],
                    team_ids: vec!["fixture-team".into()],
                    revision: None,
                };
                store
                    .put_pm_project(wave.id(), "linear", name, &observed, 1)
                    .unwrap();
                observations.push(observed);
            }
            placements.push((
                wave.id().clone(),
                place_wave_on_remote(&store, &db, wave.id()),
            ));
            projects.push(project);
        }
        let task = store
            .create_task(&NewTask {
                due_date: None,
                id: TaskId::new(),
                project_id: projects[0].id.clone(),
                title: "Refile me".into(),
                description: "Original".into(),
            })
            .unwrap();
        if mapped {
            db.execute("UPDATE tasks SET external_issue_id='provider-issue',issue_identifier='FIX-1',planning_team_id='fixture-team' WHERE id=?1", [task.id.as_str()]).unwrap();
        }
        let original = loopflow::store::PmTaskRecord {
            item: observed_issue("provider-issue", "source", "Refile me", "Original"),
            project: observations.first().cloned(),
            observed_at: 1,
        };
        if mapped {
            store
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &original,
                    Some((&projects[0].wave_id, "source")),
                )
                .unwrap();
        }
        let args = ["task", "refile", task.id.as_str(), "--wave", "destination"];
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
            projects[0].id
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
            String::from_utf8_lossy(&output.stderr).contains("pending Linear sync"),
            connected
        );
        let moved = store.task(&task.id).unwrap().unwrap();
        assert_eq!(moved.project_id, projects[1].id);
        assert_eq!(moved.wave_id, projects[1].wave_id);
        assert_eq!(moved.revision, task.revision + 1);
        assert!(moved.worktree.is_none());
        let changes = store.pending_task_changes(&task.id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "project_id");
        assert_eq!(changes[0].value, projects[1].id.as_str());
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
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &incoming,
                    Some((&projects[0].wave_id, "source")),
                )
                .unwrap();
            let retained = store.task(&task.id).unwrap().unwrap();
            assert_eq!(retained.project_id, projects[1].id);
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
                .put_pm_task(
                    &canonical.to_string(),
                    "linear",
                    &incoming,
                    Some((&projects[2].wave_id, "competing")),
                )
                .unwrap();
            assert!(store.pending_task_changes(&task.id).unwrap().is_empty());
            assert_eq!(
                store.task(&task.id).unwrap().unwrap().project_id,
                projects[2].id
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
            projects[if mapped { 2 } else { 1 }].id
        );
        assert_eq!(
            reopened.pending_task_changes(&task.id).unwrap(),
            store.pending_task_changes(&task.id).unwrap()
        );
        // A retained start cannot be moved by refiling, even without a checkout.
        db.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',17)", [task.id.as_str()]).unwrap();
        db.execute(
            "UPDATE tasks SET started_at=17 WHERE id=?1",
            [task.id.as_str()],
        )
        .unwrap();
        let pending = store.pending_task_changes(&task.id).unwrap();
        let refused = command(
            repo.path(),
            home.path(),
            &["task", "refile", task.id.as_str(), "--wave", "source"],
        )
        .output()
        .unwrap();
        assert!(
            !refused.status.success(),
            "{}",
            String::from_utf8_lossy(&refused.stdout)
        );
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().project_id,
            projects[if mapped { 2 } else { 1 }].id
        );
        assert_eq!(store.pending_task_changes(&task.id).unwrap(), pending);
        for (wave, placement) in placements {
            assert_eq!(
                store
                    .placement(&loopflow::durable::WorkRef::Wave(wave))
                    .unwrap(),
                placement
            );
        }
        assert!(repo.path().join("wave").exists());
    }
}

#[test]
fn wave_documents_follow_files_while_workflows_remain_stored() {
    for connected in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let wave_id = loopflow::id::WaveId::new();
        let root = repo.path().join("wave/tools");
        std::fs::create_dir_all(root.join("parser")).unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
        if connected {
            std::fs::write(
                repo.path().join(".lf/config.yaml"),
                "pm:\n  provider: linear\n  linear_team: fixture-team\n",
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
        let workflow = "nodes: {}\nedges: [{from: start, to: end}]\n";
        std::fs::write(repo.path().join(".lf/workflows/finish.yaml"), workflow).unwrap();
        let project = lf(
            repo.path(),
            home.path(),
            &["wave", "ensure", "tools/parser", "--json"],
        );
        let project_id = project["id"].as_str().unwrap();
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
            &["project", "workflow", "set", project_id, "finish"],
        );
        std::fs::write(root.join("parser/MEMORY.md"), "Edited child memory λ.\n").unwrap();
        let context = lf(
            repo.path(),
            home.path(),
            &["context", "--wave", "tools/parser", "--json"],
        );
        let text = context.to_string();
        assert!(text.contains("wave/tools/MEMORY.md"), "{text}");
        assert!(text.contains("wave/tools/parser/MEMORY.md"), "{text}");
        assert_eq!(std::fs::read_to_string(root.join("GOAL.md")).unwrap(), goal);
        std::fs::rename(repo.path().join("wave"), repo.path().join("authored-wave")).unwrap();
        std::fs::remove_file(repo.path().join(".lf/workflows/finish.yaml")).unwrap();
        let absent = lf(
            repo.path(),
            home.path(),
            &["context", "--wave", "tools/parser", "--json"],
        );
        assert!(!absent.to_string().contains("wave/tools/parser/MEMORY.md"));
        let source = lf(
            repo.path(),
            home.path(),
            &["project", "workflow", "source", project_id, "finish"],
        );
        assert!(source.as_str().unwrap().contains("from: start"));
        std::fs::write(repo.path().join(".lf/workflows/unimported.yaml"), workflow).unwrap();
        std::fs::write(
            repo.path().join(".lf/workflows/finish.yaml"),
            "invalid: true",
        )
        .unwrap();
        let catalog = lf(
            repo.path(),
            home.path(),
            &[
                "project",
                "workflow",
                "list",
                "--project",
                project_id,
                "--json",
            ],
        );
        let entries = catalog.as_array().unwrap();
        let saved = entries
            .iter()
            .find(|entry| entry["name"] == "finish")
            .unwrap();
        assert_eq!(saved["source"], "stored");
        assert!(saved["unavailable"].is_null());
        let unimported = entries
            .iter()
            .find(|entry| entry["name"] == "unimported")
            .unwrap();
        assert!(unimported["workflow"].is_null());
        assert!(unimported["unavailable"]
            .as_str()
            .unwrap()
            .contains("lf wave ensure"));
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
        assert!(!repo.path().join("wave").exists());
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
                "pm:\n  provider: linear\n  linear_team: fixture-team\n",
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
        let changes = store.pending_task_changes(&task_id).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "deleted");
        assert_eq!(changes[0].value, true);
        lf(repo.path(), home.path(), &["task", "delete", id]);
        assert_eq!(store.pending_task_changes(&task_id).unwrap(), changes);
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
        assert_eq!(reopened.pending_task_changes(&task_id).unwrap(), changes);
    }
}

#[test]
fn task_selectors_use_repository_scope_and_preserve_exact_identity_through_aliases() {
    let first = TestRepo::new();
    let second = TestRepo::new();
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
    db.execute(
        "UPDATE waves SET repo=?1 WHERE id=?2",
        rusqlite::params![
            alias.to_string_lossy(),
            tasks[0]["wave_id"].as_str().unwrap()
        ],
    )
    .unwrap();
    for (repo, task) in [
        (first.path(), &tasks[0]),
        (alias.as_path(), &tasks[0]),
        (second.path(), &tasks[1]),
    ] {
        let status = lf(repo, home.path(), &["task", "status", "DUP-1", "--json"]);
        assert_eq!(status["task"]["id"], task["id"]);
    }
    // Exact identity still permits inspection across repository scope.
    let foreign = tasks[1]["id"].as_str().unwrap();
    let status = lf(
        first.path(),
        home.path(),
        &["task", "status", foreign, "--json"],
    );
    assert_eq!(status["task"]["id"], foreign);
    let rejected = command(
        first.path(),
        home.path(),
        &[
            "task",
            "edit",
            foreign,
            "--wave",
            tasks[0]["wave_id"].as_str().unwrap(),
            "--title",
            "Wrong owner",
        ],
    )
    .output()
    .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .to_lowercase()
            .contains("wave"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    lf(
        &alias,
        home.path(),
        &["task", "edit", "DUP-1", "--title", "First only"],
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
fn ordinary_task_creation_returns_saved_identity_without_a_public_retry_token() {
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
    let id = output.split_whitespace().next().unwrap();
    loopflow::durable::TaskId::parse(id).unwrap();
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
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
        "pm:\n  linear_team: fixture-team\n",
    )
    .unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Saved plan", "--json"],
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
    for field in ["creation", "name", "comment"] {
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
            .find(|p| p["work_id"] == project)
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
    assert_eq!(
        read_project()["changes"][0]["error"],
        "Lost attachment reply"
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
    conn.execute(r#"UPDATE task_changes SET conflict_json='{"value":"Linear title"}',error=NULL WHERE task_id=?1"#, [id]).unwrap();
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
fn wave_files_are_checkout_local_and_edits_reach_status_without_ensure() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let checkout = repo.create_named_worktree("wave-documents");
    lf(
        &checkout,
        home.path(),
        &["wave", "ensure", "local", "--json"],
    );
    let goal = checkout.join("wave/local/GOAL.md");
    let memory = checkout.join("wave/local/MEMORY.md");
    assert!(goal.is_file());
    assert!(memory.is_file());
    assert!(
        !repo.path().join("wave/local").exists(),
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
        .find(|entry| entry["source"] == "wave/local/MEMORY.md")
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
    assert!(String::from_utf8_lossy(&removed.stderr).contains("unrecognized subcommand 'edit'"));
}

#[test]
fn canonical_status_retains_content_and_execution_across_acquisition_states() {
    use loopflow::durable::TaskId;
    use loopflow::store::PmTaskRecord;
    use serde_json::json;

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Original", "--json"],
    );
    let id = created["id"].as_str().unwrap();
    let id = TaskId::parse(id).unwrap();
    let path = home.path().join("loopflow.db");
    let store = loopflow::store::sqlite::SqliteStore::new(&path).unwrap();
    let task = store.task(&id).unwrap().unwrap();
    let mut project = store.project(&task.project_id).unwrap().unwrap();
    project.linear_id = Some(loopflow::pm::LinearProjectId::new("provider-project").unwrap());
    store.insert_project(&project).unwrap();
    let observed: loopflow::pm::linear::LinearProject = serde_json::from_value(json!({
        "id":"provider-project", "slug":project.slug, "name":project.name,
        "summary":"", "description":"Saved Project prose", "workflow":"", "status":"started",
        "krs":[], "metric_targets":[], "initiative_ids":["initiative"], "team_ids":["team"], "revision":null
    })).unwrap();
    store
        .put_pm_project(&task.wave_id, "linear", "initiative", &observed, 17)
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute(
        "UPDATE tasks SET external_issue_id='provider-task',issue_identifier='FIX-1' WHERE id=?1",
        [id.as_str()],
    )
    .unwrap();
    let record: PmTaskRecord = serde_json::from_value(json!({
        "item":{"id":"provider-task","identifier":"FIX-1","name":"Original", "description":"Provider prose",
            "rank":0,"completed":false,"state":"unstarted","project_id":"provider-project", "project":project.slug,
            "team_id":"team","due_date":"2026-10-11", "assignee":null, "url":null},
        "project":observed,"observed_at":17
    })).unwrap();
    let scope = loopflow::repository::CanonicalRepo::discover(repo.path())
        .unwrap()
        .to_string();
    store
        .put_pm_task(
            &scope,
            "linear",
            &record,
            Some((&task.wave_id, "initiative")),
        )
        .unwrap();
    let task = store.task(&id).unwrap().unwrap();
    store
        .edit_task(
            &id,
            task.revision,
            &loopflow::pm::PmItemUpdate {
                name: Some("Saved locally".into()),
                description: Some("Saved Task prose".into()),
                ..Default::default()
            },
        )
        .unwrap();
    // Seed a locally retained date differing from the provider observation, and
    // a captured Workflow. Reading must not copy either value from another owner.
    db.execute(
        "UPDATE tasks SET planning_due_date='2026-10-20' WHERE id=?1",
        [id.as_str()],
    )
    .unwrap();
    db.execute("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','22222222-2222-4222-8222-222222222222','lf run code','/retained/task',2)", []).unwrap();
    db.execute("INSERT INTO task_workflows(task_id,graph,node,edge,lf_process_id,updated_at) VALUES(?1,?2,'review',0,'11111111-1111-4111-8111-111111111111',7)", rusqlite::params![id.as_str(),json!({"name":"review","nodes":[],"edges":[{"from":"review","to":"end","flow":"implement"}]}).to_string()]).unwrap();
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
            "UPDATE pm_items SET project_id='another-project',body=json_set(body,'$.project_id','another-project')",
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
        assert_eq!(status["project"]["id"], saved.project_id.as_str());
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
    db.execute("INSERT INTO task_changes(id,task_id,field,value_json,base_json,attempted) VALUES('pending-membership',?1,'project_id',?2,'\"provider-project\"',0)", rusqlite::params![id.as_str(),serde_json::to_string(saved.project_id.as_str()).unwrap()]).unwrap();
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
    let node: String = db
        .query_row(
            "SELECT node FROM task_workflows WHERE task_id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(node, "review");
    let process: (Option<i64>, Option<String>) = db.query_row("SELECT completed_at,outcome FROM processes WHERE id='11111111-1111-4111-8111-111111111111'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(process, (None, None));
}

fn observed_issue(id: &str, project: &str, title: &str, description: &str) -> loopflow::pm::PmItem {
    loopflow::pm::PmItem {
        id: id.into(),
        identifier: "FIX-1".into(),
        name: title.into(),
        description: description.into(),
        project_id: Some(project.into()),
        project: Some(project.into()),
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
