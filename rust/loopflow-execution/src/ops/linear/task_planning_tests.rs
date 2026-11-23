//! Task planning operations against stateful, isolated Linear responses.

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::test_fixture::{now, Fixture, PlanningEnvironment};
use super::LINEAR_TEST_CONTEXT;
use crate::ops::wave_lifecycle;
use crate::ops::NullProgress;
use loopflow_model::durable::WorkStatus;
use loopflow_storage::sqlite::tracker_actions::ActionPhase;
use loopflow_storage::work::task::{
    LinearWritebackState, Observation, PrPhase, Task, TaskPr, TaskPrId,
};

use crate::test_support::planning_tasks::*;

#[test]
fn connect_links_a_new_wave_without_creating_a_linear_project() {
    with_planning_task(|runtime, _, repo, _, state| {
        super::test_fixture::commit_planning_repo(repo);
        let existing = json!({
            "id": "refunds-project", "name": "Refunds", "description": "", "content": "",
            "status": {"type": "started"}, "teams": {"nodes": [{"id": "team-1"}]}
        });
        {
            let mut provider = runtime.block_on(state.lock());
            provider.export_mode = true;
            provider.extra_projects.push(existing.clone());
        }
        let result = super::connect(
            repo,
            &super::LinearInitOptions {
                wave: Some("ops".into()),
                project: Some("Refunds".into()),
                ..Default::default()
            },
            &NullProgress,
        )
        .unwrap();

        assert_eq!(result.project_id.as_deref(), existing["id"].as_str());
        assert!(!result.created);
        let provider = runtime.block_on(state.lock());
        assert!(!provider.operations.contains_key("DeliverProjectCreation"));
        assert_eq!(provider.extra_projects, [existing]);
    });
}

