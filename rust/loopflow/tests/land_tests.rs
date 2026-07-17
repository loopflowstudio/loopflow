mod support;

use std::fs;
use std::process::{Command, Stdio};

use loopflow::engine::worktrees::create_named_worktree;
use loopflow::ops::{land, submit, LandOptions, NullProgress, OpsError};
use loopflow::task::{
    AfterMerge, CiObservation, CiState, GithubPr, PrPublication, TaskGateProposal,
    TaskLifecyclePhase, TaskSessionStatus, TaskSettlementIntent,
};
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};
use support::{counting_open_script, presentation_attempts, register_task, EnvGuard};
use time::OffsetDateTime;

fn push_branch(repo: &TestRepo, name: &str) {
    let _ = Command::new("git")
        .args(["push", "-u", "origin", name])
        .current_dir(repo.path())
        .status();
}

fn local_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .current_dir(repo.path())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn remote_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn gh_no_pr_script() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi

if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi

exit 0
"#
}

fn noop_open_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn gh_land_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"

if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi

if [ "$1 $2" = "pr create" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

if [ "$1 $2" = "pr view" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

exit 0
"#
    )
}

fn gh_existing_pr_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  echo '[{{"url":"https://example.com/pr/912","state":"OPEN","isDraft":false,"number":912,"mergeCommit":null}}]'
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/912'
  exit 0
fi
exit 0
"#
    )
}

fn gh_replay_safe_task_land_script(log_path: &str, armed_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"
if [ "$1 $2" = "pr list" ]; then
  echo '[{{"url":"https://example.com/pr/912","state":"OPEN","isDraft":false,"number":912,"mergeCommit":null}}]'
  exit 0
fi
if [ "$1 $2 $3 $4" = "pr view --json autoMergeRequest" ]; then
  if [ -f "{armed_path}" ]; then
    echo true
  else
    echo false
  fi
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  echo 'https://example.com/pr/912'
  exit 0
fi
if [ "$1 $2" = "pr merge" ]; then
  touch "{armed_path}"
  exit 0
fi
exit 0
"#
    )
}

fn claude_script() -> &'static str {
    "#!/bin/sh\necho '{\"title\":\"generated title\",\"body\":\"generated body\"}'\nexit 0\n"
}

#[test]
fn land_local_squash_merges_to_main() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land");

    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo.path())
        .output()
        .expect("git rev-parse");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "main");
    assert!(!local_branch_exists(&repo, "feature"));
    assert!(repo.path().join("feature.txt").exists());
}

#[test]
fn land_preserves_main_on_failure() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();
    let main_head = repo.head_sha();

    repo.checkout("feature");
    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    );

    assert!(result.is_err());
    let _ = Command::new("git")
        .args(["rebase", "--abort"])
        .current_dir(repo.path())
        .status();
    let _ = Command::new("git")
        .args(["reset", "--hard"])
        .current_dir(repo.path())
        .status();
    repo.checkout("main");
    assert_eq!(repo.head_sha(), main_head);
}

#[test]
fn land_cleans_up_remote_branch() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land");

    assert!(!remote_branch_exists(&repo, "feature"));
}

#[test]
fn land_clears_scratch_and_preserves_gitkeep() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(scratch.join("nested")).expect("create nested scratch dir");
    fs::write(scratch.join("notes.md"), "review notes").expect("write scratch note");
    fs::write(scratch.join("nested").join("todo.md"), "todo").expect("write nested scratch note");
    let status = Command::new("git")
        .args(["add", "scratch"])
        .current_dir(repo.path())
        .status()
        .expect("git add scratch");
    assert!(status.success(), "git add scratch should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "add scratch docs"])
        .current_dir(repo.path())
        .status()
        .expect("git commit scratch");
    assert!(status.success(), "git commit scratch should succeed");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land should clear scratch");

    let scratch_entries = fs::read_dir(repo.path().join("scratch"))
        .expect("read scratch after land")
        .map(|entry| {
            entry
                .expect("scratch entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(scratch_entries, vec![".gitkeep"]);
}

#[test]
fn land_missing_pr_error_includes_branch_name() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_no_pr_script()),
            ("claude", claude_script()),
            ("open", noop_open_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("cached title".to_string()),
            pr_body: Some("cached body".to_string()),
            agent: None,
        },
        &NullProgress,
    );

    let Err(OpsError::Message(message)) = result else {
        panic!("expected missing PR message");
    };
    assert!(message.contains("no open PR found for branch 'feature'"));
}

