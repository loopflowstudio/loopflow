mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::ops::task::task_status;
use loopflow::ops::task_actions::TaskAction;
use loopflow::work::task::{GithubPr, PrPublication, TaskEventKind};
use loopflow_test_support::TestRepo;
use support::{register_unrun_task, EnvGuard};

fn unbound_command(cli: &Path, repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(cli);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    command.current_dir(repo).args(args);
    command
}

#[test]
fn stacked_checkout_starts_with_one_scratch_deletion_commit() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    repo.create_file("scratch/design.md", "parent design");
    repo.create_file("scratch/review/notes.md", "parent review");
    repo.stage_all();
    repo.commit("Parent notes");
    let parent_head = repo.head_sha();
    let parent = register_unrun_task(home.path(), repo.path(), "parent", &parent_head);
    let child =
        support::register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut parent_pr = parent.pr.clone();
    parent_pr.publication = Some(PrPublication {
        requested_at: parent_pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(parent_pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute("DELETE FROM task_prs WHERE task_id=?1", [child.id.as_str()])
        .unwrap();
    runtime
        .block_on(parent.store.stack_task_placement(&child, &parent.pr.id))
        .unwrap();

    let checkout = || {
        loopflow::ops::task::task_checkout(
            repo.path(),
            "INF-124",
            loopflow::ops::task::TaskCheckoutOptions::default(),
        )
        .unwrap()
    };
    checkout();
    assert!(!child.worktree.join("scratch").exists());
    assert!(runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .is_none());
    assert_eq!(
        loopflow::engine::git::rev_parse(&child.worktree, "HEAD^").unwrap(),
        parent_head
    );
    let subject = Command::new("git")
        .current_dir(&child.worktree)
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    assert!(subject.status.success());
    assert_eq!(
        String::from_utf8_lossy(&subject.stdout).trim(),
        "Clear inherited scratch"
    );
    let child_head = loopflow::engine::git::rev_parse(&child.worktree, "HEAD").unwrap();
    fs::create_dir(child.worktree.join("scratch")).unwrap();
    fs::write(child.worktree.join("scratch/design.md"), "child design").unwrap();
    checkout();
    assert_eq!(
        loopflow::engine::git::rev_parse(&child.worktree, "HEAD").unwrap(),
        child_head
    );
    assert_eq!(
        fs::read_to_string(child.worktree.join("scratch/design.md")).unwrap(),
        "child design"
    );
    assert_eq!(repo.head_sha(), parent_head);
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/design.md")).unwrap(),
        "parent design"
    );
    assert!(repo.path().join("scratch/review/notes.md").exists());
}

#[test]
fn checkout_restores_exact_task_history_from_a_dirty_checkout() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut fixture = register_unrun_task(
        home.path(),
        repo.path(),
        "test/checkout-recovery",
        &repo.head_sha(),
    );
    fixture.task.worktree = target.path().join("checkout");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE tasks SET worktree=?2 WHERE id=?1",
            rusqlite::params![
                fixture.task.id.as_str(),
                fixture.task.worktree.display().to_string()
            ],
        )
        .unwrap();
    fixture.pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap();
    let invoking = repo.create_named_worktree("dirty-invoker");
    fs::write(repo.path().join("main-notes"), "keep main edits").unwrap();
    fs::write(invoking.join("caller-notes"), "keep caller edits").unwrap();
    let checkout = || {
        unbound_command(
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &invoking,
            &["task", "checkout", "INF-123", "--json"],
        )
        .env("LF_HOME", home.path())
        .output()
        .unwrap()
    };
    let first = checkout();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap(),
        fixture.pr.base_commit
    );
    fs::write(
        fixture.task.worktree.join("work.txt"),
        "committed Task work",
    )
    .unwrap();
    for args in [
        vec!["add", "work.txt"],
        vec!["commit", "-m", "Preserve Task work"],
    ] {
        assert!(Command::new("git")
            .current_dir(&fixture.task.worktree)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    let head = loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap();
    fs::remove_dir_all(&fixture.task.worktree).unwrap();
    let restored = checkout();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap(),
        head
    );
    assert_eq!(
        fs::read_to_string(fixture.task.worktree.join("work.txt")).unwrap(),
        "committed Task work"
    );
    let persisted = runtime
        .block_on(fixture.store.get_task(&fixture.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(persisted.worktree, fixture.task.worktree);
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap(),
        fixture.pr
    );
    let events = runtime
        .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, TaskEventKind::CheckoutReady { .. }))
            .count(),
        1
    );
    fs::remove_dir_all(&fixture.task.worktree).unwrap();
    fs::create_dir(&fixture.task.worktree).unwrap();
    fs::write(fixture.task.worktree.join("notes"), "unregistered work").unwrap();
    let occupied = checkout();
    assert!(!occupied.status.success());
    assert!(String::from_utf8_lossy(&occupied.stderr).contains("occupied"));
    assert_eq!(
        fs::read_to_string(fixture.task.worktree.join("notes")).unwrap(),
        "unregistered work"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("main-notes")).unwrap(),
        "keep main edits"
    );
    assert_eq!(
        fs::read_to_string(invoking.join("caller-notes")).unwrap(),
        "keep caller edits"
    );
}