#[test]
fn adding_and_renaming_waves_send_their_projects_ahead_of_older_work() {
    with_planning_task(|runtime, fixture, repo, _, state| {
        super::test_fixture::commit_planning_repo(repo);
        runtime.block_on(async {
            let repo_key = repo.to_str().unwrap();
            for n in 0..(2 * crate::ops::tracker::drain::Bound::PASS.requests) {
                fixture
                    .store
                    .create_wave(&crate::work::wave::Wave::new(
                        loopflow_model::id::WaveId::new(),
                        format!("older-{n}"),
                        repo_key.into(),
                    ))
                    .await
                    .unwrap();
            }
            fixture
                .store
                .sqlite
                .enqueue_tracker_creations(
                    repo_key,
                    loopflow_storage::planning_sync::TrackerKind::Linear,
                )
                .unwrap();
            state.lock().await.export_mode = true;

            wave_lifecycle::add(repo, "billing", None).await.unwrap();
            assert!(state
                .lock()
                .await
                .extra_projects
                .iter()
                .any(|p| p["name"] == "billing"));

            wave_lifecycle::rename(repo, "product", Some("payments"), None)
                .await
                .unwrap();
            let provider = state.lock().await;
            let renamed = provider.project_fields.as_ref().unwrap();
            assert_eq!(renamed["id"], "project-1");
            assert_eq!(renamed["name"], "payments");
        });
    });
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_codex_task_headless() {
    assert_native_runner_is_quiet_until_its_turn_ends("codex", true);
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_claude_task_headless() {
    assert_native_runner_is_quiet_until_its_turn_ends("claude", true);
}

#[test]
fn native_runner_is_quiet_until_its_turn_ends_claude_taskless_headless() {
    assert_native_runner_is_quiet_until_its_turn_ends("claude", false);
}

fn assert_native_runner_is_quiet_until_its_turn_ends(agent: &str, task_bound: bool) {
    with_planning_task(|runtime, fixture, repo, task, state| {
        let _environment = crate::test_ambient::EnvGuard::clear(&[
            "HOME",
            "LF_HOME",
            "LF_BIN",
            "PATH",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
        ]);
        let home = fixture.directory.path().join("provider-home");
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("LF_HOME", fixture.directory.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let path = format!("{}:/usr/bin:/bin", bin.display());
        std::env::set_var("PATH", &path);
        let started = home.join("started");
        let stop = home.join("stop");
        if agent == "codex" {
            crate::test_support::install_native_codex(&bin);
        } else {
            let executable = bin.join(agent);
            std::fs::write(
                &executable,
                include_str!("../../../tests/support/holding_provider.py"),
            )
            .unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        runtime.block_on(async {
            seed_finished_tasks(fixture, repo, &state).await;
            state.lock().await.requests = 0;
        });
        let process = crate::agent::ProcessConfig {
            task_input: task_bound.then(|| {
                crate::ops::task_input::TaskInput::new(
                    fixture.store.clone(),
                    crate::ops::task_input::TaskSeed {
                        task: task.clone(),
                        message: String::new(),
                        interrupt: 0,
                    },
                )
            }),
            ..Default::default()
        };
        let launch = crate::agent::AgentConfig {
            agent: crate::config::Agent::parse(agent).unwrap(),
            cwd: Some(repo.to_path_buf()),
            task_prompt: "Hold the turn open".into(),
            env: std::collections::BTreeMap::from([
                ("PATH".into(), path),
                ("FIXTURE_STARTED".into(), started.display().to_string()),
                ("FIXTURE_STOP".into(), stop.display().to_string()),
            ]),
            ..crate::agent::AgentConfig::default()
        };
        let context = LINEAR_TEST_CONTEXT.with(Clone::clone);
        let database = fixture.database.clone();
        runtime.block_on(async {
            let running = tokio::task::spawn_blocking(move || {
                crate::journal::with_test_ledger(database, || {
                    LINEAR_TEST_CONTEXT.sync_scope(context, || {
                        crate::agent::run_agent(
                            &launch,
                            &process,
                            crate::harness::LaunchKind::Batch,
                        )
                    })
                })
            });
            let observed = tokio::time::timeout(std::time::Duration::from_secs(70), async {
                while !started.exists() {
                    assert!(
                        !running.is_finished(),
                        "fixture provider exited before readiness"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                // Native agent stays open without a watcher. Only the turn
                // boundary may acquire the change made while it was idle.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                assert_eq!(state.lock().await.requests, 0);
                let mut provider = state.lock().await;
                provider.issues[0]["title"] = json!("Changed during the turn");
                mark_issue_updated(&mut provider.issues[0]);
            })
            .await;
            std::fs::write(&stop, "stop").unwrap();
            assert_eq!(running.await.unwrap().unwrap().exit_code, 0);
            observed.unwrap();
            // The turn's delta, then the Project its changed issue names.
            assert_eq!(state.lock().await.requests, 2);
            assert_eq!(
                fixture
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                "Changed during the turn"
            );
        });
    });
}

#[test]
fn task_abandon_composes_cancellation_pr_and_git_from_anywhere() {
    let _lock = crate::journal::test_env_lock();
    for selector in [Some("FIX-1"), Some("cancel-me"), None] {
        let _restore = PlanningEnvironment::isolate();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let fixture = runtime.block_on(Fixture::new());
        std::env::set_var("LF_HOME", fixture.directory.path());
        let (repo, wave) = runtime.block_on(planning_repo(&fixture));
        runtime.block_on(fixture.seed(now() + 86_400));
        let remote = fixture.directory.path().join("loopflowstudio/fixture.git");
        std::fs::create_dir_all(&remote).unwrap();
        let git = |cwd: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(args)
                .current_dir(cwd)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };
        git(&remote, &["init", "--bare", "-q"]);
        git(&repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        git(&repo, &["config", "user.name", "Fixture"]);
        git(&repo, &["config", "user.email", "fixture@example.com"]);
        git(
            &repo,
            &["remote", "set-url", "origin", remote.to_str().unwrap()],
        );
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "baseline"]);
        git(&repo, &["push", "-u", "origin", "main"]);
        let checkout = fixture.directory.path().join("task");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "cancel-me",
                checkout.to_str().unwrap(),
            ],
        );
        git(&checkout, &["push", "-u", "origin", "cancel-me"]);
        let bin = fixture.directory.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let gh = bin.join("gh");
        // Git is real; only GitHub is simulated. State survives separate calls.
        std::fs::write(&gh, r#"#!/bin/sh
root=$(git rev-parse --git-common-dir)
case "$2" in
  list) if [ -f "$root/pr-closed" ]; then echo '[{"number":42,"state":"CLOSED","headRepository":{"nameWithOwner":"loopflowstudio/fixture"}}]'; else echo '[{"number":42,"state":"OPEN","headRepository":{"nameWithOwner":"loopflowstudio/fixture"}}]'; fi ;;
  close) if [ -f "$root/fail-close" ]; then echo 'GitHub unavailable' >&2; exit 1; fi; touch "$root/pr-closed" ;;
  *) echo 'Unexpected GitHub operation' >&2; exit 1 ;;
esac
"#).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH").unwrap();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let state = Arc::new(tokio::sync::Mutex::new(PlanningState::default()));
        let (url, server) = runtime.block_on(serve(state.clone()));
        LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
            let item =
                seed_provider_task(&runtime, &state, &repo, "Cancel me", "Keep history").unwrap();
            let timestamp = time::OffsetDateTime::now_utc();
            let project = runtime
                .block_on(fixture.store.project_of_wave(wave.id()))
                .unwrap()
                .unwrap();
            assert_eq!(project.tracker_id.as_ref().unwrap().as_str(), "project-1");
            let task = Task {
                parent_task_id: None,
                repository_id: crate::repository::location::ensure_repository(
                    &fixture.store.sqlite,
                    repo.to_str().unwrap(),
                )
                .unwrap(),
                id: runtime
                    .block_on(fixture.store.get_task_by_issue(&item.id))
                    .unwrap()
                    .unwrap()
                    .id,

                revision: 0,
                tracker_id: Some(crate::linear::LinearIssueId::new(&item.id).unwrap()),
                identifier: item.identifier.clone(),
                title: item.name,
                description: item.description,
                tracker_synced_at: Some(1),

                tracker_writeback: LinearWritebackState::Current,
                wave_id: Some(wave.id().clone()),
                project_id: Some(project.id),
                worktree: Some(checkout.clone()),
                workspace_slug: "cancel-me".into(),
                branch: "cancel-me".into(),
                base_commit: git(&repo, &["rev-parse", "HEAD"]).trim().into(),
                parent_pr_id: None,
                created_at: timestamp,
                updated_at: timestamp,
                rank: 0,
                assignee: None,
                due_date: None,
                state: None,
                completed_at: None,
                observation: Observation::NotRequired,
            };
            let pr = TaskPr {
                id: TaskPrId::new(),
                task_id: task.id.clone(),
                sequence: 1,
                slug: "cancel-me".into(),
                branch: "cancel-me".into(),
                base_commit: git(&repo, &["rev-parse", "HEAD"]).trim().into(),
                parent_pr_id: None,
                publication: None,
                merge_commit: None,
                abandoned_at: None,
                ci_observation: None,
                github_observation: None,
                linear_attachment_id: None,
                linear_comment_id: None,
                linear_link_error: None,
                created_at: timestamp,
                updated_at: timestamp,
            };
            runtime
                .block_on(fixture.store.seed_task(&task, &pr))
                .unwrap();
            fixture.store.sqlite.test_graph(
                "review",
                &task.worktree.as_ref().unwrap().to_string_lossy(),
                &[("review", Some("failed"))],
                Some("failed"),
            );
            assert_eq!(
                crate::ops::task::task_repository(&checkout, None).unwrap(),
                checkout.canonicalize().unwrap()
            );
            let caller = match selector {
                None => &checkout,
                Some("FIX-1") => fixture.directory.path(),
                Some(_) => &repo,
            };
            if selector == Some("cancel-me") {
                let progress = LifecycleMessages::default();
                crate::ops::abandon_branch(
                    &repo,
                    &crate::ops::AbandonOptions {
                        branch: Some("cancel-me".into()),
                        force: true,
                    },
                    &progress,
                )
                .unwrap();
                assert!(progress
                    .0
                    .borrow()
                    .iter()
                    .any(|message| message.contains("Task FIX-1")
                        && message.contains("remain unchanged")));
                assert_eq!(
                    runtime
                        .block_on(
                            fixture
                                .store
                                .work_status(&loopflow_model::durable::WorkRef::Task(
                                    task.id.clone()
                                ))
                        )
                        .unwrap(),
                    WorkStatus::Ready
                );
                assert_eq!(
                    runtime
                        .block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
                    json!("unstarted")
                );
                assert!(!checkout.exists());
            } else {
                // Cleanup failure cannot undo cancellation; retry retains the PR and checkout.
                std::fs::write(repo.join(".git/fail-close"), "").unwrap();
                assert_eq!(
                    crate::ops::task::task_abandon(caller, selector, false).unwrap(),
                    "FIX-1"
                );
                assert_eq!(
                    runtime
                        .block_on(
                            fixture
                                .store
                                .work_status(&loopflow_model::durable::WorkRef::Task(
                                    task.id.clone()
                                ))
                        )
                        .unwrap(),
                    WorkStatus::Abandoned
                );
                assert_eq!(
                    runtime
                        .block_on(async { state.lock().await.issues[0]["state"]["type"].clone() }),
                    json!("canceled")
                );
                assert!(matches!(
                    fixture
                        .store
                        .sqlite
                        .task(&task.id)
                        .unwrap()
                        .unwrap()
                        .tracker_writeback,
                    LinearWritebackState::Current
                ));
                assert!(checkout.exists());
                std::fs::remove_file(repo.join(".git/fail-close")).unwrap();
            }
            assert_eq!(
                crate::ops::task::task_abandon(caller, selector, false).unwrap(),
                "FIX-1"
            );
            assert!(!checkout.exists());
            assert!(git(
                &repo,
                &["ls-remote", "--heads", "origin", "refs/heads/cancel-me"]
            )
            .is_empty());
            assert!(git(&repo, &["branch", "--list", "cancel-me"]).is_empty());
            assert!(repo.join(".git/pr-closed").exists());
            // The Graph's Processes remain as history; abandonment retires nothing.
            assert_eq!(fixture.store.sqlite.task_graphs(&task.id).unwrap().len(), 1);
            assert_eq!(
                runtime
                    .block_on(
                        fixture
                            .store
                            .work_status(&loopflow_model::durable::WorkRef::Task(task.id.clone()))
                    )
                    .unwrap(),
                WorkStatus::Abandoned
            );
            assert_eq!(
                runtime.block_on(fixture.store.task_prs(&task.id)).unwrap()[0].phase(),
                PrPhase::Abandoned
            );
            // Retry survives deletion of the checkout and refs.
            crate::ops::task::task_abandon(&repo, Some("cancel-me"), false).unwrap();
        });
        server.abort();
        std::env::set_var("PATH", path);
    }
}

