//! Inspect memory and scratch size targets without restricting launch input.

pub mod preview;
pub use crate::ops::human_session::{
    SessionResumeAction, SessionResumeExplanation, SessionResumeIntent,
};

use std::io::Read;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::prompt::context_budget::{measure_context, ContextBudgetReport, ContextBudgets};
use crate::prompt::{gather_documents, GatherSpec};
use crate::store::RecordingGap;

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
        let Some(store) = crate::store::open_existing_store().await? else {
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
    let budgets = ContextBudgets::load(&repo, wave.as_deref())?;
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
    explain_in(&std::env::current_dir()?, wave, task, session, process)
}

/// [`explain`] as if run in `cwd`.
pub fn explain_in(
    cwd: &std::path::Path,
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
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        use crate::ops::context::ContextExplanation;
        match read_registry() {
            Ok(store) => {
                crate::ops::context::explain_context(
                    &store,
                    cwd,
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

/// Explain resume without admitting a provider conversation or acquiring its client.
pub fn explain_session_resume(
    target: crate::ops::human_session::SessionTarget<'_>,
    fork: bool,
    prepare_only: bool,
    start_only: bool,
) -> Result<SessionResumeExplanation> {
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        match read_registry() {
            Ok(store) => {
                crate::ops::human_session::explain_resume(
                    &store,
                    &cwd,
                    target,
                    fork,
                    prepare_only,
                    start_only,
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
/// A turn or tool boundary opens no store: the Machine server records it and
/// answers steers, and the context block comes from checkout files.
pub fn emit_block(
    delivery: Option<&std::path::Path>,
    moment: crate::context_block::ContextMoment,
) -> Result<()> {
    use crate::context_block::ContextMoment;
    use crate::server::wire::BoundaryMoment;
    let input = read_callback()?;
    if input.get("agent_id").is_some_and(|id| !id.is_null()) {
        return Ok(());
    }
    let (path, delivery) = load_delivery(delivery, &input)?;
    // Hooks installed before Task updates rode the boundary answer still
    // name this moment in live conversations; it has nothing left to do.
    // Deleted with `ContextMoment::TaskUpdates` (LOO-838).
    if moment == ContextMoment::TaskUpdates {
        return Ok(());
    }
    let boundary = match moment {
        ContextMoment::Start => BoundaryMoment::Start,
        ContextMoment::Compact => BoundaryMoment::Compact,
        ContextMoment::Tool => BoundaryMoment::Tool,
        ContextMoment::Stop => BoundaryMoment::Stop,
        other => anyhow::bail!("{other:?} is not a turn or tool boundary"),
    };
    let adapter = std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)
        .ok()
        .map(|value| serde_json::from_str::<crate::steer::SteerAdapter>(&value))
        .transpose()?;
    // Mark the check before the server reads pending steers, so one saved
    // during it is newer and the next tool boundary checks again.
    let seen = std::env::var_os(crate::steer::STEER_SEEN_ENV)
        .map(std::path::PathBuf::from)
        .filter(|_| adapter.is_some() && moment == ContextMoment::Tool);
    if let Some(seen) = &seen {
        crate::steer::touch(seen)?;
    }
    // A conversation lf did not start (a Codex app-server thread) has no
    // provider context; only its turn end asks, to refresh planning.
    let printed = match &adapter {
        Some(_) => PrintedBatches::claim(&path),
        None => PrintedBatches::default(),
    };
    let answered = if adapter.is_some() || boundary == BoundaryMoment::Stop {
        ask_server(
            &path,
            &delivery.home,
            boundary,
            adapter.as_ref(),
            &input,
            &printed.batches,
        )
    } else {
        None
    };
    printed.settle(answered.is_some());
    // Steers just delivered may have more behind them, an earlier batch may
    // still be waiting out its hook, and an unanswered boundary delivered
    // none: the next tool boundary asks again.
    let ask_again = answered
        .as_ref()
        .is_none_or(|answer| answer.steers.is_some() || answer.waiting);
    if let Some(seen) = seen.filter(|_| ask_again) {
        let _ = std::fs::remove_file(seen);
    }
    if moment == ContextMoment::Stop {
        return Ok(());
    }
    // An answering server may have followed a moved checkout.
    let (delivery, steers, update) = match answered {
        Some(answer) => (
            crate::context_block::ContextDelivery::load(&path)?,
            answer.steers,
            answer.update,
        ),
        None => (delivery, None, None),
    };
    let mut prefix = steers
        .as_ref()
        .map_or(String::new(), |steers| steers.text.clone());
    if let Some(update) = update {
        prefix.push_str(&update);
        prefix.push('\n');
    }
    let text = if moment == ContextMoment::Tool {
        prefix
    } else {
        delivery.block_with_prefix(moment, &prefix)?.text
    };
    let event = input["hook_event_name"].as_str().unwrap_or("SessionStart");
    let output = serde_json::json!({"hookSpecificOutput": {
        "hookEventName": event, "additionalContext": text,
    }});
    write_output(format!("{output}\n").as_bytes())?;
    // Printed first: a hook that dies between the two has its batch answered
    // again, and a steer that arrives twice beats one that never arrives.
    if let Some(steers) = steers {
        PrintedBatches::add(&path, &steers.batch);
    }
    Ok(())
}

/// Steer batches this conversation's hooks printed and no request has told
/// the server about yet. Every hook is a fresh process, so they are lines in
/// a file beside the delivery file.
#[derive(Debug, Default)]
struct PrintedBatches {
    batches: Vec<crate::id::SteerBatchId>,
    /// The file's lines moved aside for this request, so a hook printing in
    /// parallel starts a new file and neither loses the other's batch.
    claimed: Option<(std::path::PathBuf, std::path::PathBuf)>,
}

impl PrintedBatches {
    fn file(delivery: &std::path::Path) -> std::path::PathBuf {
        delivery.with_extension("printed")
    }

    fn claim(delivery: &std::path::Path) -> Self {
        let file = Self::file(delivery);
        let aside = file.with_extension(format!("printed.{}", std::process::id()));
        if std::fs::rename(&file, &aside).is_err() {
            return Self::default();
        }
        let batches = std::fs::read_to_string(&aside)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| crate::id::SteerBatchId::parse(line).ok())
            .collect();
        Self {
            batches,
            claimed: Some((file, aside)),
        }
    }

    /// The server heard these, or it did not and the next request repeats them.
    fn settle(self, answered: bool) {
        let Some((file, aside)) = self.claimed else {
            return;
        };
        if !answered {
            for batch in &self.batches {
                Self::append(&file, batch);
            }
        }
        let _ = std::fs::remove_file(aside);
    }

    fn add(delivery: &std::path::Path, batch: &crate::id::SteerBatchId) {
        Self::append(&Self::file(delivery), batch);
    }

    fn append(file: &std::path::Path, batch: &crate::id::SteerBatchId) {
        use std::io::Write;
        let wrote = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .and_then(|mut file| writeln!(file, "{batch}"));
        if let Err(error) = wrote {
            tracing::warn!(batch = %batch, error = %error, "printed steers not noted; they will arrive again");
        }
    }
}

/// One `session.boundary` request, or `None` when the server did not answer,
/// which is a recording gap and never a failed turn.
fn ask_server(
    delivery: &std::path::Path,
    home: &std::path::Path,
    moment: crate::server::wire::BoundaryMoment,
    adapter: Option<&crate::steer::SteerAdapter>,
    input: &serde_json::Value,
    printed: &[crate::id::SteerBatchId],
) -> Option<crate::server::wire::SessionBoundaryAnswer> {
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
        printed: printed.to_vec(),
        inbox: adapter.and_then(|_| claude_inbox()),
    }));
    let answered = crate::server::socket_path()
        .and_then(|socket| {
            crate::server::client::hook_request(
                &socket,
                operation,
                crate::server::boundary::HOOK_DEADLINE,
            )
        })
        .and_then(|response| Ok(serde_json::from_value(response.result)?));
    match answered {
        Ok(answer) => {
            gap.recovered();
            Some(answer)
        }
        Err(error) => {
            gap.note("session boundary", &format!("{error:#}"));
            None
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
        crate::context_block::native_delivery_path(home, thread).with_extension("store-gap"),
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
) -> Result<(std::path::PathBuf, crate::context_block::ContextDelivery)> {
    // A Codex conversation's own binding wins over its app-server's launch
    // file, which a conversation started there later falls back to.
    let path = path
        .map(std::path::Path::to_path_buf)
        .or_else(|| {
            input["session_id"]
                .as_str()
                .map(|session| {
                    crate::context_block::native_delivery_path(
                        &crate::store::lf_home_dir(),
                        session,
                    )
                })
                .filter(|path| path.exists())
        })
        .or_else(|| std::env::var_os(crate::context_block::CONTEXT_DELIVERY_ENV).map(Into::into))
        .ok_or_else(|| {
            anyhow::anyhow!("Provider callback has no launch context or native session binding")
        })?;
    let delivery = crate::context_block::ContextDelivery::load(&path)?;
    Ok((path, delivery))
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
    use super::{load_delivery, PrintedBatches};
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
    fn printed_batches_wait_for_a_request_the_server_answers() {
        let home = tempfile::tempdir().unwrap();
        let delivery = home.path().join("delivery.json");
        let (one, two) = (
            crate::id::SteerBatchId::new(),
            crate::id::SteerBatchId::new(),
        );
        PrintedBatches::add(&delivery, &one);
        // The request went unanswered; a hook beside it printed meanwhile.
        let unanswered = PrintedBatches::claim(&delivery);
        assert_eq!(unanswered.batches, std::slice::from_ref(&one));
        PrintedBatches::add(&delivery, &two);
        unanswered.settle(false);
        let mut answered = PrintedBatches::claim(&delivery);
        answered.batches.sort_by_key(|batch| batch.to_string());
        let mut both = [one, two];
        both.sort_by_key(|batch| batch.to_string());
        assert_eq!(answered.batches, both);
        answered.settle(true);
        assert!(PrintedBatches::claim(&delivery).batches.is_empty());
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    }

    #[test]
    fn callbacks_select_exact_session_after_other_tasks_and_resume() {
        let _lock = crate::journal::test_env_lock();
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
}
