mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::flow::compile_flow;
use loopflow::flow::load_flow;
use loopflow::flow::{ConcreteStep, Skill, Step};
use support::{codex_app_server_script, register_codex_account};
use tempfile::TempDir;

fn session_id_for_capture(home: &Path, artifact: &str) -> String {
    rusqlite::Connection::open_with_flags(
        home.join("loopflow.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .query_row(
        "SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
        [artifact],
        |row| row.get(0),
    )
    .unwrap()
}

fn write_skill(repo: &Path, name: &str, content: &str) {
    let skills_dir = repo.join(".lf/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_flow(repo: &Path, name: &str, content: &str) {
    let flows_dir = repo.join(".lf/flows");
    fs::create_dir_all(&flows_dir).unwrap();
    fs::write(flows_dir.join(format!("{name}.yaml")), content).unwrap();
}

fn lf_json(repo: &Path, home: &Path, args: &[&str]) -> serde_json::Value {
    let output = run_lf(repo, home, args, None);
    assert!(
        output.status.success(),
        "lf {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Every Flow on this Machine as `lf flow show --processes --json` reads it back.
fn flow_details(repo: &Path, home: &Path) -> Vec<serde_json::Value> {
    lf_json(
        repo,
        home,
        &["flow", "list", "--processes", "--all", "--json"],
    )["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let id = entry["id"].as_str().unwrap();
            lf_json(repo, home, &["flow", "show", id, "--processes", "--json"])
        })
        .collect()
}

/// One field of each recorded step, in launch order.
fn step_fields(flow: &serde_json::Value, field: &str) -> Vec<serde_json::Value> {
    flow["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step[field].clone())
        .collect()
}

/// Wait for a condition another process brings about; a bound, never a retry.
fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !ready() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn flow_steps_use_explicit_binary_and_retain_effects_after_experimental_schema_changes() {
    for upgrade in [false, true] {
        let repo = loopflow_test_support::TestRepo::new();
        let home = TempDir::new().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();
        write_flow(
            repo.path(),
            "path-proof",
            "- cmd: sync --plan\n- cmd: sync --plan\n",
        );
        // An explicitly selected executable delegates the effect to lf, then
        // changes the experimental schema so subsequent opens must refuse it.
        let migration = if upgrade {
            r#"python3 - <<'PYTHON'
import os
import sqlite3
with sqlite3.connect(os.path.join(os.environ["LF_HOME"], "loopflow.db")) as connection:
    connection.executescript("""
BEGIN IMMEDIATE;
CREATE TABLE future_flow_feature (id INTEGER PRIMARY KEY);
COMMIT;
""")
PYTHON
"#
        } else {
            ""
        };
        write_executable(
            &bin.join("lf"),
            &format!(
                "#!/bin/sh\nset -eu\necho selected >> '{}'\n'{}' \"$@\"\n{migration}",
                home.path().join("selected").display(),
                env!("CARGO_BIN_EXE_lf")
            ),
        );
        let output = lf_command(
            repo.path(),
            home.path(),
            &["flow", "path-proof", "--batch", "--no-loopflow"],
            None,
        )
        .env("LF_BIN", bin.join("lf"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
        assert_eq!(
            output.status.success(),
            !upgrade,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Raw reads: the changed schema refuses every lf, old or new.
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let flows = support::recorded_flows(home.path());
        assert_eq!(flows.len(), 1);
        let (outcome, steps) = &flows[0];
        assert_eq!(steps.len(), if upgrade { 1 } else { 2 });
        let executables: Vec<String> = conn
            .prepare(
                "SELECT json_extract(e.command,'$[0]') FROM processes e
                 JOIN flow_process_steps s ON s.lf_process_id=e.id WHERE e.outcome='succeeded'",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            executables,
            vec![env!("CARGO_BIN_EXE_lf"); steps.len()],
            "the wrapper delegated each step to these actual bytes"
        );
        assert_eq!(
            fs::read_to_string(home.path().join("selected"))
                .unwrap()
                .lines()
                .count(),
            steps.len()
        );
        if !upgrade {
            assert_eq!(outcome.as_deref(), Some("succeeded"));
            continue;
        }
        // The completed effect stays recorded; its successor never launched.
        assert_ne!(outcome.as_deref(), Some("succeeded"));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cannot read the result of"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let driver: String = conn
            .query_row("SELECT lf_process_id FROM flow_processes", [], |row| {
                row.get(0)
            })
            .unwrap();
        let inspection = run_lf(
            repo.path(),
            home.path(),
            &["flow", "show", &driver, "--processes", "--json"],
            None,
        );
        assert!(
            !inspection.status.success(),
            "an older executable must still refuse the new store"
        );
    }
}

#[test]
fn mechanical_failure_retains_earlier_step_success() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    write_flow(
        repo.path(),
        "mechanical-failure",
        "- cmd: sync --plan\n- cmd: __telemetry-scorecard\n",
    );
    let output = run_lf(
        repo.path(),
        home.path(),
        &["flow", "mechanical-failure", "--batch", "--no-loopflow"],
        None,
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("telemetry scorecard generator not found"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let flows = flow_details(repo.path(), home.path());
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0]["entry"]["state"], "stopped");
    assert_eq!(
        step_fields(&flows[0], "label"),
        ["sync --plan", "__telemetry-scorecard"]
    );
    assert_eq!(step_fields(&flows[0], "outcome"), ["succeeded", "failed"]);
    let sessions = lf_json(
        repo.path(),
        home.path(),
        &["session", "list", "--all", "--history", "--json"],
    );
    assert_eq!(sessions, serde_json::json!([]));
}

#[test]
fn ordinary_flow_crash_retains_effects_without_repository_restart() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let scripts = repo.path().join("scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(
        scripts.join("lifecycle_scorecard.py"),
        r#"import json
import pathlib
import sys
import time
repo = pathlib.Path(sys.argv[2])
with repo.joinpath("effects").open("a") as log:
    log.write("started\n")
repo.joinpath("entered").touch()
deadline = time.monotonic() + 30
while not repo.joinpath("release").exists():
    if time.monotonic() > deadline:
        raise RuntimeError("fixture release timed out")
    time.sleep(0.02)
print(json.dumps({"report": {}, "metric_observations": [], "text": "finished"}))
"#,
    )
    .unwrap();
    write_flow(
        repo.path(),
        "survive",
        "- cmd: __telemetry-scorecard\n- cmd: sync --plan\n",
    );
    let mut driver = lf_command(
        repo.path(),
        home.path(),
        &["--batch", "flow", "survive"],
        None,
    )
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("effect never started", || {
        assert!(
            driver.try_wait().unwrap().is_none(),
            "driver exited before the effect"
        );
        repo.path().join("entered").exists()
    });
    driver.kill().unwrap();
    driver.wait().unwrap();
    let read = || {
        let mut flows = flow_details(repo.path(), home.path());
        assert_eq!(flows.len(), 1);
        flows.remove(0)
    };
    // Reconciliation before and after the surviving effect completes must not
    // continue the dead driver's Flow or execute its successor.
    for effect_finished in [false, true] {
        if effect_finished {
            fs::write(repo.path().join("release"), "").unwrap();
            wait_for("the surviving effect never recorded its exit", || {
                step_fields(&read(), "outcome") == ["succeeded"]
            });
        }
        let check = run_lf(
            repo.path(),
            home.path(),
            &["task", "reconcile", "--json"],
            None,
        );
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("effects")).unwrap(),
            "started\n"
        );
        let flow = read();
        assert_eq!(
            step_fields(&flow, "label"),
            ["__telemetry-scorecard"],
            "inspection must not launch the successor"
        );
        assert_ne!(
            flow["entry"]["state"], "completed",
            "an effect's exit does not finish its dead driver"
        );
    }
    let sessions = lf_json(
        repo.path(),
        home.path(),
        &["session", "list", "--all", "--history", "--json"],
    );
    assert_eq!(sessions, serde_json::json!([]));
}

fn expand_named_flow(repo: &Path, name: &str) -> Vec<ConcreteStep> {
    let flow = load_flow(name, repo).unwrap();
    compile_flow(&flow, repo).unwrap()
}

fn assert_skill_name(item: &ConcreteStep, expected: &str) {
    match item {
        ConcreteStep::Skill(skill) => assert_eq!(skill.skill.name, expected),
        other => panic!("expected skill {expected}, got {other:?}"),
    }
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn run_lf(repo: &Path, home: &Path, args: &[&str], path: Option<&str>) -> std::process::Output {
    lf_command(repo, home, args, path).output().unwrap()
}

fn lf_command(repo: &Path, home: &Path, args: &[&str], path: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(repo)
        .env("HOME", home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("NO_COLOR", "1");
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_lf")).parent().unwrap();
    command.env(
        "PATH",
        format!(
            "{}:{}",
            binary_dir.display(),
            path.map(str::to_owned)
                .unwrap_or_else(|| std::env::var("PATH").unwrap())
        ),
    );
    command
}

/// Record the planning a Task launch requires: its issue observed at a revision.
fn observe_planning(task: &support::RegisteredTask, checkout: &Path) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let scope = checkout.display().to_string();
    let mut record = runtime
        .block_on(task.store.pm_task_observation(&scope, "linear", "INF-123"))
        .unwrap()
        .record
        .unwrap();
    record.item.revision = Some("2026-10-04T12:00:00Z".into());
    runtime
        .block_on(task.store.put_pm_task(&scope, "linear", record, None, None))
        .unwrap();
}

fn publish_stack_fixture_pr(
    runtime: &tokio::runtime::Runtime,
    store: &loopflow::store::Store,
    task: &loopflow::work::task::TaskId,
) -> loopflow::work::task::TaskPr {
    let mut pr = runtime
        .block_on(store.active_task_pr(task))
        .unwrap()
        .unwrap_or_else(|| {
            let task = runtime.block_on(store.get_task(task)).unwrap().unwrap();
            let pr = loopflow::work::task::TaskPr {
                id: loopflow::work::task::TaskPrId::new(),
                task_id: task.id,
                sequence: 1,
                slug: task.workspace_slug,
                branch: task.branch,
                base_commit: task.base_commit,
                parent_pr_id: task.parent_pr_id,
                publication: None,
                merge_commit: None,
                abandoned_at: None,
                ci_observation: None,
                github_observation: None,
                linear_attachment_id: None,
                linear_comment_id: None,
                linear_link_error: None,
                created_at: task.created_at,
                updated_at: task.updated_at,
            };
            runtime.block_on(store.insert_task_pr(&pr)).unwrap();
            pr
        });
    pr.publication = Some(loopflow::work::task::PrPublication {
        requested_at: pr.created_at,
        presentation: None,
        github: Some(loopflow::work::task::GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime.block_on(store.update_task_pr(&pr)).unwrap();
    runtime
        .block_on(store.append_task_event(
            task,
            &loopflow::work::task::TaskEventKind::PrStarted {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                branch: pr.branch.clone(),
                base_commit: pr.base_commit.clone(),
            },
        ))
        .unwrap();
    pr
}

#[test]
fn task_checkout_selects_parent_without_rewriting_work_or_publication() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let child =
        support::register_task_with_pr(home.path(), repo.path(), "child-task", &repo.head_sha());
    let parent_path = repo.create_named_worktree("parent-task");
    let parent = support::register_sibling_task(&child, "INF-124", "parent-task", &parent_path);
    repo.create_branch("child-task");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let task_before = runtime
        .block_on(child.store.get_task(&child.task.id))
        .unwrap()
        .unwrap();
    let before = publish_stack_fixture_pr(&runtime, &child.store, &child.task.id);
    let parent_pr = publish_stack_fixture_pr(&runtime, &child.store, &parent.id);
    repo.create_file("committed.txt", "child-authored commit");
    repo.stage_all();
    repo.commit("Child work before selecting its parent");
    repo.create_file("staged.txt", "staged bytes");
    repo.stage_all();
    repo.create_file("committed.txt", "unstaged bytes");
    repo.create_file("untracked.txt", "untracked bytes");
    let head = repo.head_sha();
    let index = Command::new("git")
        .args(["diff", "--cached", "--binary"])
        .current_dir(repo.path())
        .output()
        .unwrap()
        .stdout;
    let events = runtime
        .block_on(child.store.task_events_after(&child.task.id, 0))
        .unwrap();
    for _ in 0..2 {
        let output = lf_command(
            repo.path(),
            home.path(),
            &[
                "task",
                "checkout",
                "INF-123",
                "--stack-on",
                "INF-124",
                "--json",
            ],
            None,
        )
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let returned: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(returned.is_object());
        let after = runtime
            .block_on(child.store.active_task_pr(&child.task.id))
            .unwrap()
            .unwrap();
        let mut expected = before.clone();
        expected.parent_pr_id = Some(parent_pr.id.clone());
        expected.updated_at = after.updated_at;
        assert_eq!(after, expected);
        let task_after = runtime
            .block_on(child.store.get_task(&child.task.id))
            .unwrap()
            .unwrap();
        let mut expected_task = task_before.clone();
        expected_task.parent_pr_id = Some(parent_pr.id.clone());
        expected_task.updated_at = task_after.updated_at;
        assert_eq!(task_after, expected_task);
        assert_eq!(
            runtime
                .block_on(child.store.task_events_after(&child.task.id, 0))
                .unwrap(),
            events
        );
    }
    let selected = runtime
        .block_on(child.store.active_task_pr(&child.task.id))
        .unwrap();
    let failed = lf_command(
        repo.path(),
        home.path(),
        &[
            "task",
            "prepare",
            "INF-123",
            "--stack-on",
            "INF-404",
            "--json",
        ],
        None,
    )
    .output()
    .unwrap();
    assert!(!failed.status.success());
    assert_eq!(
        runtime
            .block_on(child.store.active_task_pr(&child.task.id))
            .unwrap(),
        selected
    );
    // A successful publication observed from an older snapshot must survive,
    // while the dedicated parent writer retains the newer dependency.
    let mut publication = before.clone();
    publication
        .publication
        .as_mut()
        .unwrap()
        .github
        .as_mut()
        .unwrap()
        .head_sha = Some("published-after-selection".into());
    publication.linear_attachment_id = Some("linked-after-selection".into());
    runtime
        .block_on(child.store.update_task_pr(&publication))
        .unwrap();
    let recorded = runtime
        .block_on(child.store.get_task_pr(&before.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        recorded.parent_pr_id,
        selected.as_ref().unwrap().parent_pr_id
    );
    assert_eq!(recorded.publication, publication.publication);
    assert_eq!(
        recorded.linear_attachment_id,
        publication.linear_attachment_id
    );
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        fs::read_to_string(repo.path().join("committed.txt")).unwrap(),
        "unstaged bytes"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
        "staged bytes"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
        "untracked bytes"
    );
    assert_eq!(
        Command::new("git")
            .args(["diff", "--cached", "--binary"])
            .current_dir(repo.path())
            .output()
            .unwrap()
            .stdout,
        index
    );
    assert_eq!(
        runtime
            .block_on(child.store.get_task_pr(&parent_pr.id))
            .unwrap()
            .unwrap(),
        parent_pr
    );
}

#[test]
fn checkout_task_identity_ignores_main_and_parent_upstreams() {
    for upstream in ["main", "parent-task"] {
        let repo = loopflow_test_support::TestRepo::new();
        support::bind_task_planning(&repo);
        let home = TempDir::new().unwrap();
        let checkout = repo.path().canonicalize().unwrap();
        let child =
            support::register_task_with_pr(home.path(), &checkout, "child-task", &repo.head_sha());
        observe_planning(&child, &checkout);
        let parent_path = repo.create_named_worktree("parent-task");
        let parent = support::register_sibling_task(&child, "INF-124", "parent-task", &parent_path);
        // Both tracking configurations are real Git refs, with no network.
        run_git(repo.path(), &["push", "origin", "parent-task"]);
        repo.create_branch("child-task");
        run_git(
            repo.path(),
            &["branch", "--set-upstream-to", &format!("origin/{upstream}")],
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        if upstream == "parent-task" {
            let parent_pr = publish_stack_fixture_pr(&runtime, &child.store, &parent.id);
            runtime
                .block_on(child.store.stack_task_placement(&child.task, &parent_pr.id))
                .unwrap();
        }
        write_skill(repo.path(), "identity-proof", "Prove checkout identity.");
        write_flow(repo.path(), "identity-proof", "- cmd: sync --plan\n");
        let parent_before = runtime
            .block_on(child.store.get_task(&parent.id))
            .unwrap()
            .unwrap();
        // A Flow process from the checkout is this Task's work, never its upstream's.
        let ran = run_lf(
            repo.path(),
            home.path(),
            &["--batch", "flow", "identity-proof"],
            None,
        );
        assert!(
            ran.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let flows = |issue: &str| {
            let status = lf_json(
                repo.path(),
                home.path(),
                &["task", "status", issue, "--json"],
            );
            status["execution"]["work"]["flow_processes"]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!((flows("INF-123"), flows("INF-124")), (1, 0), "{upstream}");

        // A subdirectory still resolves the registered checkout.
        let subdir = repo.path().join("nested");
        fs::create_dir(&subdir).unwrap();
        let status = run_lf(&subdir, home.path(), &["task", "status", "--json"], None);
        assert!(
            status.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(
            status["execution"]["task_id"],
            child.task.id.as_str(),
            "{status}"
        );

        let bin = TempDir::new().unwrap();
        let launched = bin.path().join("launched");
        write_executable(&bin.path().join("codex"), &format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nif [ \"$1\" = --dangerously-bypass-hook-trust ] && [ \"$2\" = --model ]; then echo \"a value is required for '--model <MODEL>'\" >&2; exit 2; fi\nprintf '%s' '{{\"session_id\":\"ses-'\"$LF_CAPTURE_KEY\"'\"}}' | \"$LF_BIN\" __provider-session || exit $?\necho \"$LF_CAPTURE_KEY\" > '{}'\n", launched.display(),
        ));
        let path = format!(
            "{}:{}",
            bin.path().display(),
            std::env::var("PATH").unwrap()
        );
        let launch = run_lf(
            repo.path(),
            home.path(),
            &["-i", "skill", "identity-proof", "--no-loopflow"],
            Some(&path),
        );
        assert!(
            launch.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&launch.stderr)
        );
        let run_id = fs::read_to_string(launched).unwrap();
        let sessions = run_lf(
            repo.path(),
            home.path(),
            &["session", "list", "--all", "--json"],
            None,
        );
        assert!(
            sessions.status.success(),
            "{}",
            String::from_utf8_lossy(&sessions.stderr)
        );
        let sessions: serde_json::Value = serde_json::from_slice(&sessions.stdout).unwrap();
        let session = sessions
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == session_id_for_capture(home.path(), run_id.trim()))
            .unwrap();
        assert_eq!(
            session["work"],
            serde_json::json!({"kind": "task", "id": child.task.id})
        );
        assert_eq!(
            runtime
                .block_on(child.store.get_task(&parent.id))
                .unwrap()
                .unwrap(),
            parent_before
        );
    }
}

#[test]
fn flow_parsing_parity() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "sample",
        r#"
- implement
- step:
    name: review
"#,
    );

    let flow = load_flow("sample", repo).unwrap();
    assert_eq!(flow.name, "sample");
    assert_eq!(flow.items.len(), 2);
    assert!(
        matches!(&flow.items[0].target, loopflow::definition::Target::Skill(skill) if skill.name == "implement")
    );
    assert_eq!(
        flow.items[1],
        Step {
            target: loopflow::definition::Target::Skill(Skill {
                source: None,
                name: "review".to_string(),
                agent: None,
                default_agent: None,
                action_style: None,
                content: None,
            }),
            id: None,
            human: false,
            returns: None,
        }
    );
}

#[test]
fn interactive_flow_keeps_input_and_advances_only_after_each_provider_exits() {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::process::Stdio;

    for (flags, stop) in [
        (vec!["-i", "run", "conversation"], false),
        (vec!["run", "conversation", "-i"], false),
        (vec!["run", "conversation"], false),
        (vec!["-i", "run", "conversation"], true),
    ] {
        let repo = loopflow_test_support::TestRepo::new();
        let home = TempDir::new().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();
        for name in ["first", "second"] {
            write_skill(repo.path(), name, "Ask for input, then exit.");
        }
        write_flow(repo.path(), "conversation", "- first\n- second\n");
        write_executable(
            &bin.join("claude"),
            r#"#!/bin/sh
set -eu
if [ "${1-}" = --version ]; then echo '2.1.0 (fixture)'; exit 0; fi
context_file=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --print|--output-format) echo 'unexpected headless launch' >&2; exit 1;;
        --append-system-prompt-file) context_file="$2"; shift;;
    esac
    shift
done
case "$(cat "$context_file")" in
    *'<lf:skill:first>'*) step=first;;
    *'<lf:skill:second>'*) step=second;;
    *) exit 2;;
esac
touch "$LF_HOME/$step.ready"
IFS= read -r answer
printf '%s' "$answer" > "$LF_HOME/$step.answer"
[ "$answer" != stop ] || exit 130
"#,
        );

        let mut master = -1;
        let mut slave = -1;
        // SAFETY: valid output pointers; null selects default terminal settings.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: openpty returned two fresh independently owned descriptors.
        let mut master = unsafe { fs::File::from_raw_fd(master) };
        // SAFETY: slave is the other fresh descriptor returned by openpty.
        let slave = unsafe { fs::File::from_raw_fd(slave) };
        let log_path = home.path().join("output");
        let log = fs::File::create(&log_path).unwrap();
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let mut child = lf_command(repo.path(), home.path(), &flags, Some(&path))
            .args(["-a", "claude", "--no-loopflow"])
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::from(slave))
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut wait = |ready: &dyn Fn() -> bool| {
            while !ready() {
                let exited = child.try_wait().unwrap();
                if exited.is_some() || std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "{flags:?}: {exited:?}\n{}",
                        fs::read_to_string(&log_path).unwrap()
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        wait(&|| home.path().join("first.ready").exists());
        assert!(!home.path().join("second.ready").exists());
        master
            .write_all(if stop { b"stop\n" } else { b"blue\n" })
            .unwrap();
        if !stop {
            wait(&|| home.path().join("second.ready").exists());
            assert_eq!(
                fs::read_to_string(home.path().join("first.answer")).unwrap(),
                "blue"
            );
            master.write_all(b"green\n").unwrap();
            wait(&|| home.path().join("second.answer").exists());
        }
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() > deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!(
                    "Flow did not finish: {}",
                    fs::read_to_string(&log_path).unwrap()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(
            status.success(),
            !stop,
            "{}",
            fs::read_to_string(log_path).unwrap()
        );
        if stop {
            assert!(!home.path().join("second.ready").exists());
        } else {
            assert_eq!(
                fs::read_to_string(home.path().join("second.answer")).unwrap(),
                "green"
            );
        }
    }
}

#[test]
fn authored_flow_records_each_skill_as_one_session() {
    let repo = TempDir::new().unwrap();
    run_git(repo.path(), &["init", "-b", "main"]);
    run_git(repo.path(), &["config", "user.email", "test@example.com"]);
    run_git(repo.path(), &["config", "user.name", "Test"]);
    for skill in ["implement", "compress"] {
        write_skill(repo.path(), skill, &format!("Run the {skill} step."));
    }
    write_flow(repo.path(), "two-skills", "- implement\n- compress\n");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "fixture"]);

    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script("done", ""),
    );
    let path = std::env::var("PATH")
        .map(|path| format!("{}:{path}", bin.path().display()))
        .unwrap_or_else(|_| bin.path().display().to_string());

    let output = run_lf(
        repo.path(),
        home.path(),
        &["two-skills", "--batch", "--no-loopflow"],
        Some(&path),
    );
    assert!(
        output.status.success(),
        "two-skills failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_lf(
        repo.path(),
        home.path(),
        &["monitor", "usage", "--days", "7", "--json"],
        None,
    );
    assert!(
        output.status.success(),
        "lf runs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut skills = runs
        .iter()
        .filter_map(|run| run["skill"].as_str())
        .collect::<Vec<_>>();
    skills.sort_unstable();
    assert_eq!(skills, ["compress", "implement"]);
    assert!(runs
        .iter()
        .all(|run| run["recorded_outcome"] == "completed"));
    let sessions = lf_json(
        repo.path(),
        home.path(),
        &[
            "session",
            "list",
            "--all",
            "--interactive",
            "false",
            "--history",
            "--json",
        ],
    );
    let mut titles = sessions
        .as_array()
        .unwrap()
        .iter()
        .map(|session| session["title"].as_str().unwrap())
        .collect::<Vec<_>>();
    titles.sort_unstable();
    assert_eq!(titles, ["compress", "implement"]);
}

#[test]
fn flow_output_shows_steps_and_agent_messages_with_opt_in_diagnostics() {
    let repo = loopflow_test_support::TestRepo::new();
    // Even a skill named default obeys the Flow's headless execution mode.
    for name in ["default", "work"] {
        write_skill(repo.path(), name, "private-instructions-marker");
    }
    write_flow(
        repo.path(),
        "readable",
        "- default\n- cmd: flow list\n- work\n",
    );
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script("agent-readable-text", ""),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    for (verbose, log_filter) in [
        (false, None),
        (true, None),
        (true, Some("loopflow=trace,lf=trace")),
    ] {
        let home = TempDir::new().unwrap();
        let mut args = vec![
            "--batch",
            "--no-loopflow",
            "-a",
            "codex",
            "run",
            "readable",
            "private-message-marker",
        ];
        if verbose {
            args.insert(0, "--verbose");
        }
        let mut command = lf_command(repo.path(), home.path(), &args, Some(&path));
        command.env_remove("RUST_LOG");
        if let Some(filter) = log_filter {
            command.env("RUST_LOG", filter);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("agent-readable-text"));
        assert!(stderr.contains("  default"), "{stderr}");
        assert!(stderr.contains("  cmd: flow list"), "{stderr}");
        assert!(stderr.contains("  work"), "{stderr}");
        assert_eq!(stderr.contains("INFO"), verbose, "{stderr}");
        assert_eq!(stderr.contains("total"), verbose, "{stderr}");
        assert!(!stderr.contains("private-message-marker"), "{stderr}");
        assert!(!stderr.contains("private-instructions-marker"), "{stderr}");
    }
}

#[test]
fn operational_flow_rejects_review_before_launch_or_capture() {
    for source in [
        "- step:
    id: review
    name: review-proof
    human: true
",
        "- flow: nested-review
",
        "- xor:\n    paths:\n      review:\n        flow: nested-review\n        description: Discuss work\n      work:\n        skill: review-proof\n        description: Autonomous work\n",
    ] {
        let repo = loopflow_test_support::TestRepo::new();
        let home = TempDir::new().unwrap();
        write_skill(repo.path(), "review-proof", "Discuss the work.");
        write_flow(
            repo.path(),
            "nested-review",
            "- step:
    id: review
    name: review-proof
    human: true
",
        );
        write_flow(repo.path(), "operational", source);
        let output = run_lf(
            repo.path(),
            home.path(),
            &["--batch", "flow", "operational"],
            None,
        );
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("belongs in the Task conversation"),
            "{output:?}"
        );
        assert!(
            support::recorded_flows(home.path()).is_empty(),
            "a rejected definition launched a step"
        );
        let sessions: i64 = rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sessions, 0, "a rejected definition opened a Session");
    }
}