#[test]
fn foreign_projects_do_not_block_refresh() {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = runtime.block_on(planning_repo(&fixture));
    runtime.block_on(fixture.seed(now() + 86_400));
    for args in [
        vec!["branch", "-M", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "merged goals",
        ],
    ] {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    let foreign = json!({
        "id":"foreign-project", "name":"Other Repository — Technical Architecture",
        "description":"", "content":"", "status":{"type":"started"},
        "initiatives":{"nodes":[{"id":"initiative-1"},{"id":"other-initiative"}]},
        "teams":{"nodes":[{"id":"other-team"}]}
    });
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        extra_projects: vec![foreign.clone()],
        ..PlanningState::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        // Import owned planning while foreign Projects remain independently unavailable.
        seed_provider_task(
            &runtime,
            &state,
            &repo,
            "Eligible work",
            "Cancel only repository work",
        )
        .unwrap();
        let result = super::refresh(
            &repo,
            &super::LinearSyncOptions {
                wave: Some("product".into()),
            },
            &NullProgress,
        )
        .unwrap();
        assert!(!result
            .actions
            .iter()
            .any(|action| action.contains("Technical Architecture")));
        runtime.block_on(async {
            let provider = state.lock().await;
            assert_eq!(provider.extra_projects, vec![foreign.clone()]);
            let project = fixture
                .store
                .project_of_wave(wave.id())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(project.tracker_id.as_ref().unwrap().as_str(), "project-1");
            assert_eq!(project.name, "Project");
        });
    });
    server.abort();
}

