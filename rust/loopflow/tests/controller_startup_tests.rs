//! Production-shaped proof for the hidden controller startup boundary.

mod support;

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

use loopflow::durable::{ProjectId, TaskId};
use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::profile::{ProviderRoute, RouteScope};
use loopflow::provider_auth::Provider;
use loopflow::store::{
    CredentialState, CredentialType, ProviderAccount, ProviderAccountId, ProviderToken,
    RoutingState, StorageConfig, CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV,
};
use loopflow::work::project::Project;
use loopflow::work::task::{Observation, PmWritebackState, Task, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;

#[derive(Clone)]
struct LinearFixtureState {
    project_id: String,
    project_name: String,
    requests: Arc<AtomicUsize>,
}

async fn start_linear_fixture(project_id: &str, project_name: &str) -> (String, Arc<AtomicUsize>) {
    let state = LinearFixtureState {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        requests: Arc::new(AtomicUsize::new(0)),
    };
    let requests = state.requests.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind Linear fixture");
    let address = listener.local_addr().expect("Linear fixture address");
    let app = Router::new()
        .route("/", post(linear_fixture_response))
        .with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve Linear fixture");
    });
    (format!("http://{address}"), requests)
}

async fn linear_fixture_response(
    State(state): State<LinearFixtureState>,
    Json(request): Json<Value>,
) -> Json<Value> {
    state.requests.fetch_add(1, Ordering::SeqCst);
    let query = request["query"].as_str().unwrap_or_default();
    let response = if query.contains("query ListTeams") {
        json!({ "data": { "teams": { "nodes": [{
            "id": "team-controller",
            "name": "Controller",
            "key": "CTL",
            "description": "<!-- loopflow-repository: loopflowstudio/fixture -->"
        }] } } })
    } else if query.contains("query ListInitiativeProjects") {
        json!({ "data": { "initiative": { "projects": {
            "nodes": [{
                "id": state.project_id,
                "name": state.project_name,
                "description": "A completed controller proof.",
                "content": "## Definition\n\nA completed controller proof.\n\n## KRs\n\n- [x] Project controller execution is proven.\n",
                "initiatives": { "nodes": [{ "id": "initiative-controller" }] },
                "teams": { "nodes": [{ "id": "team-controller" }] }
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } } } })
    } else if query.contains("query ListProjectIssues") {
        json!({ "data": { "project": { "issues": {
            "nodes": [],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } } } })
    } else {
        json!({ "errors": [{ "message": "unexpected Linear fixture request" }] })
    };
    Json(response)
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed");
}

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write executable fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("mark fixture executable");
    }
}

fn public_lf(
    repo: &Path,
    home: &Path,
    bin: &Path,
    child_lf: &Path,
    tmux_state: &Path,
    linear_base_url: &str,
    args: &[&str],
) -> Output {
    let path = std::env::var_os("PATH").unwrap_or_default();
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(repo)
        .env(
            "PATH",
            format!("{}:{}", bin.display(), path.to_string_lossy()),
        )
        .env("LF_BIN", child_lf)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("LF_TEST_TMUX_STATE", tmux_state)
        .env(
            "LF_TEST_PROVIDER_RELEASE",
            tmux_state.join("provider-release"),
        )
        .env("LF_TEST_LINEAR_BASE_URL", linear_base_url)
        .env_remove(CONTROL_HOME_ENV)
        .env_remove(CONTROL_DB_PATH_ENV)
        .env_remove("LF_CONTROL_BIN")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_ACCOUNT_LEASE")
        .output()
        .expect("run public lf command")
}

fn running_receipts(home: &Path, kind: &str, id: &str) -> Vec<serde_json::Value> {
    let root = home.join("controller/startup");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .filter(|receipt| {
            receipt["state"] == "running"
                && receipt["work"]["kind"] == kind
                && receipt["work"]["id"] == id
        })
        .collect()
}

