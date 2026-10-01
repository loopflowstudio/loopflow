use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::durable::{FlowSession, TaskWorkerClaimOutcome, TaskWorkerOwner};
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::engine::ExecutionCursor;
use loopflow::id::{ExecId, TraceId, WaveId};
use loopflow::planning::{LinearProjectId, ProjectPlan};
use loopflow::store::{open_ephemeral_store, PmSnapshotRow, StorageConfig};
use loopflow::work::project::{Project, ProjectId};
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;
use serde_json::json;

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn task_adopts_linear_checkout_and_preserves_saved_progress() {
    for (operation, remote_only) in [("checkout", false), ("start", false), ("checkout", true)] {
        let repo = TestRepo::new();
        repo.create_file(
            ".lf/config.yaml",
            "agent: claude\npm:\n  provider: linear\n  linear_team: team-1\n",
        );
        repo.create_file(
            "wave/product/GOAL.md",
            "---\npm:\n  linear_initiative: initiative-1\n---\nKeep working.\n",
        );
        repo.create_file(".lf/flows/adoption.yaml", "- step:\n    id: design\n    name: design\n    human: true\n- step:\n    id: demo\n    name: demo\n    human: true\n");
        repo.stage_all();
        repo.commit("Fixture planning");
        repo.push();
        let base = repo.head_sha();
        let branch = "linear-existing";
        let mut checkout = repo.create_named_worktree(branch);
        // The checkout path and branch deliberately differ from the issue title.
        fs::write(
            checkout.join("authored.txt"),
            "preserve this uncommitted work\n",
        )
        .unwrap();
        repo.create_file("main-notes", "preserve the invoking checkout too\n");
        if remote_only {
            for args in [
                vec!["add", "authored.txt"],
                vec!["commit", "-m", "Existing implementation"],
                vec!["push", "origin", branch],
            ] {
                assert!(Command::new("git")
                    .args(args)
                    .current_dir(&checkout)
                    .status()
                    .unwrap()
                    .success());
            }
        }
        let before = Command::new("git")
            .args(["status", "--porcelain=v1"])
            .current_dir(&checkout)
            .output()
            .unwrap()
            .stdout;
        let head = loopflow::engine::git::rev_parse(&checkout, "HEAD").unwrap();
        if remote_only {
            for args in [
                vec!["worktree", "remove", checkout.to_str().unwrap()],
                vec!["branch", "-D", branch],
                vec!["update-ref", "-d", "refs/remotes/origin/linear-existing"],
            ] {
                assert!(Command::new("git")
                    .args(args)
                    .current_dir(repo.path())
                    .status()
                    .unwrap()
                    .success());
            }
        }
        let home = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            )))
            .unwrap();
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            repo.path().canonicalize().unwrap().display().to_string(),
        );
        let project = Project {
            id: ProjectId::new(),
            wave_id: wave.id().clone(),
            plan: ProjectPlan {
                id: LinearProjectId::new("project-1").unwrap(),
                slug: "chapter".into(),
                name: "Chapter".into(),
                flow: "adoption".into(),
                status: loopflow::pm::ProjectStatus::Started,
                prompt_context: "Adopt existing work".into(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let snapshot: loopflow::pm::PmSnapshot = serde_json::from_value(json!({
            "projects": [{"id":"project-1", "slug":"chapter", "name":"Chapter",
                "summary":"", "metric_targets":[], "flow":"adoption", "status":"started",
                "krs":[], "initiative_ids":["initiative-1"], "team_ids":["team-1"]}],
            "items": [{"id":"issue-1", "identifier":"FIX-1", "branch_name":branch, "revision":"2026-09-29T12:00:00Z",
                "url":null, "name":"A different title", "description":"Existing implementation",
                "rank":0, "completed":false, "state":"started", "project_id":"project-1",
                "project":"chapter", "team_id":"team-1", "assignee":null}]
        }))
        .unwrap();
        runtime.block_on(async {
            store.create_wave(&wave).await.unwrap();
            store.create_project(&project).await.unwrap();
            let mut historical = snapshot.clone();
            historical.items[0].branch_name = None;
            store
                .put_pm_snapshot(PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative-1".into(),
                    synced_at: now.unix_timestamp(),
                    snapshot: historical,
                })
                .await
                .unwrap();
            store
                .put_pm_snapshot(PmSnapshotRow {
                    wave_id: wave.id().clone(),
                    provider: "linear".into(),
                    initiative: "initiative-1".into(),
                    synced_at: now.unix_timestamp(),
                    snapshot,
                })
                .await
                .unwrap();
            assert!(store.list_tasks(None).await.unwrap().is_empty());
        });
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let tmux = bin.join("tmux");
        // Review transport is outside this adoption proof; never launch a provider.
        fs::write(
            &tmux,
            "#!/bin/sh\n[ \"$1\" = has-session ] && exit 1\nexit 0\n",
        )
        .unwrap();
        fs::set_permissions(&tmux, fs::Permissions::from_mode(0o755)).unwrap();
        let gh = bin.join("gh");
        // Git and worktree operations are real; only GitHub's read is simulated.
        fs::write(&gh, format!("#!/bin/sh\n[ \"$1\" = --version ] && exit 0\nif [ \"$1 $2\" = 'pr list' ]; then\n  printf '%s\\n' '[{{\"url\":\"https://example.test/pull/42\",\"number\":42,\"state\":\"OPEN\",\"isDraft\":true,\"mergeCommit\":null,\"headRefOid\":\"{head}\"}}]'\nelse\n  exit 1\nfi\n")).unwrap();
        fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
        let command = |cwd: &Path, args: &[&str]| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("LF_") {
                    command.env_remove(key);
                }
            }
            command
                .current_dir(cwd)
                .args(args)
                .env("LF_HOME", home.path())
                .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
                .env(
                    "PATH",
                    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
                );
            command
        };
        let run = |cwd: &Path, args: &[&str]| command(cwd, args).output().unwrap();
        let invoke = |op: &str| {
            let args = if op == "start" {
                vec!["--task", "FIX-1", "flow", "start", "--json"]
            } else {
                vec!["task", op, "FIX-1", "--json"]
            };
            let output = run(repo.path(), &args);
            assert!(
                output.status.success(),
                "{op}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        invoke(operation);
        let task = runtime
            .block_on(store.get_task_by_issue("FIX-1"))
            .unwrap()
            .unwrap();
        if !remote_only {
            assert_eq!(task.worktree, checkout);
        }
        checkout = task.worktree.clone();
        assert!(loopflow::engine::git::is_ancestor(&checkout, &head, "HEAD").unwrap());
        if operation == "checkout" {
            assert_eq!(
                loopflow::engine::git::rev_parse(&checkout, "HEAD").unwrap(),
                head
            );
            let status = Command::new("git")
                .args(["status", "--porcelain=v1"])
                .current_dir(&checkout)
                .output()
                .unwrap()
                .stdout;
            assert_eq!(status, before);
        }
        assert_eq!(
            loopflow::engine::git::current_branch(&checkout)
                .unwrap()
                .as_deref(),
            Some(branch)
        );
        let pr = runtime
            .block_on(store.active_task_pr(&task.id))
            .unwrap()
            .unwrap();
        assert_eq!(pr.branch, branch);
        assert_eq!(pr.base_commit, base);
        assert_eq!(pr.github().unwrap().number, 42);
        assert_eq!(pr.github().unwrap().url, "https://example.test/pull/42");
        assert!(pr.publication.as_ref().unwrap().presentation.is_none());
        assert_eq!(runtime.block_on(store.list_tasks(None)).unwrap().len(), 1);

        if operation == "checkout" {
            assert!(runtime
                .block_on(store.task_flow(&task.id))
                .unwrap()
                .is_none());
            runtime
                .block_on(store.start_task_flow(
                    &task.id,
                    FlowSession {
                        invocation: QueuedInvocation::load(&checkout, "adoption").unwrap(),
                        cursor: ExecutionCursor {
                            index: 1,
                            iteration: 3,
                            ..Default::default()
                        },
                        version: 0,
                        task_id: Some(task.id.clone()),
                        wave_id: Some(task.wave_id.clone()),
                        cwd: checkout.clone(),
                        message: None,
                        model: None,
                        current_attempt: None,
                        pending_session_id: None,
                        ready_summary: None,
                        worker_generation: 0,
                        claim: None,
                        failure: None,
                        finished: false,
                        updated_at: now,
                    },
                ))
                .unwrap();
        }
        let saved = runtime
            .block_on(store.task_flow(&task.id))
            .unwrap()
            .unwrap();
        let saved = runtime
            .block_on(store.reserve_task_review(saved.id(), saved.version))
            .unwrap();
        // Catalog changes must not replace the saved graph or reset its cursor.
        fs::write(
            checkout.join(".lf/flows/adoption.yaml"),
            "- cmd: task sync --plan\n",
        )
        .unwrap();
        invoke("checkout");
        invoke("start");
        assert_eq!(
            runtime
                .block_on(store.task_flow(&task.id))
                .unwrap()
                .unwrap(),
            saved
        );
        assert_eq!(
            runtime
                .block_on(store.active_task_pr(&task.id))
                .unwrap()
                .unwrap()
                .id,
            pr.id
        );
        assert_eq!(
            fs::read_to_string(checkout.join("authored.txt")).unwrap(),
            "preserve this uncommitted work\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("main-notes")).unwrap(),
            "preserve the invoking checkout too\n"
        );
        // Ignore only the intentionally edited Flow definition in this comparison.
        let status = Command::new("git")
            .args(["status", "--porcelain=v1", "--", "authored.txt"])
            .current_dir(&checkout)
            .output()
            .unwrap()
            .stdout;
        if operation == "checkout" {
            assert_eq!(status, before);
        }
        assert_eq!(
            loopflow::engine::worktrees::list_worktrees(repo.path())
                .unwrap()
                .len(),
            2
        );
        if operation == "checkout" && !remote_only {
            let scope = repo.path().canonicalize().unwrap().display().to_string();
            let original = runtime
                .block_on(store.pm_task_observation(&scope, "linear", "FIX-1"))
                .unwrap()
                .record
                .unwrap();
            let task_before = runtime.block_on(store.get_task(&task.id)).unwrap();
            let pr_before = runtime.block_on(store.active_task_pr(&task.id)).unwrap();
            for (index, (condition, expected)) in [
                ("canceled", "terminal"),
                ("moved", "no longer matches"),
                ("connection", "Team"),
                ("removed", "Removed"),
            ]
            .into_iter()
            .enumerate()
            {
                let mut observed = original.clone();
                observed.item.revision = Some(format!("2026-09-30T12:00:0{index}Z"));
                if condition == "canceled" {
                    observed.item.state = Some("canceled".into());
                }
                if condition == "moved" {
                    observed.item.project_id = Some("project-2".into());
                    observed.project.as_mut().unwrap().id = "project-2".into();
                }
                runtime
                    .block_on(store.put_pm_task(&scope, "linear", observed))
                    .unwrap();
                if condition == "connection" {
                    fs::write(
                        checkout.join(".lf/config.yaml"),
                        "agent: claude\npm:\n  provider: linear\n  linear_team: another-team\n",
                    )
                    .unwrap();
                }
                if condition == "removed" {
                    runtime
                        .block_on(store.observe_pm_issue_change("issue-1", None, true))
                        .unwrap();
                }
                let output = run(repo.path(), &["--task", "FIX-1", "flow", "start", "--json"]);
                assert!(
                    !output.status.success(),
                    "{condition} allowed managed continuation"
                );
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(error.contains(expected), "{condition}: {error}");
                assert_eq!(
                    runtime
                        .block_on(store.task_flow(&task.id))
                        .unwrap()
                        .unwrap(),
                    saved
                );
                assert_eq!(
                    runtime.block_on(store.get_task(&task.id)).unwrap(),
                    task_before
                );
                assert_eq!(
                    runtime.block_on(store.active_task_pr(&task.id)).unwrap(),
                    pr_before
                );
                if condition == "connection" {
                    fs::write(
                        checkout.join(".lf/config.yaml"),
                        "agent: claude\npm:\n  provider: linear\n  linear_team: team-1\n",
                    )
                    .unwrap();
                }
            }
            // A worker rechecks planning independently of the initiating command.
            // Review positions correctly cannot claim a worker, so use a captured
            // mechanical boundary for this separate admission check.
            let mut worker = saved.clone();
            worker.invocation = QueuedInvocation::load(&checkout, "adoption").unwrap();
            worker.cursor = Default::default();
            worker.version = 0;
            worker.current_attempt = None;
            worker.pending_session_id = None;
            worker.ready_summary = None;
            let worker = runtime
                .block_on(store.start_task_flow(&task.id, worker))
                .unwrap();
            let TaskWorkerClaimOutcome::Claimed(claim) = runtime
                .block_on(store.claim_task_worker(
                    &task.id,
                    worker.id(),
                    worker.version,
                    &TaskWorkerOwner {
                        trace_id: TraceId::new(),
                        exec_id: ExecId::new(),
                        pid: std::process::id(),
                        started_at: now.unix_timestamp(),
                    },
                    now,
                ))
                .unwrap()
            else {
                panic!("worker claim was not acquired")
            };
            let output = command(&checkout, &["task", "__worker", task.id.as_str()])
                .env(
                    loopflow::durable::TASK_WORKER_CLAIM_ENV,
                    serde_json::to_string(&claim).unwrap(),
                )
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("Removed"),
                "worker refusal: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stopped = runtime
                .block_on(store.task_flow(&task.id))
                .unwrap()
                .unwrap();
            assert!(stopped.claim.is_none());
            assert_eq!(stopped.invocation, worker.invocation);
            assert_eq!(stopped.cursor, worker.cursor);
            assert_eq!(stopped.pending_session_id, worker.pending_session_id);
            assert_eq!(stopped.current_attempt, worker.current_attempt);
            assert_eq!(stopped.failure, worker.failure);
            // Independent execution keeps working after the planning Task is gone.
            let output = run(&checkout, &["flow", "adoption"]);
            assert!(
                output.status.success(),
                "independent Flow: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                runtime
                    .block_on(store.task_flow(&task.id))
                    .unwrap()
                    .unwrap(),
                stopped
            );
        }
        fs::remove_dir_all(&checkout).unwrap();
    }
}