#[derive(Default)]
struct LifecycleMessages(RefCell<Vec<String>>);

impl crate::ops::Progress for LifecycleMessages {
    fn status(&self, message: &str) {
        self.0.borrow_mut().push(message.to_string());
    }
    fn error(&self, message: &str) {
        self.status(message);
    }
    fn confirm(&self, _: &str) -> bool {
        true
    }
}

fn with_export_plan(
    test: impl FnOnce(
        &tokio::runtime::Runtime,
        &Fixture,
        &Path,
        &Task,
        Arc<tokio::sync::Mutex<PlanningState>>,
    ),
) {
    let _lock = crate::journal::test_env_lock();
    let _restore = PlanningEnvironment::isolate();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(Fixture::new());
    std::env::set_var("LF_HOME", fixture.directory.path());
    let (repo, wave) = runtime.block_on(fixture.local_planning_repo());
    let project = fixture.store.sqlite.seed_project(wave.id()).unwrap().id;
    let task = fixture
        .store
        .sqlite
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: crate::repository::location::ensure_repository(
                &fixture.store.sqlite,
                repo.to_str().unwrap(),
            )
            .unwrap(),
            due_date: None,
            id: loopflow_model::durable::TaskId::new(),
            project_id: Some(project),
            title: "Same title".into(),
            description: "Saved offline".into(),
        })
        .unwrap();
    runtime.block_on(fixture.seed(now() + 86_400));
    let state = Arc::new(tokio::sync::Mutex::new(PlanningState {
        export_mode: true,
        ..Default::default()
    }));
    let (url, server) = runtime.block_on(serve(state.clone()));
    LINEAR_TEST_CONTEXT.sync_scope(fixture.context(&url), || {
        test(&runtime, &fixture, &repo, &task, state)
    });
    server.abort();
}