#[test]
fn killed_driver_leaves_its_agent_step_as_history_without_another_turn() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    write_skill(repo.path(), "work", "Finish the selected work.");
    write_skill(repo.path(), "after", "Never reached.");
    write_flow(repo.path(), "survive-agent", "- work\n- after\n");
    let provider = codex_app_server_script("done", "if [ \"$1\" = --version ]; then exit 0; fi").replace(
        "read -r turn_start",
        "read -r turn_start\nprintf 'turn\\n' >> \"$LF_HOME/turns\"",
    ).replace(
        "printf '%s\\n'",
        "touch \"$LF_HOME/entered\"\ni=0\nwhile [ ! -e \"$LF_HOME/release\" ]; do\n  i=$((i+1)); [ \"$i\" -lt 300 ] || exit 1\n  sleep 0.1\ndone\nprintf '%s\\n'",
    );
    write_executable(&bin.path().join("codex"), &provider);
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut driver = lf_command(
        repo.path(),
        home.path(),
        &["--batch", "--no-loopflow", "flow", "survive-agent"],
        Some(&path),
    )
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("agent never started", || {
        assert!(
            driver.try_wait().unwrap().is_none(),
            "driver exited before provider input"
        );
        home.path().join("entered").exists()
    });
    driver.kill().unwrap();
    driver.wait().unwrap();
    let read = || {
        let mut flows = flow_details(repo.path(), home.path());
        assert_eq!(flows.len(), 1);
        flows.remove(0)
    };
    let id = read()["entry"]["id"].as_str().unwrap().to_owned();
    // Nothing restarts the Flow, before or after its orphaned turn ends.
    for command in ["resume", "start"] {
        let refused = run_lf(
            repo.path(),
            home.path(),
            &["flow", command, &id],
            Some(&path),
        );
        assert!(!refused.status.success(), "flow {command} restarted a Flow");
    }
    fs::write(home.path().join("release"), "").unwrap();
    wait_for("the orphaned step never recorded its exit", || {
        !read()["steps"][0]["completed_at"].is_null()
    });
    let flow = read();
    assert_eq!(
        step_fields(&flow, "label"),
        ["work"],
        "the dead driver's successor never launched"
    );
    assert_ne!(flow["entry"]["state"], "completed");
    assert_eq!(
        fs::read_to_string(home.path().join("turns")).unwrap(),
        "turn\n"
    );
}