#[test]
fn land_uses_cached_pr_copy_when_available() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "cached title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "cached body").expect("write body");
    fs::write(scratch.join(".pr-copy-ref"), repo.head_sha()).expect("write ref");

    let log_path = repo.path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    land(
        repo.path(),
        &LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land with cached copy");

    let log = fs::read_to_string(log_path).expect("read gh log");
    assert!(log.contains("--title cached title"));
    assert!(log.contains("--body cached body"));
}

#[test]
fn submit_and_land_make_no_presentation_attempt() {
    let marker_dir = tempfile::TempDir::new().expect("marker dir");
    let marker = marker_dir.path().join("present.log");
    let open_script = counting_open_script(&marker);

    // submit prepares the PR for a human to merge — and presents nothing.
    let submit_repo = TestRepo::new();
    submit_repo.create_branch("feature");
    submit_repo.create_file("feature.txt", "feature");
    submit_repo.stage_all();
    submit_repo.commit("feature work");
    push_branch(&submit_repo, "feature");
    let submit_log = submit_repo.path().join("gh.log");
    let submit_script = gh_land_script(submit_log.to_string_lossy().as_ref());
    {
        let _env = EnvGuard::new(&[
            ("gh", submit_script.as_str()),
            ("open", open_script.as_str()),
            ("xdg-open", open_script.as_str()),
        ]);
        submit(
            submit_repo.path(),
            &LandOptions {
                strict: true,
                local: false,
                create_pr: true,
                complete: false,
                next_slug: None,
                worktree: None,
                commit_message: None,
                pr_title: Some("test title".to_string()),
                pr_body: Some("test body".to_string()),
                agent: None,
            },
            &NullProgress,
        )
        .expect("submit");
    }
    assert_eq!(
        presentation_attempts(&marker),
        0,
        "submit must open no review surface"
    );

    // land arms auto-merge and walks away — also presenting nothing.
    let land_repo = TestRepo::new();
    land_repo.create_branch("feature");
    land_repo.create_file("feature.txt", "feature");
    land_repo.stage_all();
    land_repo.commit("feature work");
    push_branch(&land_repo, "feature");
    let land_log = land_repo.path().join("gh.log");
    let land_script = gh_land_script(land_log.to_string_lossy().as_ref());
    {
        let _env = EnvGuard::new(&[
            ("gh", land_script.as_str()),
            ("open", open_script.as_str()),
            ("xdg-open", open_script.as_str()),
        ]);
        land(
            land_repo.path(),
            &LandOptions {
                strict: true,
                local: false,
                create_pr: true,
                complete: false,
                next_slug: None,
                worktree: None,
                commit_message: None,
                pr_title: Some("test title".to_string()),
                pr_body: Some("test body".to_string()),
                agent: None,
            },
            &NullProgress,
        )
        .expect("land");
    }
    assert_eq!(
        presentation_attempts(&marker),
        0,
        "land must open no review surface"
    );
}

#[test]
fn submit_assigns_reviewer_and_skips_auto_merge() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let log_path = repo.path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("submit");

    // submit prepares but never merges — that click is the human's.
    let log = fs::read_to_string(log_path).expect("read gh log");
    // Assigns the PR to the current user for a required, manual merge.
    assert!(log.contains("pr edit --add-assignee @me"));
    // Marks the PR ready, but does NOT arm auto-merge.
    assert!(log.contains("pr ready"));
    assert!(!log.contains("merge --auto"));
}

#[test]
fn managed_task_land_requires_its_active_provider_session() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let script = gh_existing_pr_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let base = repo.head_sha();
    let branch = "jack/task-pr-proof";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch(branch);
    let _task = register_task(home.path(), repo.path(), branch, &base);

    let submit_error = submit(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("managed Task submit must be rejected");
    assert!(submit_error
        .to_string()
        .contains("cannot decide a managed Task lifecycle"));

    let open = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["pr", "open"])
        .current_dir(repo.path())
        .output()
        .expect("run managed Task pr open");
    assert!(!open.status.success());
    assert!(
        String::from_utf8_lossy(&open.stderr).contains("cannot decide a managed Task lifecycle")
    );

    let error = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: true,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("managed Task land must not bypass its provider session");
    assert!(error
        .to_string()
        .contains("active provider-backed Task Session"));
    assert!(
        !log_path.exists() || fs::read_to_string(log_path).unwrap_or_default().is_empty(),
        "rejected Task land must not mutate GitHub"
    );
}

