//! Inspect memory and scratch size targets without restricting launch input.

pub mod preview;
pub use crate::ops::human_session::{
    OpenMode, SessionConnectAction, SessionConnectExplanation, SessionConnectIntent,
};

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::config::load_config;
use crate::prompt::context_budget::{measure_context, ContextBudgetReport, ContextBudgets};
use crate::prompt::{gather_documents, GatherSpec};
use crate::steer::NativeBoundary;

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
        .or_else(|| {
            binding
                .as_ref()
                .and_then(|binding| binding.wave_name.clone())
        })
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

/// Explain stored identity without creating a repository plan or preparing Work.
pub fn explain(
    wave: Option<&str>,
    task: Option<&str>,
    session: Option<&str>,
    process: Option<&str>,
) -> Result<crate::ops::context::ContextExplanation> {
    anyhow::ensure!(
        (wave.is_some() || task.is_some()) as usize
            + session.is_some() as usize
            + process.is_some() as usize
            <= 1,
        "select Work, a Session, or a Process, not several independent targets"
    );
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        use crate::ops::context::ContextExplanation;
        match read_registry() {
            Ok(store) => {
                crate::ops::context::explain_context(
                    &store,
                    &cwd,
                    crate::ops::WorkSelection { task, wave },
                    session,
                    process,
                )
                .await
            }
            Err(error) => ContextExplanation::unavailable(error),
        }
    }))
}

/// Read invocation effects and missing evidence using the same read-only registry.
pub fn explain_task_run(
    task: Option<&str>,
    options: &crate::ops::task::TaskProcessOptions,
    through: bool,
) -> Result<crate::ops::task::TaskRunExplanation> {
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        match read_registry() {
            Ok(store) => {
                crate::ops::task::explain_task_run(
                    &store,
                    &cwd,
                    crate::ops::WorkSelection {
                        task,
                        wave: options.wave.as_deref(),
                    },
                    options,
                    through,
                )
                .await
            }
            Err(error) => crate::ops::task::TaskRunExplanation {
                resolution: crate::ops::context::ContextExplanation::unavailable(&error),
                action: None,
                impediments: Vec::new(),
                unavailable: vec![error.to_string()],
            },
        }
    }))
}

fn read_registry() -> Result<crate::store::SharedStore> {
    crate::store::read_existing_registry()
        .map_err(|error| anyhow!("Local registry unavailable: {error}"))?
        .map(std::sync::Arc::new)
        .ok_or_else(|| anyhow!("Local registry is absent"))
}

/// Explain connection without admitting a provider conversation or acquiring its client.
pub fn explain_session_connect(
    target: crate::ops::human_session::SessionTarget<'_>,
    takeover: bool,
    try_open: bool,
    prepare_only: bool,
    fresh: bool,
) -> Result<SessionConnectExplanation> {
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        match read_registry() {
            Ok(store) => {
                crate::ops::human_session::explain_connect(
                    &store,
                    &cwd,
                    target,
                    OpenMode::from_flags(takeover, try_open),
                    prepare_only,
                    fresh,
                )
                .await
            }
            Err(error) => SessionConnectExplanation::unavailable(error),
        }
    }))
}

/// Read planning intent without importing, saving, or synchronizing a record.
pub fn explain_task_planning(
    task: Option<&str>,
    wave: Option<&str>,
    request: &crate::ops::task::TaskPlanningRequest<'_>,
) -> Result<crate::ops::task::TaskPlanningExplanation> {
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        match read_registry() {
            Ok(store) => {
                crate::ops::task::explain_task_planning(
                    &store,
                    &cwd,
                    crate::ops::WorkSelection { task, wave },
                    request,
                )
                .await
            }
            Err(error) => crate::ops::task::TaskPlanningExplanation::unavailable(error),
        }
    }))
}

