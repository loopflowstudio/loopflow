//! One Session, presented as one more view: every view of it types.
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use super::{
    conversation_view, fallback_text, focus_destination, open_desktop, route, workspace, OpenArgs,
    OpenHost, OpenReceipt, OpenRoute, OpenStatus, Opening, Resolved,
};
use crate::lf::commands::desktop::{self, DesktopOpeningStatus};
use loopflow_execution::machine::client::Connection;
use loopflow_execution::ops::human_session::{
    existing_connection, select_connection, SessionTarget,
};
use loopflow_model::id::LfSessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Here,
    Tab,
    Focus,
    Desktop,
}

fn mode(args: &OpenArgs, agent: bool) -> Result<Mode> {
    if args.focus {
        return Ok(Mode::Focus);
    }
    if args.selection.host == OpenHost::Desktop {
        anyhow::ensure!(
            !args.here && !args.tab,
            "Desktop navigation cannot use --here or --tab"
        );
        return Ok(Mode::Desktop);
    }
    if args.here {
        anyhow::ensure!(!agent, "An agent cannot attach inside its tool call. Run this command with --here from an interactive terminal.");
        return Ok(Mode::Here);
    }
    Ok(if args.tab || agent {
        Mode::Tab
    } else {
        Mode::Here
    })
}

pub(super) fn here_route() -> OpenRoute {
    OpenRoute {
        host: Some(OpenHost::Terminal),
        reason: "attach in the calling terminal".into(),
    }
}

pub(super) fn presentation(args: &OpenArgs) -> Result<OpenRoute> {
    Ok(
        match mode(args, loopflow_execution::journal::agent_caller().is_some())? {
            Mode::Here => here_route(),
            Mode::Focus => OpenRoute {
                host: None,
                reason: "focus the recorded live view without opening another".into(),
            },
            _ => route(args.selection.host),
        },
    )
}

pub(super) fn open(mut target: Resolved, args: &OpenArgs, focus_new: bool) -> OpenReceipt {
    let result = present(&mut target, args, focus_new);
    loopflow_execution::transport::end_open(&result);
    match result {
        Ok(receipt) => receipt,
        Err(error) => target.receipt(
            None,
            None,
            OpenStatus::Failed,
            Some(format!(
                "{error:#}; retry with {}",
                fallback_text(&target.fallback_argv())
            )),
        ),
    }
}

