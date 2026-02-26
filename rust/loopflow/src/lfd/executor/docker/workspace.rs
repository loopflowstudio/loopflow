use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tracing::warn;

use crate::engine::git::current_branch;
use crate::lfd::id::LfdId;
use crate::lfd::types::{Wave, WaveRun};

use super::{
    canonical_repo_url, sanitize_token, short_hash, DockerExecutor, DockerWorkspace, RepoIdentity,
    RepoVolumeIdentity, CONTAINER_REPOS_ROOT, HOST_WORKTREE_MOUNT, LOCAL_REPO_MOUNT,
};

impl DockerExecutor {
    pub(super) fn worktree_slug_from_host_path(host_worktree: &Path) -> String {
        let fallback = host_worktree.to_string_lossy().to_string();
        let slug = host_worktree
            .file_name()
            .and_then(|name| name.to_str())
            .map(sanitize_token)
            .unwrap_or_default();
        if slug.is_empty() {
            short_hash(&fallback, 12)
        } else {
            slug
        }
    }

    pub(super) fn infer_fork_branch_from_worktree(
        host_worktree: &Path,
        wave_run_id: &str,
    ) -> Option<String> {
        let name = host_worktree.file_name()?.to_str()?;
        let index = name.rsplit_once("-fork-")?.1.parse::<u32>().ok()?;
        Some(format!("{wave_run_id}-fork-{index}"))
    }

    pub(super) fn docker_workspace_for_host_worktree(
        repo_source: &Path,
        host_worktree: &Path,
        branch: &str,
    ) -> DockerWorkspace {
        let repo_identity = RepoIdentity::from_repo(repo_source);
        let volume = RepoVolumeIdentity::from_identity(&repo_identity);
        let worktree_slug = Self::worktree_slug_from_host_path(host_worktree);
        DockerWorkspace {
            container_shared_clone: format!("{CONTAINER_REPOS_ROOT}/{}/main", volume.repo_key),
            container_worktree: format!(
                "{CONTAINER_REPOS_ROOT}/{}/worktrees/{worktree_slug}",
                volume.repo_key
            ),
            volume,
            repo_source: repo_source.to_path_buf(),
            branch: branch.to_string(),
            has_remote: repo_identity.has_remote,
        }
    }

    pub(super) fn resolve_wave_run_branch(run: &WaveRun, wave: &Wave) -> String {
        if !run.branch.trim().is_empty() {
            return run.branch.clone();
        }
        let fallback = sanitize_token(wave.name());
        if fallback.is_empty() {
            "main".to_string()
        } else {
            fallback
        }
    }

    pub(super) fn resolve_host_repo(repo: &str) -> PathBuf {
        let repo_path = PathBuf::from(repo);
        crate::engine::worktrees::main_repo_root(&repo_path).unwrap_or(repo_path)
    }

    pub(super) fn resolve_host_repo_from_worktree(worktree: &Path) -> PathBuf {
        crate::engine::worktrees::main_repo_root(worktree).unwrap_or_else(|_| {
            worktree
                .canonicalize()
                .unwrap_or_else(|_| worktree.to_path_buf())
        })
    }

    pub(super) fn resolve_workspace_for_host_worktree(host_worktree: &Path) -> DockerWorkspace {
        let repo_source = Self::resolve_host_repo_from_worktree(host_worktree);
        let branch = Self::resolve_workspace_branch(host_worktree, None, "main");
        Self::docker_workspace_for_host_worktree(&repo_source, host_worktree, &branch)
    }

    pub(super) fn resolve_workspace_branch(
        cwd: &Path,
        context_branch: Option<&str>,
        fallback: &str,
    ) -> String {
        if let Some(branch) = context_branch.filter(|value| !value.trim().is_empty()) {
            return branch.to_string();
        }

        Self::checked_out_branch(cwd).unwrap_or_else(|| Self::fallback_workspace_branch(fallback))
    }

    pub(super) fn resolve_workspace_branch_for_recovery(
        cwd: &Path,
        wave_run_id: &str,
        fallback: &str,
    ) -> String {
        if let Some(branch) = Self::checked_out_branch(cwd) {
            return branch;
        }

        if let Some(branch) = Self::infer_fork_branch_from_worktree(cwd, wave_run_id) {
            return branch;
        }

        Self::fallback_workspace_branch(fallback)
    }