#[test]
fn approved_managed_task_cli_land_records_disposition_and_arms_merge() {
    let home = tempfile::TempDir::new().expect("temp home");
    let repo = TestRepo::new();
    let log_path = home.path().join("gh.log");
    let armed_path = home.path().join("auto-merge-armed");
    let script = gh_replay_safe_task_land_script(
        log_path.to_string_lossy().as_ref(),
        armed_path.to_string_lossy().as_ref(),
    );
    let _env = EnvGuard::with_lf_home(
        &[("gh", script.as_str()), ("open", noop_open_script())],
        home.path(),
    );
    let base = repo.head_sha();
    let branch = "jack/task-pr-approved";
    repo.create_branch(branch);
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let head = repo.head_sha();
    let now = OffsetDateTime::now_utc();
    let fingerprint = hex::encode(Sha256::digest(
        loopflow::engine::git::material_worktree_state(repo.path())
            .expect("worktree state")
            .as_bytes(),
    ));
    let mut pr = task.pr.clone();
    pr.publication = Some(PrPublication {
        requested_at: now,
        after_merge: AfterMerge::Review,
        next_slug: None,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(head.clone()),
        }),
    });
    pr.ci_observation = Some(CiObservation {
        head_sha: head.clone(),
        state: CiState::Passing,
        failing_checks: Vec::new(),
        observed_at: now,
    });
    let mut session = task.session.clone();
    session.lifecycle_phase = TaskLifecyclePhase::Gate;
    session.phase_epoch = 2;
    session.gate_cycle = 1;
    session.gate_proposal = Some(TaskGateProposal {
        status: TaskSessionStatus::Waiting,
        reason: "reviewed outcome is ready".to_string(),
        settlement: Some(TaskSettlementIntent {
            pr_id: pr.id.clone(),
            head_sha: head,
            worktree_fingerprint: fingerprint,
            after_merge: AfterMerge::Review,
            next_slug: None,
            requested_at: now,
            lifecycle_approved_at: None,
            armed_at: None,
        }),
    });
    let runtime = tokio::runtime::Runtime::new().expect("task runtime");
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("publish PR evidence");
    let proposal = serde_json::to_string(
        session
            .gate_proposal
            .as_ref()
            .expect("Gate proposal exists"),
    )
    .expect("serialize Gate proposal");
    let lease_token = "cl_00000000000000000000000000000001";
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open registry")
        .execute(
            "UPDATE task_sessions SET lifecycle_phase='gate', phase_epoch=?2, gate_cycle=?3, gate_proposal_json=?4, status='running', process_generation=1, process_tmux_name='task-land', process_started_at=?5, process_lease_token=?6, process_agent='codex', process_provider='codex', process_lease_state='active' WHERE id=?1",
            rusqlite::params![session.id.as_str(), session.phase_epoch, session.gate_cycle, proposal, now.unix_timestamp_nanos() as i64, lease_token],
        )
        .expect("enter Gate");
    std::env::set_var("LF_TASK_SESSION_ID", session.id.as_str());
    std::env::set_var("LF_TASK_GENERATION", "1");
    std::env::set_var("LF_TASK_LEASE_TOKEN", lease_token);
    let stored = runtime
        .block_on(task.store.get_task_session(&session.id))
        .expect("read Gate")
        .expect("Task exists");
    assert_eq!(stored.lifecycle_phase, TaskLifecyclePhase::Gate);

    let premature = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: true,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("an unfinished Gate must not land");
    assert!(premature
        .to_string()
        .contains("waiting for the current Gate lifecycle to finish"));

    session
        .gate_proposal
        .as_mut()
        .and_then(|proposal| proposal.settlement.as_mut())
        .expect("settlement exists")
        .lifecycle_approved_at = Some(now);
    let approved_proposal = serde_json::to_string(
        session
            .gate_proposal
            .as_ref()
            .expect("Gate proposal exists"),
    )
    .expect("serialize approved Gate proposal");
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open registry")
        .execute(
            "UPDATE task_sessions SET gate_proposal_json=?2 WHERE id=?1",
            rusqlite::params![session.id.as_str(), approved_proposal],
        )
        .expect("approve Gate lifecycle");

    pr.ci_observation.as_mut().expect("CI evidence").state = CiState::Pending;
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("record pending checks");
    let pending = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: true,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("pending required checks must not land");
    assert!(pending
        .to_string()
        .contains("waiting for required GitHub checks"));
    pr.ci_observation.as_mut().expect("CI evidence").state = CiState::Passing;
    runtime
        .block_on(task.store.update_task_pr(&pr))
        .expect("record passing checks");

    repo.create_file("feature.txt", "changed after review");
    let stale = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            complete: true,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect_err("changed reviewed outcome must not land");
    assert!(stale.to_string().contains("changed after Gate evidence"));
    assert!(
        !log_path.exists()
            || !fs::read_to_string(&log_path)
                .unwrap_or_default()
                .contains("pr merge")
    );
    repo.create_file("feature.txt", "feature");

    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["pr", "land", "-c"])
        .current_dir(repo.path())
        .status()
        .expect("run approved Task land");
    assert!(status.success());

    let replay = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["pr", "land", "-c"])
        .current_dir(repo.path())
        .status()
        .expect("replay approved Task land");
    assert!(replay.success());

    let landed = runtime
        .block_on(task.store.get_task_session(&session.id))
        .expect("read Task")
        .expect("Task exists");
    assert!(landed
        .gate_proposal
        .as_ref()
        .and_then(|proposal| proposal.settlement.as_ref())
        .and_then(|settlement| settlement.armed_at)
        .is_some());
    let pr = runtime
        .block_on(task.store.active_task_pr(&session.id))
        .expect("read PR")
        .expect("PR exists");
    assert_eq!(
        pr.publication.map(|publication| publication.after_merge),
        Some(AfterMerge::CompleteTask)
    );
    let log = fs::read_to_string(log_path).expect("read gh log");
    assert_eq!(
        log.lines()
            .filter(|line| line.contains("pr merge --squash --auto"))
            .count(),
        1
    );
}