fn assert_exact_live_owner(home: &Path, receipt: &serde_json::Value) {
    let pid = receipt["pid"].as_u64().expect("startup receipt pid");
    let owner: serde_json::Value = serde_json::from_slice(
        &std::fs::read(home.join(format!("runtime/exec-processes/{pid}.json")))
            .expect("live Exec ownership receipt"),
    )
    .expect("parse Exec ownership receipt");
    assert_eq!(owner["trace_id"], receipt["trace_id"]);
    assert_eq!(owner["exec_id"], receipt["process_id"]);
    assert_eq!(owner["pid"], receipt["pid"]);
    assert_eq!(owner["started_at"], receipt["process_started_at"]);
    assert!(Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe controller process")
        .success());
}

fn successful_json(output: &Output, context: &str) -> serde_json::Value {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse public JSON output")
}

fn assert_live_authority(snapshot: &serde_json::Value, receipt: &serde_json::Value) {
    let authority = &snapshot["controller_authority"];
    assert_eq!(authority["state"], "live");
    assert_eq!(authority["owner"]["attempt_id"], receipt["attempt_id"]);
    assert_eq!(authority["owner"]["run_id"], receipt["run_id"]);
    assert_eq!(authority["owner"]["trace_id"], receipt["trace_id"]);
    assert_eq!(authority["owner"]["exec_id"], receipt["process_id"]);
    assert_eq!(authority["owner"]["pid"], receipt["pid"]);
    assert_eq!(
        authority["owner"]["process_started_at"],
        receipt["process_started_at"]
    );
}

fn process_is_live(pid: u64) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe process")
        .success()
}

fn tmux_marker_for_pid(tmux_state: &Path, pid: u64) -> std::path::PathBuf {
    std::fs::read_dir(tmux_state)
        .expect("read tmux fixture")
        .filter_map(Result::ok)
        .find(|entry| {
            std::fs::read_to_string(entry.path())
                .ok()
                .is_some_and(|value| value.trim() == pid.to_string())
        })
        .expect("tmux marker for controller PID")
        .path()
}

fn run_manifest(home: &Path, run_id: &str) -> std::path::PathBuf {
    home.join("runs")
        .join(&run_id[4..6])
        .join(run_id)
        .join("manifest.json")
}