    pub(super) fn checked_out_branch(cwd: &Path) -> Option<String> {
        if !cwd.join(".git").exists() {
            return None;
        }
        let branch = current_branch(cwd).ok().flatten()?;
        if branch.trim().is_empty() || branch == "HEAD" {
            return None;
        }
        Some(branch)
    }

    pub(super) fn fallback_workspace_branch(fallback: &str) -> String {
        if fallback.trim().is_empty() {
            "main".to_string()
        } else {
            fallback.to_string()
        }
    }

    pub(super) async fn resolve_workspace_inputs(
        &self,
        wave_id: &str,
        wave_run_id: &str,
    ) -> Result<(PathBuf, String)> {
        let wave_id = LfdId::from_raw(wave_id);
        let wave = self
            .store
            .get_wave(&wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found for docker run"))?;
        let run_id = LfdId::from_raw(wave_run_id);
        let run = self
            .store
            .get_wave_run(&run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found for docker run"))?;
        let repo_source = Self::resolve_host_repo(&run.snapshot.repo);
        let fallback_branch = Self::resolve_wave_run_branch(&run, &wave);
        Ok((repo_source, fallback_branch))
    }

    pub(super) async fn resolve_workspace(
        &self,
        wave_id: &str,
        wave_run_id: &str,
        cwd: &Path,
        context_branch: Option<&str>,
    ) -> Result<DockerWorkspace> {
        let (repo_source, fallback_branch) =
            self.resolve_workspace_inputs(wave_id, wave_run_id).await?;
        let branch = Self::resolve_workspace_branch(cwd, context_branch, &fallback_branch);
        Ok(Self::docker_workspace_for_host_worktree(
            &repo_source,
            cwd,
            &branch,
        ))
    }

    pub(super) async fn resolve_workspace_for_recovery(
        &self,
        wave_id: &str,
        wave_run_id: &str,
        cwd: &Path,
    ) -> Result<DockerWorkspace> {
        let (repo_source, fallback_branch) =
            self.resolve_workspace_inputs(wave_id, wave_run_id).await?;
        let branch =
            Self::resolve_workspace_branch_for_recovery(cwd, wave_run_id, &fallback_branch);
        Ok(Self::docker_workspace_for_host_worktree(
            &repo_source,
            cwd,
            &branch,
        ))
    }

    pub(super) async fn git_command(
        &self,
        workspace: &DockerWorkspace,
        label: &str,
        cmd: Vec<String>,
        include_local_repo: bool,
    ) -> Result<String> {
        let mut mounts = Vec::new();
        if include_local_repo {
            mounts.push(Self::bind_mount(
                &workspace.repo_source,
                LOCAL_REPO_MOUNT,
                true,
            ));
        }
        self.run_helper_command(label, cmd, self.helper_mounts(workspace, mounts), None)
            .await
    }

    pub(super) async fn is_git_repo(&self, workspace: &DockerWorkspace, repo_path: &str) -> bool {
        self.git_command(
            workspace,
            "git-probe",
            vec![
                "git".to_string(),
                "-C".to_string(),
                repo_path.to_string(),
                "rev-parse".to_string(),
                "--is-inside-work-tree".to_string(),
            ],
            false,
        )
        .await
        .is_ok()
    }

    async fn ensure_workspace_directory(&self, volume_name: &str, path: &Path) -> Result<()> {
        self.run_helper_command(
            "mkdir",
            vec![
                "mkdir".to_string(),
                "-p".to_string(),
                path.to_string_lossy().to_string(),
            ],
            self.build_mounts(volume_name),
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn ensure_shared_clone(&self, workspace: &DockerWorkspace) -> Result<()> {
        if self
            .is_git_repo(workspace, &workspace.container_shared_clone)
            .await
        {
            return Ok(());
        }

        let root_path = Path::new(&workspace.container_shared_clone)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("invalid shared clone path"))?;
        self.ensure_workspace_directory(&workspace.volume.volume_name, &root_path)
            .await?;

        let remote = canonical_repo_url(&workspace.repo_source);
        let (source, include_local_repo) = if let Some(url) = remote {
            (url, false)
        } else {
            (LOCAL_REPO_MOUNT.to_string(), true)
        };

        self.git_command(
            workspace,
            "git-clone",
            vec![
                "git".to_string(),
                "clone".to_string(),
                source,
                workspace.container_shared_clone.clone(),
            ],
            include_local_repo,
        )
        .await?;

        Ok(())
    }

    pub(super) async fn fetch_shared_clone(&self, workspace: &DockerWorkspace) -> Result<()> {
        self.git_command(
            workspace,
            "git-fetch",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "fetch".to_string(),
                "--prune".to_string(),
                "origin".to_string(),
            ],
            !workspace.has_remote,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn ensure_worktree(&self, workspace: &DockerWorkspace) -> Result<()> {
        if self
            .is_git_repo(workspace, &workspace.container_worktree)
            .await
        {
            return Ok(());
        }

        let parent = Path::new(&workspace.container_worktree)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("invalid worktree path"))?;
        self.ensure_workspace_directory(&workspace.volume.volume_name, &parent)
            .await?;

        let local_branch_ref = format!("refs/heads/{}", workspace.branch);
        let has_local_branch = self
            .git_command(
                workspace,
                "git-show-ref-local",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_shared_clone.clone(),
                    "show-ref".to_string(),
                    "--verify".to_string(),
                    "--quiet".to_string(),
                    local_branch_ref,
                ],
                false,
            )
            .await
            .is_ok();

        let command = if has_local_branch {
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "worktree".to_string(),
                "add".to_string(),
                "--force".to_string(),
                workspace.container_worktree.clone(),
                workspace.branch.clone(),
            ]
        } else {
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "worktree".to_string(),
                "add".to_string(),
                "--force".to_string(),
                "-B".to_string(),
                workspace.branch.clone(),
                workspace.container_worktree.clone(),
                "HEAD".to_string(),
            ]
        };

        self.git_command(workspace, "git-worktree-add", command, false)
            .await?;
        Ok(())
    }