fn present(target: &mut Resolved, args: &OpenArgs, focus_new: bool) -> Result<OpenReceipt> {
    let mode = mode(args, loopflow_execution::journal::agent_caller().is_some())?;
    let host = if mode == Mode::Here {
        OpenHost::Terminal
    } else {
        route(args.selection.host)
            .host
            .expect("host selection always names a host")
    };
    anyhow::ensure!(
        args.selection.server.is_none() || host == OpenHost::Rex,
        "--server requires --tab --host rex"
    );
    if mode == Mode::Here
        && args.selection.host != OpenHost::Auto
        && args.selection.host != OpenHost::Terminal
    {
        anyhow::ensure!(
            route(OpenHost::Auto).host == Some(args.selection.host),
            "--here uses the current terminal; use --tab for another host"
        );
    }
    if let Some(reason) = &target.execution_unavailable {
        anyhow::bail!("{reason}");
    }
    let routed = target.execution_machine.is_some();
    if routed && mode == Mode::Here {
        let opening = Opening::lock(target)?;
        let (socket, session) = routed_terminal(target)?;
        drop(opening);
        let returns = loopflow_execution::transport::ReturnCommand::for_session(
            session.as_str(),
            fallback_text(&target.fallback_argv()),
        );
        loopflow_execution::transport::relay_socket(socket, Some(&returns))?;
        return Ok(target.receipt(Some(host), None, OpenStatus::Usable, None));
    }
    if routed && mode == Mode::Tab {
        refuse_tabless(target, mode, host)?;
        // The local tab resolves and locks its selection when --here runs.
        return open_tab(target, args, host, focus_new);
    }
    // Read-only focus must not prepare a first Session or recreate its checkout.
    resolve(target, false)?;
    let opening = Opening::lock(target)?;
    resolve(target, false)?;
    if let Some(view) = conversation_view(target)? {
        let destination = view.placement.as_deref().and_then(super::placement_host);
        let focus_existing = mode == Mode::Focus
            || (mode == Mode::Desktop
                && focus_new
                && view.attachment == DesktopOpeningStatus::Usable
                && destination.is_some_and(|(host, _)| host == OpenHost::Desktop));
        if focus_existing {
            let (host, destination) = destination.context(
                "Not open in a view lf can focus; lf open without --focus adds a view here",
            )?;
            focus_destination(host, destination).with_context(|| view.reason.clone())?;
            return Ok(target.receipt(
                Some(host),
                Some(destination.into()),
                OpenStatus::Focused,
                Some(view.reason),
            ));
        }
        // Desktop shows a conversation once, and an agent is already in its own;
        // anywhere else, opening adds a view or takes it over from a client lf
        // can't share.
        let own = loopflow_execution::journal::agent_caller()
            .is_some_and(|caller| target.conversation() == Some(caller.session_id.as_str()));
        if ((mode == Mode::Desktop && view.in_desktop) || own) && view.retains_attachment() {
            return Ok(target.receipt(
                destination.map(|(host, _)| host),
                view.placement.clone(),
                view.status(),
                Some(view.reason),
            ));
        }
    }
    anyhow::ensure!(mode != Mode::Focus, "Not open");
    refuse_tabless(target, mode, host)?;
    resolve(target, true)?;
    if mode == Mode::Desktop {
        return desktop_open(target, focus_new);
    }

    if mode == Mode::Here {
        drop(opening);
        let argv = target
            .resume_argv(&loopflow_execution::os_process::resolve_lf_binary()?.to_string_lossy());
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .env_remove("LF_AGENT_CALLER")
            .env_remove(loopflow_execution::config::AGENT_OVERRIDE_ENV)
            .current_dir(target.cwd());
        loopflow_execution::transport::configure_child(&mut command);
        let status = command.status()?;
        anyhow::ensure!(status.success(), "Conversation exited with {status}");
        return Ok(target.receipt(Some(host), None, OpenStatus::Usable, None));
    }
    open_tab(target, args, host, focus_new)
}

fn refuse_tabless(target: &Resolved, mode: Mode, host: OpenHost) -> Result<()> {
    if mode == Mode::Tab && matches!(host, OpenHost::Terminal | OpenHost::Desktop) {
        anyhow::bail!(
            "{} has no sibling-tab support. Open a terminal and run: {} --here",
            host.label(),
            fallback_text(&target.fallback_argv())
        );
    }
    Ok(())
}

fn open_tab(
    target: &Resolved,
    args: &OpenArgs,
    host: OpenHost,
    focus_new: bool,
) -> Result<OpenReceipt> {
    let lf = loopflow_execution::os_process::resolve_lf_binary()?;
    let attachment = Attachment::new()?;
    let argv = attachment.argv(target.resume_argv(&lf.to_string_lossy()));
    let mut receipt = if host == OpenHost::Tmux {
        let pane = super::tmux::open(target, focus_new, &argv)?;
        target.receipt(Some(host), Some(pane), OpenStatus::Opening, None)
    } else {
        workspace::conversation_tab(
            target,
            host,
            args.selection.server.as_deref(),
            argv,
            focus_new,
        )?
    };
    attachment.settle(&mut receipt);
    Ok(receipt)
}

