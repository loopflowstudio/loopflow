//! Inspect memory and scratch size targets without restricting launch input.

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

use crate::config::load_config;
use crate::prompt::context_budget::{measure_context, ContextBudgetReport, ContextBudgets};
use crate::prompt::{gather_documents, GatherSpec};

#[derive(Debug, Serialize)]
struct ContextReport {
    checkout: std::path::PathBuf,
    wave: Option<String>,
    task: Option<String>,
    context: ContextBudgetReport,
}

pub fn run(json: bool, wave: Option<&str>, task: Option<&str>) -> Result<()> {
    let mut repo = crate::repo::require_repo_root(&std::env::current_dir()?, "lf context")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let binding = runtime.block_on(async {
        let Some(store) = crate::store::open_existing_store().await else {
            anyhow::ensure!(task.is_none(), "Task usage unavailable: no local registry");
            return Ok::<_, anyhow::Error>(None);
        };
        let store = std::sync::Arc::new(store);
        if let Some(task) = task {
            Ok(Some(
                crate::ops::resolve_work_binding(&store, &repo, &format!("task:{task}")).await?,
            ))
        } else if wave.is_none() {
            Ok(crate::ops::resolve_execution_binding(&store, &repo).await?)
        } else {
            Ok(None)
        }
    })?;
    if let Some(binding) = &binding {
        repo = binding.cwd.clone();
    }
    let wave = wave
        .map(str::to_owned)
        .or_else(|| binding.as_ref().map(|binding| binding.wave_name.clone()))
        .or_else(crate::work::wave::context::resolve_ambient_wave_name);
    let task = binding.as_ref().and_then(|binding| match &binding.work {
        crate::durable::WorkRef::Task(id) => Some(id.to_string()),
        _ => None,
    });
    let config = load_config(Some(&repo))?.unwrap_or_default();
    let budgets = ContextBudgets::resolve(&config, &repo, wave.as_deref())?;
    // Size targets concern authored memory and scratch, not a prospective launch.
    let docs = gather_documents(&GatherSpec {
        repo_root: repo.clone(),
        wave: wave.clone(),
        ..Default::default()
    })?;
    let report = ContextReport {
        checkout: repo,
        wave,
        task,
        context: measure_context(&docs, budgets),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Checkout: {}\nWave: {}\nTask: {}\n{}",
            report.checkout.display(),
            report.wave.as_deref().unwrap_or("none"),
            report.task.as_deref().unwrap_or("none"),
            report.context.render()
        );
    }
    Ok(())
}

/// Provider callbacks use launch-owned authority rather than the current client.
pub fn emit_block(
    delivery: &std::path::Path,
    moment: crate::context_block::ContextMoment,
) -> Result<()> {
    use crate::context_block::{ContextDelivery, ContextMoment};
    let delivery: ContextDelivery = serde_json::from_slice(&std::fs::read(delivery)?)?;
    let mut input = String::new();
    std::io::stdin()
        .take(1_048_577)
        .read_to_string(&mut input)?;
    anyhow::ensure!(
        input.len() <= 1_048_576,
        "Context callback exceeds input limit"
    );
    let input: serde_json::Value = if input.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&input)?
    };
    if input.get("agent_id").is_some_and(|id| !id.is_null()) {
        return Ok(());
    }
    let event = input["hook_event_name"].as_str().unwrap_or("SessionStart");
    let write = || -> crate::store::StoreResult<()> {
        let text = if matches!(moment, ContextMoment::Tool) {
            String::new()
        } else {
            delivery
                .block(moment)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?
                .text
        };
        let output = serde_json::json!({"hookSpecificOutput": {
            "hookEventName": event, "additionalContext": text,
        }});
        write_output(format!("{output}\n").as_bytes())
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
    };
    let adapter = std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)
        .ok()
        .map(|value| serde_json::from_str::<crate::steer::SteerAdapter>(&value))
        .transpose()?;
    if let Some(adapter) = &adapter {
        let thread = input["session_id"].as_str().ok_or_else(|| {
            anyhow::anyhow!("Provider context callback has no native conversation")
        })?;
        let store = crate::store::sqlite::SqliteStore::new(&delivery.home.join("loopflow.db"))?;
        store.with_provider_context(
            adapter,
            thread,
            event == "SessionStart" || event == "OpenCodeMessagesTransform",
            |_| write(),
        )?;
    } else {
        write()?;
    }
    Ok(())
}

/// A stalled provider cannot retain the takeover fence through an unread pipe.
fn write_output(mut bytes: &[u8]) -> std::io::Result<()> {
    let stdout = std::io::stdout().lock();
    let fd = stdout.as_raw_fd();
    // SAFETY: stdout owns this live descriptor for the whole call.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: preserve all original flags; only enable nonblocking writes.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !bytes.is_empty() {
            // SAFETY: bytes is readable for its full length and stdout keeps fd live.
            let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            let written = if written < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(written as usize)
            };
            match written {
                Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(written) => bytes = &bytes[written..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Provider context output timed out; emission is uncertain",
                        ));
                    }
                    let mut poll = libc::pollfd {
                        fd,
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    // SAFETY: one initialized pollfd; the timeout is at most two seconds.
                    if unsafe { libc::poll(&mut poll, 1, remaining.as_millis() as i32) } < 0 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() != std::io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    })();
    // SAFETY: stdout remains locked and the original descriptor is still live.
    let restored = unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
    if restored < 0 {
        return Err(std::io::Error::last_os_error());
    }
    result
}
