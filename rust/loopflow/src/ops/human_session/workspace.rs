use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::durable::{MachineId, WorkRef};
use crate::store::sqlite::TaskCheckout;
use crate::store::SharedStore;
use crate::work::task::TaskId;

use super::SessionRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkspace {
    pub machine_id: MachineId,
    pub worktree: PathBuf,
    pub task_id: Option<TaskId>,
    pub unavailable: Option<String>,
}

struct WorkspaceResolver {
    home: MachineId,
    checkouts: Vec<TaskCheckout>,
    roots: HashMap<PathBuf, Option<PathBuf>>,
    tasks_by_root: HashMap<PathBuf, Vec<TaskId>>,
}

impl WorkspaceResolver {
    fn new(home: MachineId, checkouts: Vec<TaskCheckout>) -> Self {
        let mut resolver = Self {
            home,
            checkouts,
            roots: HashMap::new(),
            tasks_by_root: HashMap::new(),
        };
        for checkout in resolver.checkouts.clone() {
            if checkout.machine_id.as_ref() != Some(&resolver.home) {
                continue;
            }
            if let Some(root) = resolver.root(&checkout.worktree) {
                resolver
                    .tasks_by_root
                    .entry(root)
                    .or_default()
                    .push(checkout.task_id);
            }
        }
        resolver
    }

    fn root(&mut self, path: &Path) -> Option<PathBuf> {
        // The filesystem reader resolves aliases. Cache missing checkouts too:
        // retained Sessions can share the same retired path within this page.
        self.roots
            .entry(path.to_path_buf())
            .or_insert_with(|| crate::engine::git::worktree_root(path).ok())
            .clone()
    }

    fn resolve(
        &mut self,
        cwd: &Path,
        recorded_root: Option<&Path>,
        work: Option<&WorkRef>,
    ) -> Option<SessionWorkspace> {
        if let Some(root) = self.root(cwd) {
            let tasks = self
                .tasks_by_root
                .get(&root)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let missing_placement = self.checkouts.iter().any(|checkout| {
                checkout.machine_id.is_none()
                    && (checkout.worktree == root || checkout.worktree == cwd)
            });
            return Some(SessionWorkspace {
                machine_id: self.home.clone(),
                worktree: root,
                task_id: (tasks.len() == 1).then(|| tasks[0].clone()),
                unavailable: if tasks.len() > 1 {
                    Some("Multiple Tasks claim this checkout".into())
                } else if missing_placement {
                    Some("Task placement is unavailable".into())
                } else {
                    None
                },
            });
        }
        // An unavailable cwd is not an ancestor match. Only exact recorded
        // checkout evidence or explicit Task attribution can retain association.
        let mut candidates = self.checkouts.iter().filter(|checkout| {
            checkout
                .machine_id
                .as_ref()
                .is_none_or(|home| home == &self.home)
                && (checkout.worktree == cwd
                    || recorded_root == Some(checkout.worktree.as_path())
                    || matches!(work, Some(WorkRef::Task(id)) if id == &checkout.task_id))
        });
        let checkout = candidates.next()?;
        let ambiguous = candidates.next().is_some();
        Some(SessionWorkspace {
            machine_id: self.home.clone(),
            worktree: checkout.worktree.clone(),
            task_id: (!ambiguous).then(|| checkout.task_id.clone()),
            unavailable: Some(
                if ambiguous {
                    "Multiple Tasks claim this unavailable checkout"
                } else if checkout.machine_id.is_none() {
                    "Task placement and checkout are unavailable"
                } else {
                    "Task checkout is unavailable"
                }
                .into(),
            ),
        })
    }
}