#[test]
fn task_create_in_linear_repository_returns_published_task_without_execution() {
    with_export_plan(|runtime, fixture, repo, task, _state| {
        let wave = runtime
            .block_on(fixture.store.get_wave(task.wave_id.as_ref().unwrap()))
            .unwrap()
            .unwrap();
        let saved = crate::ops::task::task_create(
            repo,
            Some(wave.slug()),
            Some("New Linear Task".into()),
            Some("Retain this work".into()),
            crate::ops::task::TaskAdoptionOptions::default(),
            Default::default(),
            None,
        )
        .unwrap();
        assert!(saved.tracker_id.is_some());
        assert!(saved.identifier.starts_with("FIX-"));
        assert!(saved.worktree.is_none());
    });
}

#[test]
fn a_follow_up_is_created_in_linear_under_its_parents_issue() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        let wave = runtime
            .block_on(fixture.store.get_wave(task.wave_id.as_ref().unwrap()))
            .unwrap()
            .unwrap();
        let file = |title: &str, parent: Option<&Task>| {
            crate::ops::task::task_create(
                repo,
                Some(wave.slug()),
                Some(title.into()),
                Some("Retain this work".into()),
                crate::ops::task::TaskAdoptionOptions::default(),
                parent.map(|parent| parent.identifier.clone()),
                None,
            )
            .unwrap()
        };
        let parent = file("Parent", None);
        let child = file("Follow-up", Some(&parent));
        let issue = runtime
            .block_on(state.lock())
            .issues
            .iter()
            .find(|issue| issue["id"] == child.tracker_id.as_ref().unwrap().as_str())
            .cloned()
            .expect("the follow-up reached Linear");
        assert_eq!(
            issue["parent"]["id"],
            parent.tracker_id.as_ref().unwrap().as_str()
        );
        // What Linear answered names the same parent, so the link stands.
        assert_eq!(child.parent_task_id, Some(parent.id.clone()));

        // Reparenting via `task_edit` refuses cycles and syncs clear/set to Linear.
        let cycle = crate::ops::task::task_edit(
            repo,
            &parent.identifier,
            None,
            loopflow_storage::work::task::TaskUpdate::default(),
            Some(&child.identifier),
        );
        assert!(cycle
            .unwrap_err()
            .to_string()
            .contains("cannot be its own ancestor"));

        crate::ops::task::task_edit(
            repo,
            &child.identifier,
            None,
            loopflow_storage::work::task::TaskUpdate::default(),
            Some("none"),
        )
        .unwrap();
        let unparented = runtime
            .block_on(fixture.store.get_task(&child.id))
            .unwrap()
            .unwrap();
        assert_eq!(unparented.parent_task_id, None);
        let remote = runtime
            .block_on(state.lock())
            .issues
            .iter()
            .find(|issue| issue["id"] == child.tracker_id.as_ref().unwrap().as_str())
            .cloned()
            .unwrap();
        assert!(remote["parent"].is_null(), "{remote}");

        crate::ops::task::task_edit(
            repo,
            &child.identifier,
            None,
            loopflow_storage::work::task::TaskUpdate::default(),
            Some(&parent.identifier),
        )
        .unwrap();
        let adopted = runtime
            .block_on(fixture.store.get_task(&child.id))
            .unwrap()
            .unwrap();
        assert_eq!(adopted.parent_task_id, Some(parent.id));
        let remote = runtime
            .block_on(state.lock())
            .issues
            .iter()
            .find(|issue| issue["id"] == child.tracker_id.as_ref().unwrap().as_str())
            .cloned()
            .unwrap();
        assert_eq!(
            remote["parent"]["id"],
            parent.tracker_id.as_ref().unwrap().as_str()
        );
    });
}