    pub(super) async fn run_hygiene(&self, workspace: &DockerWorkspace) -> Result<()> {
        let target_ref = format!("refs/heads/{}", workspace.branch);
        let target = if self
            .git_command(
                workspace,
                "git-rev-parse",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_worktree.clone(),
                    "rev-parse".to_string(),
                    "--verify".to_string(),
                    target_ref.clone(),
                ],
                false,
            )
            .await
            .is_ok()
        {
            target_ref
        } else {
            "HEAD".to_string()
        };

        self.git_command(
            workspace,
            "git-reset",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_worktree.clone(),
                "reset".to_string(),
                "--hard".to_string(),
                target,
            ],
            false,
        )
        .await?;

        self.git_command(
            workspace,
            "git-clean",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_worktree.clone(),
                "clean".to_string(),
                "-fdx".to_string(),
            ],
            false,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn sync_to_host_worktree(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        let script = format!(
                "set -eu\nfind {HOST_WORKTREE_MOUNT} -mindepth 1 -maxdepth 1 ! -name .git ! -name .lf -exec rm -rf {{}} +\ntar -C '{}' --exclude=.git --exclude=.lf -cf - . | tar -C {HOST_WORKTREE_MOUNT} -xf -",
                workspace.container_worktree
            );
        self.run_helper_command(
            "sync-host",
            vec!["sh".to_string(), "-lc".to_string(), script],
            self.helper_mounts(
                workspace,
                vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, false)],
            ),
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn sync_lf_to_volume(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        let host_lf = host_worktree.join(".lf");
        if !host_lf.exists() {
            return Ok(());
        }
        let script = format!(
            "set -eu\nrm -rf '{0}/.lf'\ncp -a {HOST_WORKTREE_MOUNT}/.lf '{0}/.lf'",
            workspace.container_worktree
        );
        self.run_helper_command(
            "sync-lf",
            vec!["sh".to_string(), "-lc".to_string(), script],
            self.helper_mounts(
                workspace,
                vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, true)],
            ),
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn prepare_workspace(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        self.ensure_volume(&workspace.volume.volume_name).await?;

        let should_hygiene = {
            let mut prepared = self.prepared_runs.lock().await;
            prepared.insert(Self::prepared_workspace_key(workspace, host_worktree))
        };

        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        {
            let _guard = lock.lock().await;
            self.ensure_shared_clone(workspace).await?;
            if should_hygiene {
                self.fetch_shared_clone(workspace).await?;
            }
            self.ensure_worktree(workspace).await?;
        }

        if should_hygiene {
            self.run_hygiene(workspace).await?;
        }

        self.sync_to_host_worktree(workspace, host_worktree).await?;

        Ok(())
    }

    pub(super) fn normalize_relative_workspace_path(relative_path: &str) -> Result<PathBuf> {
        let path = Path::new(relative_path);
        if path.is_absolute() {
            return Err(anyhow!("workspace path must be relative"));
        }

        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::Normal(segment) => normalized.push(segment),
                _ => {
                    return Err(anyhow!(
                        "workspace path may not contain parent traversal components"
                    ));
                }
            }
        }

        if normalized.as_os_str().is_empty() {
            return Err(anyhow!("workspace path must not be empty"));
        }
        Ok(normalized)
    }

    pub(super) async fn ensure_container_worktree(
        &self,
        workspace: &DockerWorkspace,
    ) -> Result<()> {
        self.ensure_volume(&workspace.volume.volume_name).await?;
        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        let _guard = lock.lock().await;
        self.ensure_shared_clone(workspace).await?;
        self.ensure_worktree(workspace).await?;
        Ok(())
    }

    pub(super) async fn write_file_to_volume(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
        relative_path: &str,
    ) -> Result<()> {
        let normalized = Self::normalize_relative_workspace_path(relative_path)?;
        let host_source = Path::new(HOST_WORKTREE_MOUNT).join(&normalized);
        let container_target = Path::new(&workspace.container_worktree).join(&normalized);
        let container_parent = container_target
            .parent()
            .ok_or_else(|| anyhow!("workspace file target has no parent"))?
            .to_string_lossy()
            .to_string();

        let helper_mounts = self.helper_mounts(
            workspace,
            vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, true)],
        );
        self.run_helper_command(
            "workspace-write-mkdir",
            vec!["mkdir".to_string(), "-p".to_string(), container_parent],
            helper_mounts.clone(),
            None,
        )
        .await?;
        self.run_helper_command(
            "workspace-write-copy",
            vec![
                "cp".to_string(),
                host_source.to_string_lossy().to_string(),
                container_target.to_string_lossy().to_string(),
            ],
            helper_mounts,
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn remove_file_from_volume(
        &self,
        workspace: &DockerWorkspace,
        relative_path: &str,
    ) -> Result<()> {
        let normalized = Self::normalize_relative_workspace_path(relative_path)?;
        let container_target = Path::new(&workspace.container_worktree).join(&normalized);

        self.run_helper_command(
            "workspace-remove-file",
            vec![
                "rm".to_string(),
                "-f".to_string(),
                container_target.to_string_lossy().to_string(),
            ],
            self.build_mounts(&workspace.volume.volume_name),
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn cleanup_container_worktree(
        &self,
        workspace: &DockerWorkspace,
    ) -> Result<()> {
        if self
            .docker
            .inspect_volume(&workspace.volume.volume_name)
            .await
            .is_err()
        {
            return Ok(());
        }

        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        let _guard = lock.lock().await;
        if !self
            .is_git_repo(workspace, &workspace.container_shared_clone)
            .await
        {
            return Ok(());
        }

        if let Err(err) = self
            .git_command(
                workspace,
                "git-worktree-remove",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_shared_clone.clone(),
                    "worktree".to_string(),
                    "remove".to_string(),
                    "--force".to_string(),
                    workspace.container_worktree.clone(),
                ],
                false,
            )
            .await
        {
            warn!(
                worktree = %workspace.container_worktree,
                error = %err,
                "failed removing docker fork worktree"
            );
        }

        Ok(())
    }

    pub(super) fn prepared_workspace_key(
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> String {
        format!(
            "{}:{}",
            workspace.volume.repo_key,
            host_worktree.to_string_lossy()
        )
    }
}
