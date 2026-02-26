use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bollard::models::VolumeCreateOptions;
use bollard::query_parameters::{BuildImageOptions, BuilderVersion, CreateImageOptions};
use bytes::Bytes;
use futures_util::StreamExt;
use tracing::{info, warn};

use super::{
    DockerExecutor, RepoIdentity, RepoVolumeIdentity, CONTAINER_PREFIX_AGENT, LABEL_KIND,
    LABEL_KIND_REPO_VOLUME, LABEL_MANAGED,
};

pub(super) fn repo_volume_labels() -> HashMap<String, String> {
    HashMap::from([
        (LABEL_MANAGED.to_string(), "true".to_string()),
        (LABEL_KIND.to_string(), LABEL_KIND_REPO_VOLUME.to_string()),
    ])
}

impl DockerExecutor {
    pub(super) async fn ensure_volume(&self, volume_name: &str) -> Result<()> {
        if self.docker.inspect_volume(volume_name).await.is_ok() {
            return Ok(());
        }

        let _ = self
            .docker
            .create_volume(VolumeCreateOptions {
                name: Some(volume_name.to_string()),
                labels: Some(repo_volume_labels()),
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    pub(super) async fn image_exists(&self, image: &str) -> bool {
        self.docker.inspect_image(image).await.is_ok()
    }

    pub(super) async fn pull_image(&self, image: &str) -> Result<()> {
        let options = CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        };
        let mut stream = self.docker.create_image(Some(options), None, None);
        while let Some(result) = stream.next().await {
            result?;
        }
        Ok(())
    }

    pub(super) async fn ensure_base_image(&self) -> Result<()> {
        if self.image_exists(&self.image).await {
            return Ok(());
        }

        info!(image = %self.image, "base image not found locally, pulling");
        match self.pull_image(&self.image).await {
            Ok(()) => {
                info!(image = %self.image, "base image pulled");
                Ok(())
            }
            Err(err) => Err(anyhow!(
                "base image '{}' not found and pull failed: {}. \
                     Build it with: docker build -t {} docker/agent/",
                self.image,
                err,
                self.image
            )),
        }
    }

    pub(super) async fn repo_image_needs_build(&self, image: &str, stale_marker: &Path) -> bool {
        !self.image_exists(image).await || stale_marker.exists()
    }

    pub(super) fn build_context_dockerignore(repo_source: &Path) -> ignore::gitignore::Gitignore {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(repo_source);
        let _ = builder.add(repo_source.join(".dockerignore"));
        match builder.build() {
            Ok(ignore) => ignore,
            Err(_) => ignore::gitignore::Gitignore::empty(),
        }
    }

    pub(super) fn list_context_paths(repo_source: &Path) -> Vec<PathBuf> {
        let dockerignore = Self::build_context_dockerignore(repo_source);
        let walker = ignore::WalkBuilder::new(repo_source)
            .hidden(false)
            .standard_filters(false)
            .build();

        let mut paths = Vec::new();
        for entry in walker.flatten() {
            let path = entry.path();
            if path == repo_source {
                continue;
            }
            let is_dir = path.is_dir();
            if dockerignore
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
            {
                continue;
            }
            let rel = match path.strip_prefix(repo_source) {
                Ok(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
                _ => continue,
            };
            paths.push(rel);
        }
        paths.sort();
        paths
    }

    pub(super) fn build_image_context(repo_source: &Path) -> Result<Bytes> {
        let dockerfile_rel = PathBuf::from(".lf/Dockerfile");
        let mut included = Self::list_context_paths(repo_source);
        if repo_source.join(&dockerfile_rel).exists() && !included.contains(&dockerfile_rel) {
            included.push(dockerfile_rel);
            included.sort();
        }

        let mut archive = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut archive);
            for rel in included {
                let abs = repo_source.join(&rel);
                if abs.is_dir() {
                    tar.append_dir(&rel, &abs)?;
                } else {
                    tar.append_path_with_name(&abs, &rel)?;
                }
            }
            tar.finish()?;
        }
        Ok(Bytes::from(archive))
    }

    pub(super) async fn build_repo_image(&self, repo_source: &Path, tag: &str) -> Result<()> {
        let context = Self::build_image_context(repo_source)?;
        let options = BuildImageOptions {
            dockerfile: ".lf/Dockerfile".to_string(),
            t: Some(tag.to_string()),
            rm: true,
            forcerm: true,
            version: BuilderVersion::BuilderBuildKit,
            ..Default::default()
        };
        let mut stream = self
            .docker
            .build_image(options, None, Some(bollard::body_full(context)));
        let mut build_error = None;
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(error) = info.error {
                        build_error = Some(error);
                    }
                }
                Err(err) => {
                    build_error = Some(err.to_string());
                }
            }
        }
        if let Some(error) = build_error {
            return Err(anyhow!("docker api build for '{}' failed: {}", tag, error));
        }

        Ok(())
    }

    pub(super) async fn ensure_repo_image(&self, repo_source: &Path) -> Result<String> {
        self.ensure_base_image().await?;

        let dockerfile_path = repo_source.join(".lf/Dockerfile");
        if !dockerfile_path.exists() {
            return Ok(self.image.clone());
        }

        let identity = RepoIdentity::from_repo(repo_source);
        let volume_id = RepoVolumeIdentity::from_identity(&identity);
        let repo_image = format!("{CONTAINER_PREFIX_AGENT}{}:latest", volume_id.repo_key);
        let stale_marker = repo_source.join(".lf/.docker-stale");
        if !self
            .repo_image_needs_build(&repo_image, &stale_marker)
            .await
        {
            return Ok(repo_image);
        }

        // Serialize concurrent builds for the same repo image.
        let lock = self.image_build_locks.for_key(&repo_image).await;
        let _guard = lock.lock().await;

        // Re-check after acquiring lock — another wave may have built it.
        if !self
            .repo_image_needs_build(&repo_image, &stale_marker)
            .await
        {
            return Ok(repo_image);
        }

        info!(
            image = %repo_image,
            repo = %repo_source.display(),
            "building per-repo agent image"
        );
        self.build_repo_image(repo_source, &repo_image).await?;

        // Remove stale marker after successful build.
        if stale_marker.exists() {
            if let Err(err) = std::fs::remove_file(&stale_marker) {
                warn!(
                    path = %stale_marker.display(),
                    error = %err,
                    "failed to remove .docker-stale marker"
                );
            }
        }

        Ok(repo_image)
    }
}
