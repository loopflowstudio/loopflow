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
use crate::id::LfSessionId;
use crate::ops::human_session::tests::SessionHome;
use crate::session::{LfSession, SessionWork, WorkSource};
use crate::session_record::{SessionCaptureSpec, SessionGraphMembership, SessionRecorder};
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};

struct Table {
    repo: loopflow_test_support::TestRepo,
    store: SharedStore,
    runtime: tokio::runtime::Runtime,
    home: SessionHome,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Table {
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
        self.store
            .sqlite
            .session(session.id.as_str())
            .unwrap()
            .unwrap()
    }

    fn task(&self, id: &TaskId) -> Task {
        self.store.sqlite.task(id).unwrap().unwrap()
    }

    fn primary(&self, task: &Task) -> Option<LfSessionId> {
        self.store
            .sqlite
            .task_primary(&task.id)
            .unwrap()
            .map(|session| session.id)
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
    let table = Table::new(false);
    let root = table.worktree("prototype");
    std::fs::write(root.join("draft.txt"), "unsaved prototype\n").unwrap();
    let research = table.conversation(&root, "tui", None);
    let other_root = table.worktree("other");
    let other = table
        .create("Other question", None, Some(&other_root))
        .unwrap();
    let newer = table.conversation(&root, "tui", None);
    table.bind(newer.id.as_str(), &other).unwrap();

    // Plain creation never adopts a checkout or a conversation.
    let plain = table.create("Independent work", None, None).unwrap();
    assert!(plain.worktree.is_none());
    assert_eq!(table.primary(&plain), None);
    assert_eq!(table.session(&research).task_id, None);

    // A checkout alone selects no conversation, however recent.
    let adopted = table.create("Prototype", None, Some(&root)).unwrap();
    assert_eq!(adopted.worktree.as_deref(), Some(root.as_path()));
    assert_eq!(adopted.branch, "prototype");
    assert_eq!(table.primary(&adopted), None);
    assert_eq!(table.session(&research).task_id, None);
    assert_eq!(
        table.create("Retry", None, Some(&root)).unwrap().id,
        adopted.id,
        "the checkout identifies a retry, not its title"
    );

    // An exact selection finds the checkout's Task and becomes its conversation.
    let selected = table.create("Prototype", Some(&research), None).unwrap();
    assert_eq!(selected.id, adopted.id);
    assert_eq!(table.primary(&adopted), Some(research.id.clone()));
    assert_eq!(table.session(&research).cwd, root);
    assert_eq!(table.session(&newer).task_id.as_ref(), Some(&other.id));
    assert_eq!(table.count("tasks"), 3);
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
    let table = Table::new(false);
    let root = table.worktree("prototype");

    // Linked checkout: the Task takes the checkout and the conversation.
    let linked = table.conversation(&root, "tui", None);
    let task = table.create("Research", Some(&linked), None).unwrap();
    assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
    assert_eq!(task.branch, "prototype");
    assert_eq!(table.primary(&task), Some(linked.id.clone()));
    assert_eq!(table.session(&linked).task_id.as_ref(), Some(&task.id));
    assert_eq!(table.session(&linked).cwd, root);
    assert_eq!(
        table.create("Research", Some(&linked), None).unwrap().id,
        task.id
    );

    // A recorded subdirectory survives creation and binding, assigned or not.
    for assigned in [false, true] {
        let root = table.worktree(if assigned { "assigned" } else { "free" });
        let research = root.join("research");
        std::fs::create_dir(&research).unwrap();
        let existing = assigned.then(|| table.create("Existing", None, Some(&root)).unwrap());
        let session = table.conversation(&research, "tui", existing.as_ref());
        let task = table.create("Research", Some(&session), None).unwrap();
        assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
        table.bind(session.id.as_str(), &task).unwrap();
        assert_eq!(table.session(&session).cwd, research, "assigned={assigned}");
        assert_eq!(table.primary(&task), Some(session.id.clone()));
    }

    // Main checkout: the conversation moves to the Task; the checkout stays shared.
    let main = table.conversation(&table.main(), "tui", None);
    let kept = table
        .create("Keep conversation", Some(&main), None)
        .unwrap();
    assert!(kept.worktree.is_none());
    assert_eq!(table.primary(&kept), Some(main.id.clone()));
    assert_eq!(table.session(&main).cwd, table.main());
    assert_eq!(
        table
            .create("Keep conversation", Some(&main), None)
            .unwrap()
            .id,
        kept.id
    );
}

#[test]
fn a_shared_primary_moves_to_the_task_and_its_scope_gets_a_new_one() {
    for wave in [None, Some("infrastructure")] {
        let table = Table::new(false);
        let store = &table.store;
        if let Some(name) = wave {
            table
                .runtime
                .block_on(crate::work::wave::ensure_wave_row(
                    store,
                    table.repo.path(),
                    name,
                ))
                .unwrap();
        }
        let ensure = || {
            let (record, _) = table
                .runtime
                .block_on(crate::ops::human_session::primary::ensure(
                    store,
                    table.repo.path(),
                    wave,
                ))
                .unwrap();
            store.sqlite.session(record.id.as_str()).unwrap().unwrap()
        };
        let scope = |session: &LfSession| store.sqlite.primary_scope(session.id.as_str()).unwrap();

        let session = ensure();
        let shared = scope(&session);
        assert!(shared.is_some());
        let task = table
            .create("Continue research", Some(&session), None)
            .unwrap();
        assert!(task.worktree.is_none(), "wave={wave:?}");
        assert_eq!(table.primary(&task), Some(session.id.clone()));
        assert_eq!(
            scope(&session),
            Some(crate::session::PrimaryScope::Task(task.id.clone()))
        );
        assert_eq!(table.session(&session).cwd, session.cwd);
        assert_eq!(
            table
                .create("Continue research", Some(&session), None)
                .unwrap()
                .id,
            task.id
        );
        let replacement = ensure();
        assert_ne!(replacement.id, session.id);
        assert_eq!(replacement.cwd, session.cwd);
        assert_eq!(scope(&replacement), shared);
    }
}

#[test]
fn binding_selects_the_task_conversation() {
    let table = Table::new(false);
    let root = table.worktree("prototype");
    let session = table.conversation(&root, "tui", None);
    let filed = table.create("Research prototype", None, None).unwrap();

    // Preview resolves identity without binding; artifact keys are not Session ids.
    let preview = table
        .runtime
        .block_on(crate::ops::human_session::preview_binding(
            &table.store,
            session.id.as_str(),
            filed.id.as_str(),
        ))
        .unwrap();
    assert_eq!(preview.session_id, session.id);
    assert_eq!(preview.task_id, filed.id.as_str());
    assert_eq!(table.session(&session).task_id, None);
    let artifact = session.artifact_key.clone().unwrap();
    assert!(refusal(table.bind(&artifact, &filed)).contains("was not found"));

    // Binding adopts the conversation's checkout for a Task without one.
    for _ in 0..2 {
        table.bind(session.id.as_str(), &filed).unwrap();
        let task = table.task(&filed.id);
        assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
        assert_eq!(task.branch, "prototype");
        assert_eq!(table.primary(&task), Some(session.id.clone()));
        assert_eq!(table.session(&session).cwd, root);
    }
    assert_eq!(
        table
            .create("Research prototype", Some(&session), None)
            .unwrap()
            .id,
        filed.id
    );
    let another = table.create("Another", None, None).unwrap();
    assert!(refusal(table.bind(session.id.as_str(), &another)).contains("already belongs"));

    // A Task with its own checkout sets the conversation's directory there.
    let origin = table.worktree("research");
    let destination = table.worktree("delivery");
    let delivery = table.create("Delivery", None, Some(&destination)).unwrap();
    let moving = table.conversation(&origin, "tui", None);
    for _ in 0..2 {
        table.bind(moving.id.as_str(), &delivery).unwrap();
        let task = table.task(&delivery.id);
        assert_eq!(task.worktree, delivery.worktree);
        assert_eq!(task.branch, delivery.branch);
    }
    assert_eq!(table.session(&moving).cwd, destination);
    // Selecting another primary leaves this conversation's directory.
    let later = table.conversation(&destination, "tui", None);
    table.bind(later.id.as_str(), &delivery).unwrap();
    assert_eq!(table.primary(&delivery), Some(later.id.clone()));
    assert_eq!(table.session(&moving).cwd, destination);
}

#[test]
fn binding_refuses_background_and_closed_sessions() {
    let table = Table::new(false);
    let root = table.worktree("prototype");
    let task = table.create("Prototype", None, Some(&root)).unwrap();
    let background = table.conversation(&root, "headless", None);
    let closed = table.conversation(&root, "tui", None);
    // Closing follows the agent process's exit; a row has no process to end.
    table
        .db()
        .execute(
            "UPDATE agent_sessions SET completed_at=1 WHERE id=?1",
            [closed.id.as_str()],
        )
        .unwrap();
    let closed = table.session(&closed);
    assert!(closed.completed_at.is_some());
    for session in [&background, &closed] {
        assert!(
            refusal(table.bind(session.id.as_str(), &task))
                .contains("unfinished interactive Session"),
            "{}",
            session.id
        );
        assert!(refusal(table.create("Same work", Some(session), None))
            .contains("unfinished interactive Session"));
        assert_eq!(table.session(session).task_id, None);
    }
    assert_eq!(table.primary(&task), None);
    assert_eq!(table.count("tasks"), 1);
}

#[test]
fn binding_a_done_task_keeps_it_done() {
    for (shared, placed) in [(false, false), (true, false), (false, true), (true, true)] {
        let table = Table::new(false);
        let origin = if shared {
            table.main()
        } else {
            table.worktree("research")
        };
        let destination = placed.then(|| table.worktree("delivery"));
        let created = table
            .create("Finished work", None, destination.as_deref())
            .unwrap();
        crate::ops::task::task_complete(table.repo.path(), created.id.as_str(), None).unwrap();
        let completed = table.task(&created.id);
        assert!(completed.completed_at.is_some());
        std::fs::write(origin.join("draft.txt"), "untracked draft\n").unwrap();
        let session = table.conversation(&origin, "tui", None);
        for _ in 0..2 {
            table.bind(session.id.as_str(), &created).unwrap();
            let bound = table.session(&session);
            assert_eq!(bound.task_id.as_ref(), Some(&created.id));
            assert_eq!(
                bound.cwd,
                destination.clone().unwrap_or_else(|| origin.clone()),
                "shared={shared}, placed={placed}"
            );
            assert_eq!(table.primary(&created), Some(session.id.clone()));
            let current = table.task(&created.id);
            assert_eq!(current.completed_at, completed.completed_at);
            assert_eq!(current.state, completed.state);
            assert_eq!(current.worktree, completed.worktree);
            assert_eq!(current.branch, completed.branch);
            assert_eq!(current.base_commit, completed.base_commit);
            assert!(table
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
        assert_eq!(table.count("tasks"), 1);
    }
}

#[test]
fn refusals_leave_ownership_unchanged() {
    let table = Table::new(false);

    // A failed reservation leaves no Task; the retry creates exactly one.
    let root = table.worktree("prototype");
    git(&root, &["config", "user.email", ""]);
    assert!(table.create("Keep prototype", None, Some(&root)).is_err());
    assert_eq!(table.count("tasks"), 0);
    git(&root, &["config", "user.email", "test@example.com"]);
    let created = table.create("Keep prototype", None, Some(&root)).unwrap();
    assert_eq!(
        table
            .create("Keep prototype", None, Some(&root))
            .unwrap()
            .id,
        created.id
    );
    let ready: String = table
        .db()
        .query_row(
            "SELECT json_extract(kind_json,'$.kind') FROM task_events WHERE task_id=?1 ORDER BY rowid DESC LIMIT 1",
            [created.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(ready, "checkout_ready");

    let elsewhere = loopflow_test_support::TestRepo::new();
    let foreign = table.conversation(&elsewhere.path().canonicalize().unwrap(), "tui", None);
    let other_root = table.worktree("other");
    let in_other = table.conversation(&other_root, "tui", None);
    let assigned = table.create("Assigned", None, None).unwrap();
    let crossed = table.conversation(&root, "tui", Some(&assigned));
    for (row, result, message) in [
        (
            "another repository",
            table.create("Foreign", Some(&foreign), None),
            "belongs to another repository",
        ),
        (
            "different checkouts",
            table.create("Mixed", Some(&in_other), Some(&root)),
            "different checkouts",
        ),
        (
            "different Tasks",
            table.create("Crossed", Some(&crossed), None),
            "belong to different Tasks",
        ),
        (
            "shared checkout",
            table.create("Shared", None, Some(&table.main())),
            "shared by its repository",
        ),
    ] {
        let error = refusal(result);
        assert!(error.contains(message), "{row}: {error}");
    }
    assert_eq!(table.count("tasks"), 2);
    assert_eq!(table.primary(&created), None);
    assert_eq!(table.session(&in_other).task_id, None);
    assert_eq!(table.session(&crossed).task_id.as_ref(), Some(&assigned.id));
}

#[test]
fn checkout_adoption_records_its_open_pull_request() {
    let table = Table::new(true);
    let root = table.worktree("prototype");
    let bare = table.repo.bare_path().to_str().unwrap().to_owned();
    git(
        table.repo.path(),
        &[
            "config",
            &format!("url.{bare}.insteadOf"),
            "https://github.com/test/repo",
        ],
    );
    git(
        table.repo.path(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/test/repo",
        ],
    );
    let bin = table.home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(&gh, format!("#!/bin/sh\ncat <<'JSON'\n[{{\"number\":42,\"url\":\"https://github.com/test/repo/pull/42\",\"state\":\"OPEN\",\"isDraft\":true,\"mergeCommit\":null,\"headRefOid\":\"{}\"}}]\nJSON\n", table.repo.head_sha())).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));

    let task = table
        .create("Existing prototype", None, Some(&root))
        .unwrap();
    assert_eq!(task.worktree.as_deref(), Some(root.as_path()));
    let publication: String = table
        .db()
        .query_row(
            "SELECT github_url FROM task_prs WHERE task_id=?1",
            [task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(publication, "https://github.com/test/repo/pull/42");
    assert_eq!(table.count("agent_sessions"), 0);
    assert_eq!(table.count("task_graphs"), 0);
}