#[test]
fn observing_and_preparing_a_task_are_not_execution() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task = support::register_task_with_pr(
        home.path(),
        repo.path(),
        "task-observation",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Started is `tasks.started_at`, set by the first Run that names the Task.
    let started_at = || -> Option<i64> {
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.task.id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let starts = || started_at().iter().count();
    // The shared evidence the desktop sidebar consumes.
    let started = || {
        runtime
            .block_on(task.store.task_started(&task.task.id))
            .unwrap()
    };
    assert_eq!(starts(), 0);
    assert!(!started(), "a prepared, unrun Task is not started");
    for args in [
        vec!["monitor", "active", "--task", "INF-123", "--json"],
        vec!["session", "list", "--task", "INF-123", "--json"],
        vec!["monitor", "usage", "--task", "INF-123", "--json"],
    ] {
        let read = run_lf(repo.path(), home.path(), &args, None);
        assert!(read.status.success(), "{read:?}");
        assert_eq!(started_at(), None, "inspection cannot start its Task");
    }

    write_skill(repo.path(), "review-proof", "Review the fixture.");
    write_flow(
        repo.path(),
        "review-first",
        "- step:\n    id: review\n    name: review-proof\n    human: true\n",
    );
    let prepared = run_lf(
        repo.path(),
        home.path(),
        &[
            "--task",
            "INF-123",
            "flow",
            "review-first",
            "--batch",
            "--no-loopflow",
        ],
        None,
    );
    assert!(!prepared.status.success());
    assert!(
        String::from_utf8_lossy(&prepared.stderr).contains("belongs in the Task conversation"),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let sessions = run_lf(
        repo.path(),
        home.path(),
        &["session", "list", "--json"],
        None,
    );
    assert!(
        sessions.status.success(),
        "{}",
        String::from_utf8_lossy(&sessions.stderr)
    );
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&sessions.stdout).unwrap();
    assert!(sessions.is_empty());
    assert_eq!(starts(), 0, "rejected review does not start its Task");
    assert!(!started());
}

#[test]
fn task_run_history_reads_only_that_tasks_runs_without_starting_it() {
    let repo = loopflow_test_support::TestRepo::new();
    repo.create_branch("task-history");
    let home = TempDir::new().unwrap();
    let task =
        support::register_task_with_pr(home.path(), repo.path(), "task-history", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let events = || {
        runtime
            .block_on(task.store.task_events_after(&task.task.id, 0))
            .unwrap()
            .len()
    };
    let read = |selector: &str| -> Vec<serde_json::Value> {
        let output = run_lf(
            repo.path(),
            home.path(),
            &[
                "monitor", "usage", "--days", "0", "--task", selector, "--json",
            ],
            None,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };

    // An unstarted Task: an empty list, and asking neither prepares nor starts it.
    let before = events();
    assert!(read("INF-123").is_empty());
    assert_eq!(events(), before, "reading Run history writes no Task event");
    assert!(!runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());

    write_skill(repo.path(), "history-work", "Do proof-owned work.");
    register_codex_account(home.path());
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script("done", ""),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    // Explicit and checkout selection both name this Task. An unrelated checkout stays unbound.
    let outside = loopflow_test_support::TestRepo::new();
    write_skill(
        outside.path(),
        "history-work",
        "Do unrelated proof-owned work.",
    );
    for args in [
        &[
            "--task",
            "INF-123",
            "history-work",
            "--batch",
            "--no-loopflow",
        ][..],
        &["history-work", "--batch", "--no-loopflow"][..],
    ] {
        let launched = run_lf(repo.path(), home.path(), args, Some(&path));
        assert!(
            launched.status.success(),
            "{}",
            String::from_utf8_lossy(&launched.stderr)
        );
    }

    let unrelated = run_lf(
        outside.path(),
        home.path(),
        &["history-work", "--batch", "--no-loopflow"],
        Some(&path),
    );
    assert!(
        unrelated.status.success(),
        "{}",
        String::from_utf8_lossy(&unrelated.stderr)
    );
    let runs = read("INF-123");
    assert_eq!(
        runs.len(),
        2,
        "explicit and checkout work, excluding unrelated work: {runs:?}"
    );
    assert_eq!(runs[0]["harness"], "codex");
    assert_eq!(runs[0]["skill"], "history-work");
    assert!(
        !session_id_for_capture(home.path(), runs[0]["artifact_key"].as_str().unwrap()).is_empty()
    );
    assert!(runs[0]["observed_at"].as_i64().is_some());
    let after_launch = events();
    assert!(read("INF-999").is_empty(), "another Task sees none of them");
    assert_eq!(
        events(),
        after_launch,
        "reads leave the Task's events alone"
    );
}

#[test]
fn lf_launches_inside_a_task_checkout_bind_to_that_task() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task =
        support::register_task_with_pr(home.path(), repo.path(), "task-binding", &repo.head_sha());
    let sibling_worktree = repo.create_named_worktree("task-sibling");
    let sibling =
        support::register_sibling_task(&task, "INF-124", "task-sibling", &sibling_worktree);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Started is `tasks.started_at`, set by the first Run that names the Task.
    let starts = |id: &loopflow::work::task::TaskId| {
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT count(*) FROM tasks WHERE id=?1 AND started_at IS NOT NULL",
                [id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let events = || {
        runtime
            .block_on(task.store.task_events_after(&task.task.id, 0))
            .unwrap()
            .len()
    };
    let json = |args: &[&str]| -> serde_json::Value {
        let output = run_lf(repo.path(), home.path(), args, None);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let session = |run_id: &str| -> serde_json::Value {
        json(&["session", "list", "--all", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == session_id_for_capture(home.path(), run_id))
            .cloned()
            .unwrap_or_else(|| panic!("Session {run_id} is not listed"))
    };
    let task_runs = |identifier: &str| -> Vec<String> {
        json(&[
            "monitor", "usage", "--days", "0", "--task", identifier, "--json",
        ])
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["artifact_key"].as_str().unwrap().to_string())
        .collect()
    };

    write_skill(repo.path(), "binding-work", "Do proof-owned work.");
    // Both branches keep their definitions when an unbound command checkpoints.
    repo.stage_all();
    repo.commit("Keep the shared launch fixture skill across branches");
    // An explicit `--task` launch runs in that Task's worktree, which has its
    // own uncommitted catalog.
    write_skill(&sibling_worktree, "binding-work", "Do proof-owned work.");
    // A codex TUI stand-in. It records its provider Session the way the real
    // client's session-start hook does and notes which Run launched it.
    let bin = TempDir::new().unwrap();
    let launched = bin.path().join("launched");
    write_executable(
        &bin.path().join("codex"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nif [ \"$1\" = --dangerously-bypass-hook-trust ] && [ \"$2\" = --model ]; then echo \"a value is required for '--model <MODEL>'\" >&2; exit 2; fi\n\
             printf '%s' '{{\"session_id\":\"ses-'\"$LF_CAPTURE_KEY\"'\"}}' \
             | \"$LF_BIN\" __provider-session || exit $?\necho \"$LF_CAPTURE_KEY\" >> '{}'\n",
            launched.display()
        ),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let launch = |cwd: &Path, args: &[&str]| -> String {
        let before = std::fs::read_to_string(&launched).unwrap_or_default();
        let output = run_lf(cwd, home.path(), args, Some(&path));
        assert!(
            output.status.success(),
            "lf {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let after = std::fs::read_to_string(&launched).unwrap();
        after[before.len()..].trim().to_string()
    };

    assert_eq!(starts(&task.task.id), 0);

    // In the Task's checkout, a plain launch binds to that Task.
    repo.create_branch("task-binding");
    let bound = launch(repo.path(), &["-i", "binding-work", "--no-loopflow"]);
    let listed = session(&bound);
    assert_eq!(
        listed["work"],
        serde_json::json!({"kind": "task", "id": task.task.id}),
        "{listed}"
    );
    assert_eq!(listed["wave_id"], serde_json::json!(task.task.wave_id));
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);
    assert_eq!(
        starts(&task.task.id),
        1,
        "a launch bound from its checkout is work beginning"
    );
    assert!(runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());

    // A checkout no Task owns stays unbound and retires on exit.
    let unrelated = repo.create_named_worktree("unregistered");
    let unbound = launch(&unrelated, &["-i", "binding-work", "--no-loopflow"]);
    let history = json(&["session", "list", "--all", "--history", "--json"]);
    let retired = history
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == session_id_for_capture(home.path(), &unbound))
        .unwrap();
    assert_eq!(retired["work"], serde_json::Value::Null);
    assert_eq!(retired["state"], "closed");
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);

    // Explicit selection wins over the checkout.
    repo.checkout("task-binding");
    let explicit = launch(
        repo.path(),
        &["--task", "INF-124", "-i", "binding-work", "--no-loopflow"],
    );
    assert_eq!(
        session(&explicit)["work"],
        serde_json::json!({"kind": "task", "id": sibling.id})
    );
    assert_eq!(task_runs("INF-124"), vec![explicit]);
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);
    assert_eq!(starts(&task.task.id), 1);
    assert_eq!(starts(&sibling.id), 1);

    // Observation stays observation.
    let settled = events();
    let _ = json(&["session", "list", "--all", "--json"]);
    let _ = json(&[
        "monitor", "usage", "--days", "0", "--task", "INF-123", "--json",
    ]);
    let _ = json(&["monitor", "active", "--task", "INF-123", "--json"]);
    assert_eq!(events(), settled, "reads write no Task event");

    // Landing preserves the checkout's Task context and historical attribution.
    let mut landed = task.pr.clone();
    landed.publication = Some(loopflow::work::task::PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(loopflow::work::task::GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
        merge: None,
    });
    landed.merge_commit = Some(repo.head_sha());
    runtime
        .block_on(task.store.update_task_pr(&landed))
        .unwrap();
    let after_landing = launch(repo.path(), &["-i", "binding-work", "--no-loopflow"]);
    assert_eq!(
        session(&after_landing)["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(task_runs("INF-123"), vec![after_landing, bound]);
}

#[test]
fn bound_flows_keep_task_context_and_leave_other_flows_and_shared_edits_alone() {
    use loopflow_test_support::TestRepo;

    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let caller = TestRepo::new();
    let home = TempDir::new().unwrap();
    let checkout = repo.path().canonicalize().unwrap();
    let task = support::register_task_with_pr(
        home.path(),
        &checkout,
        "task-contribution",
        &repo.head_sha(),
    );
    observe_planning(&task, &checkout);
    for skill in ["first", "second"] {
        write_skill(repo.path(), skill, &format!("Execute {skill}."));
    }
    write_flow(repo.path(), "contribution", "- first\n- second\n");
    // A real collision: bare and explicit run select the flow; typed skill
    // selects the single skill, including when Work-bound.
    write_skill(
        repo.path(),
        "contribution",
        "Execute the single contribution skill.",
    );
    fs::create_dir_all(repo.path().join("scratch")).unwrap();
    fs::write(
        repo.path().join("scratch/existing.md"),
        "Another contributor's unfinished work.",
    )
    .unwrap();
    let original_head = repo.head_sha();
    // An earlier Flow of this Task that stopped: later launches leave it alone.
    let earlier = support::record_flow(home.path(), &checkout, "code", "implement", "failed");
    let earlier_flow = || {
        lf_json(
            repo.path(),
            home.path(),
            &["flow", "show", &earlier, "--processes", "--json"],
        )
    };
    let stopped = earlier_flow();
    assert_eq!(stopped["entry"]["state"], "stopped");

    register_codex_account(home.path());
    let bin = TempDir::new().unwrap();
    let provider = codex_app_server_script("done", "if [ \"$1\" = --version ]; then exit 0; fi").replace(
        "read -r turn_start",
        "read -r turn_start\npwd >> \"$LF_HOME/cwds\"\nprintf '%s\\n' \"$thread_start\" >> \"$LF_HOME/prompts\"\nprintf '%s\\n' 'Evidence from preceding step.' > scratch/step.md",
    );
    write_executable(&bin.path().join("codex"), &provider);
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    // Distinct name tests bare flow dispatch without the collision above.
    write_flow(repo.path(), "two-steps", "- first\n- second\n");
    for args in [
        vec!["--task", "INF-123", "two-steps"],
        vec!["--task", "INF-123", "contribution"],
        vec!["--task", "INF-123", "run", "contribution"],
        vec!["--task", "INF-123", "flow", "contribution"],
        vec!["--task", "INF-123", "flow", "contribution"],
    ] {
        let _ = fs::remove_file(home.path().join("prompts"));
        let _ = fs::remove_file(home.path().join("cwds"));
        let _ = fs::remove_file(repo.path().join("scratch/step.md"));
        let mut args = args;
        args.extend(["--batch", "--no-loopflow", "Keep the Task context."]);
        let output = run_lf(caller.path(), home.path(), &args, Some(&path));
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let prompts = received_contexts(home.path());
        assert_eq!(
            prompts.len(),
            2,
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for prompt in &prompts {
            assert!(prompt.contains("Exercise the persisted lifecycle."));
            assert!(prompt.contains(task.task.id.as_str()));
            assert!(prompt.contains("Another contributor's unfinished work."));
            assert!(prompt.contains("Keep the Task context."));
        }
        assert!(!prompts[0].contains("Evidence from preceding step."));
        assert!(prompts[1].contains("Evidence from preceding step."));
        let cwds = fs::read_to_string(home.path().join("cwds")).unwrap();
        for cwd in cwds.lines() {
            assert_eq!(
                Path::new(cwd).canonicalize().unwrap(),
                repo.path().canonicalize().unwrap()
            );
        }
        assert_eq!(repo.head_sha(), original_head);
        assert_eq!(earlier_flow(), stopped);
        let staged = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(staged.stdout.is_empty());
    }
    // Each launch was its own Flow of two steps; none continued another.
    let flows = support::recorded_flows(home.path());
    assert_eq!(flows.len(), 6);
    for (outcome, steps) in &flows[1..] {
        assert_eq!(outcome.as_deref(), Some("succeeded"));
        let labels: Vec<_> = steps.iter().map(|step| step["label"].clone()).collect();
        assert_eq!(labels, ["first", "second"]);
    }
    let output = run_lf(
        repo.path(),
        home.path(),
        &["monitor", "usage", "--days", "7", "--json"],
        None,
    );
    assert!(output.status.success());
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(runs.len(), 10);
    for run in runs {
        assert_eq!(run["task_identifier"], task.task.plan.identifier);
        assert_eq!(run["recorded_outcome"], "completed");
    }
    for invocation in [vec!["skill", "contribution"], vec!["design"]] {
        let _ = fs::remove_file(home.path().join("prompts"));
        let mut args = vec!["--task", "INF-123"];
        args.extend(invocation);
        args.extend(["--batch", "--no-loopflow"]);
        let output = run_lf(caller.path(), home.path(), &args, Some(&path));
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(home.path().join("prompts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
    // A human boundary remains explicit and cannot silently run the next step
    // just because the invocation has Task attribution.
    write_flow(
        repo.path(),
        "review-contribution",
        "- step:\n    id: accept\n    name: first\n    human: true\n- second\n",
    );
    fs::remove_file(home.path().join("prompts")).unwrap();
    let output = run_lf(
        caller.path(),
        home.path(),
        &["--task", "INF-123", "review-contribution", "--batch"],
        Some(&path),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("belongs in the Task conversation"));
    assert!(!home.path().join("prompts").exists());
    let listed = run_lf(
        repo.path(),
        home.path(),
        &["session", "list", "--all", "--json"],
        Some(&path),
    );
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let sessions: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert!(sessions
        .as_array()
        .unwrap()
        .iter()
        .all(|session| session["kind"] != "flow"));
    assert!(!home.path().join("prompts").exists());
    assert_eq!(support::recorded_flows(home.path()).len(), 6);
    assert_eq!(earlier_flow(), stopped);
    assert_eq!(repo.head_sha(), original_head);
}

#[test]
fn flow_names_load_into_targets() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- realign
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    assert!(matches!(
        &flow.items[0].target,
        loopflow::definition::Target::Flow(_)
    ));
    assert!(matches!(
        flow.items[1],
        Step {
            target: loopflow::definition::Target::Skill(_),
            ..
        }
    ));
}

#[test]
fn command_item_parses_and_expands() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "ship-ish",
        r#"
- implement
- cmd: pr land
"#,
    );

    let flow = load_flow("ship-ish", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    match &flow.items[1] {
        Step {
            target: loopflow::definition::Target::Command(item),
            ..
        } => {
            assert_eq!(item.command, "pr");
            assert_eq!(item.args, vec!["land"]);
        }
        other => panic!("expected command item, got {other:?}"),
    }

    let expanded = compile_flow(&flow, repo).unwrap();
    assert!(matches!(&expanded[1], ConcreteStep::Command(_)));
}

#[test]
fn scheduled_release_flow_propagates_the_operation_failure() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    write_executable(&bin.join("gh"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &bin.join("release-publisher"),
        "#!/bin/sh\necho 'fixture publisher unavailable' >&2\nexit 27\n",
    );
    write_flow(
        repo.path(),
        "release-run",
        include_str!("../../../.lf/flows/release-run.yaml"),
    );
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [release-publisher]\n",
    )
    .unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());

    let output = run_lf(
        repo.path(),
        home.path(),
        &["--batch", "flow", "release-run"],
        Some(&path),
    );

    assert!(
        !output.status.success(),
        "a failed release must fail its scheduled target"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fixture publisher unavailable"), "{stderr}");
    assert!(!stderr.contains("launching agent"), "{stderr}");
}

#[test]
fn compile_flow_tracks_sources() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- realign
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    let items = compile_flow(&flow, repo).unwrap();
    match &items[0] {
        ConcreteStep::Skill(skill) => {
            assert_eq!(skill.skill.name, "implement");
            assert_eq!(skill.sources, vec!["parent", "child"]);
        }
        _ => panic!("expected expanded skill"),
    }
}

/// Plain string items in flow YAML that match a sub-flow name should be
/// expanded as sub-flows, not treated as skill names.
#[test]
fn compile_flow_resolves_plain_string_as_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_skill(repo, "skill-a", "First captured skill.");
    write_skill(repo, "skill-b", "Second captured skill.");
    write_flow(repo, "publish", "- skill-a\n- skill-b");
    write_skill(repo, "review", "Review the supplied evidence.");
    write_flow(repo, "parent", "- step: review\n- publish");

    let items = expand_named_flow(repo, "parent");

    assert_eq!(items.len(), 3, "publish should expand into its sub-skills");
    assert_skill_name(&items[0], "review");
    match &items[1] {
        ConcreteStep::Skill(s) => {
            assert_eq!(s.skill.name, "skill-a");
            assert_eq!(s.sources, vec!["parent", "publish"]);
        }
        _ => panic!("expected skill from publish sub-flow"),
    }
    match &items[2] {
        ConcreteStep::Skill(s) => {
            assert_eq!(s.skill.name, "skill-b");
            assert_eq!(s.sources, vec!["parent", "publish"]);
        }
        _ => panic!("expected skill from publish sub-flow"),
    }
}

#[test]
fn adding_a_flow_changes_an_untyped_reference_but_not_an_explicit_skill() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_skill(repo, "custom-review", "Review the code.");
    write_skill(repo, "replacement", "Follow the new review workflow.");
    write_flow(repo, "parent", "- custom-review\n- step: custom-review");
    let initial = expand_named_flow(repo, "parent");
    assert_skill_name(&initial[0], "custom-review");
    write_flow(repo, "custom-review", "- step: replacement");
    let changed = expand_named_flow(repo, "parent");
    assert_skill_name(&changed[0], "replacement");
    assert_skill_name(&changed[1], "custom-review");
}

#[test]
fn builtin_deploy_uses_ops_land_item() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "deploy");
    assert!(!items.is_empty());
    assert!(matches!(&items[1], ConcreteStep::Command(_)));
}

fn roadmap_task(repo: &Path, home: &Path) -> serde_json::Value {
    lf_json(repo, home, &["roadmap", "--json"])["waves"][0]["tasks"]["items"][0].clone()
}

fn labels(graph: &serde_json::Value) -> Vec<&str> {
    graph["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["label"].as_str().unwrap())
        .collect()
}

fn received_contexts(home: &Path) -> Vec<String> {
    fs::read_to_string(home.join("prompts"))
        .unwrap()
        .lines()
        .map(|line| {
            let thread: serde_json::Value = serde_json::from_str(line).unwrap();
            let path = thread["params"]["config"]["model_instructions_file"]
                .as_str()
                .unwrap();
            fs::read_to_string(path).unwrap()
        })
        .collect()
}

const WORK: &str = "done";
const ITERATE: &str = r#"{"decision":"iterate","summary":"More to do","reason":null}"#;
const ADVANCE: &str = r#"{"decision":"advance","summary":"Proof observed","reason":null}"#;

/// A Codex stand-in whose nth turn returns the nth answer, and the PATH that
/// finds it.
fn scripted_provider(home: &Path, answers: &[&str]) -> (TempDir, String) {
    fs::write(
        home.join("answers"),
        answers
            .iter()
            .map(|answer| serde_json::to_string(answer).unwrap() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let provider = codex_app_server_script("@answer@", "if [ \"$1\" = --version ]; then exit 0; fi")
        .replace(
            "read -r turn_start",
            "read -r turn_start\nprintf '%s\\n' \"$thread_start\" >> \"$LF_HOME/prompts\"\necho turn >> \"$LF_HOME/turns\"\nanswer=$(sed -n \"$(grep -c turn \"$LF_HOME/turns\")p\" \"$LF_HOME/answers\")",
        )
        .replace("\"@answer@\"", "'\"$answer\"'");
    assert!(
        provider.contains("$answer\"'"),
        "the stand-in takes a script"
    );
    register_codex_account(home);
    let bin = TempDir::new().unwrap();
    write_executable(&bin.path().join("codex"), &provider);
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    (bin, path)
}

#[test]
fn task_flow_read_keeps_captured_topology_and_counts_both_returns() {
    let repo = loopflow_test_support::TestRepo::new();
    support::bind_task_planning(&repo);
    let home = TempDir::new().unwrap();
    // A Task's Flows are those whose Processes ran in its checkout, as Processes name it.
    let checkout = repo.path().canonicalize().unwrap();
    repo.create_branch("task-flow-read");
    let task =
        support::register_task_with_pr(home.path(), &checkout, "task-flow-read", &repo.head_sha());
    observe_planning(&task, &checkout);
    for skill in [
        "implement-proof",
        "decide-proof",
        "route-proof",
        "demo-proof",
    ] {
        write_skill(repo.path(), skill, "Fixture step.");
    }
    write_flow(repo.path(), "plan-check", "- cmd: sync --plan\n");
    // Two returns to one node, one skill deciding both, an XOR, and a last
    // operation that fails so the Flow stops where that step did.
    write_flow(
        repo.path(),
        "two-loops",
        "- cmd: sync --plan\n- implement-proof\n- loop: implement-proof\n  step: decide-proof\n- xor:\n    router: route-proof\n    paths:\n      alpha:\n        description: Check the plan\n        flow: plan-check\n      zeta:\n        description: Show the work\n        skill: demo-proof\n- loop: implement-proof\n  step: decide-proof\n- cmd: __telemetry-scorecard\n",
    );
    let task_flow = || {
        lf_json(
            repo.path(),
            home.path(),
            &["task", "status", "INF-123", "--json"],
        )["execution"]
            .clone()
    };

    // Before any Flow: the recommendation, Start, and no invented history.
    let flow = roadmap_task(repo.path(), home.path());
    assert_eq!(flow["workflow_name"], "feature");
    assert!(flow["latest_flow_process"].is_null());
    assert!(flow["run_control"]["unavailable"].is_null(), "{flow}");
    assert_eq!(task_flow()["work"]["flow_processes"], serde_json::json!([]));

    // The catalogue previews the authored topology through the shared loader.
    let catalog = lf_json(repo.path(), home.path(), &["flow", "list", "--json"]);
    let preview = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "two-loops")
        .unwrap();
    let returns: Vec<_> = preview["graph"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["returns_to"].as_u64())
        .collect();
    assert_eq!(returns, [None, None, Some(1), None, Some(1), None]);

    let (work, iterate, advance, route) = (WORK, ITERATE, ADVANCE, r#"{"path":"alpha"}"#);
    let answers = [
        work, iterate, work, iterate, work, advance, route, iterate, work, advance, route, advance,
    ];
    let (_bin, path) = scripted_provider(home.path(), &answers);
    let ran = run_lf(
        repo.path(),
        home.path(),
        &[
            "--task",
            "INF-123",
            "flow",
            "two-loops",
            "--batch",
            "--no-loopflow",
        ],
        Some(&path),
    );
    assert!(!ran.status.success(), "the last operation fails");
    assert!(
        String::from_utf8_lossy(&ran.stderr).contains("telemetry scorecard generator not found"),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );

    // Read back from Processes alone: where it stopped and each edge's returns.
    let execution = task_flow();
    let flows = execution["work"]["flow_processes"].as_array().unwrap();
    assert_eq!(flows.len(), 1, "{execution}");
    let id = flows[0]["id"].as_str().unwrap();
    assert_eq!(flows[0]["name"], "two-loops");
    assert_eq!(flows[0]["state"], "stopped");
    let sessions = execution["work"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), answers.len(), "one conversation per turn");
    assert!(sessions
        .iter()
        .all(|session| session["flow_lf_process_id"] == id));
    // The failed step is red and stays with the Flow for its caller.
    assert_eq!(execution["execution"]["state"], "blocked");
    assert_eq!(execution["execution"]["step"], "__telemetry-scorecard");
    let flow = roadmap_task(repo.path(), home.path());
    let record = &flow["latest_flow_process"];
    assert_eq!(record["entry"]["id"], id);
    assert_eq!(flow["execution"]["state"], "blocked");
    let shown = lf_json(
        repo.path(),
        home.path(),
        &["flow", "show", id, "--processes", "--json"],
    );
    for read in [record, &shown] {
        assert_eq!(
            labels(&read["graph"]),
            [
                "sync --plan",
                "implement-proof",
                "decide-proof",
                "route-proof",
                "decide-proof",
                "__telemetry-scorecard"
            ]
        );
        assert_eq!(read["current"], 7);
        assert_eq!(read["completed"], serde_json::json!([0, 1, 2, 3, 6]));
        assert_eq!(read["iterations"], serde_json::json!([[2, 1]]));
        assert_eq!(
            read["returns"],
            serde_json::json!([
                {"decider": 2, "traversals": 2},
                {"decider": 6, "traversals": 1}
            ])
        );
    }
    // Every pass is its own step Process at the node and return counts it ran with.
    assert_eq!(
        step_fields(&shown, "key"),
        [0, 1, 2, 1, 2, 1, 2, 3, 4, 6, 1, 2, 3, 4, 6, 7]
    );
    assert_eq!(
        shown["steps"][8]["iterations"],
        serde_json::json!([[2, 0], []])
    );
    assert_eq!(
        shown["steps"][10]["iterations"],
        serde_json::json!([[2, 1]])
    );

    // A stopped Flow is history: a fresh launch stays legal, and no command
    // restarts or resumes this one.
    assert!(flow["run_control"]["unavailable"].is_null(), "{flow}");
    for removed in [
        vec!["task", "restart", "INF-123", "--flow", "two-loops"],
        vec!["--task", "INF-123", "flow", "start", "two-loops"],
        vec!["flow", "resume", id],
    ] {
        let rejected = run_lf(repo.path(), home.path(), &removed, Some(&path));
        assert!(!rejected.status.success(), "{removed:?}");
    }
    assert_eq!(
        task_flow()["work"]["flow_processes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let status = run_lf(
        repo.path(),
        home.path(),
        &["task", "status", "INF-123"],
        None,
    );
    let status = String::from_utf8(status.stdout).unwrap();
    assert!(
        status.lines().any(|line| line == "INF-123  not ready"),
        "{status}"
    );
    assert!(
        status.contains("telemetry scorecard generator not found"),
        "{status}"
    );

    // Its YAML changed since: the past Flow keeps the graph it launched with.
    write_flow(repo.path(), "two-loops", "- implement-proof\n");
    let redrawn = lf_json(
        repo.path(),
        home.path(),
        &["flow", "show", id, "--processes", "--json"],
    );
    assert_eq!(redrawn["graph"], shown["graph"]);
    assert_eq!(redrawn["current"], shown["current"]);
    assert_eq!(redrawn["completed"], shown["completed"]);
    assert_eq!(
        redrawn["steps"], shown["steps"],
        "nothing ran or was rewritten"
    );
    assert_eq!(
        roadmap_task(repo.path(), home.path())["latest_flow_process"]["current"],
        7
    );
}

#[test]
fn three_nested_loops_return_to_named_occurrences_of_one_skill() {
    let repo = loopflow_test_support::TestRepo::new();
    support::bind_task_planning(&repo);
    let home = TempDir::new().unwrap();
    let checkout = repo.path().canonicalize().unwrap();
    let task =
        support::register_task_with_pr(home.path(), &checkout, "nested-loops", &repo.head_sha());
    observe_planning(&task, &checkout);
    repo.create_branch("nested-loops");
    write_skill(repo.path(), "work-proof", "Fixture step.");
    // One skill three times, so each loop names its occurrence; the default
    // decider closes the inner region first and the outer one last.
    write_flow(
        repo.path(),
        "nested",
        "- step: {name: work-proof, id: outer}\n- step: {name: work-proof, id: middle}\n- step: {name: work-proof, id: inner}\n- loop: inner\n- loop: middle\n- loop: outer\n",
    );
    let catalog = lf_json(repo.path(), home.path(), &["flow", "list", "--json"]);
    let preview = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "nested")
        .unwrap();
    assert_eq!(
        labels(&preview["graph"]),
        [
            "work-proof",
            "work-proof",
            "work-proof",
            "loop-or-next",
            "loop-or-next",
            "loop-or-next"
        ]
    );
    assert_eq!(
        step_fields(&preview["graph"], "returns_to"),
        [None, None, None, Some(2), Some(1), Some(0)].map(serde_json::Value::from)
    );

    // Each loop returns once: the inner alone, then inside the middle's pass,
    // then all three inside the outer's.
    let (w, i, a) = (WORK, ITERATE, ADVANCE);
    let answers = [w, w, w, i, w, a, i, w, w, a, a, i, w, w, w, a, a, a];
    let (_bin, path) = scripted_provider(home.path(), &answers);
    let ran = run_lf(
        repo.path(),
        home.path(),
        &[
            "--task",
            "INF-123",
            "flow",
            "nested",
            "--batch",
            "--no-loopflow",
        ],
        Some(&path),
    );
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );

    let status = lf_json(
        repo.path(),
        home.path(),
        &["task", "status", "INF-123", "--json"],
    );
    let flows = status["execution"]["work"]["flow_processes"]
        .as_array()
        .unwrap();
    assert_eq!(flows.len(), 1, "{status}");
    assert_eq!(flows[0]["state"], "completed");
    let shown = lf_json(
        repo.path(),
        home.path(),
        &[
            "flow",
            "show",
            flows[0]["id"].as_str().unwrap(),
            "--processes",
            "--json",
        ],
    );
    assert_eq!(
        step_fields(&shown, "key"),
        [0, 1, 2, 3, 2, 3, 4, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        shown["returns"],
        serde_json::json!([
            {"decider": 3, "traversals": 1},
            {"decider": 4, "traversals": 1},
            {"decider": 5, "traversals": 1}
        ])
    );
}

#[test]
fn a_repeated_node_receives_only_task_direction_newer_than_its_last_run() {
    let repo = loopflow_test_support::TestRepo::new();
    support::bind_task_planning(&repo);
    let home = TempDir::new().unwrap();
    let checkout = repo.path().canonicalize().unwrap();
    let task =
        support::register_task_with_pr(home.path(), &checkout, "repeat-steers", &repo.head_sha());
    observe_planning(&task, &checkout);
    repo.create_branch("repeat-steers");
    write_skill(repo.path(), "work-proof", "Fixture step.");
    write_flow(repo.path(), "twice", "- work-proof\n- loop: work-proof\n");
    let steer = |text: &str| -> i64 {
        let kind = serde_json::json!({"kind": "steer", "author": {"kind": "user"}, "text": text});
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,?2,unixepoch()) RETURNING id",
                [task.task.id.as_str(), &kind.to_string()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let earlier = steer("Keep the parser strict.");
    let (_bin, path) = scripted_provider(home.path(), &[WORK, ITERATE, WORK, ADVANCE, WORK]);
    let run = |args: &[&str]| {
        let mut args = args.to_vec();
        args.extend(["--batch", "--no-loopflow"]);
        let ran = run_lf(repo.path(), home.path(), &args, Some(&path));
        assert!(
            ran.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        received_contexts(home.path())
            .iter()
            .map(|prompt| prompt.contains("Keep the parser strict."))
            .collect::<Vec<_>>()
    };
    // work, decide, work, decide: each node's first run is given the steer,
    // its second is spared it.
    assert_eq!(
        run(&["--task", "INF-123", "flow", "twice"]),
        [true, true, false, false]
    );
    // The driver asked for that with an option any run takes.
    steer("Report the first error only.");
    let after = earlier.to_string();
    let given = run(&[
        "--task",
        "INF-123",
        "--steers-after",
        &after,
        "skill",
        "work-proof",
    ]);
    assert_eq!(given.last(), Some(&false));
    let prompts = received_contexts(home.path());
    assert!(prompts
        .last()
        .unwrap()
        .contains("Report the first error only."));
}

#[test]
fn the_research_workflow_ends_on_its_edge_that_runs_nothing() {
    let repo = loopflow_test_support::TestRepo::new();
    support::bind_task_planning(&repo);
    let home = TempDir::new().unwrap();
    let checkout = repo.path().canonicalize().unwrap();
    let task =
        support::register_task_without_pr(home.path(), &checkout, "research-end", &repo.head_sha());
    observe_planning(&task, &checkout);
    repo.create_branch("research-end");
    let (_bin, path) = scripted_provider(home.path(), &[WORK]);
    let run = |args: &[&str]| run_lf(repo.path(), home.path(), args, Some(&path));
    let position = || {
        lf_json(
            repo.path(),
            home.path(),
            &["task", "status", "INF-123", "--json"],
        )["execution"]["work"]["workflow"]["position"]["node"]
            .clone()
    };
    let ran = run(&["-b", "--no-loopflow", "task", "run", "INF-123", "research"]);
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(position(), "findings");
    // Two edges leave findings; the refusal names the one that runs nothing.
    let refused = run(&["task", "run", "INF-123"]);
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(error.contains("end (runs no Flow)"), "{error}");
    assert_eq!(position(), "findings");
    let ended = run(&["task", "run", "INF-123", "end"]);
    assert!(
        ended.status.success(),
        "{}",
        String::from_utf8_lossy(&ended.stderr)
    );
    assert_eq!(position(), "end");
    // Running the Flow left a file uncommitted; ending kept it.
    assert!(checkout.join(".gitignore").exists());
}

#[test]
fn mixed_provider_flow_keeps_launch_accounts_across_steps() {
    use base64::prelude::*;
    use loopflow::store::{
        CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
    };
    use sha2::{Digest, Sha256};

    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    for provider in ["claude", "codex"] {
        for label in ["chosen", "other"] {
            let id = format!("{provider}-{label}");
            let email = format!("{id}@example.com");
            let account_home = home.path().join("accounts").join(provider).join(&id);
            fs::create_dir_all(&account_home).unwrap();
            let credential = if provider == "claude" {
                serde_json::json!({"claudeAiOauth":{"accessToken":format!("fixture-{id}"),"expiresAt":4102444800000i64}}).to_string()
            } else {
                let claims = BASE64_URL_SAFE_NO_PAD
                    .encode(serde_json::json!({"email":email,"sub":id}).to_string());
                serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()
            };
            fs::write(
                account_home.join(if provider == "claude" {
                    ".credentials.json"
                } else {
                    "auth.json"
                }),
                &credential,
            )
            .unwrap();
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            runtime
                .block_on(
                    store.upsert_provider_account(&ProviderAccount {
                        provider: provider.into(),
                        account_id: ProviderAccountId::parse(&id).unwrap(),
                        home: Some(account_home),
                        login_email: Some(loopflow::profile::EmailAddress::parse(&email).unwrap()),
                        observed_email: Some(email),
                        observed_subject: Some(id),
                        observed_credential_digest: (provider == "claude")
                            .then(|| format!("{:x}", Sha256::digest(credential.as_bytes()))),
                        observed_plan: None,
                        credential_state: CredentialState::Connected,
                        // Explicit selection must work even when automatic routing prefers another login.
                        routing_state: if label == "chosen" {
                            RoutingState::ExplicitOnly
                        } else {
                            RoutingState::Automatic
                        },
                        plan: None,
                        paid_through: None,
                        utilization_percent: None,
                        cooldown_until: None,
                        cooldown_reason: None,
                        last_selected_at: None,
                        created_at: now,
                        updated_at: now,
                    }),
                )
                .unwrap();
        }
    }
    for (skill, provider) in [
        ("c1", "claude"),
        ("d1", "codex"),
        ("c2", "claude"),
        ("d2", "codex"),
    ] {
        write_skill(
            repo.path(),
            skill,
            &format!("---\nagent: {provider}\n---\nRun {skill}."),
        );
    }
    write_flow(repo.path(), "pair", "- c1\n- d1\n- c2\n- d2\n");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "mixed provider fixture"]);
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("claude"),
        r#"#!/bin/sh
case "$1" in --version) exit 0;; esac
printf 'claude:%s\n' "$CLAUDE_CONFIG_DIR" >> "$LF_HOME/selected"
read -r input
echo '{"type":"system","subtype":"init","session_id":"account-fixture"}'
echo '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"account-fixture"}'
"#,
    );
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script(
            "done",
            r#"if [ "$1" = --version ]; then exit 0; fi
printf 'codex:%s\n' "$CODEX_HOME" >> "$LF_HOME/selected"
"#,
        ),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let output = run_lf(
        repo.path(),
        home.path(),
        &[
            "--isolate",
            "--account",
            "claude=claude-chosen@",
            "--account",
            "codex=codex-chosen@",
            "--batch",
            "flow",
            "pair",
        ],
        Some(&path),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let selected = fs::read_to_string(home.path().join("selected")).unwrap();
    let expected = ["claude", "codex", "claude", "codex"].map(|provider| {
        format!(
            "{provider}:{}",
            home.path()
                .join("accounts")
                .join(provider)
                .join(format!("{provider}-chosen"))
                .display()
        )
    });
    assert_eq!(
        selected.lines().collect::<Vec<_>>(),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
}

#[test]
fn wave_context_keeps_task_location_and_rejects_another_owner() {
    use loopflow::id::WaveId;
    use loopflow::work::wave::Wave;
    use loopflow_test_support::TestRepo;

    let repo = TestRepo::new();
    repo.create_branch("wave-context");
    let home = TempDir::new().unwrap();
    let task =
        support::register_task_with_pr(home.path(), repo.path(), "wave-context", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let other = Wave::new(
        WaveId::new(),
        "other".into(),
        repo.path().display().to_string(),
    );
    runtime.block_on(task.store.create_wave(&other)).unwrap();
    let output = run_lf(
        repo.path(),
        home.path(),
        &["--wave", "other", "repo", "tokens", "--json"],
        None,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not own Task"));
    assert!(output.stdout.is_empty());
    let output = run_lf(
        repo.path(),
        home.path(),
        &["--wave", "task-pr-tests", "repo", "tokens", "--json"],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
}

#[test]
fn worktree_selector_uses_the_named_checkout_without_changing_the_caller() {
    use loopflow_test_support::TestRepo;
    let repo = TestRepo::new();
    let home = TempDir::new().unwrap();
    let selected = repo.create_named_worktree("selected");
    fs::create_dir_all(selected.join("selected-only")).unwrap();
    fs::write(selected.join("selected-only/proof.rs"), "fn proof() {}\n").unwrap();
    assert!(Command::new("git")
        .args(["add", "selected-only"])
        .current_dir(&selected)
        .status()
        .unwrap()
        .success());
    let output = run_lf(
        repo.path(),
        home.path(),
        &["--wt", "selected", "repo", "tokens", "--json"],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("selected-only"));
    assert!(!repo.path().join("selected-only").exists());
    write_skill(
        &selected,
        "checkout-proof",
        "Inspect the selected checkout.",
    );
    for args in [
        vec!["--wt", "selected", "help", "skill", "checkout-proof"],
        vec!["--wt", "selected", "skill", "checkout-proof", "--help"],
        vec!["skill", "checkout-proof", "--wt=selected", "--help"],
    ] {
        let output = run_lf(repo.path(), home.path(), &args, None);
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Inspect the selected checkout."));
    }
}