#[test]
fn initializing_worktree_keeps_status_wait_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/initializing-task";
    repo.create_branch(branch);
    let mut task = register_unrun_task(home.path(), repo.path(), branch, &base);
    let missing_worktree = home.path().join("not-yet-created-worktree");
    task.task.worktree = missing_worktree.clone();
    let runtime = tokio::runtime::Runtime::new().expect("initialization fixture runtime");
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE tasks SET worktree=?2 WHERE id=?1",
            rusqlite::params![
                task.task.id.as_str(),
                missing_worktree.display().to_string()
            ],
        )
        .expect("seed declared Task worktree");
    runtime
        .block_on(task.store.append_task_event(
            &task.task.id,
            &TaskEventKind::WorktreeInitializing {
                pr_id: task.pr.id.clone(),
                sequence: task.pr.sequence,
                branch: task.pr.branch.clone(),
                path: missing_worktree.display().to_string(),
                base_commit: task.pr.base_commit.clone(),
            },
        ))
        .expect("publish initialization marker");
    std::fs::create_dir_all(&missing_worktree)
        .expect("simulate a partially created worktree directory");
    let run_lf = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env_remove("LF_WAVE_ID")
            .current_dir(repo.path())
            .output()
            .expect("run lf read surface")
    };
    let status = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        status.status.success(),
        "status stays readable: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let status = &status["execution"];
    assert_eq!(status["execution"]["state"], "idle");
    assert_eq!(status["work"]["sessions"], serde_json::json!([]));
    assert_eq!(status["work"]["flow_processes"], serde_json::json!([]));
    assert_eq!(status["actions"]["recommended"], "no_action");
    assert!(status["actions"]["reason"]
        .as_str()
        .expect("status action reason")
        .contains("is initializing worktree"));

    let wait = run_lf(&["task", "wait", "INF-123", "--timeout", "0s", "--json"]);
    assert!(
        wait.status.success(),
        "wait stays readable: {}",
        String::from_utf8_lossy(&wait.stderr)
    );
    let wait: serde_json::Value = serde_json::from_slice(&wait.stdout).expect("wait JSON");
    assert_eq!(wait["actions"], status["actions"]);

    let roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        roadmap.status.success(),
        "roadmap stays readable: {}",
        String::from_utf8_lossy(&roadmap.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&roadmap.stdout).expect("roadmap JSON");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["tasks"]["state"], "ok", "roadmap wave: {wave:#}");
    let roadmap_task = &wave["tasks"]["items"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(roadmap_task["actions"]["recommended"], "no_action");
    assert!(roadmap_task["condition"]["reason"]
        .as_str()
        .expect("roadmap condition reason")
        .contains("is initializing worktree"));
    let projected = task_status(repo.path(), Some("INF-123"))
        .expect("read Task")
        .execution
        .expect("execution");
    assert_eq!(projected.actions.recommended, Some(TaskAction::NoAction));

    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open stale initialization fixture")
        .execute(
            "UPDATE task_events SET created_at=?2 WHERE task_id=?1",
            rusqlite::params![
                task.task.id.as_str(),
                time::OffsetDateTime::now_utc().unix_timestamp() - 301,
            ],
        )
        .expect("age the initialization marker");
    let stale = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        stale.status.success(),
        "stale initialization stays readable"
    );
    let stale: serde_json::Value =
        serde_json::from_slice(&stale.stdout).expect("stale status JSON");
    let stale = &stale["execution"];
    assert_eq!(stale["actions"]["recommended"], "no_action");
    assert!(stale["actions"]["reason"]
        .as_str()
        .expect("stale action reason")
        .contains("initialization did not complete"));
    let stale_roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        stale_roadmap.status.success(),
        "stale roadmap stays readable"
    );
    let stale_roadmap: serde_json::Value =
        serde_json::from_slice(&stale_roadmap.stdout).expect("stale roadmap JSON");
    let stale_condition = &stale_roadmap["waves"][0]["tasks"]["items"][0]["condition"];
    assert_eq!(stale_condition["state"], "blocked");
    assert!(stale_condition["reason"]
        .as_str()
        .expect("stale roadmap reason")
        .contains("initialization did not complete"));
}