#[test]
fn submit_does_not_rotate_worktree() {
    let repo = TestRepo::new();
    let log_path = repo.path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    let worktree = create_named_worktree(repo.path(), "sub", None, false).expect("create worktree");
    fs::write(worktree.path.join("feature.txt"), "feature").expect("write feature file");
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree.path)
        .status()
        .expect("git add");
    assert!(status.success(), "git add should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "feature work"])
        .current_dir(&worktree.path)
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");
    push_branch(&repo, &worktree.branch);

    submit(
        &worktree.path,
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: Some("test title".to_string()),
            pr_body: Some("test body".to_string()),
            agent: None,
        },
        &NullProgress,
    )
    .expect("submit from worktree");

    // The worktree stays put — no preserve, no next-item rotation.
    assert!(worktree.path.exists());
}

#[test]
fn land_generates_copy_when_cached_pr_copy_is_stale() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_home(
        &[
            ("gh", gh_no_pr_script()),
            ("claude", claude_script()),
            ("open", noop_open_script()),
        ],
        Some(home.path()),
    );
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "stale title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "stale body").expect("write body");
    fs::write(
        scratch.join(".pr-copy-ref"),
        "0000000000000000000000000000000000000000",
    )
    .expect("write stale ref");
    let status = Command::new("git")
        .args(["add", "scratch"])
        .current_dir(repo.path())
        .status()
        .expect("git add scratch");
    assert!(status.success(), "git add scratch should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "add stale gate copy"])
        .current_dir(repo.path())
        .status()
        .expect("git commit scratch");
    assert!(status.success(), "git commit scratch should succeed");

    land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: true,
            complete: false,
            next_slug: None,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
            agent: None,
        },
        &NullProgress,
    )
    .expect("land with stale cached copy should regenerate");
}

#[test]
fn lf_ops_land_leaves_worktree_in_place() {
    let repo = TestRepo::new();
    let log_path = repo.path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str()), ("open", noop_open_script())]);

    let worktree =
        create_named_worktree(repo.path(), "land", None, false).expect("create worktree");

    fs::write(worktree.path.join("feature.txt"), "feature").expect("write feature file");
    let status = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree.path)
        .status()
        .expect("git add");
    assert!(status.success(), "git add should succeed");
    let status = Command::new("git")
        .args(["commit", "-m", "feature work"])
        .current_dir(&worktree.path)
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");
    push_branch(&repo, &worktree.branch);

    let directive_path = repo.path().join("directive.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "pr",
            "land",
            "--strict",
            "--create-pr",
            "--title",
            "test title",
            "--body",
            "test body",
        ])
        .current_dir(&worktree.path)
        .env("LOOPFLOW_DIRECTIVE_FILE", &directive_path)
        .status()
        .expect("run lf pr land");
    assert!(status.success(), "lf pr land should succeed");

    // The wave home is permanent: land never rotates the worktree or cds away.
    assert!(
        worktree.path.exists(),
        "worktree should stay in place after land"
    );
    let directive = fs::read_to_string(&directive_path).unwrap_or_default();
    assert!(
        !directive.contains("cd "),
        "land should not emit a cd directive, got: {directive}"
    );
}
