//! Inspect Wave memory size targets without restricting launch input.

pub mod preview;
pub use loopflow_execution::ops::human_session::{
    SessionResumeAction, SessionResumeExplanation, SessionResumeIntent,
};

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Serialize;

use loopflow_execution::prompt::context_budget::{
    measure_context, ContextBudgetReport, ContextBudgets,
};
use loopflow_execution::prompt::{gather_documents, GatherSpec};
use loopflow_execution::store::RecordingGap;

#[derive(Debug, Serialize)]
struct ContextReport {
    checkout: std::path::PathBuf,
    wave: Option<String>,
    task: Option<String>,
    context: ContextBudgetReport,
}

pub fn run(json: bool, wave: Option<&str>, task: Option<&str>) -> Result<()> {
    let mut repo =
        loopflow_execution::repo::require_repo_root(&std::env::current_dir()?, "lf context")?;
    let binding = loopflow_execution::runtime::block_on(async {
        let Some(store) = loopflow_execution::store::open_existing_store().await? else {
            anyhow::ensure!(task.is_none(), "Task usage unavailable: no local registry");
            return Ok::<_, anyhow::Error>(None);
        };
        let store = std::sync::Arc::new(store);
        if let Some(task) = task {
            Ok(Some(
                loopflow_execution::ops::resolve_work_binding(
                    &store,
                    &repo,
                    &format!("task:{task}"),
                )
                .await?,
            ))
        } else if wave.is_none() {
            Ok(loopflow_execution::ops::resolve_execution_binding(&store, &repo).await?)
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
        .or_else(loopflow_execution::work::wave::context::resolve_ambient_wave_name);
    let task = binding.as_ref().and_then(|binding| match &binding.work {
        loopflow_model::durable::WorkRef::Task(id) => Some(id.to_string()),
        _ => None,
    });
    let budgets = ContextBudgets::load(&repo, wave.as_deref())?;
    // Size targets concern authored memory, not a prospective launch.
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
) -> Result<loopflow_execution::ops::context::ContextExplanation> {
    explain_in(&std::env::current_dir()?, wave, task, session)
}

/// [`explain`] as if run in `cwd`.
pub fn explain_in(
    cwd: &std::path::Path,
    wave: Option<&str>,
    task: Option<&str>,
    session: Option<&str>,
) -> Result<loopflow_execution::ops::context::ContextExplanation> {
    anyhow::ensure!(
        !((wave.is_some() || task.is_some()) && session.is_some()),
        "select Work or a Session, not both"
    );
    Ok(loopflow_execution::runtime::block_on(async {
        use loopflow_execution::ops::context::ContextExplanation;
        match read_registry() {
            Ok(store) => {
                loopflow_execution::ops::context::explain_context(
                    &store,
                    cwd,
                    loopflow_execution::ops::WorkSelection { task, wave },
                    session,
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
    options: &loopflow_execution::ops::task::TaskProcessOptions,
    through: bool,
) -> Result<loopflow_execution::ops::task::TaskRunExplanation> {
    let cwd = std::env::current_dir()?;
    Ok(loopflow_execution::runtime::block_on(async {
        match read_registry() {
            Ok(store) => {
                loopflow_execution::ops::task::explain_task_run(
                    &store,
                    &cwd,
                    loopflow_execution::ops::WorkSelection {
                        task,
                        wave: options.wave.as_deref(),
                    },
                    options,
                    through,
                    crate::lf::navigation::command_merges,
                )
                .await
            }
            Err(error) => loopflow_execution::ops::task::TaskRunExplanation {
                resolution: loopflow_execution::ops::context::ContextExplanation::unavailable(
                    &error,
                ),
                action: None,
                impediments: Vec::new(),
                unavailable: vec![error.to_string()],
            },
        }
    }))
}

fn read_registry() -> Result<loopflow_execution::store::SharedStore> {
    loopflow_execution::store::read_existing_registry()
        .map_err(|error| anyhow!("Local registry unavailable: {error}"))?
        .map(std::sync::Arc::new)
        .ok_or_else(|| anyhow!("Local registry is absent"))
}

/// Explain resume without admitting a provider conversation or acquiring its client.
pub fn explain_session_resume(
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    fork: bool,
    prepare_only: bool,
) -> Result<SessionResumeExplanation> {
    let cwd = std::env::current_dir()?;
    Ok(loopflow_execution::runtime::block_on(async {
        match read_registry() {
            Ok(store) => {
                loopflow_execution::ops::human_session::explain_resume(
                    &store,
                    &cwd,
                    target,
                    fork,
                    prepare_only,
                )
                .await
            }
            Err(error) => SessionResumeExplanation::unavailable(error),
        }
    }))
}

/// Read planning intent without importing, saving, or synchronizing a record.
pub fn explain_task_planning(
    task: Option<&str>,
    wave: Option<&str>,
    request: &loopflow_execution::ops::task::TaskPlanningRequest<'_>,
) -> Result<loopflow_execution::ops::task::TaskPlanningExplanation> {
    let cwd = std::env::current_dir()?;
    Ok(loopflow_execution::runtime::block_on(async {
        match read_registry() {
            Ok(store) => {
                loopflow_execution::ops::task::explain_task_planning(
                    &store,
                    &cwd,
                    loopflow_execution::ops::WorkSelection { task, wave },
                    request,
                )
                .await
            }
            Err(error) => {
                loopflow_execution::ops::task::TaskPlanningExplanation::unavailable(error)
            }
        }
    }))
}

/// Inspect checkout preparation without acquiring a lease or fetching refs.
pub fn explain_task_checkout(
    task: Option<&str>,
    options: &loopflow_execution::ops::task::TaskProcessOptions,
) -> Result<loopflow_execution::ops::task::TaskCheckoutExplanation> {
    let cwd = std::env::current_dir()?;
    Ok(loopflow_execution::runtime::block_on(async {
        match read_registry() {
            Ok(store) => {
                loopflow_execution::ops::task::explain_task_checkout(
                    &store,
                    &cwd,
                    loopflow_execution::ops::WorkSelection {
                        task,
                        wave: options.wave.as_deref(),
                    },
                    options,
                )
                .await
            }
            Err(error) => {
                loopflow_execution::ops::task::TaskCheckoutExplanation::unavailable(error)
            }
        }
    }))
}

/// Provider callbacks use launch-owned authority rather than the current client.
/// A launch the Machine server or an owner holds reports each boundary to the
/// server, which records it. Every launch then reads its own news from the
/// store, in this process: the Session's unread steers and what changed on its
/// Tasks. The context block comes from checkout files.
/// `harness` names the provider whose hook this is; it reads the callback and
/// shapes the reply. A hook that names none sends Claude's shape.
pub fn emit_block(
    delivery: Option<&std::path::Path>,
    moment: loopflow_execution::context_block::ContextMoment,
    harness: Option<loopflow_harness::HarnessKind>,
) -> Result<()> {
    use crate::server::wire::BoundaryMoment;
    use loopflow_execution::context_block::{reported_thread_path, ContextMoment};
    use loopflow_execution::harness::HookCallback;
    let harness = loopflow_execution::harness::launcher(
        harness.unwrap_or(loopflow_harness::HarnessKind::Claude),
    );
    let raw = read_callback()?;
    let thread_file = delivery_path(delivery, &raw)
        .ok()
        .map(|path| reported_thread_path(&path));
    let callback = harness.hook_callback(raw, moment, thread_file.as_deref());
    let Some(HookCallback {
        moment,
        payload: input,
        prompt,
    }) = callback
    else {
        return Ok(());
    };
    let (path, delivery) = load_delivery(delivery, &input)?;
    // A launch whose provider names the conversation keeps it in the thread
    // file beside its delivery. A bare Codex launch creates the file to ask,
    // and the start callback fills it here; Antigravity's `hook_callback`
    // keeps it on the conversation the person is in. Writing it opens no store; a failed write
    // only leaves the conversation unresumable.
    if moment == ContextMoment::Start {
        let reported = reported_thread_path(&path);
        if let (Some(thread), true) = (input["session_id"].as_str(), reported.exists()) {
            if let Err(error) = std::fs::write(&reported, thread) {
                tracing::warn!(%error, "cannot keep the provider's thread for resume");
            }
        }
    }
    let boundary = match moment {
        ContextMoment::Start => Some(BoundaryMoment::Start),
        ContextMoment::Compact => Some(BoundaryMoment::Compact),
        ContextMoment::Tool => Some(BoundaryMoment::Tool),
        ContextMoment::Stop => Some(BoundaryMoment::Stop),
        ContextMoment::Steer => None,
    };
    let read_adapter = |name: &str| -> Result<Option<loopflow_execution::steer::SteerAdapter>> {
        Ok(std::env::var(name)
            .ok()
            .map(|value| serde_json::from_str(&value))
            .transpose()?)
    };
    // The capability of a launch whose boundaries the server records, or of
    // one that reports to no server.
    let held = read_adapter(loopflow_execution::steer::PROVIDER_CONTEXT_ENV)?;
    let adapter = match held.clone() {
        Some(held) => Some(held),
        None => read_adapter(loopflow_execution::steer::BARE_STEER_ENV)?,
    };
    // Mark the check before reading unread steers, so one saved during it is
    // newer and the next tool boundary checks again.
    let seen = std::env::var_os(loopflow_execution::steer::STEER_SEEN_ENV)
        .map(std::path::PathBuf::from)
        .filter(|_| {
            adapter.is_some() && matches!(moment, ContextMoment::Tool | ContextMoment::Steer)
        });
    if let Some(seen) = &seen {
        loopflow_storage::steer_markers::touch(seen)?;
    }
    // A conversation lf did not start (a Codex app-server thread) has no
    // provider context; only its turn end reports, to refresh planning.
    let answered = match boundary {
        Some(boundary) if held.is_some() || boundary == BoundaryMoment::Stop => {
            ask_server(&path, &delivery.home, boundary, held.as_ref(), &input)
        }
        _ => false,
    };
    // The start was also the prompt: the server marks the turn working.
    if prompt && held.is_some() {
        let mut opening = input.clone();
        opening["hook_event_name"] = "UserPromptSubmit".into();
        ask_server(
            &path,
            &delivery.home,
            BoundaryMoment::Tool,
            held.as_ref(),
            &opening,
        );
    }
    if moment == ContextMoment::Stop {
        return Ok(());
    }
    // An answering server may have followed a moved checkout.
    let delivery = match answered {
        true => loopflow_execution::context_block::ContextDelivery::load(&path)?,
        false => delivery,
    };
    let news = match (&adapter, input["session_id"].as_str()) {
        (Some(adapter), Some(thread)) => {
            read_news(&delivery, adapter, thread, &input, !answered)
        }
        _ => Ok(News::default()),
    }
    .unwrap_or_else(|error| {
        tracing::warn!(error = %format!("{error:#}"), "steers unread at this boundary; the next one asks again");
        News { again: true, ..News::default() }
    });
    // More may wait behind the steers just claimed, and a store that did not
    // answer delivered none: the next tool boundary asks again.
    if let Some(seen) = seen.filter(|_| news.again) {
        let _ = std::fs::remove_file(seen);
    }
    let prefix = news
        .steers
        .as_ref()
        .map_or(String::new(), |(_, steers)| steers.text.clone());
    let text = match moment {
        ContextMoment::Start | ContextMoment::Compact => {
            delivery.block_with_prefix(moment, &prefix)?.text
        }
        _ if prefix.is_empty() => return Ok(()),
        _ => prefix,
    };
    let event = input["hook_event_name"].as_str().unwrap_or("SessionStart");
    let output = harness.hook_reply(event, &text);
    write_output(format!("{output}\n").as_bytes())?;
    // Printed first: a hook that dies between the two leaves a claim that
    // lapses, and a steer that arrives twice beats one that never arrives.
    if let (Some(adapter), Some((store, steers))) = (&adapter, &news.steers) {
        loopflow_execution::steer::delivered(store, &adapter.session, steers);
    }
    Ok(())
}

/// What a boundary brings its conversation from the store.
#[derive(Default)]
struct News {
    steers: Option<(
        loopflow_storage::sqlite::SqliteStore,
        loopflow_execution::steer::ClaimedSteers,
    )>,
    /// Whether the next tool boundary should read again.
    again: bool,
}

/// Claim the Session's unread steers for the conversation `thread`, after
/// writing what changed on its Tasks as steers. `unrecorded`: no server
/// recorded this boundary, so a provider start selects its conversation here.
fn read_news(
    delivery: &loopflow_execution::context_block::ContextDelivery,
    adapter: &loopflow_execution::steer::SteerAdapter,
    thread: &str,
    input: &serde_json::Value,
    unrecorded: bool,
) -> Result<News> {
    use loopflow_execution::steer::NativeBoundary;
    let path = loopflow_execution::store::default_db_path();
    anyhow::ensure!(path.exists(), "Machine registry is absent");
    let store = loopflow_execution::store::open_sqlite(&path)?;
    let event = input["hook_event_name"].as_str().unwrap_or("SessionStart");
    let boundary = match event {
        "SessionStart" => NativeBoundary::Select {
            source: input["source"].as_str().unwrap_or("startup"),
        },
        // Fires for subagents too, so it only fills a missing selection.
        _ => NativeBoundary::Activate { source: event },
    };
    if unrecorded && matches!(boundary, NativeBoundary::Select { .. }) {
        store.with_provider_context(adapter, thread, boundary, || Ok(()))?;
    }
    let started = matches!(boundary, NativeBoundary::Select { source: "startup" });
    if let Err(error) = loopflow_execution::harness::task_updates::write(&store, &adapter.session)
        .and_then(|()| match started {
            true => store.baseline_task_updates(&adapter.session),
            false => Ok(()),
        })
    {
        tracing::warn!(session = adapter.session, %error, "Task updates unwritten at this boundary");
    }
    // A start the server recorded on a conversation it can wake leaves the
    // waiting steers to that wake, which runs them as a turn. A chat or a
    // worker reads them here; the next boundary claims what the wake could
    // not deliver.
    if !unrecorded && matches!(boundary, NativeBoundary::Select { .. }) {
        let wakes = store
            .idle_conversation_candidates(Some(&adapter.session))?
            .iter()
            .any(|idle| idle.conversation.adapter.agent_process_id == adapter.agent_process_id);
        if wakes {
            return Ok(News::default());
        }
    }
    let steers =
        loopflow_execution::steer::claim(&store, &delivery.repo, adapter, thread, boundary)?;
    Ok(News {
        again: steers.is_some(),
        steers: steers.map(|steers| (store, steers)),
    })
}

/// One `session.boundary` request; false when the server did not answer,
/// which is a recording gap and never a failed turn.
fn ask_server(
    delivery: &std::path::Path,
    home: &std::path::Path,
    moment: crate::server::wire::BoundaryMoment,
    adapter: Option<&loopflow_execution::steer::SteerAdapter>,
    input: &serde_json::Value,
) -> bool {
    use crate::server::wire::{BoundaryGap, Operation, ServerOperation, SessionBoundary};
    // Only a Session lf recorded has history to hold a gap; the server refuses
    // an adapter's callback that names no conversation.
    let mut gap = match (adapter, input["session_id"].as_str()) {
        (Some(_), Some(thread)) => recording_gap(home, thread),
        _ => RecordingGap::default(),
    };
    let operation = Operation::Server(ServerOperation::SessionBoundary(SessionBoundary {
        moment,
        delivery: delivery.to_owned(),
        adapter: adapter.cloned(),
        callback: input.clone(),
        gap: gap.span().map(|span| BoundaryGap {
            first: span.first_miss,
            last: span.last_miss,
        }),
        printed: Vec::new(),
        inbox: adapter
            .and_then(|_| claude_inbox())
            .map(loopflow_execution::steer::Inbox::Claude),
    }));
    let answered = crate::server::socket_path().and_then(|socket| {
        loopflow_execution::machine::client::hook_request(
            &socket,
            operation,
            loopflow_execution::machine::client::HOOK_DEADLINE,
        )
    });
    match answered {
        Ok(_) => {
            gap.recovered();
            true
        }
        Err(error) => {
            gap.note("session boundary", &format!("{error:#}"));
            false
        }
    }
}

/// The key the Claude conversation running this hook publishes for peers of
/// its cross-session inbox, `sessions/<pid>.<sha256 of socket>.key` in its
/// configuration directory, for the socket Claude names in its children's
/// environment.
fn claude_inbox() -> Option<std::path::PathBuf> {
    use sha2::{Digest, Sha256};
    let socket = std::path::PathBuf::from(std::env::var_os("CLAUDE_CODE_MESSAGING_SOCKET")?);
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".claude"))
        })?;
    let suffix = format!(
        ".{}.key",
        hex::encode(Sha256::digest(socket.as_os_str().as_encoded_bytes()))
    );
    std::fs::read_dir(config.join("sessions"))
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&suffix))
        })
}

/// Every hook is a fresh process, so the native conversation keeps its open gap
/// beside its delivery file.
fn recording_gap(home: &std::path::Path, thread: &str) -> RecordingGap {
    RecordingGap::marked(
        loopflow_execution::context_block::native_delivery_path(home, thread)
            .with_extension("store-gap"),
    )
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

/// The delivery file a callback names, and what it holds.
fn load_delivery(
    path: Option<&std::path::Path>,
    input: &serde_json::Value,
) -> Result<(
    std::path::PathBuf,
    loopflow_execution::context_block::ContextDelivery,
)> {
    let path = delivery_path(path, input)?;
    let delivery = loopflow_execution::context_block::ContextDelivery::load(&path)?;
    Ok((path, delivery))
}

/// The delivery file a callback names.
fn delivery_path(
    path: Option<&std::path::Path>,
    input: &serde_json::Value,
) -> Result<std::path::PathBuf> {
    // A Codex conversation's own binding wins over its app-server's launch
    // file, which a conversation started there later falls back to.
    path.map(std::path::Path::to_path_buf)
        .or_else(|| {
            input["session_id"]
                .as_str()
                .map(|session| {
                    loopflow_execution::context_block::native_delivery_path(
                        &loopflow_execution::store::lf_home_dir(),
                        session,
                    )
                })
                .filter(|path| path.exists())
        })
        .or_else(|| {
            std::env::var_os(loopflow_execution::context_block::CONTEXT_DELIVERY_ENV)
                .map(Into::into)
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Provider callback has no launch context or native session binding")
        })
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
    use super::{load_delivery, read_news};
    use loopflow_execution::context_block::{ContextDelivery, ContextMoment};

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
        let _lock = loopflow_execution::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_CONTEXT_DELIVERY", "LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let repos = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let mut contexts = Vec::new();
        for (index, repo) in repos.iter().enumerate() {
            let skill = repo.path().join("skill.md");
            std::fs::write(&skill, format!("SESSION_{index}_SKILL")).unwrap();
            let context = ContextDelivery {
                identity: None,
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
            let restored = load_delivery(None, &serde_json::json!({})).unwrap().1;
            assert_eq!(restored, contexts[index]);
            let compact = restored.block(ContextMoment::Compact).unwrap().text;
            assert!(compact.contains(&format!("SESSION_{index}_SKILL")));
            assert!(!compact.contains(&format!("SESSION_{}_SKILL", 1 - index)));
        }
        std::env::remove_var("LF_CONTEXT_DELIVERY");
        for index in [0, 1, 0] {
            let input = serde_json::json!({"session_id":format!("thread-{index}")});
            let restored = load_delivery(None, &input).unwrap().1;
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
            .unwrap()
            .1;
            assert_eq!(restored, contexts[index]);
        }
    }

    /// A reopened work session's start leaves its waiting steer to the wake,
    /// which runs it as a turn; a start no server recorded, a chat's, reads it.
    #[test]
    fn a_wakeable_start_leaves_waiting_steers_to_the_wake() {
        use loopflow_execution::process::attachment;
        let _lock = loopflow_execution::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let store = loopflow_execution::store::opening::open_ephemeral(
            &loopflow_execution::store::default_db_path(),
        )
        .unwrap();
        store.test_session(
            "session",
            &loopflow_execution::session_record::new_artifact_key(),
        );
        let process = store.test_lf_process();
        let claim =
            attachment::claim_session_attachment(&store, "session", None, &process).unwrap();
        store
            .record_agent_process_identity("session", &claim, std::process::id(), 1)
            .unwrap();
        let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
        store
            .with_provider_context(
                &adapter,
                "native",
                loopflow_execution::steer::NativeBoundary::Activate { source: "startup" },
                || Ok(()),
            )
            .unwrap();
        store
            .record_agent_inbox(
                &adapter,
                &loopflow_execution::steer::Inbox::Socket("/tmp/lf-wake-fixture/s".into()),
            )
            .unwrap();
        assert!(store.record_first_boundary(&adapter).unwrap());
        store
            .save_session_steer("session", "Fixture author", "WAITING")
            .unwrap();
        let delivery = ContextDelivery {
            identity: None,
            repository: None,
            repo: home.path().to_owned(),
            home: home.path().to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let start = serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"resume"});
        let recorded = read_news(&delivery, &adapter, "native", &start, false).unwrap();
        assert!(recorded.steers.is_none());
        let chat = read_news(&delivery, &adapter, "native", &start, true).unwrap();
        assert!(chat.steers.unwrap().1.text.contains("WAITING"));
    }
}
