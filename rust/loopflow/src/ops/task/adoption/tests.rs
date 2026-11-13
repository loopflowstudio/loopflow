//! Promotion and binding decisions as table rows.
//!
//! Each row calls what the CLI handler calls: `lf task create` reaches
//! `engine::creation::create` (`lf/commands/task_create.rs`); `lf session bind`
//! reaches `human_session::bind`, and `--dry-run` reaches `preview_binding`
//! (`lf/commands/session.rs`). Clap selectors, `LF_AGENT_CALLER`, provider
//! launches and `resume` stay with the full-Machine tests in `tests/README.md`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::creation::{self, TaskCreation};
use crate::ops::human_session::tests::SessionHome;
use crate::session::{LfSession, SessionWork, WorkSource};
use crate::session_record::{SessionCaptureSpec, SessionGraphMembership, SessionRecorder};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

struct Machine {
    repo: loopflow_test_support::TestRepo,
    store: SharedStore,
    runtime: tokio::runtime::Runtime,
    home: SessionHome,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Machine {
    /// Planning sync and base fetches fail fast without an origin; rows about
    /// the remote keep it.
    fn new(origin: bool) -> Self {
        let lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime.block_on(home.store());
        let repo = loopflow_test_support::TestRepo::new();
        if !origin {
            git(repo.path(), &["remote", "remove", "origin"]);
        }
        Self {
            repo,
            store,
            runtime,
            home,
            _lock: lock,
        }
    }

    fn main(&self) -> PathBuf {
        self.repo.path().canonicalize().unwrap()
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.repo
            .create_named_worktree(name)
            .canonicalize()
            .unwrap()
    }

    /// A conversation recorded by the writer a launch uses.
    fn conversation(&self, cwd: &Path, surface: &str, task: Option<&Task>) -> LfSession {
        let recorder = SessionRecorder::begin_at(
            self.home.path(),
            SessionCaptureSpec {
                harness: "opencode".into(),
                model: None,
                surface: surface.into(),
                cwd: cwd.into(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: vec![],
                graph: SessionGraphMembership::Independent,
                work: task.map(|task| SessionWork {
                    task_id: Some(task.id.clone()),
                    wave_id: None,
                    source: WorkSource::Declared,
                }),
            },
        )
        .unwrap();
        self.store
            .sqlite
            .session_for_artifact(&recorder.artifact_key())
            .unwrap()
            .unwrap()
    }

    fn create(
        &self,
        title: &str,
        session: Option<&LfSession>,
        worktree: Option<&Path>,
    ) -> anyhow::Result<Task> {
        creation::create(
            &uuid::Uuid::new_v4().to_string(),
            TaskCreation {
                directory: self.repo.path().into(),
                title: Some(title.into()),
                request: None,
                wave: None,
                graph: None,
                session: session.map(|session| session.id.to_string()),
                worktree: worktree.map(Path::to_path_buf),
                run: false,
                agent: None,
                approve: false,
            },
        )
        .map(|receipt| receipt.task)
    }

    fn bind(&self, session: &str, task: &Task) -> anyhow::Result<()> {
        self.runtime
            .block_on(crate::ops::human_session::bind(
                &self.store,
                session,
                task.id.as_str(),
            ))
            .map(drop)
    }

    fn session(&self, session: &LfSession) -> LfSession {
        self.session_by_id(session.id.as_str())
    }

    fn session_by_id(&self, id: &str) -> LfSession {
        self.store.sqlite.session(id).unwrap().unwrap()
    }

    fn task(&self, id: &TaskId) -> Task {
        self.store.sqlite.task(id).unwrap().unwrap()
    }

    fn primary(&self, task: &Task) -> Option<String> {
        self.db()
            .query_row(
                "SELECT primary_session_id FROM tasks WHERE id=?1",
                [task.id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        self.db()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap()
    }
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

fn refusal(result: anyhow::Result<impl std::fmt::Debug>) -> String {
    format!("{:#}", result.unwrap_err())
}

#[test]
fn creation_adopts_only_the_selected_work() {
    let machine = Machine::new(false);
    let root = machine.worktree("prototype");
    std::fs::write(root.join("draft.txt"), "unsaved prototype\n").unwrap();
    let research = machine.conversation(&root, "tui", None);
    let other_root = machine.worktree("other");
    let other = machine
        .create("Other question", None, Some(&other_root))
        .unwrap();
    let newer = machine.conversation(&root, "tui", None);
    machine.bind(newer.id.as_str(), &other).unwrap();

    // Plain creation never adopts a checkout or a conversation.
    let plain = machine.create("Independent work", None, None).unwrap();
    assert!(plain.worktree.is_none());
    assert_eq!(machine.primary(&plain), None);
    assert_eq!(machine.session(&research).task_id, None);

    // A checkout alone selects no conversation, however recent.
    let adopted = machine.create("Prototype", None, Some(&root)).unwrap();
    assert_eq!(adopted.worktree.as_deref(), Some(root.as_path()));
    assert_eq!(adopted.branch, "prototype");
    assert_eq!(machine.primary(&adopted), None);
    assert_eq!(machine.session(&research).task_id, None);
    assert_eq!(
        machine.create("Retry", None, Some(&root)).unwrap().id,
        adopted.id,
        "the checkout identifies a retry, not its title"
    );

    // An exact selection finds the checkout's Task and becomes its conversation.
    let selected = machine.create("Prototype", Some(&research), None).unwrap();
    assert_eq!(selected.id, adopted.id);
    assert_eq!(
        machine.primary(&adopted).as_deref(),
        Some(research.id.as_str())
    );
    assert_eq!(machine.session(&research).cwd, root);
    assert_eq!(machine.session(&newer).task_id.as_ref(), Some(&other.id));
    assert_eq!(machine.count("tasks"), 3);
    assert_eq!(
        std::fs::read_to_string(root.join("draft.txt")).unwrap(),
        "unsaved prototype\n"
    );
    assert_eq!(
        crate::git::current_branch(&root).unwrap().as_deref(),
        Some("prototype")
    );
}

#[test]
fn creation_from_a_conversation_keeps_its_directory() {
    let machine = Machine::new(false);
    let root = machine.worktree("prototype");

    // Linked checkout: the Task takes the checkout and the conversation.
    let linked = machine.conversation(&root, "tui", None);
    let task = machine.create("Research", Some(&linked), None).unwrap();
    assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
    assert_eq!(task.branch, "prototype");
    assert_eq!(machine.primary(&task).as_deref(), Some(linked.id.as_str()));
    assert_eq!(machine.session(&linked).task_id.as_ref(), Some(&task.id));
    assert_eq!(machine.session(&linked).cwd, root);
    assert_eq!(
        machine.create("Research", Some(&linked), None).unwrap().id,
        task.id
    );

    // A recorded subdirectory survives creation and binding, assigned or not.
    for assigned in [false, true] {
        let root = machine.worktree(if assigned { "assigned" } else { "free" });
        let research = root.join("research");
        std::fs::create_dir(&research).unwrap();
        let existing = assigned.then(|| machine.create("Existing", None, Some(&root)).unwrap());
        let session = machine.conversation(&research, "tui", existing.as_ref());
        let task = machine.create("Research", Some(&session), None).unwrap();
        assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
        machine.bind(session.id.as_str(), &task).unwrap();
        assert_eq!(
            machine.session(&session).cwd,
            research,
            "assigned={assigned}"
        );
        assert_eq!(machine.primary(&task).as_deref(), Some(session.id.as_str()));
    }

    // Main checkout: the conversation moves to the Task; the checkout stays shared.
    let main = machine.conversation(&machine.main(), "tui", None);
    let kept = machine
        .create("Keep conversation", Some(&main), None)
        .unwrap();
    assert!(kept.worktree.is_none());
    assert_eq!(machine.primary(&kept).as_deref(), Some(main.id.as_str()));
    assert_eq!(machine.session(&main).cwd, machine.main());
    assert_eq!(
        machine
            .create("Keep conversation", Some(&main), None)
            .unwrap()
            .id,
        kept.id
    );
}

#[test]
fn a_shared_primary_moves_to_the_task_and_its_scope_gets_a_new_one() {
    for wave in [None, Some("infrastructure")] {
        let machine = Machine::new(false);
        let ensure = || {
            machine
                .runtime
                .block_on(async {
                    if let Some(name) = wave {
                        crate::work::wave::ensure_wave_row(
                            &machine.store,
                            machine.repo.path(),
                            name,
                        )
                        .await
                        .unwrap();
                    }
                    crate::ops::human_session::primary::ensure(
                        &machine.store,
                        machine.repo.path(),
                        wave,
                    )
                    .await
                })
                .unwrap()
                .0
        };
        let before = ensure();
        let session = machine.session_by_id(before.id.as_str());
        let scope = machine
            .store
            .sqlite
            .primary_scope(session.id.as_str())
            .unwrap();
        assert!(scope.is_some());
        let task = machine
            .create("Continue research", Some(&session), None)
            .unwrap();
        assert!(task.worktree.is_none(), "wave={wave:?}");
        assert_eq!(machine.primary(&task).as_deref(), Some(session.id.as_str()));
        assert_eq!(
            machine
                .store
                .sqlite
                .primary_scope(session.id.as_str())
                .unwrap(),
            Some(crate::session::PrimaryScope::Task(task.id.clone()))
        );
        assert_eq!(machine.session(&session).cwd, session.cwd);
        assert_eq!(
            machine
                .create("Continue research", Some(&session), None)
                .unwrap()
                .id,
            task.id
        );
        let replacement = machine.session_by_id(ensure().id.as_str());
        assert_ne!(replacement.id, session.id);
        assert_eq!(replacement.cwd, session.cwd);
        assert_eq!(
            machine
                .store
                .sqlite
                .primary_scope(replacement.id.as_str())
                .unwrap(),
            scope
        );
    }
}

#[test]
fn binding_selects_the_task_conversation() {
    let machine = Machine::new(false);
    let root = machine.worktree("prototype");
    let session = machine.conversation(&root, "tui", None);
    let filed = machine.create("Research prototype", None, None).unwrap();

    // Preview resolves identity without binding; artifact keys are not Session ids.
    let preview = machine
        .runtime
        .block_on(crate::ops::human_session::preview_binding(
            &machine.store,
            session.id.as_str(),
            filed.id.as_str(),
        ))
        .unwrap();
    assert_eq!(preview.session_id, session.id);
    assert_eq!(preview.task_id, filed.id.as_str());
    assert_eq!(machine.session(&session).task_id, None);
    let artifact = session.artifact_key.clone().unwrap();
    assert!(refusal(machine.bind(&artifact, &filed)).contains("was not found"));

    // Binding adopts the conversation's checkout for a Task without one.
    for _ in 0..2 {
        machine.bind(session.id.as_str(), &filed).unwrap();
        let task = machine.task(&filed.id);
        assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
        assert_eq!(task.branch, "prototype");
        assert_eq!(machine.primary(&task).as_deref(), Some(session.id.as_str()));
        assert_eq!(machine.session(&session).cwd, root);
    }
    assert_eq!(
        machine
            .create("Research prototype", Some(&session), None)
            .unwrap()
            .id,
        filed.id
    );
    let another = machine.create("Another", None, None).unwrap();
    assert!(refusal(machine.bind(session.id.as_str(), &another)).contains("already belongs"));

    // A Task with its own checkout sets the conversation's directory there.
    let origin = machine.worktree("research");
    let destination = machine.worktree("delivery");
    let delivery = machine
        .create("Delivery", None, Some(&destination))
        .unwrap();
    let moving = machine.conversation(&origin, "tui", None);
    for _ in 0..2 {
        machine.bind(moving.id.as_str(), &delivery).unwrap();
        let task = machine.task(&delivery.id);
        assert_eq!(task.worktree, delivery.worktree);
        assert_eq!(task.branch, delivery.branch);
    }
    assert_eq!(machine.session(&moving).cwd, destination);
    // Selecting another primary leaves this conversation's directory.
    let later = machine.conversation(&destination, "tui", None);
    machine.bind(later.id.as_str(), &delivery).unwrap();
    assert_eq!(
        machine.primary(&delivery).as_deref(),
        Some(later.id.as_str())
    );
    assert_eq!(machine.session(&moving).cwd, destination);
}

#[test]
fn binding_refuses_background_and_closed_sessions() {
    let machine = Machine::new(false);
    let root = machine.worktree("prototype");
    let task = machine.create("Prototype", None, Some(&root)).unwrap();
    let background = machine.conversation(&root, "headless", None);
    let closed = machine.conversation(&root, "tui", None);
    // Closing follows the agent process's exit; a row has no process to end.
    machine
        .db()
        .execute(
            "UPDATE agent_sessions SET completed_at=1 WHERE id=?1",
            [closed.id.as_str()],
        )
        .unwrap();
    let closed = machine.session(&closed);
    assert!(closed.completed_at.is_some());
    for session in [&background, &closed] {
        assert!(
            refusal(machine.bind(session.id.as_str(), &task))
                .contains("unfinished interactive Session"),
            "{}",
            session.id
        );
        assert!(refusal(machine.create("Same work", Some(session), None))
            .contains("unfinished interactive Session"));
        assert_eq!(machine.session(session).task_id, None);
    }
    assert_eq!(machine.primary(&task), None);
    assert_eq!(machine.count("tasks"), 1);
}

#[test]
fn binding_a_done_task_keeps_it_done() {
    for (shared, placed) in [(false, false), (true, false), (false, true), (true, true)] {
        let machine = Machine::new(false);
        let origin = if shared {
            machine.main()
        } else {
            machine.worktree("research")
        };
        let destination = placed.then(|| machine.worktree("delivery"));
        let created = machine
            .create("Finished work", None, destination.as_deref())
            .unwrap();
        crate::ops::task::task_complete(machine.repo.path(), created.id.as_str(), None).unwrap();
        let completed = machine.task(&created.id);
        assert!(completed.completed_at.is_some());
        std::fs::write(origin.join("draft.txt"), "untracked draft\n").unwrap();
        let session = machine.conversation(&origin, "tui", None);
        for _ in 0..2 {
            machine.bind(session.id.as_str(), &created).unwrap();
            let bound = machine.session(&session);
            assert_eq!(bound.task_id.as_ref(), Some(&created.id));
            assert_eq!(
                bound.cwd,
                destination.clone().unwrap_or_else(|| origin.clone()),
                "shared={shared}, placed={placed}"
            );
            assert_eq!(
                machine.primary(&created).as_deref(),
                Some(session.id.as_str())
            );
            let current = machine.task(&created.id);
            assert_eq!(current.completed_at, completed.completed_at);
            assert_eq!(current.state, completed.state);
            assert_eq!(current.worktree, completed.worktree);
            assert_eq!(current.branch, completed.branch);
            assert_eq!(current.base_commit, completed.base_commit);
            assert!(machine
                .store
                .sqlite
                .task_state(&created.id)
                .unwrap()
                .is_terminal());
        }
        assert_eq!(
            std::fs::read_to_string(origin.join("draft.txt")).unwrap(),
            "untracked draft\n"
        );
        if let Some(path) = &destination {
            assert!(!path.join("draft.txt").exists());
        }
        assert_eq!(machine.count("tasks"), 1);
    }
}

#[test]
fn refusals_leave_ownership_unchanged() {
    let machine = Machine::new(false);

    // A failed reservation leaves no Task; the retry creates exactly one.
    let root = machine.worktree("prototype");
    git(&root, &["config", "user.email", ""]);
    assert!(machine.create("Keep prototype", None, Some(&root)).is_err());
    assert_eq!(machine.count("tasks"), 0);
    git(&root, &["config", "user.email", "test@example.com"]);
    let created = machine.create("Keep prototype", None, Some(&root)).unwrap();
    assert_eq!(
        machine
            .create("Keep prototype", None, Some(&root))
            .unwrap()
            .id,
        created.id
    );
    let ready: String = machine
        .db()
        .query_row(
            "SELECT json_extract(kind_json,'$.kind') FROM task_events WHERE task_id=?1 ORDER BY rowid DESC LIMIT 1",
            [created.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(ready, "checkout_ready");

    let elsewhere = loopflow_test_support::TestRepo::new();
    let foreign = machine.conversation(&elsewhere.path().canonicalize().unwrap(), "tui", None);
    let other_root = machine.worktree("other");
    let in_other = machine.conversation(&other_root, "tui", None);
    let assigned = machine.create("Assigned", None, None).unwrap();
    let crossed = machine.conversation(&root, "tui", Some(&assigned));
    for (row, result, message) in [
        (
            "another repository",
            machine.create("Foreign", Some(&foreign), None),
            "belongs to another repository",
        ),
        (
            "different checkouts",
            machine.create("Mixed", Some(&in_other), Some(&root)),
            "different checkouts",
        ),
        (
            "different Tasks",
            machine.create("Crossed", Some(&crossed), None),
            "belong to different Tasks",
        ),
        (
            "shared checkout",
            machine.create("Shared", None, Some(&machine.main())),
            "shared by its repository",
        ),
    ] {
        let error = refusal(result);
        assert!(error.contains(message), "{row}: {error}");
    }
    assert_eq!(machine.count("tasks"), 2);
    assert_eq!(machine.primary(&created), None);
    assert_eq!(machine.session(&in_other).task_id, None);
    assert_eq!(
        machine.session(&crossed).task_id.as_ref(),
        Some(&assigned.id)
    );
}

#[test]
fn checkout_adoption_records_its_open_pull_request() {
    let machine = Machine::new(true);
    let root = machine.worktree("prototype");
    let bare = machine.repo.bare_path().to_str().unwrap().to_owned();
    git(
        machine.repo.path(),
        &[
            "config",
            &format!("url.{bare}.insteadOf"),
            "https://github.com/test/repo",
        ],
    );
    git(
        machine.repo.path(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/test/repo",
        ],
    );
    let bin = machine.home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(&gh, format!("#!/bin/sh\ncat <<'JSON'\n[{{\"number\":42,\"url\":\"https://github.com/test/repo/pull/42\",\"state\":\"OPEN\",\"isDraft\":true,\"mergeCommit\":null,\"headRefOid\":\"{}\"}}]\nJSON\n", machine.repo.head_sha())).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));

    let task = machine
        .create("Existing prototype", None, Some(&root))
        .unwrap();
    assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
    let publication: String = machine
        .db()
        .query_row(
            "SELECT github_url FROM task_prs WHERE task_id=?1",
            [task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(publication, "https://github.com/test/repo/pull/42");
    assert_eq!(machine.count("agent_sessions"), 0);
    assert_eq!(machine.count("task_graphs"), 0);
}