#[test]
fn missing_worktree_status_is_actionable_and_read_only() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let (task, missing_path, branch) = {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/missing-worktree";
        repo.create_branch(branch);
        let task = register_unrun_task(home.path(), repo.path(), branch, &base);
        (task, repo.path().to_path_buf(), branch.to_string())
    };
    assert!(!missing_path.exists(), "fixture worktree is absent");
    let runtime = tokio::runtime::Runtime::new().expect("missing Task runtime");
    let before_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task before status")
        .expect("Task exists before status");
    let before_prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read PRs before status");

    let snapshot = task_status(&missing_path, Some("INF-123"))
        .expect("status survives the absent worktree")
        .execution
        .expect("execution");

    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot
        .actions
        .reason
        .contains(&missing_path.display().to_string()));
    assert!(snapshot.actions.reason.contains(&branch));
    assert!(snapshot.actions.reason.contains("lf task run INF-123"));
    assert!(snapshot
        .actions
        .reason
        .contains("identity and PR history are unchanged"));
    assert_eq!(
        runtime
            .block_on(task.store.get_task(&task.task.id))
            .expect("reread Task after status")
            .expect("Task remains registered"),
        before_task
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_prs(&task.task.id))
            .expect("reread PRs after status"),
        before_prs
    );
}

#[test]
fn research_checkout_restores_and_reads_files_without_a_pull_request() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let parent = register_unrun_task(home.path(), repo.path(), "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut research = parent.task.clone();
    research.id = loopflow::work::task::TaskId::new();
    research.plan.id = loopflow::planning::LinearIssueId::new("research-issue").unwrap();
    research.plan.identifier = "INF-124".into();
    research.plan.title = "Recommend a storage approach".into();
    research.workspace_slug = "research".into();
    research.branch = "research".into();
    research.worktree = target.path().join("research");
    runtime
        .block_on(
            parent
                .store
                .create_task_with_worktree(&research, None, None),
        )
        .unwrap();

    let restored = loopflow::ops::task::task_checkout(
        repo.path(),
        "INF-124",
        loopflow::ops::task::TaskCheckoutOptions::default(),
    )
    .unwrap();
    assert_eq!(restored.id, research.id);
    fs::write(
        research.worktree.join("findings.md"),
        "Use the existing store.\n",
    )
    .unwrap();
    let result = Command::new("git")
        .current_dir(&research.worktree)
        .args(["add", "findings.md"])
        .status()
        .unwrap();
    assert!(result.success());
    let result = Command::new("git")
        .current_dir(&research.worktree)
        .args(["commit", "-m", "Record research findings"])
        .status()
        .unwrap();
    assert!(result.success());
    fs::write(research.worktree.join("draft.md"), "Keep this draft.\n").unwrap();
    let changes = loopflow::ops::task::task_changes("INF-124", "parent").unwrap();
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "findings.md" && file.committed));
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "draft.md" && file.untracked));
    let snapshot = loopflow::ops::task::task_snapshot(&restored).unwrap();
    assert!(snapshot.pr.is_none());
    assert_eq!(snapshot.branch, "research");
    assert!(runtime
        .block_on(parent.store.task_prs(&research.id))
        .unwrap()
        .is_empty());
    let json = serde_json::to_value(snapshot).unwrap();
    assert!(json["pr"].is_null());
    assert!(json.get("prs").is_none());
    assert!(json.get("active_pr").is_none());
    let binding = runtime
        .block_on(loopflow::ops::resolve_work_binding(
            &std::sync::Arc::new(parent.store),
            &research.worktree,
            "task:INF-124",
        ))
        .unwrap();
    assert!(binding.context.contains("Branch: research"));
    assert!(!binding.context.contains("PR 1:"));
}

