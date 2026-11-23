use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::store::SharedStore;
use loopflow_model::durable::{MachineId, WorkRef};
use loopflow_storage::sqlite::TaskCheckout;
use loopflow_storage::work::task::TaskId;

use super::SessionRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionWorkspace {
    pub machine_id: MachineId,
    pub worktree: PathBuf,
    pub task_id: Option<TaskId>,
    pub unavailable: Option<String>,
}

/// Where each listed Session's directory sits: the Git worktree holding it and
/// the Task whose checkout that worktree is, by `task_checkout_owner`'s rule
/// (root equality, lower id on a shared root). A Session records its own Task separately.
#[derive(Debug)]
struct WorkspaceResolver {
    home: MachineId,
    checkouts: Vec<TaskCheckout>,
    roots: HashMap<PathBuf, Option<PathBuf>>,
    owners: HashMap<PathBuf, TaskId>,
}

impl WorkspaceResolver {
    fn new(home: MachineId, mut checkouts: Vec<TaskCheckout>) -> Self {
        checkouts.sort_by(|a, b| a.task_id.as_str().cmp(b.task_id.as_str()));
        let mut resolver = Self {
            home,
            checkouts: Vec::with_capacity(checkouts.len()),
            roots: HashMap::new(),
            owners: HashMap::new(),
        };
        for checkout in checkouts {
            if let Some(root) = resolver.root(&checkout.worktree) {
                resolver
                    .owners
                    .entry(root)
                    .or_insert_with(|| checkout.task_id.clone());
            }
            resolver.checkouts.push(checkout);
        }
        resolver
    }

    fn root(&mut self, path: &Path) -> Option<PathBuf> {
        // The filesystem reader resolves aliases. Cache missing checkouts too:
        // retained Sessions can share the same retired path within this page.
        self.roots
            .entry(path.to_path_buf())
            .or_insert_with(|| crate::git::worktree_root(path).ok())
            .clone()
    }

    fn resolve(&mut self, cwd: &Path, work: Option<&WorkRef>) -> Option<SessionWorkspace> {
        if let Some(root) = self.root(cwd) {
            return Some(SessionWorkspace {
                machine_id: self.home.clone(),
                task_id: self.owners.get(&root).cloned(),
                worktree: root,
                unavailable: None,
            });
        }
        // An unavailable cwd is not an ancestor match: only a checkout recorded
        // at it, or the Session's own Task's checkout, places it.
        let checkout = self.checkouts.iter().find(|checkout| {
            checkout.worktree == cwd
                || matches!(work, Some(WorkRef::Task(id)) if id == &checkout.task_id)
        })?;
        Some(SessionWorkspace {
            machine_id: self.home.clone(),
            worktree: checkout.worktree.clone(),
            task_id: Some(checkout.task_id.clone()),
            unavailable: Some("Task checkout is unavailable".into()),
        })
    }
}

/// Places each local registry Session in its workspace. A failed snapshot
/// propagates: callers must retain their last good inventory.
pub async fn associate(store: &SharedStore, sessions: &mut [SessionRecord]) -> Result<()> {
    let checkouts = store.task_checkouts().await?;
    let home = store.local_machine().await?.id;
    let mut resolver = WorkspaceResolver::new(home.clone(), checkouts);
    for session in sessions {
        let cwd = Path::new(&session.cwd);
        session.workspace = if matches!(
            session.primary_scope.as_deref(),
            Some("repository" | "wave")
        ) {
            Some(SessionWorkspace {
                machine_id: home.clone(),
                worktree: resolver.root(cwd).unwrap_or_else(|| cwd.to_path_buf()),
                task_id: None,
                unavailable: (!crate::observe::exists(cwd))
                    .then(|| "Persistent checkout is unavailable".into()),
            })
        } else {
            resolver.resolve(cwd, session.work.as_ref())
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::WorkspaceResolver;
    use loopflow_model::durable::{MachineId, WorkRef};
    use loopflow_storage::sqlite::TaskCheckout;
    use loopflow_storage::work::task::TaskId;

    #[tokio::test]
    async fn unavailable_checkout_snapshot_is_not_an_empty_inventory() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("store.db");
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(path.clone()))
                .await
                .unwrap(),
        );
        rusqlite::Connection::open(path)
            .unwrap()
            .execute_batch("ALTER TABLE task_checkouts RENAME COLUMN worktree TO unavailable;")
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
            issue_identifier: "TEST-1".into(),
            worktree: repo.path().to_path_buf(),
        };
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![checkout.clone()]);
        std::fs::create_dir(repo.path().join("sub")).unwrap();
        let aliases = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(repo.path(), aliases.path().join("alias")).unwrap();
        let other_work = WorkRef::Task(TaskId::new());
        for path in [repo.path().join("sub"), aliases.path().join("alias/sub")] {
            let workspace = resolver.resolve(&path, Some(&other_work)).unwrap();
            assert_eq!(workspace.task_id, Some(id.clone()));
            assert_eq!(workspace.machine_id, home);
            assert_eq!(workspace.worktree, repo.path().canonicalize().unwrap());
        }
        assert!(resolver
            .resolve(sibling.path(), Some(&WorkRef::Task(id.clone())))
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
            .resolve(&repo.path().join("sub"), None)
            .unwrap()
            .task_id
            .is_none());
        assert!(resolver
            .resolve(&repo.path().join("missing"), None)
            .is_none());
        let mut missing = checkout.clone();
        missing.worktree = repo.path().join("gone");
        let mut resolver = WorkspaceResolver::new(home.clone(), vec![missing.clone()]);
        assert_eq!(
            resolver.resolve(&missing.worktree, None).unwrap().task_id,
            Some(id.clone())
        );
        assert!(resolver
            .resolve(&missing.worktree.join("unbound"), None)
            .is_none());
        assert_eq!(
            resolver
                .resolve(
                    &missing.worktree.join("bound"),
                    Some(&WorkRef::Task(id.clone()))
                )
                .unwrap()
                .task_id,
            Some(id.clone())
        );
        let mut duplicate = checkout.clone();
        duplicate.task_id = TaskId::new();
        let lower = id.as_str().min(duplicate.task_id.as_str()).to_owned();
        let mut resolver = WorkspaceResolver::new(home, vec![checkout, duplicate]);
        let shared = resolver.resolve(repo.path(), None).unwrap();
        assert_eq!(shared.task_id.unwrap().as_str(), lower);
        assert!(shared.unavailable.is_none());
    }
}