#[test]
fn task_sync_still_adopts_a_newer_linear_state() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(state.lock()).fail_completion = true;
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"started"});
            mark_issue_updated(&mut provider.issues[0]);
        });
        crate::ops::task::task_sync(repo, task.id.as_str(), None).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert_eq!(saved.state.as_deref(), Some("started"));
        assert_eq!(runtime.block_on(state.lock()).completion_writes, 0);
    });
}

/// Completes the Task while Linear moves it to started first, so the
/// completion loses; then answers it.
fn answer_a_lost_completion(
    answer: loopflow_storage::planning_sync::LostAnswer,
) -> (Option<String>, usize) {
    let mut answered = None;
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(state.lock()).fail_completion = true;
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        runtime.block_on(async {
            let mut provider = state.lock().await;
            provider.issues[0]["state"] = json!({"type":"started"});
            mark_issue_updated(&mut provider.issues[0]);
        });
        let lost = || {
            fixture
                .store
                .task_tracker_sync(&task.id)
                .unwrap()
                .actions
                .into_iter()
                .filter(|action| action.phase == ActionPhase::Conflicted)
                .map(|action| action.line())
                .collect::<Vec<_>>()
        };
        crate::ops::task::task_sync(repo, task.id.as_str(), None).unwrap();
        assert_eq!(
            lost(),
            [r#"complete · conflicted · lost: "completed" · linear: "started""#]
        );
        crate::ops::task::task_sync(repo, task.id.as_str(), Some(answer)).unwrap();
        assert!(lost().is_empty());
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        answered = Some((
            saved.state,
            runtime.block_on(state.lock()).completion_writes,
        ));
    });
    answered.unwrap()
}