/// Select the conversation on the owner. A missing running terminal, or an
/// `--agent` on another harness than the Session's, calls for a continuation;
/// a peer outage leaves the Session alone.
fn routed_terminal(target: &Resolved) -> Result<(UnixStream, LfSessionId)> {
    use loopflow_execution::engine::{Error, ErrorCode};
    use loopflow_execution::machine::{
        client,
        wire::{ServerOperation, SessionStarted},
    };

    let socket = loopflow_execution::machine::socket_path()?;
    let mut client = Connection::open_at(&socket)?;
    let task = target.task.as_ref().map(|task| task.id.as_str());
    let machine = target.url.machine.as_ref();
    let agent = loopflow_execution::config::agent_override();
    let (session, provider) = match task {
        Some(task) => routed_task_session(&mut client, target, task, agent.as_deref())?,
        None => (
            target
                .session
                .as_deref()
                .context("select an exact Session on this Machine")?
                .into(),
            None,
        ),
    };
    let continuing = |client: &mut Connection| -> Result<_> {
        let (rows, cols) = loopflow_execution::transport::size();
        let started: SessionStarted = serde_json::from_value(
            client
                .request(ServerOperation::SessionContinue {
                    session: session.clone(),
                    request: uuid::Uuid::new_v4().to_string(),
                    rows,
                    cols,
                    task: task.map(str::to_owned),
                    machine: machine.cloned(),
                    agent: agent.clone(),
                })?
                .result,
        )?;
        client.session_ticket(&session, Some(started.terminal_id), task, machine)
    };
    // A running terminal on the Session's own harness is attached as it is.
    let switching = agent.as_deref().is_some_and(|agent| {
        provider.as_deref() != Some(loopflow_execution::config::parse_agent(agent).0.as_str())
    });
    let issued = if switching {
        continuing(&mut client)?
    } else {
        match client.session_ticket(&session, None, task, machine) {
            Err(error)
                if error.downcast_ref::<Error>().is_some_and(|e| {
                    e.code == ErrorCode::NotFound
                        && e.data.as_ref().and_then(|data| data["session"].as_str())
                            == Some(session.as_str())
                }) =>
            {
                continuing(&mut client)?
            }
            answer => answer?,
        }
    };
    Ok((client::redeem_terminal(&socket, issued)?, session))
}

/// The Task's Session on its owner and, for its primary, the harness it last ran.
fn routed_task_session(
    client: &mut Connection,
    target: &Resolved,
    task: &str,
    agent: Option<&str>,
) -> Result<(LfSessionId, Option<String>)> {
    use crate::lf::commands::{read::Read, waves::DesktopTaskPane, write::Write};
    use loopflow_execution::ops::human_session::SessionRecord;
    let pane: DesktopTaskPane = serde_json::from_value(
        client
            .query(Read::TaskGet {
                directory: target.repository_path.clone(),
                issue: task.to_owned(),
            })?
            .result,
    )?;
    let row = pane.row.context("Task is unavailable on its Machine")?;
    if row.reference.workspace.is_none() {
        anyhow::ensure!(
            row.lifecycle != loopflow_execution::durable::TaskState::Unstarted,
            "Not started; run lf task run {}",
            row.task.identifier
        );
        anyhow::bail!(
            "The checkout for {} is unavailable on its Machine",
            row.task.identifier
        );
    }
    if let Some(exact) = &target.url.session {
        let id = loopflow_model::id::LfSessionId::from(exact.as_str());
        anyhow::ensure!(
            pane.status
                .overview
                .is_some_and(|overview| overview.sessions.iter().any(|s| s.id == id)),
            "Session does not belong to the selected Work. Nothing was opened."
        );
        Ok((id, None))
    } else {
        let record: SessionRecord = serde_json::from_value(
            client
                .request(Write::SessionResume {
                    directory: target.repository_path.clone(),
                    id: None,
                    task: Some(task.to_owned()),
                    wave: None,
                    agent: agent.map(str::to_owned),
                })?
                .result,
        )?;
        Ok((record.id, record.provider))
    }
}

