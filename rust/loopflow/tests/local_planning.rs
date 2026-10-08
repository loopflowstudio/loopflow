use std::path::Path;
use std::process::Command;

use loopflow_test_support::TestRepo;
use serde_json::Value;

fn lf(repo: &Path, home: &Path, args: &[&str]) -> Value {
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
    let output = command
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_USER_NAME", "Fixture Person")
        .args(args)
        .output()
        .unwrap();
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
    repo.create_file(".lf/workflows/proof.yaml", "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: end, flow: proof}\n");
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
        &["project", "workflow", "set", project_id, "proof"],
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