#[test]
fn a_lost_completion_closes_when_linear_changes_the_state_again() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(state.lock()).fail_completion = true;
        crate::ops::task::task_complete(repo, task.id.as_str(), None).unwrap();
        let move_to = |kind: &str| {
            runtime.block_on(async {
                let mut provider = state.lock().await;
                provider.issues[0]["state"] = json!({"type": kind});
                mark_issue_updated(&mut provider.issues[0]);
            })
        };
        let lost = || {
            fixture
                .store
                .task_tracker_sync(&task.id)
                .unwrap()
                .actions
                .iter()
                .filter(|action| action.phase == ActionPhase::Conflicted)
                .count()
        };
        move_to("started");
        crate::ops::task::task_sync(repo, task.id.as_str(), None).unwrap();
        assert_eq!(lost(), 1);
        // A coworker moves it again; the next refresh brings that event in.
        move_to("canceled");
        runtime
            .block_on(crate::ops::linear::refresh_wave(repo, "product"))
            .unwrap();
        assert_eq!(lost(), 0);
        assert_eq!(runtime.block_on(state.lock()).completion_writes, 0);
    });
}

#[test]
fn keeping_the_trackers_state_closes_the_lost_completion_and_sends_nothing() {
    assert_eq!(
        answer_a_lost_completion(loopflow_storage::planning_sync::LostAnswer::KeepTheirs),
        (Some("started".into()), 0)
    );
}

#[test]
fn using_mine_sends_the_lost_completion_again() {
    assert_eq!(
        answer_a_lost_completion(loopflow_storage::planning_sync::LostAnswer::UseMine),
        (Some("completed".into()), 1)
    );
}

#[test]
fn task_sync_publishes_saved_task_and_reuses_its_identity() {
    with_export_plan(|runtime, fixture, repo, task, state| {
        assert!(task.tracker_id.is_none());
        let result = crate::ops::task::task_sync(repo, task.id.as_str(), None).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert!(saved.tracker_id.is_some());
        assert!(result.contains(&saved.identifier));
        assert!(!saved.identifier.starts_with("lf-"));
        crate::ops::task::task_sync(repo, task.id.as_str(), None).unwrap();
        assert_eq!(runtime.block_on(state.lock()).creation_writes, 2);
    });
}

#[path = "planning_order_tests.rs"]
mod planning_order_tests;

#[path = "planning_action_tests.rs"]
mod planning_action_tests;

async fn seed_finished_tasks(
    fixture: &Fixture,
    repo: &Path,
    state: &tokio::sync::Mutex<PlanningState>,
) {
    {
        let mut provider = state.lock().await;
        for index in 2..=351 {
            let mut issue = provider.issues[0].clone();
            issue["id"] = json!(format!("issue-{index}"));
            issue["identifier"] = json!(format!("FIX-{index}"));
            issue["state"]["type"] = json!(if index % 2 == 0 {
                "completed"
            } else {
                "canceled"
            });
            provider.issues.push(issue);
        }
    }
    // Bulk fixture setup is not an opportunistic foreground refresh. Its disk
    // writes must finish before measuring on-demand requests on slower CI hosts.
    let context = super::resolve_repository_context(repo).await.unwrap();
    super::refresh_inventory(repo, "product", &context)
        .await
        .unwrap();
    rusqlite::Connection::open(&fixture.database)
        .unwrap()
        .execute(
            "UPDATE tasks SET abandoned_at=?1 WHERE planning_state='canceled'",
            [now()],
        )
        .unwrap();
}

