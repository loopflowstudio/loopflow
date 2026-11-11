//! One Session, presented without replacing another viewer's input.
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use super::{
    conversation_view, fallback_text, focus_destination, open_desktop, route, workspace, OpenArgs,
    OpenHost, OpenReceipt, OpenRoute, OpenStatus, Opening, Resolved,
};
use crate::lf::commands::desktop::{self, DesktopOpeningStatus};
use crate::lf::commands::write::SessionOpenIntent;
use crate::ops::human_session::{select_connection, SessionTarget};

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
        match mode(args, crate::journal::agent_caller().is_some())? {
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
    match present(&mut target, args, focus_new) {
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
    let mode = mode(args, crate::journal::agent_caller().is_some())?;
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
    anyhow::ensure!(
        !target.url.diff || mode == Mode::Desktop,
        "--diff requires --host desktop; use lf diff for terminal changes"
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
    // Read-only focus must not prepare a first Session or recreate its checkout.
    resolve(target, false)?;
    let opening = Opening::lock(target)?;
    resolve(target, false)?;
    if let Some(view) = conversation_view(target)? {
        let destination = view.placement.as_deref().and_then(super::placement_host);
        // Changes needs its own opening even when the conversation is ready.
        let companion = mode == Mode::Desktop
            && target.url.diff
            && view.attachment == DesktopOpeningStatus::Usable;
        let focus_existing = mode == Mode::Focus
            || (mode == Mode::Desktop
                && !companion
                && focus_new
                && view.attachment == DesktopOpeningStatus::Usable
                && destination.is_some_and(|(host, _)| host == OpenHost::Desktop));
        if focus_existing {
            let (host, destination) = destination
                .with_context(|| format!("Not open in a focusable view; {}", view.reason))?;
            focus_destination(host, destination).with_context(|| view.reason.clone())?;
            return Ok(target.receipt(
                Some(host),
                Some(destination.into()),
                OpenStatus::Focused,
                Some(view.reason),
            ));
        }
        if view.retains_attachment() && !companion {
            return Ok(target.receipt(
                destination.map(|(host, _)| host),
                view.placement.clone(),
                view.status(),
                Some(view.reason),
            ));
        }
    }
    anyhow::ensure!(mode != Mode::Focus, "Not open");
    if mode == Mode::Tab && matches!(host, OpenHost::Terminal | OpenHost::Desktop) {
        anyhow::bail!(
            "{} has no sibling-tab support. Open a terminal and run: {} --here",
            host.label(),
            fallback_text(&target.fallback_argv())
        );
    }
    resolve(target, true)?;
    if mode == Mode::Desktop {
        return desktop_open(target, focus_new);
    }

    let lf = crate::os_process::resolve_lf_binary()?
        .to_string_lossy()
        .into_owned();
    if mode == Mode::Here {
        drop(opening);
        let argv = target.resume_argv(&lf);
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .env_remove("LF_AGENT_CALLER")
            .env_remove(crate::config::AGENT_OVERRIDE_ENV)
            .current_dir(target.cwd());
        crate::transport::configure_child(&mut command);
        let status = command.status()?;
        anyhow::ensure!(status.success(), "Conversation exited with {status}");
        return Ok(target.receipt(Some(host), None, OpenStatus::Usable, None));
    }
    let attachment = Attachment::new()?;
    let argv = attachment.argv(target.resume_argv(&lf));
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
                DesktopOpeningStatus::Elsewhere => {
                    return Ok(target.receipt(
                        Some(OpenHost::Desktop),
                        None,
                        OpenStatus::Elsewhere,
                        opening.reason,
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
    let existing = crate::store::read_existing_registry()?;
    if !prepare && existing.is_none() {
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = std::sync::Arc::new(
            crate::store::open_store(&crate::store::storage_config_from_env()?).await?,
        );
        let selector = match (&target.url.session, &target.task, &target.wave) {
            (Some(id), _, _) => SessionTarget::Session(id),
            (_, Some(task), _) => SessionTarget::Task(task.id.as_str()),
            (_, _, Some(wave)) => SessionTarget::Wave(wave),
            _ => SessionTarget::Repository,
        };
        let selected = select_connection(
            &store,
            &target.repository_path,
            selector,
            if prepare {
                SessionOpenIntent::Start
            } else {
                SessionOpenIntent::Reveal
            },
        )
        .await;
        // No primary is the ordinary absent-focus result, not an admission.
        let selected = match selected {
            Ok(selected) => selected,
            Err(error)
                if !prepare
                    && error.to_string() == "No existing conversation; start one explicitly" =>
            {
                return Ok(())
            }
            Err(error) => return Err(error),
        };
        let session = store
            .session(&selected.id)
            .await?
            .context("Session not found")?;
        target.agent = match &session.provider {
            Some(provider) => Some(
                session
                    .model
                    .as_ref()
                    .map_or_else(|| provider.clone(), |model| format!("{provider}:{model}")),
            ),
            None => crate::config::agent_override(),
        };
        if prepare && session.provider.is_none() {
            crate::config::load_wave_config(Some(&session.cwd), target.wave.as_deref())?
                .unwrap_or_default()
                .agent()?;
        }
        target.session = Some(selected.id);
        target.session_cwd = Some(session.cwd);
        Ok(())
    })
}

/// A launch-scoped acknowledgement, removed after its caller settles. The
/// transport writes it only after the native conversation is ready for input.
/// A failed child leaves its error visible and reports failure through this file.
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
            "-u".into(), crate::config::AGENT_OVERRIDE_ENV.into(),
        ];
        if crate::store::custom_home_selected() {
            argv.push(format!("LF_HOME={}", crate::store::lf_home_dir().display()));
        }
        argv.push(format!(
            "{}={}",
            crate::transport::OPEN_RECEIPT_ENV,
            self.path().display()
        ));
        argv.extend(command);
        argv
    }
    pub(super) fn settle(&self, receipt: &mut OpenReceipt) {
        match self.wait(Duration::from_secs(30)) {
            Ok(()) => receipt.status = OpenStatus::Usable,
            Err(error) => {
                receipt.status = OpenStatus::Failed;
                receipt.reason = Some(format!(
                    "{error:#}; see the destination for diagnostics and retry the same command"
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