pub(super) async fn associate(store: &SharedStore, sessions: &mut [SessionRecord]) -> Result<()> {
    // A failed snapshot propagates: callers must retain their last good inventory.
    let checkouts = store.task_checkouts().await?;
    let home = store.local_machine().await?.id;
    let mut resolver = WorkspaceResolver::new(home.clone(), checkouts);
    for session in sessions {
        if session.primary_scope.is_some() {
            let cwd = Path::new(&session.cwd);
            session.workspace = Some(SessionWorkspace {
                machine_id: home.clone(),
                worktree: resolver.root(cwd).unwrap_or_else(|| cwd.to_path_buf()),
                task_id: None,
                unavailable: (!cwd.exists()).then(|| "Persistent checkout is unavailable".into()),
            });
            session.task_ids.clear();
            continue;
        }
        if session
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.machine_id != home)
        {
            continue;
        }
        session.workspace = resolver.resolve(Path::new(&session.cwd), None, session.work.as_ref());
        if let Some(workspace) = &session.workspace {
            session.task_ids = workspace.task_id.iter().cloned().collect();
        }
        // Checkout placement and explicit binding both grant membership.
        if let Some(WorkRef::Task(task)) = &session.work {
            if !session.task_ids.contains(task) {
                session.task_ids.push(task.clone());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::WorkspaceResolver;
    use crate::durable::{MachineId, WorkRef};
    use crate::store::sqlite::TaskCheckout;
    use crate::work::task::TaskId;

    #[tokio::test]
    async fn unavailable_placement_snapshot_is_not_an_empty_inventory() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("store.db");
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(path.clone()))
                .await
                .unwrap(),
        );
        rusqlite::Connection::open(path)
            .unwrap()
            .execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE work_placements;")
            .unwrap();
        assert!(super::associate(&store, &mut []).await.is_err());
    }

    #[test]
    fn workspace_resolves_checkout_identity_without_changing_attribution() {
        let repo = loopflow_test_support::TestRepo::new();
        let sibling = loopflow_test_support::TestRepo::new();
        let home = MachineId::new();
        let id = TaskId::new();
        let checkout = TaskCheckout {
            task_id: id.clone(),
            issue_id: "issue".into(),
            issue_identifier: "TEST-1".into(),
            worktree: repo.path().to_path_buf(),
            machine_id: Some(home.clone()),
        };
        let mut remote = checkout.clone();
        remote.task_id = TaskId::new();
        remote.machine_id = Some(MachineId::new());
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![checkout.clone(), remote]);
        std::fs::create_dir(repo.path().join("sub")).unwrap();
        let aliases = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(repo.path(), aliases.path().join("alias")).unwrap();
        let other_work = WorkRef::Task(TaskId::new());
        for path in [repo.path().join("sub"), aliases.path().join("alias/sub")] {
            let workspace = resolver.resolve(&path, None, Some(&other_work)).unwrap();
            assert_eq!(workspace.task_id, Some(id.clone()));
            assert_eq!(workspace.machine_id, home);
            assert_eq!(workspace.worktree, repo.path().canonicalize().unwrap());
        }
        assert!(resolver
            .resolve(sibling.path(), None, Some(&WorkRef::Task(id.clone())))
            .unwrap()
            .task_id
            .is_none());
        assert!(std::process::Command::new("git")
            .current_dir(repo.path().join("sub"))
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![checkout.clone()]);
        assert!(resolver
            .resolve(&repo.path().join("sub"), None, None)
            .unwrap()
            .task_id
            .is_none());
        assert!(resolver
            .resolve(&repo.path().join("missing"), None, None)
            .is_none());
        let retained = resolver
            .resolve(&repo.path().join("missing"), Some(repo.path()), None)
            .unwrap();
        assert_eq!(retained.task_id, Some(id.clone()));
        assert!(retained.unavailable.is_some());
        let mut missing = checkout.clone();
        missing.worktree = repo.path().join("gone");
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![missing.clone()]);
        assert_eq!(
            resolver
                .resolve(&missing.worktree, None, None)
                .unwrap()
                .task_id,
            Some(id.clone())
        );
        assert!(resolver
            .resolve(&missing.worktree.join("unbound"), None, None)
            .is_none());
        assert_eq!(
            resolver
                .resolve(
                    &missing.worktree.join("bound"),
                    None,
                    Some(&WorkRef::Task(id.clone()))
                )
                .unwrap()
                .task_id,
            Some(id.clone())
        );
        let mut unplaced = checkout.clone();
        unplaced.machine_id = None;
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![unplaced]);
        let unplaced = resolver.resolve(repo.path(), None, None).unwrap();
        assert!(unplaced.task_id.is_none());
        assert!(unplaced.unavailable.is_some());
        let mut duplicate = checkout.clone();
        duplicate.task_id = TaskId::new();
        let mut resolver = WorkspaceResolver::new(home, vec![checkout, duplicate]);
        let ambiguous = resolver.resolve(repo.path(), None, None).unwrap();
        assert!(ambiguous.task_id.is_none());
        assert!(ambiguous.unavailable.is_some());
    }
}