#[test]
fn optional_task_pr_preserves_placement_and_freezes_prior_delivery() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    loopflow::store::migrations::apply_sqlite(&conn).unwrap();
    conn.execute_batch(r#"
        INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1);
        INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('p','w','linear-p',1);
        INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,created_at,updated_at)
        VALUES('research','p','research-issue','R-1','/research','research',1,1),
              ('single','p','single-issue','R-2','/single','single',1,1),
              ('chain','p','chain-issue','R-3','/chain','chain',1,1),
              ('continuing','p','continuing-issue','R-4','/continuing','continuing',1,1);
        INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
        VALUES('placeholder','research',1,'research','research','research-base',1,1);
        INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,
          github_number,github_url,merge_commit,created_at,updated_at)
        VALUES('single-pr','single',1,'single','single','single-base',1,1,'https://github.com/a/b/pull/1',NULL,1,1),
              ('old-pr','chain',1,'old','old','old-base',1,2,'https://github.com/a/b/pull/2','merged',1,1),
              ('current-pr','chain',2,'current','current','current-base',1,3,'https://github.com/a/b/pull/3',NULL,1,1);
        INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,
          github_number,github_url,merge_commit,after_merge,next_slug,merge_mode,merge_requested_at,merge_head_sha,github_head_sha,created_at,updated_at)
        VALUES('continued-pr','continuing',1,'prior','prior','prior-base',1,4,'https://github.com/a/b/pull/4','merged','continue_task','continuing','auto',1,'head','head',1,1);
        INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
        VALUES('successor-placeholder','continuing',2,'continuing','continuing','prior-base',2,2);
        INSERT INTO task_events(task_id,kind_json,created_at) VALUES('chain',
          '{"kind":"follow_up","remaining":{"outcome":"Check release","evidence":"Installed command works","check_at":42},"reason":"Accepted release check"}',1);
    "#).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    conn.execute_batch(&loopflow::store::migrations::migration_sql_for_test(
        "optional_task_pr",
    ))
    .unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    let placement: (String, String) = conn
        .query_row(
            "SELECT branch,base_commit FROM tasks WHERE id='research'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(placement, ("research".into(), "research-base".into()));
    let current: Vec<String> = conn
        .prepare("SELECT id FROM task_prs WHERE historical=0 ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(current, vec!["current-pr", "single-pr"]);
    assert!(conn
        .execute(
            "UPDATE task_prs SET base_commit='changed' WHERE id='old-pr'",
            []
        )
        .is_err());
    assert!(conn
        .execute("DELETE FROM task_prs WHERE id='old-pr'", [])
        .is_err());
    // The retained placeholder cannot block first publication on this branch.
    conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
      VALUES('research-pr','research',2,'research','research','research-base',2,2,2)").unwrap();
    assert!(conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
      VALUES('duplicate','research',3,'research','research','research-base',3,3,3)").is_err());
    let obligation: String = conn.query_row("SELECT kind_json FROM task_events WHERE task_id='chain' AND json_extract(kind_json,'$.kind')='follow_up'", [], |row| row.get(0)).unwrap();
    assert!(obligation.contains("Check release"));
    let conversion: String = conn.query_row("SELECT kind_json FROM task_events WHERE task_id='continuing' AND json_extract(kind_json,'$.kind')='follow_through_conversion'", [], |row| row.get(0)).unwrap();
    assert!(conversion.contains("continuing"));
    let current_continuation: i64 = conn
        .query_row(
            "SELECT count(*) FROM task_prs WHERE task_id='continuing' AND historical=0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current_continuation, 0);
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
}