fn desktop_open(target: &mut Resolved, focus: bool) -> Result<OpenReceipt> {
    target.url.session = target.session.clone();
    let url = target.url.to_string();
    let previous = desktop::conversation_opening(&url)?;
    open_desktop(&url, focus)?;
    let deadline = SystemTime::now() + Duration::from_secs(30);
    loop {
        if let Some(opening) = desktop::conversation_opening(&url)?.filter(|opening| {
            previous
                .as_ref()
                .is_none_or(|old| old.revision != opening.revision)
        }) {
            match opening.status {
                DesktopOpeningStatus::Usable => {
                    return Ok(target.receipt(
                        Some(OpenHost::Desktop),
                        None,
                        OpenStatus::Usable,
                        None,
                    ))
                }
                DesktopOpeningStatus::Failed => anyhow::bail!(
                    "{}",
                    opening
                        .reason
                        .as_deref()
                        .unwrap_or("Desktop could not attach the conversation")
                ),
                DesktopOpeningStatus::Opening => {}
            }
        }
        anyhow::ensure!(
            SystemTime::now() < deadline,
            "Desktop attachment was not confirmed; see the destination and retry"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Use the connection owner's resolver for every scope. Resolution never launches
/// a provider. An exact Session can be closed; its continuation keeps its history.
pub(super) fn resolve(target: &mut Resolved, prepare: bool) -> Result<()> {
    if target.execution_machine.is_some() || target.primary_machine.is_some() {
        // The Session resolver on the execution Machine owns its identity.
        return Ok(());
    }
    let existing = loopflow_execution::store::read_existing_registry()?;
    if !prepare && existing.is_none() {
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = std::sync::Arc::new(
            loopflow_execution::store::open_store(
                &loopflow_execution::store::storage_config_from_env()?,
            )
            .await?,
        );
        let selector = match (&target.url.session, &target.task, &target.wave) {
            (Some(id), _, _) => SessionTarget::Session(id),
            (_, Some(task), _) => SessionTarget::Task(task.id.as_str()),
            (_, _, Some(wave)) => SessionTarget::Wave(wave),
            _ => SessionTarget::Repository,
        };
        let id = if prepare {
            select_connection(&store, &target.repository_path, selector)
                .await?
                .id
        } else {
            // No conversation yet is the ordinary absent-focus result.
            match existing_connection(&store, &target.repository_path, selector).await? {
                (_, Some(id)) => id,
                (_, None) => return Ok(()),
            }
        };
        let session = store.session(&id).await?.context("Session not found")?;
        if prepare && session.provider.is_none() {
            loopflow_execution::config::load_wave_config(
                Some(&session.cwd),
                target.wave.as_deref(),
            )?
            .unwrap_or_default()
            .agent()?;
        }
        target.session = Some(id);
        target.session_cwd = Some(session.cwd);
        Ok(())
    })
}

/// A launch-scoped receipt, removed after its caller settles. The transport
/// writes "attached" once the native conversation has started, or the error
/// that stopped it; a failed child also leaves that error on screen.
#[derive(Debug)]
pub(super) struct Attachment {
    directory: tempfile::TempDir,
}

impl Attachment {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            directory: tempfile::tempdir()?,
        })
    }
    fn path(&self) -> PathBuf {
        self.directory.path().join("receipt")
    }
    pub(super) fn argv(&self, command: Vec<String>) -> Vec<String> {
        let mut argv = vec![
            "/bin/sh".into(), "-c".into(),
            r#"receipt=$1
shift
"$@"
result=$?
failed=false
if [ -d "${receipt%/*}" ] && [ ! -f "$receipt" ]; then
    printf 'Conversation exited before attachment (status %s)\n' "$result" > "$receipt.tmp"
    mv "$receipt.tmp" "$receipt"
    failed=true
fi
if [ "$failed" = true ] || [ "$result" -ne 0 ]; then
    printf '\nConversation ended with an error. Correct the error above and retry lf open. Press Enter to close.\n'
    read -r answer
fi
exit "$result""#.into(),
            "lf-open".into(), self.path().to_string_lossy().into_owned(),
            "env".into(), "-u".into(), "LF_AGENT_CALLER".into(),
            "-u".into(), loopflow_execution::config::AGENT_OVERRIDE_ENV.into(),
        ];
        if loopflow_execution::store::custom_home_selected() {
            argv.push(format!(
                "LF_HOME={}",
                loopflow_execution::store::lf_home_dir().display()
            ));
        }
        argv.push(format!(
            "{}={}",
            loopflow_execution::transport::OPEN_RECEIPT_ENV,
            self.path().display()
        ));
        argv.extend(command);
        argv
    }
    pub(super) fn settle(&self, receipt: &mut OpenReceipt) {
        match self.wait(Duration::from_secs(30)) {
            Ok(()) => receipt.status = OpenStatus::Usable,
            Err(error) => {
                let reason = format!("{error:#}");
                tracing::warn!(
                    url = receipt.url,
                    destination = receipt.destination.as_deref().unwrap_or_default(),
                    error = %reason,
                    "lf open: the conversation did not attach"
                );
                receipt.status = OpenStatus::Failed;
                receipt.reason = Some(format!(
                    "{}; retry the same command",
                    reason.trim_end().trim_end_matches('.')
                ));
            }
        }
    }
    fn wait(&self, timeout: Duration) -> Result<()> {
        let deadline = SystemTime::now() + timeout;
        loop {
            match std::fs::read_to_string(self.path()) {
                Ok(text) if text == "attached" => return Ok(()),
                Ok(text) => anyhow::bail!("{text}"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            anyhow::ensure!(
                SystemTime::now() < deadline,
                "Conversation attachment was not confirmed within {}s",
                timeout.as_secs()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mode, Attachment, Mode};
    use crate::lf::{Cli, Commands};
    use clap::Parser;
    use std::process::Command;
    use std::time::Duration;

    fn selected(words: &[&str]) -> super::OpenArgs {
        let cli =
            Cli::try_parse_from(["lf", "open"].into_iter().chain(words.iter().copied())).unwrap();
        let Some(Commands::Open(args)) = cli.command else {
            panic!("open command")
        };
        args
    }

    #[test]
    fn direct_and_agent_defaults_do_not_depend_on_installed_hosts() {
        let args = selected(&[]);
        assert_eq!(mode(&args, false).unwrap(), Mode::Here);
        assert_eq!(mode(&args, true).unwrap(), Mode::Tab);
        assert_eq!(mode(&selected(&["--tab"]), false).unwrap(), Mode::Tab);
        assert_eq!(mode(&selected(&["--focus"]), true).unwrap(), Mode::Focus);
        assert_eq!(
            mode(&selected(&["--host", "desktop"]), false).unwrap(),
            Mode::Desktop
        );
        assert!(mode(&selected(&["--here"]), true)
            .unwrap_err()
            .to_string()
            .contains("interactive terminal"));
        assert!(mode(&selected(&["--host", "desktop", "--tab"]), false).is_err());
    }

    #[test]
    fn acknowledged_terminal_exit_is_not_reported_as_failure() {
        use std::io::Write;
        use std::process::Stdio;
        let attachment = Attachment::new().unwrap();
        let argv = attachment.argv(vec![
            "/bin/sh".into(),
            "-c".into(),
            r#"printf attached > "$LF_OPEN_RECEIPT"; read -r answer"#.into(),
        ]);
        let mut terminal = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        attachment.wait(Duration::from_secs(5)).unwrap();
        drop(attachment);
        terminal
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"done\n")
            .unwrap();
        let output = terminal.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }

    #[test]
    fn conversation_keeps_the_hosts_path_not_the_callers() {
        let attachment = Attachment::new().unwrap();
        let argv = attachment.argv(vec!["lf".into(), "session".into(), "resume".into()]);
        assert!(!argv.iter().any(|arg| arg.starts_with("PATH=")), "{argv:?}");
    }

    #[test]
    fn a_failed_opening_reports_the_conversations_own_reason() {
        let attachment = Attachment::new().unwrap();
        let argv = attachment.argv(vec![
            "/bin/sh".into(),
            "-c".into(),
            r#"printf "'claude' is not installed." > "$LF_OPEN_RECEIPT"; exit 1"#.into(),
        ]);
        let output = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let mut receipt = super::OpenReceipt {
            label: "LOO-1".into(),
            url: String::new(),
            host: None,
            destination: None,
            container: None,
            status: super::OpenStatus::Usable,
            reason: None,
            fallback_argv: Vec::new(),
        };
        attachment.settle(&mut receipt);
        assert_eq!(receipt.status, super::OpenStatus::Failed);
        assert_eq!(
            receipt.reason.as_deref(),
            Some("'claude' is not installed; retry the same command")
        );
    }

    #[test]
    fn attachment_requires_the_handshake_and_keeps_exit_failure() {
        let attachment = Attachment::new().unwrap();
        assert!(attachment
            .wait(Duration::ZERO)
            .unwrap_err()
            .to_string()
            .contains("not confirmed"));
        let argv = attachment.argv(vec!["/bin/sh".into(), "-c".into(), "exit 7".into()]);
        let output = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stdout).contains("Correct the error above"));
        assert!(attachment
            .wait(Duration::ZERO)
            .unwrap_err()
            .to_string()
            .contains("status 7"));
        std::fs::write(attachment.path(), "attached").unwrap();
        attachment.wait(Duration::ZERO).unwrap();
    }
}
