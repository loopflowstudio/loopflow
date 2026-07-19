//! `lf memory show` — read a Wave's origin `MEMORY.md`.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::lf::commands::chat::{resolve_target, CliContext, ResolvedWave};
use crate::lf::{MemoryCommand, WaveTargetArgs};

pub fn run(cmd: &MemoryCommand) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, cmd).await
    })
}

pub(crate) async fn run_with_context(context: &CliContext, cmd: &MemoryCommand) -> Result<()> {
    match cmd {
        MemoryCommand::Show { target } => show(context, target).await,
    }
}

async fn show(context: &CliContext, target: &WaveTargetArgs) -> Result<()> {
    let resolved = resolve_target(
        target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
    )
    .await?
    .ok_or_else(|| {
        anyhow!(
            "cannot resolve a target wave: no LF_WAVE_ID in env and \
             not inside a wave worktree — pass --wave <name>"
        )
    })?;
    print!("{}", read_memory(&resolved)?);
    Ok(())
}

fn memory_path(resolved: &ResolvedWave) -> Result<PathBuf> {
    let root = resolved.repo_root.as_deref().ok_or_else(|| {
        anyhow!(
            "wave '{}' has no local wave directory to read",
            resolved.name
        )
    })?;
    Ok(root.join("wave").join(&resolved.name).join("MEMORY.md"))
}

/// Read the origin file even when a stale or unreachable endpoint is present.
pub(crate) fn read_memory(resolved: &ResolvedWave) -> Result<String> {
    let path = memory_path(resolved)?;
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn resolved(name: &str, endpoint: Option<String>, root: Option<&Path>) -> ResolvedWave {
        ResolvedWave {
            name: name.to_string(),
            endpoint,
            repo_root: root.map(Path::to_path_buf),
        }
    }

    #[test]
    fn show_reads_the_origin_file_without_a_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("wave directory");
        std::fs::write(dir.join("MEMORY.md"), "offline read\n").expect("memory file");

        let content = read_memory(&resolved("ship", None, Some(tmp.path()))).expect("read");
        assert_eq!(content, "offline read\n");
    }

    #[test]
    fn show_ignores_the_server_endpoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("wave directory");
        std::fs::write(dir.join("MEMORY.md"), "file is truth\n").expect("memory file");

        let content = read_memory(&resolved(
            "ship",
            Some("http://127.0.0.1:1".to_string()),
            Some(tmp.path()),
        ))
        .expect("read");
        assert_eq!(content, "file is truth\n");
    }

    #[test]
    fn missing_memory_file_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let content = read_memory(&resolved("ship", None, Some(tmp.path()))).expect("read");
        assert_eq!(content, "");
    }
}
