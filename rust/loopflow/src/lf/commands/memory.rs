//! `lf memory` — read or curate a wave's MEMORY.md through its live server.
//!
//! The live server holds the pen: `update` (full replacement from stdin) POSTs
//! the compiled checkpoint and `add "fact"` publishes one replayable fact to
//! the stream. `show` (the bare default) reads through the server when one is
//! live and falls back to the origin file otherwise. `log` prints add-stream
//! facts oldest to newest, using the live replay buffer or the journal fold.
//! Targeting (`--wave`, `--parent`) matches `lf chat` ([`super::chat`]),
//! including the drop rule: a write with no wave context anywhere is a publish
//! to no subscriber — exit 0, one stderr note. Reads require a wave context.

use std::io::Read;

use anyhow::{anyhow, Result};

use crate::lf::commands::chat::{get_json, post_json, resolve_target, CliContext, ResolvedWave};
use crate::lf::{MemoryCommand, WaveTargetArgs};
use crate::wave::journal::{fold_thread, journal_path, read_events};
use crate::wave::memory::Memory;

pub fn run(cmd: Option<&MemoryCommand>, default_target: &WaveTargetArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, cmd, default_target).await
    })
}

pub(crate) async fn run_with_context(
    context: &CliContext,
    cmd: Option<&MemoryCommand>,
    default_target: &WaveTargetArgs,
) -> Result<()> {
    match cmd {
        None => show(context, default_target).await,
        Some(MemoryCommand::Show { target }) => show(context, target).await,
        Some(MemoryCommand::Log { target }) => log(context, target).await,
        Some(MemoryCommand::Update { summary, target }) => {
            // Resolve before touching stdin so a no-wave drop never blocks.
            let Some(resolved) = resolve(context, target).await? else {
                drop_note();
                return Ok(());
            };
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)?;
            let summary = write_memory(&resolved, "update", &content, summary.as_deref()).await?;
            println!("memory updated for wave '{}': {summary}", resolved.name);
            Ok(())
        }
        Some(MemoryCommand::Add { fact, target }) => {
            let Some(resolved) = resolve(context, target).await? else {
                drop_note();
                return Ok(());
            };
            let summary = write_memory(&resolved, "add", fact, None).await?;
            println!("memory fact added for wave '{}': {summary}", resolved.name);
            Ok(())
        }
    }
}

/// Publish-to-no-subscriber: writes outside any wave drop with exit 0.
fn drop_note() {
    eprintln!("no wave here; memory write dropped");
}

/// Memory is wave-level (MEMORY.md is wave identity; work lines have no
/// memory — their notes are files), so this resolves the FAMILY HEAD even
/// inside a work-line worktree and ignores the channel arm entirely.
async fn resolve(context: &CliContext, target: &WaveTargetArgs) -> Result<Option<ResolvedWave>> {
    resolve_target(
        target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
        context.env_channel.as_deref(),
    )
    .await
}

/// Reads are not publishes: no wave context is an error, not a drop.
async fn show(context: &CliContext, target: &WaveTargetArgs) -> Result<()> {
    let resolved = resolve(context, target).await?.ok_or_else(|| {
        anyhow!(
            "cannot resolve a target wave: no LFD_WAVE_ID in env and \
             not inside a wave worktree — pass --wave <name>"
        )
    })?;
    print!("{}", read_memory(&resolved).await?);
    Ok(())
}

/// Reads are not publishes: no wave context is an error, not a drop.
async fn log(context: &CliContext, target: &WaveTargetArgs) -> Result<()> {
    let resolved = resolve(context, target).await?.ok_or_else(|| {
        anyhow!(
            "cannot resolve a target wave: no LFD_WAVE_ID in env and \
             not inside a wave worktree — pass --wave <name>"
        )
    })?;
    for fact in read_memory_log(&resolved).await? {
        println!("{fact}");
    }
    Ok(())
}