fn stop_process_tree(pid: u64) {
    let pid = pid.to_string();
    let _ = Command::new("pkill")
        .args(["-TERM", "-P", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let status = Command::new("kill")
        .args(["-TERM", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("stop controller process");
    assert!(status.success());
    for _ in 0..100 {
        if !Command::new("kill")
            .args(["-0", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("probe stopped controller")
            .success()
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("controller process {pid} remained live after intentional stop");
}

fn wait_for_task_cursor(
    repo: &Path,
    home: &Path,
    bin: &Path,
    tmux_state: &Path,
    linear_base_url: &str,
    task: &Task,
    expected: u64,
) -> serde_json::Value {
    for _ in 0..200 {
        let status = public_lf(
            repo,
            home,
            bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            tmux_state,
            linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        );
        if status.status.success() {
            let snapshot: serde_json::Value =
                serde_json::from_slice(&status.stdout).expect("parse Task status");
            if snapshot["controller"]["phase_cursor"].as_u64() == Some(expected) {
                return snapshot;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("Task controller did not advance to phase cursor {expected}");
}

fn wait_for_project_iteration(
    repo: &Path,
    home: &Path,
    bin: &Path,
    tmux_state: &Path,
    linear_base_url: &str,
    project: &Project,
    expected: u64,
) -> serde_json::Value {
    for _ in 0..200 {
        let status = public_lf(
            repo,
            home,
            bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            tmux_state,
            linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        );
        if status.status.success() {
            let snapshot: serde_json::Value =
                serde_json::from_slice(&status.stdout).expect("parse Project status");
            if snapshot["iteration"].as_u64() == Some(expected) {
                return snapshot;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("Project controller did not advance to iteration {expected}");
}

#[test]
fn hidden_work_entrypoint_persists_its_immediate_startup_error() {
    let directory = tempfile::tempdir().expect("temporary controller Home");
    let repo = directory.path().join("repo");
    let home = directory.path().join("home");
    std::fs::create_dir_all(&repo).expect("create repository");
    std::fs::create_dir_all(&home).expect("create Home");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "controller-startup-test"]);
    git(
        &repo,
        &["config", "user.email", "controller-startup@test.invalid"],
    );
    std::fs::write(repo.join("README.md"), "controller startup\n").expect("write fixture");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "seed"]);

    let attempt_id = "work-entrypoint-failure";
    let receipt = home.join("controller/startup/work-entrypoint-failure.json");
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["__work", "task", TaskId::new().as_str()])
        .current_dir(&repo)
        .env("LF_HOME", &home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_WORK_STARTUP_ATTEMPT", attempt_id)
        .env("LF_WORK_STARTUP_RECEIPT", &receipt)
        .env_remove("LF_CONTROL_BIN")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .output()
        .expect("run hidden Work body");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("no Loopflow registry"),
        "unexpected hidden body error: {stderr}"
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&receipt).expect("hidden body persisted its startup receipt"),
    )
    .expect("parse startup receipt");
    assert_eq!(receipt["attempt_id"], attempt_id);
    assert_eq!(receipt["state"], "failed");
    assert!(receipt["reason"].as_str().is_some_and(
        |reason| reason.contains("not found") || reason.contains("no Loopflow registry")
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_project_and_task_controllers_prove_startup_and_resume() {
    let directory = tempfile::tempdir().expect("temporary controller fixture");
    let repo = directory.path().join("repo");
    let task_worktree = directory.path().join("repo.controller-startup");
    let home = directory.path().join("home");
    let bin = directory.path().join("bin");
    let tmux_state = directory.path().join("tmux");
    for path in [&repo, &home, &bin, &tmux_state] {
        std::fs::create_dir_all(path).expect("create fixture directory");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "controller-startup-test"]);
    git(
        &repo,
        &["config", "user.email", "controller-startup@test.invalid"],
    );
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/loopflowstudio/fixture.git",
        ],
    );
    std::fs::write(repo.join("README.md"), "controller startup\n").expect("write fixture");
    std::fs::create_dir_all(repo.join(".lf")).expect("create Loopflow config directory");
    std::fs::write(
        repo.join(".lf/config.yaml"),
        "pm:\n  provider: linear\n  linear_team: team-controller\n",
    )
    .expect("write repository PM config");
    std::fs::create_dir_all(repo.join("wave/controller-startup")).expect("create Wave directory");
    std::fs::write(
        repo.join("wave/controller-startup/GOAL.md"),
        "---\npm:\n  linear_initiative: initiative-controller\n---\n\n## Objective\n\nProve controller startup.\n",
    )
    .expect("write Wave goal");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "seed"]);
    git(&repo, &["branch", "jack/controller-startup"]);
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            task_worktree.to_str().expect("UTF-8 Task worktree"),
            "jack/controller-startup",
        ],
    );
    let base_commit = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .expect("read fixture head")
            .stdout,
    )
    .expect("head is UTF-8");
    let now = time::OffsetDateTime::now_utc();
    let store =
        loopflow::store::open_ephemeral_store(&StorageConfig::sqlite(home.join("loopflow.db")))
            .await
            .expect("open fixture store");
    let wave = Wave::new(
        WaveId::new(),
        "controller-startup".to_string(),
        repo.display().to_string(),
    );
    let task_project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("task-controller-startup-project").expect("Project id"),
            slug: "task-controller-startup-project".to_string(),
            name: "Task controller startup Project".to_string(),
            prompt_context: "Prove failed Task startup state.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        id: TaskId::new(),
        plan: TaskPlan {
            id: LinearIssueId::new("controller-startup-issue").expect("issue id"),
            identifier: "LOO-STARTUP".to_string(),
            title: "Prove controller startup".to_string(),
            description: "Exercise the public resume boundary.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: task_project.id.clone(),
        worktree: task_worktree.clone(),
        workspace_slug: "controller-startup".to_string(),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: "jack/controller-startup".to_string(),
        base_commit: base_commit.trim().to_string(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    store.create_wave(&wave).await.expect("create Wave fixture");
    store
        .upsert_provider_token(&ProviderToken {
            provider: "linear".to_string(),
            access_token: "linear-fixture-token".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: now.unix_timestamp(),
            credential_type: CredentialType::OAuth,
        })
        .await
        .expect("seed Home-local Linear credential");
    store
        .create_project(&task_project)
        .await
        .expect("create Task Project fixture");
    store
        .create_task(&task, &pr)
        .await
        .expect("create Task fixture");
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("controller-startup-project").expect("Project id"),
            slug: "controller-startup-project".to_string(),
            name: "Controller startup Project".to_string(),
            prompt_context: "Prove failed startup state.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .create_project(&project)
        .await
        .expect("create Project fixture");

    let account_home = home.join("accounts/codex/ready");
    std::fs::create_dir_all(&account_home).expect("create managed Codex home");
    std::fs::write(
        account_home.join("auth.json"),
        r#"{"access_token":"test-oauth-token"}"#,
    )
    .expect("seed managed Codex login");
    let account_id = ProviderAccountId::parse("ready").expect("account id");
    store
        .upsert_provider_account(&ProviderAccount {
            provider: "codex".to_string(),
            account_id: account_id.clone(),
            home: Some(account_home),
            login_email: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: now.unix_timestamp(),
            updated_at: now.unix_timestamp(),
        })
        .await
        .expect("seed managed account");
    store
        .set_provider_route(&ProviderRoute {
            scope: RouteScope::Default,
            provider: Provider::Codex,
            accounts: vec![account_id],
            created_at: now.unix_timestamp(),
            updated_at: now.unix_timestamp(),
        })
        .await
        .expect("seed default provider route");

    write_executable(
        &bin.join("codex"),
        r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r request
case "$request" in
  *account/read*)
    echo '{"jsonrpc":"2.0","id":2,"result":{"account":{}}}'
    exit 0
    ;;
esac
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-controller"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-controller"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread-controller","turn":{"id":"turn-controller","status":"inProgress"}}}'
while [ ! -f "$LF_TEST_PROVIDER_RELEASE" ]; do sleep 0.02; done
echo '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread-controller","turnId":"turn-controller","itemId":"message-controller","delta":"controller phase completed"}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-controller","turn":{"id":"turn-controller","status":"completed"}}}'
while read -r line; do :; done
"#,
    );
    let child_lf = bin.join("lf-child");
    write_executable(
        &child_lf,
        r#"#!/bin/sh
exit 7
"#,
    );
    write_executable(
        &bin.join("ps"),
        r#"#!/bin/sh
if [ -f "$LF_TEST_TMUX_STATE/fail-ps" ]; then
  echo "process inspection refused" >&2
  exit 8
fi
exec /bin/ps "$@"
"#,
    );
    write_executable(
        &bin.join("tmux"),
        r#"#!/bin/sh
case "$1" in
  has-session)
    session="${3#=}"
    marker="$LF_TEST_TMUX_STATE/$session"
    if [ -f "$marker" ] && kill -0 "$(sed -n '1p' "$marker")" 2>/dev/null; then
      exit 0
    fi
    echo "can't find session" >&2
    exit 1
    ;;
  display-message)
    session="${4#=}"
    marker="$LF_TEST_TMUX_STATE/$session"
    if [ -f "$marker" ] && kill -0 "$(sed -n '1p' "$marker")" 2>/dev/null; then
      sed -n '1p' "$marker"
      exit 0
    fi
    echo "can't find session" >&2
    exit 1
    ;;
  new-session)
    if [ -f "$LF_TEST_TMUX_STATE/fail-new-session" ]; then
      echo "tmux launch refused" >&2
      exit 9
    fi
    session="$4"
    marker="$LF_TEST_TMUX_STATE/$session"
    (
      cd "$6" || exit 1
      sleep 0.02
      exec /bin/sh -lc "$9"
      rm -f "$marker"
    ) </dev/null >/dev/null 2>&1 &
    printf '%s\n' "$!" > "$marker"
    exit 0
    ;;
  set-option)
    exit 0
    ;;
esac
exit 1
"#,
    );
    let (linear_base_url, linear_requests) = start_linear_fixture(
        project.plan.id.as_str(),
        &format!("Controller Startup — {}", project.plan.name),
    )
    .await;

    let project_output = public_lf(
        &repo,
        &home,
        &bin,
        &child_lf,
        &tmux_state,
        &linear_base_url,
        &["project", "resume", &project.plan.slug, "--json"],
    );
    assert!(!project_output.status.success());
    let project_error = String::from_utf8_lossy(&project_output.stderr);
    assert!(
        project_error.contains("controller process exited before acknowledging startup"),
        "unexpected Project error: {project_error}"
    );
    assert!(project_error.contains("controller/startup/"));
    let project_status = public_lf(
        &repo,
        &home,
        &bin,
        &child_lf,
        &tmux_state,
        &linear_base_url,
        &["project", "status", &project.plan.slug, "--json"],
    );
    assert!(project_status.status.success());
    let project_status: serde_json::Value =
        serde_json::from_slice(&project_status.stdout).expect("parse Project status");
    assert_eq!(project_status["latest_event"]["kind"]["kind"], "failed");
    assert!(project_status["last_failure"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("controller/startup/")));
    assert_eq!(project_status["controller_authority"]["state"], "inactive");

    let task_output = public_lf(
        &repo,
        &home,
        &bin,
        &child_lf,
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(!task_output.status.success());
    let task_error = String::from_utf8_lossy(&task_output.stderr);
    assert!(
        task_error.contains("controller process exited before acknowledging startup"),
        "unexpected Task error: {task_error}"
    );
    assert!(task_error.contains("controller/startup/"));
    let task_status = public_lf(
        &repo,
        &home,
        &bin,
        &child_lf,
        &tmux_state,
        &linear_base_url,
        &["task", "status", &task.plan.identifier, "--json"],
    );
    assert!(task_status.status.success());
    let task_status: serde_json::Value =
        serde_json::from_slice(&task_status.stdout).expect("parse Task status");
    assert_eq!(task_status["latest_event"]["kind"]["kind"], "failed");
    assert!(task_status["latest_event"]["kind"]["error"]
        .as_str()
        .is_some_and(|message| message.contains("controller/startup/")));
    assert_eq!(task_status["latest_event"]["kind"]["resumable"], true);
    assert_eq!(task_status["controller_authority"]["state"], "inactive");

    let fail_new_session = tmux_state.join("fail-new-session");
    std::fs::write(&fail_new_session, "fail\n").expect("reject the next tmux launch");
    let transport_failure = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(!transport_failure.status.success());
    let transport_error = String::from_utf8_lossy(&transport_failure.stderr);
    assert!(transport_error.contains("tmux failed to launch session"));
    assert!(transport_error.contains("controller/startup/"));
    let transport_receipt = std::fs::read_dir(home.join("controller/startup"))
        .expect("startup receipt directory")
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .find(|receipt| {
            receipt["state"] == "failed"
                && receipt["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("tmux failed to launch session"))
        })
        .expect("transport rejection persisted a failed startup receipt");
    let transport_attempt = transport_receipt["attempt_id"]
        .as_str()
        .expect("transport receipt attempt id");
    assert!(transport_error.contains(transport_attempt));
    let transport_status = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "status", &task.plan.identifier, "--json"],
    );
    assert!(transport_status.status.success());
    let transport_status: serde_json::Value =
        serde_json::from_slice(&transport_status.stdout).expect("parse transport failure status");
    let transport_work_error = transport_status["latest_event"]["kind"]["error"]
        .as_str()
        .expect("transport Work failure");
    assert!(transport_work_error.contains("tmux failed to launch session"));
    assert!(transport_work_error.contains(transport_attempt));
    assert_eq!(transport_status["latest_event"]["kind"]["resumable"], true);
    std::fs::remove_file(fail_new_session).expect("restore tmux launch");

    let provider_release = tmux_state.join("provider-release");
    let _ = std::fs::remove_file(&provider_release);
    let first_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(
        first_resume.status.success(),
        "first real Task resume failed: {}",
        String::from_utf8_lossy(&first_resume.stderr)
    );
    let first_receipts = running_receipts(&home, "task", task.id.as_str());
    assert_eq!(first_receipts.len(), 1);
    let first = &first_receipts[0];
    assert_exact_live_owner(&home, first);
    let first_run = first["run_id"].as_str().expect("first Run id").to_string();
    let runs = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["runs", "--task", &task.plan.identifier, "--json"],
    );
    assert!(runs.status.success());
    assert!(String::from_utf8_lossy(&runs.stdout).contains(&first_run));

    let first_pid = first["pid"].as_u64().expect("first controller pid");
    let task_tmux_marker = tmux_marker_for_pid(&tmux_state, first_pid);
    let live_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "live Task status",
    );
    assert_live_authority(&live_status, first);

    let live_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(live_resume.status.success());
    assert_eq!(
        running_receipts(&home, "task", task.id.as_str()).len(),
        1,
        "a live owner suppresses duplicate launch"
    );

    let manifest_path = run_manifest(&home, &first_run);
    let manifest = std::fs::read(&manifest_path).expect("read live Run manifest");
    std::fs::remove_file(&manifest_path).expect("remove advisory Run manifest");
    let missing_run_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "Task status without Run telemetry",
    );
    assert_live_authority(&missing_run_status, first);
    std::fs::write(&manifest_path, manifest).expect("restore advisory Run manifest");

    let exec_path = home.join(format!("runtime/exec-processes/{first_pid}.json"));
    let exec_receipt = std::fs::read(&exec_path).expect("read exact Exec receipt");
    std::fs::remove_file(&exec_path).expect("remove exact Exec receipt");
    let missing_exec_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "Task status without Exec ownership",
    );
    assert_eq!(
        missing_exec_status["controller_authority"]["state"],
        "unverifiable"
    );
    assert!(missing_exec_status["controller_authority"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("no matching Exec receipt")));
    for command in ["resume", "restart"] {
        let blocked = public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", command, &task.plan.identifier, "--json"],
        );
        assert!(!blocked.status.success(), "{command} must fail closed");
        assert!(String::from_utf8_lossy(&blocked.stderr).contains("unverifiable"));
        assert!(process_is_live(first_pid));
        assert_eq!(running_receipts(&home, "task", task.id.as_str()).len(), 1);
    }
    std::fs::write(&exec_path, &exec_receipt).expect("restore exact Exec receipt");

    let duplicate_path = home.join("controller/startup/duplicate-owner.json");
    let mut duplicate = first.clone();
    duplicate["attempt_id"] = serde_json::Value::String("duplicate-owner".to_string());
    std::fs::write(
        &duplicate_path,
        serde_json::to_vec(&duplicate).expect("serialize duplicate owner"),
    )
    .expect("write duplicate owner");
    let duplicate_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "Task status with duplicate owner",
    );
    assert_eq!(
        duplicate_status["controller_authority"]["state"],
        "unverifiable"
    );
    assert!(duplicate_status["controller_authority"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("multiple live controller owners")));
    std::fs::remove_file(&duplicate_path).expect("remove duplicate owner");

    let fail_ps = tmux_state.join("fail-ps");
    std::fs::write(&fail_ps, "fail\n").expect("fail OS inspection");
    let failed_os_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "Task status with failed OS inspection",
    );
    assert_eq!(
        failed_os_status["controller_authority"]["state"],
        "unverifiable"
    );
    assert!(failed_os_status["controller_authority"]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("ps failed")));
    std::fs::remove_file(&fail_ps).expect("restore OS inspection");

    stop_process_tree(first_pid);
    let inactive_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "inactive Task status",
    );
    assert_eq!(inactive_status["controller_authority"]["state"], "inactive");
    assert!(inactive_status["controller"]["provider_session_id"].is_string());

    let mut unrelated = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("start unrelated tmux occupant");
    std::fs::write(&task_tmux_marker, format!("{}\n", unrelated.id()))
        .expect("point Task transport at unrelated process");
    let unowned_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "Task status with unowned tmux transport",
    );
    assert_eq!(
        unowned_status["controller_authority"]["state"],
        "unverifiable"
    );
    for args in [
        vec!["task", "resume", &task.plan.identifier, "--json"],
        vec![
            "task",
            "steer",
            &task.plan.identifier,
            "blocked steer",
            "--json",
        ],
        vec!["task", "interrupt", &task.plan.identifier, "--json"],
    ] {
        let blocked = public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &args,
        );
        assert!(!blocked.status.success());
        assert!(String::from_utf8_lossy(&blocked.stderr).contains("unverifiable"));
        assert!(process_is_live(u64::from(unrelated.id())));
    }
    unrelated.kill().expect("stop unrelated tmux occupant");
    unrelated.wait().expect("reap unrelated tmux occupant");
    std::fs::remove_file(&task_tmux_marker).expect("remove unowned tmux transport");

    let parked_path = home.join("controller/startup/parked-task.json");
    let parked_observed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format parked observation");
    std::fs::write(
        &parked_path,
        serde_json::to_vec(&json!({
            "attempt_id": "parked-task",
            "observed_at": parked_observed_at,
            "state": "parked",
            "work": { "kind": "task", "id": task.id.as_str() }
        }))
        .expect("serialize parked Task receipt"),
    )
    .expect("write parked Task receipt");
    let parked_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["task", "status", &task.plan.identifier, "--json"],
        ),
        "parked Task status",
    );
    assert_eq!(parked_status["controller_authority"]["state"], "parked");
    let parked_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(!parked_resume.status.success());
    assert!(String::from_utf8_lossy(&parked_resume.stderr).contains("parked"));
    std::fs::remove_file(&parked_path).expect("remove parked Task receipt");

    let second_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["task", "resume", &task.plan.identifier, "--json"],
    );
    assert!(
        second_resume.status.success(),
        "Task did not resume after an intentional stop: {}",
        String::from_utf8_lossy(&second_resume.stderr)
    );
    let second_receipts = running_receipts(&home, "task", task.id.as_str());
    assert_eq!(second_receipts.len(), 2);
    let second = second_receipts
        .iter()
        .find(|receipt| receipt["run_id"].as_str() != Some(&first_run))
        .expect("a fresh Run acknowledges the resumed controller");
    assert_exact_live_owner(&home, second);
    let second_pid = second["pid"].as_u64().expect("second controller pid");

    std::fs::write(&provider_release, "complete\n").expect("release provider turn");
    let advanced =
        wait_for_task_cursor(&repo, &home, &bin, &tmux_state, &linear_base_url, &task, 1);
    assert_eq!(advanced["controller"]["lifecycle_phase"], "first");
    if Command::new("kill")
        .args(["-0", &second_pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe second controller process")
        .success()
    {
        stop_process_tree(second_pid);
    }

    std::fs::remove_file(&provider_release).expect("reset provider for Project start");
    let first_project_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["project", "resume", &project.plan.slug, "--json"],
    );
    assert!(
        first_project_resume.status.success(),
        "first real Project resume failed after {} Linear requests: {}",
        linear_requests.load(Ordering::SeqCst),
        String::from_utf8_lossy(&first_project_resume.stderr),
    );
    let first_project_receipts = running_receipts(&home, "project", project.id.as_str());
    assert_eq!(first_project_receipts.len(), 1);
    let first_project = &first_project_receipts[0];
    assert_exact_live_owner(&home, first_project);
    let first_project_run = first_project["run_id"]
        .as_str()
        .expect("first Project Run id")
        .to_string();
    let project_runs = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["runs", "--project", &project.plan.slug, "--json"],
    );
    assert!(project_runs.status.success());
    assert!(String::from_utf8_lossy(&project_runs.stdout).contains(&first_project_run));
    assert_eq!(linear_requests.load(Ordering::SeqCst), 3);

    let first_project_pid = first_project["pid"]
        .as_u64()
        .expect("first Project controller pid");
    let project_tmux_marker = tmux_marker_for_pid(&tmux_state, first_project_pid);
    let live_project_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        ),
        "live Project status",
    );
    assert_live_authority(&live_project_status, first_project);

    let project_manifest_path = run_manifest(&home, &first_project_run);
    let project_manifest =
        std::fs::read(&project_manifest_path).expect("read live Project Run manifest");
    std::fs::remove_file(&project_manifest_path).expect("remove Project Run telemetry");
    let missing_project_run_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        ),
        "Project status without Run telemetry",
    );
    assert_live_authority(&missing_project_run_status, first_project);
    std::fs::write(&project_manifest_path, project_manifest)
        .expect("restore Project Run telemetry");

    let project_exec_path = home.join(format!("runtime/exec-processes/{first_project_pid}.json"));
    let project_exec =
        std::fs::read(&project_exec_path).expect("read Project Exec ownership receipt");
    std::fs::remove_file(&project_exec_path).expect("remove Project Exec ownership receipt");
    let missing_project_exec_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        ),
        "Project status without Exec ownership",
    );
    assert_eq!(
        missing_project_exec_status["controller_authority"]["state"],
        "unverifiable"
    );
    for args in [
        vec!["project", "resume", &project.plan.slug, "--json"],
        vec!["project", "attach", &project.plan.slug],
    ] {
        let blocked = public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &args,
        );
        assert!(!blocked.status.success());
        assert!(String::from_utf8_lossy(&blocked.stderr).contains("unverifiable"));
        assert!(process_is_live(first_project_pid));
    }
    std::fs::write(&project_exec_path, project_exec)
        .expect("restore Project Exec ownership receipt");

    stop_process_tree(first_project_pid);
    let inactive_project_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        ),
        "inactive Project status",
    );
    assert_eq!(
        inactive_project_status["controller_authority"]["state"],
        "inactive"
    );

    let mut unrelated_project = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("start unrelated Project tmux occupant");
    std::fs::write(
        &project_tmux_marker,
        format!("{}\n", unrelated_project.id()),
    )
    .expect("point Project transport at unrelated process");
    let unowned_project_status = successful_json(
        &public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &["project", "status", &project.plan.slug, "--json"],
        ),
        "Project status with unowned tmux transport",
    );
    assert_eq!(
        unowned_project_status["controller_authority"]["state"],
        "unverifiable"
    );
    for args in [
        vec!["project", "resume", &project.plan.slug, "--json"],
        vec![
            "project",
            "steer",
            &project.plan.slug,
            "blocked steer",
            "--json",
        ],
        vec!["project", "interrupt", &project.plan.slug, "--json"],
        vec!["project", "attach", &project.plan.slug],
    ] {
        let blocked = public_lf(
            &repo,
            &home,
            &bin,
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &tmux_state,
            &linear_base_url,
            &args,
        );
        assert!(!blocked.status.success());
        assert!(String::from_utf8_lossy(&blocked.stderr).contains("unverifiable"));
        assert!(process_is_live(u64::from(unrelated_project.id())));
    }
    unrelated_project
        .kill()
        .expect("stop unrelated Project tmux occupant");
    unrelated_project
        .wait()
        .expect("reap unrelated Project tmux occupant");
    std::fs::remove_file(&project_tmux_marker).expect("remove unowned Project transport");

    let second_project_resume = public_lf(
        &repo,
        &home,
        &bin,
        Path::new(env!("CARGO_BIN_EXE_lf")),
        &tmux_state,
        &linear_base_url,
        &["project", "resume", &project.plan.slug, "--json"],
    );
    assert!(
        second_project_resume.status.success(),
        "Project did not resume after an intentional stop: {}",
        String::from_utf8_lossy(&second_project_resume.stderr)
    );
    let second_project_receipts = running_receipts(&home, "project", project.id.as_str());
    assert_eq!(second_project_receipts.len(), 2);
    let second_project = second_project_receipts
        .iter()
        .find(|receipt| receipt["run_id"].as_str() != Some(&first_project_run))
        .expect("a fresh Run acknowledges the resumed Project controller");
    assert_exact_live_owner(&home, second_project);
    let second_project_pid = second_project["pid"]
        .as_u64()
        .expect("second Project controller pid");
    assert_eq!(linear_requests.load(Ordering::SeqCst), 6);

    std::fs::write(&provider_release, "complete\n").expect("release Project provider turn");
    let completed = wait_for_project_iteration(
        &repo,
        &home,
        &bin,
        &tmux_state,
        &linear_base_url,
        &project,
        1,
    );
    assert_eq!(completed["status"], "done");
    if Command::new("kill")
        .args(["-0", &second_project_pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe second Project controller process")
        .success()
    {
        stop_process_tree(second_project_pid);
    }
}
