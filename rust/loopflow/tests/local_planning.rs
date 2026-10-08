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
    command
}

fn lf(repo: &Path, home: &Path, args: &[&str]) -> Value {
    let output = command(repo, home, args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| Value::String(String::from_utf8(output.stdout).unwrap()))
}

#[test]
fn public_local_plan_matches_desktop() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let id = "task_0123456789ab40008000000000000001";
    lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--title",
            "Retain quoted input",
            "--notes",
            "Keep the escaped quote",
            "--creation-id",
            id,
            "--json",
        ],
    );
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
        .replace(&comment_id, "00000000-0000-4000-8000-000000000001")
        .replace(&comment_date, "2026-10-08T00:00:00Z");
    let actual: Value = serde_json::from_str(&canonical).unwrap();
    let expected: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/dto/local_task.json")).unwrap();
    assert_eq!(actual, expected);
    serde_json::from_value::<loopflow::lf::commands::waves::RoadmapTask>(actual["task"].clone())
        .unwrap();
    serde_json::from_value::<loopflow::ops::pm::TaskComments>(actual["comments"].clone()).unwrap();
}

#[test]
fn personal_nested_waves_keep_definitions_and_projects_in_the_store() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let created = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--wave",
            "personal:tools/parser",
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
            "personal:tools/parser",
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
    assert_eq!(
        status["planning"]["project"]["id"],
        other["planning"]["project"]["id"]
    );
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "personal:tools/parser", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert!(!repo.path().join("wave").exists());
    assert!(!repo.path().join(".lf/config.yaml").exists());
    assert!(!repo.path().join(".lf/workflows").exists());
}

#[test]
fn public_local_planning_survives_creation_reply_loss_and_restart() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let identity = loopflow::durable::TaskId::new();
    let args = [
        "task",
        "create",
        "--title",
        "Fix parser",
        "--notes",
        "Keep quoted input",
        "--creation-id",
        identity.as_str(),
        "--json",
    ];
    // Discard the first response, then invoke a new CLI process with the retained identity.
    lf(repo.path(), home.path(), &args);
    let created = lf(repo.path(), home.path(), &args);
    assert_eq!(created["id"], identity.as_str());
    assert!(created["team_id"].is_null());
    assert!(created["url"].is_null());
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
    let retried = lf(repo.path(), home.path(), &args);
    assert_eq!(retried["name"], "Retain quoted input");
    let status = lf(
        repo.path(),
        home.path(),
        &["task", "status", identity.as_str(), "--json"],
    );
    assert_eq!(status["planning"]["item"]["name"], "Retain quoted input");
    assert_eq!(status["execution"]["task_id"], identity.as_str());
    assert!(status["execution"]["issue_id"].is_null());
    assert!(status["execution"]["worktree"].is_null());
    assert!(status["execution"]["prs"].as_array().unwrap().is_empty());
    assert!(status["planning_error"].is_null());
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "personal:inbox", "--json"],
    );
    assert_eq!(wave["tasks"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(wave["project_readiness"]["state"], "ready");
    let goal = home.path().join("goal.md");
    let memory = home.path().join("memory.md");
    std::fs::write(&goal, "## Objective\n\nKeep private parser work local.\n").unwrap();
    std::fs::write(&memory, "Retain the escaped quote.").unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "wave",
            "edit",
            "personal:inbox",
            "--goal",
            goal.to_str().unwrap(),
            "--memory",
            memory.to_str().unwrap(),
        ],
    );
    let context = lf(
        repo.path(),
        home.path(),
        &["context", "--wave", "personal:inbox", "--json"],
    );
    let memory_usage = context["context"]["usage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["source"] == "personal:inbox/MEMORY.md")
        .unwrap();
    assert_eq!(memory_usage["submitted_bytes"], 25);
    let edited_wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "personal:inbox", "--json"],
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
            "personal:inbox",
            "--plan",
            plan_file.to_str().unwrap(),
        ],
    );
    let project_id = status["planning"]["project"]["id"].as_str().unwrap();
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
    assert!(completed["planning"]["item"]["completed_at"].is_string());
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
    assert!(!repo.path().join("wave").exists());
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
    let project_id = status["planning"]["project"]["id"].as_str().unwrap();
    lf(
        repo.path(),
        home.path(),
        &[
            "project",
            "workflow",
            "set",
            project_id,
            "personal:proof",
            "--file",
            definition.to_str().unwrap(),
        ],
    );
    let placed = lf(
        repo.path(),
        home.path(),
        &["task", "checkout", id, "--json"],
    );
    let worktree = placed["worktree"].as_str().unwrap();
    assert_eq!(placed["task_id"], id);
    assert!(placed["prs"][0]["branch"]
        .as_str()
        .unwrap()
        .starts_with(&format!("lf/{}/", id.trim_start_matches("task_"))));
    lf(repo.path(), home.path(), &["-b", "task", "run", id]);
    let status = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(status["execution"]["status"], "active");
    assert_eq!(status["execution"]["task_id"], id);
    assert!(status["planning_error"].is_null());
    assert!(status["execution"]["work"]["workflow"].is_object());
    let landing = command(
        Path::new(worktree),
        home.path(),
        &["--task", id, "land", "-c"],
    )
    .output()
    .unwrap();
    assert!(!landing.status.success());
    let error = String::from_utf8_lossy(&landing.stderr);
    assert!(error.contains("remote"), "{error}");
    let retained = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(retained["execution"]["status"], "active");
    assert!(retained["execution"]["prs"][0]["merge_commit"].is_null());
    let backlog = lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Later work", "--json"],
    );
    let wave = lf(
        repo.path(),
        home.path(),
        &["wave", "status", "personal:inbox", "--json"],
    );
    let successor = uuid::Uuid::new_v4();
    let plan_file = home.path().join("rotation.json");
    std::fs::write(&plan_file,serde_json::to_vec(&serde_json::json!({"name":"Next", "waves":[{"wave_id":wave["wave"]["id"],"successor_id":successor.to_string(),"create":true,"project_name":"Next","content":{"workflow":"proof","metric_targets":[],"krs":[{"text":"Retain active work","holds":false}]}}]})).unwrap()).unwrap();
    let rotate = [
        "wave",
        "new-chapter",
        "personal:inbox",
        "Next",
        "--plan",
        plan_file.to_str().unwrap(),
        "--json",
    ];
    let rotation = lf(repo.path(), home.path(), &rotate);
    assert_eq!(rotation["waves"][0]["successor"]["status"], "started");
    lf(repo.path(), home.path(), &rotate);
    let moved = lf(repo.path(), home.path(), &["task", "status", id, "--json"]);
    assert_eq!(
        moved["planning"]["project"]["id"],
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
    assert_eq!(retained["planning"]["project"]["id"], project_id);
    let retry = lf(
        repo.path(),
        home.path(),
        &[
            "task",
            "create",
            "--title",
            "Local execution",
            "--creation-id",
            id,
            "--json",
        ],
    );
    assert_eq!(retry["id"], id);
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