/// The wave's MEMORY.md: through the live server when one answers, else a
/// direct read of the origin file (reads don't need the pen).
pub(crate) async fn read_memory(resolved: &ResolvedWave) -> Result<String> {
    if let Some(endpoint) = &resolved.endpoint {
        let body = get_json(endpoint, "/memory").await?;
        return Ok(body["content"].as_str().unwrap_or_default().to_string());
    }
    let root = resolved.repo_root.as_deref().ok_or_else(|| {
        anyhow!(
            "wave '{}' has no live server and no local wave directory to read",
            resolved.name
        )
    })?;
    Ok(Memory::for_wave(root, &resolved.name).read())
}

/// Facts added to the wave's memory stream, oldest to newest. Prefer the live
/// server's replay buffer; fall back to the journal fold when no server is
/// running.
pub(crate) async fn read_memory_log(resolved: &ResolvedWave) -> Result<Vec<String>> {
    if let Some(endpoint) = &resolved.endpoint {
        let body = get_json(endpoint, "/memory/log").await?;
        return Ok(body["facts"]
            .as_array()
            .map(|facts| {
                facts
                    .iter()
                    .filter_map(|fact| fact.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default());
    }
    let root = resolved.repo_root.as_deref().ok_or_else(|| {
        anyhow!(
            "wave '{}' has no live server and no local wave directory to read",
            resolved.name
        )
    })?;
    let events = read_events(&journal_path(root, &resolved.name));
    Ok(fold_thread(&events).memory_adds)
}

/// Write through the live server (the sole holder of MEMORY.md's pen).
/// Returns the summary the server journaled. No live server is an error —
/// there is deliberately no offline write path.
pub(crate) async fn write_memory(
    resolved: &ResolvedWave,
    op: &str,
    content: &str,
    summary: Option<&str>,
) -> Result<String> {
    let endpoint = resolved.require_endpoint()?;
    let body = post_json(
        &endpoint,
        "/memory",
        &serde_json::json!({ "op": op, "content": content, "summary": summary }),
    )
    .await?;
    Ok(body["summary"].as_str().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    use crate::wave::journal::{journal_path, EventKind, Journal};
    use crate::wave::runtime::WaveRuntime;
    use crate::wave::server;

    fn resolved(name: &str, endpoint: Option<String>, root: Option<&Path>) -> ResolvedWave {
        ResolvedWave {
            name: name.to_string(),
            endpoint,
            repo_root: root.map(Path::to_path_buf),
            own_name: None,
            channel: None,
        }
    }

    async fn boot_server(origin: &Path, wave: &str) -> (String, Arc<WaveRuntime>) {
        let runtime =
            WaveRuntime::open(wave.to_string(), origin.to_path_buf()).expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            server::ResidentDoor::new("test-token"),
            None,
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (addr.to_string(), runtime)
    }

    fn memory_events(origin: &Path, wave: &str) -> Vec<EventKind> {
        let (_, events) = Journal::open(&journal_path(origin, wave)).expect("journal");
        events.into_iter().map(|event| event.kind).collect()
    }

    /// `update` replaces the ORIGIN repo's file through the server and
    /// journals `MemoryUpdated`; `add` publishes a replayable fact without
    /// mutating the compiled file.
    #[tokio::test]
    async fn update_writes_the_origin_file_and_add_journals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path();
        let (addr, _runtime) = boot_server(origin, "ship").await;
        let target = resolved("ship", Some(addr), None);

        let summary = write_memory(&target, "update", "# Ship\n\nfold is truth\n", None)
            .await
            .expect("update");
        assert_eq!(summary, "# Ship", "summary defaults to the first line");
        assert_eq!(
            std::fs::read_to_string(origin.join("wave/ship/MEMORY.md")).expect("origin file"),
            "# Ship\n\nfold is truth\n",
            "the ORIGIN file is the one replaced"
        );

        let summary = write_memory(&target, "add", "workers report via lf chat", None)
            .await
            .expect("add");
        assert_eq!(summary, "workers report via lf chat");
        assert_eq!(
            std::fs::read_to_string(origin.join("wave/ship/MEMORY.md")).expect("origin file"),
            "# Ship\n\nfold is truth\n",
            "add publishes a stream fact without accreting raw bullets"
        );
        assert_eq!(
            memory_events(origin, "ship")
                .into_iter()
                .filter(|event| matches!(
                    event,
                    EventKind::MemoryUpdated { .. } | EventKind::MemoryAdded { .. }
                ))
                .collect::<Vec<_>>(),
            vec![
                EventKind::MemoryUpdated {
                    summary: "# Ship".to_string()
                },
                EventKind::MemoryAdded {
                    fact: "workers report via lf chat".to_string()
                },
            ]
        );
    }

    #[tokio::test]
    async fn log_reads_through_the_server_when_live() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path();
        let (addr, runtime) = boot_server(origin, "ship").await;
        runtime.append_memory("first").expect("append");
        runtime.append_memory("second").expect("append");

        let facts = read_memory_log(&resolved("ship", Some(addr), None))
            .await
            .expect("read log");
        assert_eq!(facts, vec!["first".to_string(), "second".to_string()]);
    }

    #[tokio::test]
    async fn log_reads_the_journal_without_a_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path();
        {
            let runtime =
                WaveRuntime::open("ship".to_string(), origin.to_path_buf()).expect("runtime");
            runtime.append_memory("offline first").expect("append");
            runtime.append_memory("offline second").expect("append");
            runtime
                .update_memory("# Ship\n\ncompiled\n", "compiled")
                .expect("update");
            runtime.append_memory("after update").expect("append");
        }

        let facts = read_memory_log(&resolved("ship", None, Some(origin)))
            .await
            .expect("read log");
        assert_eq!(facts, vec!["after update".to_string()]);
    }

    /// `show` works with no server at all: a direct read of the origin file.
    #[tokio::test]
    async fn show_reads_the_origin_file_without_a_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "offline read\n").unwrap();

        let content = read_memory(&resolved("ship", None, Some(tmp.path())))
            .await
            .expect("read");
        assert_eq!(content, "offline read\n");
    }

    /// `show` prefers the live server's view when one answers.
    #[tokio::test]
    async fn show_reads_through_the_server_when_live() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MEMORY.md"), "served content\n").unwrap();
        let (addr, _runtime) = boot_server(tmp.path(), "ship").await;

        let content = read_memory(&resolved("ship", Some(addr), None))
            .await
            .expect("read");
        assert_eq!(content, "served content\n");
    }

    /// No wave context at all: a write is a publish to no subscriber — it
    /// drops with exit 0 instead of erroring.
    #[tokio::test]
    async fn add_without_wave_context_drops_with_exit_zero() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let context = CliContext {
            store: None,
            repo: Some(tmp.path().to_path_buf()),
            env_wave_id: None,
            env_channel: None,
        };
        run_with_context(
            &context,
            Some(&MemoryCommand::Add {
                fact: "dropped fact".to_string(),
                target: WaveTargetArgs::default(),
            }),
            &WaveTargetArgs::default(),
        )
        .await
        .expect("dropped write exits 0");
    }

    /// `show` is a read, not a publish: no wave context stays a clear error.
    #[tokio::test]
    async fn show_without_wave_context_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let context = CliContext {
            store: None,
            repo: Some(tmp.path().to_path_buf()),
            env_wave_id: None,
            env_channel: None,
        };
        let err = run_with_context(&context, None, &WaveTargetArgs::default())
            .await
            .expect_err("read with no wave context");
        assert!(err.to_string().contains("--wave"), "{err}");
    }

    /// Writes have no offline path: no live server is an error.
    #[tokio::test]
    async fn update_without_a_server_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = write_memory(
            &resolved("ship", None, Some(tmp.path())),
            "update",
            "x",
            None,
        )
        .await
        .expect_err("no server");
        assert!(err.to_string().contains("no live server"), "{err}");
    }
}