#[test]
fn linear_on_demand_shares_checks_and_excludes_finished_tasks() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            seed_finished_tasks(fixture, repo, &state).await;
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            state.lock().await.requests = 0;
            let results = futures_util::future::join_all((0..4).map(|_| {
                crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap())
            }))
            .await;
            for result in results {
                result.unwrap();
            }
            // One shared delta, then one read of the Project its changed issue names.
            assert_eq!(state.lock().await.requests, 2);
            conn.execute(
                "UPDATE tasks SET abandoned_at=?1 WHERE id=?2",
                rusqlite::params![now(), task.id.as_str()],
            )
            .unwrap();
            assert!(!crate::ops::tracker_events::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .unwrap());
            assert_eq!(
                state.lock().await.requests,
                2,
                "a team without live Tasks is not checked"
            );
        });
    });
}

#[test]
fn linear_on_demand_acquires_comment_only_changes_and_completion() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            let timestamp = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap();
            state.lock().await.comments.push(json!({"id":"new-direction","body":"Keep the saved identity", "createdAt":timestamp,"updatedAt":timestamp,"user":{"id":"person-1","displayName":"Jack Heart","name":"Jack Heart"}}));
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", []).unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            assert!(fixture.store.task_steers(&task.id).await.unwrap().iter().any(|steer| steer.text.contains("Keep the saved identity")));
            {
                let mut provider = state.lock().await;
                provider.issues[0]["state"]["type"] = json!("completed");
                mark_issue_updated(&mut provider.issues[0]);
            }
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", []).unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap()).await.unwrap();
            assert!(fixture.store.task_state(&task.id).await.unwrap().is_terminal());
        });
    });
}

#[test]
fn linear_on_demand_failure_throttles_attempts_without_losing_changes() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let conn = rusqlite::Connection::open(&fixture.database).unwrap();
            conn.execute("DELETE FROM linear_team_checks", []).unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            let through: String = conn
                .query_row("SELECT synced_through FROM linear_team_checks", [], |row| {
                    row.get(0)
                })
                .unwrap();
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                .unwrap();
            state.lock().await.field_outage = true;
            assert!(crate::ops::tracker_events::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .is_err());
            let requests = state.lock().await.requests;
            assert!(!crate::ops::tracker_events::check_repository(
                &fixture.store,
                repo.to_str().unwrap()
            )
            .await
            .unwrap());
            assert_eq!(state.lock().await.requests, requests);
            assert_eq!(
                conn.query_row("SELECT synced_through FROM linear_team_checks", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
                through
            );
            {
                let mut provider = state.lock().await;
                provider.field_outage = false;
                provider.issues[0]["title"] = json!("Retained through the outage");
                mark_issue_updated(&mut provider.issues[0]);
            }
            conn.execute("UPDATE linear_team_checks SET attempted_at=0", [])
                .unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            assert_eq!(
                fixture
                    .store
                    .get_task(&task.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .title,
                "Retained through the outage"
            );
        });
    });
}

#[test]
fn linear_on_demand_adopts_new_open_issues_without_allocating_execution() {
    with_planning_task(|runtime, fixture, repo, task, state| {
        runtime.block_on(async {
            let mut issue = state.lock().await.issues[0].clone();
            issue["id"] = json!("new-open-issue");
            issue["identifier"] = json!("FIX-352");
            issue["title"] = json!("New work in the Wave's Project");
            mark_issue_updated(&mut issue);
            state.lock().await.issues.push(issue);
            rusqlite::Connection::open(&fixture.database)
                .unwrap()
                .execute("DELETE FROM linear_team_checks", [])
                .unwrap();
            crate::ops::tracker_events::check_repository(&fixture.store, repo.to_str().unwrap())
                .await
                .unwrap();
            let adopted = fixture
                .store
                .get_task_by_issue("new-open-issue")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(adopted.wave_id, task.wave_id);
            assert_eq!(adopted.title, "New work in the Wave's Project");
            assert_eq!(adopted.project_id, task.project_id);
            assert!(adopted.worktree.is_none());
            assert!(
                crate::store::task_work::task_graph(&fixture.store.sqlite, &adopted.id)
                    .unwrap()
                    .is_none()
            );
            assert!(fixture
                .store
                .sqlite
                .task_prs(&adopted.id)
                .unwrap()
                .is_empty());
        });
    });
}
