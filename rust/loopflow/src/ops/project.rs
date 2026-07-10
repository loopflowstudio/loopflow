use std::path::Path;
use std::time::Duration;

use crate::lfd::executor::helpers::{resolve_lf_binary, spawn_detached_lf, tmux_session_slug};
use crate::lfd::id::LfdId;
use crate::lfd::types::Wave;
use crate::lfdb::{open_existing_store, Store};
use crate::ops::{OpsError, OpsResult};

/// Complete the mechanical half of an authored project-promotion flow: pin
/// the registry ancestry, start the child residency, and wait for its endpoint.
pub fn complete_promotion(repo: &Path, parent: &str, child: &str) -> OpsResult<String> {
    let origin = crate::engine::wave_context::wave_origin(repo);
    let goal = origin.join("wave").join(child).join("GOAL.md");
    if !goal.is_file() {
        return Err(OpsError::Message(format!(
            "promotion is authored but not visible to the wave listener at {}; land the migration before starting residency",
            goal.display()
        )));
    }

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to build promotion runtime: {err}")))?;
    runtime.block_on(async {
        let store = open_existing_store().await.ok_or_else(|| {
            OpsError::Message(
                "project promotion requires the wave registry; start the parent wave first"
                    .to_string(),
            )
        })?;
        link_parent(&store, &origin, parent, child).await?;

        if crate::wave::server::live_endpoint(&origin, child)
            .await
            .is_none()
        {
            launch_residency(&origin, child).await?;
        }
        for _ in 0..100 {
            if crate::wave::server::live_endpoint(&origin, child)
                .await
                .is_some()
            {
                wake_child(&origin, child).await?;
                return Ok(promotion_session_name(&origin, child));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(OpsError::Message(format!(
            "child wave '{child}' did not publish .wave-endpoint within 10s"
        )))
    })
}

async fn wake_child(repo: &Path, wave: &str) -> OpsResult<()> {
    let status = tokio::process::Command::new(resolve_lf_binary())
        .args([
            "chat",
            "--from",
            "project-promote",
            "--wave",
            wave,
            "Promotion complete. Run the first child-wave pass, report what you now own in this thread, then publish the same concise report to the parent with `lf chat --parent`.",
        ])
        .current_dir(repo)
        .status()
        .await
        .map_err(|err| OpsError::Message(format!("failed to wake promoted wave: {err}")))?;
    if !status.success() {
        return Err(OpsError::Message(format!(
            "promoted wave '{wave}' started, but its bootstrap message failed"
        )));
    }
    Ok(())
}

async fn link_parent(store: &Store, repo: &Path, parent: &str, child: &str) -> OpsResult<()> {
    let parent = store
        .get_wave_by_name(parent)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read parent wave: {err}")))?
        .ok_or_else(|| OpsError::Message(format!("parent wave '{parent}' is not registered")))?;
    let mut child_wave = match store
        .get_wave_by_name(child)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read child wave: {err}")))?
    {
        Some(wave) => wave,
        None => Wave::new(LfdId::new(), child.to_string(), repo.display().to_string()),
    };
    if child_wave
        .parent_wave_id()
        .is_some_and(|current| current != parent.id())
    {
        return Err(OpsError::Message(format!(
            "child wave '{child}' already belongs to another parent"
        )));
    }
    child_wave.parent_wave_id = Some(parent.id().clone());
    if store
        .get_wave(child_wave.id())
        .await
        .map_err(|err| OpsError::Message(format!("failed to check child wave: {err}")))?
        .is_some()
    {
        store
            .update_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to link child wave: {err}")))?;
    } else {
        store
            .create_wave(&child_wave)
            .await
            .map_err(|err| OpsError::Message(format!("failed to register child wave: {err}")))?;
    }
    Ok(())
}

/// Promotion grants residency, so it boots a listener — `lf serve`, never
/// `lf loop`. The child is spawned through tmux, which inherits the promoting
/// pass's environment (`WAVE_SERVER_ENDPOINT`, `RESIDENT_TOKEN`). Naming the
/// listener explicitly is what keeps that inheritance from deciding which half
/// of the wave the child becomes.
fn residency_argv(executable: &Path, wave: &str) -> Vec<String> {
    vec![
        executable.display().to_string(),
        "serve".to_string(),
        wave.to_string(),
    ]
}

async fn launch_residency(repo: &Path, wave: &str) -> OpsResult<()> {
    let argv = residency_argv(&resolve_lf_binary(), wave);
    spawn_detached_lf(&promotion_session_name(repo, wave), repo, &argv)
        .await
        .map_err(|err| OpsError::Message(format!("failed to start child wave residency: {err}")))
}

fn promotion_session_name(repo: &Path, wave: &str) -> String {
    let repo = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    format!("lf-{}-{}", tmux_session_slug(repo), tmux_session_slug(wave))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lf::{Cli, Commands};
    use crate::lfdb::{open_store, StorageConfig};
    use clap::Parser;

    /// Promotion grants residency: the spawned child must be the steerable
    /// half. `lf loop` would need a seed and would never publish an endpoint.
    #[test]
    fn promotion_spawns_a_listener_not_a_batch_loop() {
        let argv = residency_argv(Path::new("/opt/lf"), "release-stability");
        assert_eq!(argv, ["/opt/lf", "serve", "release-stability"]);

        let full = std::iter::once("lf".to_string()).chain(argv.into_iter().skip(1));
        assert!(
            matches!(
                Cli::try_parse_from(full).expect("promotion argv parses").command,
                Some(Commands::Serve { name, force: false }) if name == "release-stability"
            ),
            "what promotion spawns must parse as the serve entrypoint"
        );
    }

    #[tokio::test]
    async fn link_parent_registers_the_promoted_wave_as_a_child() {
        let tmp = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
            .await
            .unwrap();
        let parent = Wave::new(
            LfdId::new(),
            "platform".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&parent).await.unwrap();

        link_parent(&store, tmp.path(), "platform", "release-stability")
            .await
            .unwrap();

        let child = store
            .get_wave_by_name("release-stability")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(child.parent_wave_id(), Some(parent.id()));
        assert_eq!(child.repo(), tmp.path().display().to_string());
    }
}