/// Inspect checkout preparation without acquiring a lease or fetching refs.
pub fn explain_task_checkout(
    task: Option<&str>,
    options: &crate::ops::task::TaskProcessOptions,
) -> Result<crate::ops::task::TaskCheckoutExplanation> {
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        match read_registry() {
            Ok(store) => {
                crate::ops::task::explain_task_checkout(
                    &store,
                    &cwd,
                    crate::ops::WorkSelection {
                        task,
                        wave: options.wave.as_deref(),
                    },
                    options,
                )
                .await
            }
            Err(error) => crate::ops::task::TaskCheckoutExplanation::unavailable(error),
        }
    }))
}

/// Provider callbacks use launch-owned authority rather than the current client.
pub fn emit_block(
    delivery: Option<&std::path::Path>,
    moment: crate::context_block::ContextMoment,
) -> Result<()> {
    let input = read_callback()?;
    if input.get("agent_id").is_some_and(|id| !id.is_null()) {
        return Ok(());
    }
    let delivery = load_delivery(delivery, &input)?;
    if moment == crate::context_block::ContextMoment::Stop {
        return acquire_planning(&delivery);
    }
    emit_context(delivery, input, moment)
}

fn read_callback() -> Result<serde_json::Value> {
    let mut input = String::new();
    std::io::stdin()
        .take(1_048_577)
        .read_to_string(&mut input)?;
    anyhow::ensure!(
        input.len() <= 1_048_576,
        "Context callback exceeds input limit"
    );
    Ok(if input.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&input)?
    })
}

fn load_delivery(
    path: Option<&std::path::Path>,
    input: &serde_json::Value,
) -> Result<crate::context_block::ContextDelivery> {
    let path = path
        .map(std::path::Path::to_path_buf)
        .or_else(|| std::env::var_os(crate::context_block::CONTEXT_DELIVERY_ENV).map(Into::into))
        .or_else(|| {
            input["session_id"].as_str().map(|session| {
                crate::context_block::codex_delivery_path(&crate::store::lf_home_dir(), session)
            })
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Provider callback has no launch context or native session binding")
        })?;
    let bytes = std::fs::read(&path).map_err(|error| {
        anyhow!("Read provider context delivery {}: {error}; reconnect the Loopflow Session to restore its binding", path.display())
    })?;
    let mut delivery: crate::context_block::ContextDelivery = serde_json::from_slice(&bytes)
        .map_err(|error| {
            anyhow!(
                "Decode provider context delivery {}: {error}",
                path.display()
            )
        })?;
    delivery.relocate()?;
    Ok(delivery)
}

