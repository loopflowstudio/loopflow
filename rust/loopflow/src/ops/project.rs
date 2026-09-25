use std::path::Path;
use std::sync::Arc;

use crate::engine::git::{current_branch, get_default_branch, is_clean, worktree_root};
use crate::ops::{OpsError, OpsResult};
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;

fn project_error(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

fn block_on_project<T>(future: impl std::future::Future<Output = OpsResult<T>>) -> OpsResult<T> {
    tokio::runtime::Runtime::new()
        .map_err(|error| project_error(format!("failed to build project runtime: {error}")))?
        .block_on(future)
}

async fn project_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        project_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

pub(crate) fn ensure_project_for_task(
    repo: &Path,
    resolved: &crate::ops::task_pm::ResolvedTask,
) -> OpsResult<Project> {
    let locator = crate::work::wave::WaveLocator::discover(repo, &resolved.snapshot.wave)
        .map_err(|error| project_error(error.to_string()))?;
    block_on_project(async move {
        let store = project_store().await?;
        let wave = store
            .get_wave_at(&locator)
            .await
            .map_err(|cause| project_error(cause.to_string()))?
            .ok_or_else(|| project_error("owning Wave is not initialized"))?;
        let current = store
            .chapter(wave.id(), None)
            .await
            .map_err(|cause| project_error(cause.to_string()))?
            .ok_or_else(|| project_error("Wave has no chapter; run `lf wave new-chapter`"))?;
        if current.project_id != resolved.project.id {
            return Err(project_error(
                "this plan is chapter history; use the Wave's current chapter",
            ));
        }
        store
            .get_project_by_project(&current.project_id)
            .await
            .map_err(|cause| project_error(cause.to_string()))?
            .ok_or_else(|| {
                project_error(
                    "current chapter record is unavailable; resume its chapter transition",
                )
            })
    })
}

pub(crate) fn project_plan(
    project: &crate::pm::PmProject,
    pm_snapshot_synced_at: i64,
) -> OpsResult<ProjectPlan> {
    Ok(ProjectPlan {
        id: LinearProjectId::new(project.id.clone())
            .map_err(|error| project_error(error.to_string()))?,
        slug: project.slug.clone(),
        name: project.name.clone(),
        prompt_context: crate::ops::task::project_context(project),
        pm_snapshot_synced_at,
    })
}

pub(crate) fn ensure_clean_main(repo: &Path, subject: &str) -> OpsResult<std::path::PathBuf> {
    let worktree = worktree_root(repo).map_err(|error| project_error(error.to_string()))?;
    let main = crate::engine::worktrees::main_repo_root(repo)
        .map_err(|error| project_error(error.to_string()))?;
    let worktree = std::fs::canonicalize(&worktree).unwrap_or(worktree);
    let main = std::fs::canonicalize(&main).unwrap_or(main);
    if worktree != main {
        return Err(project_error(format!(
            "cannot run {subject} from {}: Wave turns require the canonical main checkout; run existing work with `lf task run <issue-id>` or create it with `lf task start --wave <wave> \"<title>\"`",
            worktree.display()
        )));
    }
    let default_branch =
        get_default_branch(&main).map_err(|error| project_error(error.to_string()))?;
    let branch = current_branch(&main).map_err(|error| project_error(error.to_string()))?;
    if branch.as_deref() != Some(default_branch.as_str()) {
        return Err(project_error(format!(
            "cannot run {subject}: canonical checkout is on {}, expected {default_branch}",
            branch.as_deref().unwrap_or("detached HEAD")
        )));
    }
    if !is_clean(&main).map_err(|error| project_error(error.to_string()))? {
        return Err(project_error(format!(
            "cannot run {subject}: canonical {default_branch} checkout is dirty; Wave turns never edit repository files"
        )));
    }
    Ok(main)
}

#[cfg(test)]
mod tests {
    use super::ensure_clean_main;
    use std::process::Command;

    #[test]
    fn control_plane_requires_a_clean_canonical_main() {
        let tmp = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(tmp.path().join("README.md"), "hello\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "."])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(tmp.path())
            .status()
            .unwrap()
            .success());

        assert_eq!(
            ensure_clean_main(tmp.path(), "Project").unwrap(),
            std::fs::canonicalize(tmp.path()).unwrap()
        );
        std::fs::write(tmp.path().join("README.md"), "changed\n").unwrap();
        let error = ensure_clean_main(tmp.path(), "Project").unwrap_err();
        assert!(error
            .to_string()
            .contains("canonical main checkout is dirty"));
    }
}