fn emit_context(
    delivery: crate::context_block::ContextDelivery,
    input: serde_json::Value,
    moment: crate::context_block::ContextMoment,
) -> Result<()> {
    let event = input["hook_event_name"].as_str().unwrap_or("SessionStart");
    let render = |prefix: &str| -> crate::store::StoreResult<String> {
        delivery
            .block_with_prefix(moment, prefix)
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
    };
    let write = |text: &str| -> crate::store::StoreResult<()> {
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
    // Mark the check before reading pending steers, so one saved during it is
    // newer and the next tool boundary checks again.
    let seen = std::env::var_os(crate::steer::STEER_SEEN_ENV)
        .map(std::path::PathBuf::from)
        .filter(|_| adapter.is_some() && moment == crate::context_block::ContextMoment::Tool);
    if let Some(seen) = &seen {
        crate::steer::touch(seen)?;
    }
    let more = std::cell::Cell::new(false);
    let result = deliver_steers(
        &delivery,
        &input,
        event,
        adapter.as_ref(),
        &render,
        &write,
        &more,
    );
    // Steers left beyond this batch, or a failed delivery, need the next boundary.
    if let Some(seen) = seen.filter(|_| result.is_err() || more.get()) {
        let _ = std::fs::remove_file(seen);
    }
    result
}

fn deliver_steers(
    delivery: &crate::context_block::ContextDelivery,
    input: &serde_json::Value,
    event: &str,
    adapter: Option<&crate::steer::SteerAdapter>,
    render: &dyn Fn(&str) -> crate::store::StoreResult<String>,
    write: &dyn Fn(&str) -> crate::store::StoreResult<()>,
    more: &std::cell::Cell<bool>,
) -> Result<()> {
    if let Some(adapter) = adapter {
        let thread = input["session_id"].as_str().ok_or_else(|| {
            anyhow::anyhow!("Provider context callback has no native conversation")
        })?;
        let store = crate::store::sqlite::SqliteStore::new(&delivery.home.join("loopflow.db"))?;
        store.recover_prepared_steers(&adapter.session)?;
        let boundary = match event {
            "SessionStart" => NativeBoundary::Select {
                source: input["source"].as_str().unwrap_or("startup"),
            },
            "OpenCodeMessagesTransform" | "PiContext" => NativeBoundary::Activate { source: event },
            _ => NativeBoundary::Turn,
        };
        store.with_provider_context(adapter, thread, boundary, || {
            let invocation = input["tool_use_id"]
                .as_str()
                .or_else(|| input["adapter_invocation_id"].as_str())
                .or_else(|| input["turn_id"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let boundary = format!(
                "{event}:{}:{invocation}",
                input["turn_id"].as_str().unwrap_or("")
            );
            let emitted = store.emit_session_steers(
                adapter,
                thread,
                &boundary,
                |batch, steers, remaining| {
                    more.set(remaining > 0);
                    let (prefix, excerpts) =
                        render_steers(delivery, &adapter.session, batch, steers, remaining)
                            .map_err(|error| {
                                crate::store::StoreError::InvalidData(error.to_string())
                            })?;
                    Ok(crate::steer::SteerContext {
                        text: render(&prefix)?,
                        excerpts,
                    })
                },
                write,
            )?;
            if !emitted {
                write(&render("")?)?;
            }
            Ok(())
        })?;
    } else {
        write(&render("")?)?;
    }
    Ok(())
}

/// Native turn hooks consume the same team freshness as CLI and watcher reads.
fn acquire_planning(delivery: &crate::context_block::ContextDelivery) -> Result<()> {
    let database = delivery.home.join("loopflow.db");
    if !database.exists() {
        return Ok(());
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store =
                crate::store::open_store(&crate::store::StorageConfig::sqlite(database)).await?;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                crate::ops::linear_observe::sync_repository(
                    &store,
                    &delivery.repo.to_string_lossy(),
                ),
            )
            .await;
            Ok(())
        })
}

fn excerpt(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn render_steers(
    delivery: &crate::context_block::ContextDelivery,
    session: &str,
    batch: &str,
    steers: &[crate::steer::Steer],
    remaining: usize,
) -> Result<(String, Vec<crate::id::SteerId>)> {
    let mut excerpts = Vec::new();
    let mut text = format!("<lf:steers batch=\"{batch}\">\n");
    for steer in steers {
        let path = crate::prompt::write_prompt_log(
            &delivery.repo,
            &serde_json::to_string_pretty(steer)?,
            "steer",
            None,
        )?;
        let path = path.strip_prefix(&delivery.repo).unwrap_or(&path);
        let mut item = format!(
            "Steer {}. Source {} revision {}; author {}. Complete source: {}\n",
            steer.id,
            serde_json::to_string(excerpt(&steer.source, 96))?,
            serde_json::to_string(excerpt(&steer.revision, 64))?,
            serde_json::to_string(excerpt(&steer.author.to_string(), 96))?,
            serde_json::to_string(path)?
        );
        let mut body = excerpt(&steer.text, 400);
        while !body.is_empty()
            && item.len() + crate::prompt::render_reference(body).len() + 32 > 1024
        {
            body = excerpt(body, body.len() - 1);
        }
        item.push_str(&crate::prompt::render_reference(body));
        if body.len() != steer.text.len() {
            excerpts.push(steer.id.clone());
            item.push_str(" [excerpt; full source above]");
        }
        item.push('\n');
        anyhow::ensure!(
            item.len() <= 1024,
            "Steer attribution and source pointer exceed the item budget"
        );
        text.push_str(&item);
    }
    text.push_str(&format!(
        "{remaining} pending. Inspect: lf session steers {session} --json\n</lf:steers>\n"
    ));
    Ok((text, excerpts))
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

#[cfg(test)]
mod tests {
    use super::load_delivery;
    use crate::context_block::{ContextDelivery, ContextMoment};

    #[test]
    fn missing_and_invalid_delivery_name_the_source_without_printing_contents() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("delivery.json");
        let error = load_delivery(Some(&path), &serde_json::json!({}))
            .unwrap_err()
            .to_string();
        assert!(error.contains(&path.display().to_string()));
        assert!(error.contains("Read provider context delivery"));
        assert!(error.contains("reconnect"));
        std::fs::write(&path, "PRIVATE_MALFORMED_CONTENT").unwrap();
        let error = load_delivery(Some(&path), &serde_json::json!({}))
            .unwrap_err()
            .to_string();
        assert!(error.contains(&path.display().to_string()));
        assert!(error.contains("Decode provider context delivery"));
        assert!(!error.contains("PRIVATE_MALFORMED_CONTENT"));
    }

    #[test]
    fn callbacks_select_exact_session_after_other_tasks_and_resume() {
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_CONTEXT_DELIVERY", "LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let repos = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let mut contexts = Vec::new();
        for (index, repo) in repos.iter().enumerate() {
            let skill = repo.path().join("skill.md");
            std::fs::write(&skill, format!("SESSION_{index}_SKILL")).unwrap();
            let context = ContextDelivery {
                repository: None,
                repo: repo.path().to_owned(),
                home: home.path().to_owned(),
                wave: None,
                skill_file: Some(skill),
                submitted_docs: None,
                references: Vec::new(),
            };
            context
                .bind_codex_session(&format!("thread-{index}"))
                .unwrap();
            contexts.push(context);
        }
        // Native launches retain independent files even when another Task has
        // launched since. Resume restores that Session's environment from capture.
        let paths: Vec<_> = contexts
            .iter()
            .map(|context| {
                let mut command = std::process::Command::new("provider");
                context.configure_hooks(&mut command).unwrap();
                command
                    .get_envs()
                    .find(|(key, _)| *key == "LF_CONTEXT_DELIVERY")
                    .unwrap()
                    .1
                    .unwrap()
                    .to_owned()
            })
            .collect();
        for index in [0, 1, 0] {
            std::env::set_var("LF_CONTEXT_DELIVERY", &paths[index]);
            let restored = load_delivery(None, &serde_json::json!({})).unwrap();
            assert_eq!(restored, contexts[index]);
            let compact = restored.block(ContextMoment::Compact).unwrap().text;
            assert!(compact.contains(&format!("SESSION_{index}_SKILL")));
            assert!(!compact.contains(&format!("SESSION_{}_SKILL", 1 - index)));
        }
        std::env::remove_var("LF_CONTEXT_DELIVERY");
        for index in [0, 1, 0] {
            let input = serde_json::json!({"session_id":format!("thread-{index}")});
            let restored = load_delivery(None, &input).unwrap();
            assert_eq!(restored, contexts[index]);
            let compact = restored.block(ContextMoment::Compact).unwrap().text;
            assert!(compact.contains(&format!("SESSION_{index}_SKILL")));
            assert!(!compact.contains(&format!("SESSION_{}_SKILL", 1 - index)));
        }
        assert!(load_delivery(None, &serde_json::json!({"session_id":"unbound"})).is_err());
        assert!(load_delivery(None, &serde_json::json!({})).is_err());
        // Resume refreshes only the chosen native conversation's binding.
        contexts[0].wave = Some("resumed".into());
        contexts[0].bind_codex_session("thread-0").unwrap();
        for index in [0, 1] {
            let restored = load_delivery(
                None,
                &serde_json::json!({"session_id":format!("thread-{index}")}),
            )
            .unwrap();
            assert_eq!(restored, contexts[index]);
        }
    }
}
