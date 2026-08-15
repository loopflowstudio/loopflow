use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use crate::durable::{
    AgentInvocationId, Ask, AskId, AskResult, AskTarget, AuthenticatedRequest, ControlCtx,
    InvocationSurface,
};
use crate::engine::wave_home::HomeRoute;
use crate::lf::{AskArgs, AskCommand};
use crate::store::{open_store, storage_config_from_env, Store};
use anyhow::{anyhow, bail, Context};

const WAIT_INTERVAL: Duration = Duration::from_millis(250);
const TARGET_WAKE_INTERVAL: Duration = Duration::from_secs(5);

pub fn run(args: &AskArgs) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(args))
}

async fn run_async(args: &AskArgs) -> anyhow::Result<()> {
    if args.command.is_some() && (args.user || args.noblock || args.json) {
        bail!("place Ask command flags after the subcommand (for example, `lf ask list --user --json`)");
    }
    let store = open_shared_store().await?;
    match &args.command {
        None => create_and_maybe_wait(&store, args).await,
        Some(AskCommand::Wait { ask_id, json }) => {
            wait_command(&store, ask_id.as_ref(), *json).await
        }
        Some(AskCommand::List {
            user,
            outgoing,
            json,
        }) => list_command(&store, *user, *outgoing, *json).await,
        Some(AskCommand::Open {
            ask_id,
            prepare,
            json,
        }) => open_command(&store, ask_id, *prepare, *json).await,
        Some(AskCommand::Presented {
            ask_id,
            invocation_id,
            json,
        }) => presented_command(&store, ask_id, invocation_id, *json).await,
        Some(AskCommand::Resolve {
            ask_id,
            summary,
            json,
        }) => {
            let summary = required_text(summary, "resolution summary")?;
            settle_command(&store, ask_id, AskResult::Resolved { summary }, *json).await
        }
        Some(AskCommand::Decline {
            ask_id,
            reason,
            json,
        }) => {
            let reason = optional_text(reason, "Ask declined");
            settle_command(&store, ask_id, AskResult::Declined { reason }, *json).await
        }
        Some(AskCommand::Release {
            ask_id,
            reason,
            json,
        }) => {
            let reason = optional_text(reason, "Ask session closed");
            release_command(&store, ask_id, Some(&reason), *json).await
        }
        Some(AskCommand::Escalate { ask_id, json, .. }) => {
            escalate_command(&store, ask_id, *json).await
        }
        Some(AskCommand::Cancel {
            ask_id,
            reason,
            json,
        }) => {
            let reason = optional_text(reason, "Ask cancelled");
            cancel_command(&store, ask_id, &reason, *json).await
        }
        Some(AskCommand::Serve { ask_id, headless }) => {
            let invocation_id = ambient_invocation_id()?;
            crate::ops::ask::serve(store, ask_id.clone(), invocation_id, *headless).await
        }
    }
}

async fn create_and_maybe_wait(store: &Arc<Store>, args: &AskArgs) -> anyhow::Result<()> {
    if args.request.is_empty() {
        bail!("usage: lf ask [--user] [--noblock] REQUEST");
    }
    let prompt = args.request.join(" ").trim().to_string();
    if prompt.is_empty() {
        bail!("Ask request cannot be empty");
    }
    let lease = crate::ops::required_run_lease(store)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let invocation_id = ambient_invocation_id()?;
    let ask =
        crate::ops::ask::request_intervention(store, &lease, &invocation_id, &prompt, args.user)
            .await?;
    publish_comments(store);
    if args.noblock {
        if args.json {
            println!("{}", serde_json::to_string_pretty(&ask)?);
        } else {
            println!("{}", ask.id);
        }
        return Ok(());
    }
    print_wait_selection(&ask, args.json);
    wait_for_terminal(store, &lease, ask, args.json).await
}

async fn wait_command(
    store: &Arc<Store>,
    ask_id: Option<&AskId>,
    json: bool,
) -> anyhow::Result<()> {
    let lease = crate::ops::required_run_lease(store)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let invocation_id = if ask_id.is_none() {
        ambient_invocation_id_if_present()?
    } else {
        None
    };
    let ask = ask_for_wait(store, &lease, invocation_id.as_ref(), ask_id).await?;
    print_wait_selection(&ask, json);
    crate::ops::ask::wake(store, &ask.target).await;
    wait_for_terminal(store, &lease, ask, json).await
}

async fn wait_for_terminal(
    store: &Arc<Store>,
    lease: &crate::durable::RunLease,
    mut ask: Ask,
    json: bool,
) -> anyhow::Result<()> {
    let mut next_wake = tokio::time::Instant::now() + TARGET_WAKE_INTERVAL;
    loop {
        if ask.state.is_terminal() {
            return print_terminal_ask(&ask, json);
        }
        tokio::time::sleep(WAIT_INTERVAL).await;
        ask = ask_for_wait(store, lease, None, Some(&ask.id)).await?;
        if tokio::time::Instant::now() >= next_wake {
            crate::ops::ask::wake(store, &ask.target).await;
            next_wake = tokio::time::Instant::now() + TARGET_WAKE_INTERVAL;
        }
    }
}

async fn ask_for_wait(
    store: &Arc<Store>,
    lease: &crate::durable::RunLease,
    invocation_id: Option<&AgentInvocationId>,
    ask_id: Option<&AskId>,
) -> anyhow::Result<Ask> {
    let asks = store.asks_for_work_epoch(lease).await?;
    match ask_id {
        Some(ask_id) => asks
            .into_iter()
            .find(|ask| &ask.id == ask_id)
            .ok_or_else(|| anyhow!("Ask {ask_id} does not belong to this Work Epoch")),
        None => select_default_wait(asks, &lease.run_id, invocation_id)
            .ok_or_else(|| anyhow!("this Work Epoch has no unresolved outgoing Ask")),
    }
}

fn select_default_wait(
    asks: Vec<Ask>,
    run_id: &crate::durable::RunId,
    invocation_id: Option<&AgentInvocationId>,
) -> Option<Ask> {
    let unresolved = |ask: &&Ask| !ask.state.is_terminal();
    if let Some(invocation_id) = invocation_id {
        if let Some(ask) = asks
            .iter()
            .filter(unresolved)
            .find(|ask| ask.origin.invocation_id.as_ref() == Some(invocation_id))
        {
            return Some(ask.clone());
        }
    }
    asks.iter()
        .filter(unresolved)
        .find(|ask| &ask.origin.run_id == run_id)
        .or_else(|| asks.iter().find(unresolved))
        .cloned()
}

async fn list_command(
    store: &Arc<Store>,
    user: bool,
    outgoing: bool,
    json: bool,
) -> anyhow::Result<()> {
    if outgoing {
        let lease = crate::ops::required_run_lease(store)
            .await
            .map_err(|error| anyhow!(error.to_string()))?;
        let asks = store
            .asks_for_work_epoch(&lease)
            .await?
            .into_iter()
            .filter(|ask| !ask.state.is_terminal())
            .collect::<Vec<_>>();
        if json {
            println!("{}", serde_json::to_string_pretty(&asks)?);
        } else if asks.is_empty() {
            println!("No unresolved outgoing Asks.");
        } else {
            for ask in asks {
                println!(
                    "{}  {:<8} to={:<32} from={}:{} run={}  {}",
                    ask.id,
                    ask.state.as_str(),
                    ask.target,
                    ask.origin.work.kind(),
                    ask.origin.work.id(),
                    ask.origin.run_id,
                    ask.request,
                );
            }
        }
        return Ok(());
    }
    let attention = if user {
        let request = AuthenticatedRequest::cli();
        crate::ops::ask::pending_attention(store, &ControlCtx::User(&request), &AskTarget::User)
            .await?
    } else {
        let lease = crate::ops::required_run_lease(store)
            .await
            .map_err(|_| anyhow!("parent Ask listing requires an active Run; use `lf ask list --user` for User attention"))?;
        crate::ops::ask::pending_attention(
            store,
            &ControlCtx::Run(&lease),
            &AskTarget::Parent(lease.work.clone()),
        )
        .await?
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&attention)?);
    } else if attention.is_empty() {
        println!("No queued Ask sessions.");
    } else {
        for item in attention {
            println!(
                "{}  {:<13} {}",
                item.ask.id, item.attention, item.ask.request
            );
        }
    }
    Ok(())
}

async fn open_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    prepare: bool,
    json: bool,
) -> anyhow::Result<()> {
    let ask = store.ask_by_id(ask_id).await?;
    let ambient = crate::ops::ambient_run_lease(store).await?;
    let request = AuthenticatedRequest::cli();
    let surface = match &ask.target {
        AskTarget::User => {
            crate::ops::ask::prepare_open(store, &ControlCtx::User(&request), ask_id).await?
        }
        AskTarget::Parent(_) => {
            let lease = ambient.as_ref().ok_or_else(|| {
                anyhow!("opening a parent Ask requires the active parent Work Run")
            })?;
            crate::ops::ask::prepare_open(store, &ControlCtx::Run(lease), ask_id).await?
        }
    };
    if !prepare {
        present_in_external_terminal(&surface)?;
        match &ask.target {
            AskTarget::User => {
                store
                    .mark_presented_by_target(
                        &ControlCtx::User(&request),
                        ask_id,
                        &surface.invocation.id,
                    )
                    .await?;
            }
            AskTarget::Parent(_) => {
                let lease = ambient
                    .as_ref()
                    .expect("parent Ask open already required an ambient Run");
                store
                    .mark_presented_by_target(
                        &ControlCtx::Run(lease),
                        ask_id,
                        &surface.invocation.id,
                    )
                    .await?;
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&surface)?);
    } else if !prepare {
        println!("opened {} in a sibling terminal", ask.id);
    }
    Ok(())
}

async fn presented_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    invocation_id: &AgentInvocationId,
    json: bool,
) -> anyhow::Result<()> {
    let ask = store.ask_by_id(ask_id).await?;
    let ambient = crate::ops::ambient_run_lease(store).await?;
    let request = AuthenticatedRequest::cli();
    let invocation = match &ask.target {
        AskTarget::User => {
            store
                .mark_presented_by_target(&ControlCtx::User(&request), ask_id, invocation_id)
                .await?
        }
        AskTarget::Parent(_) => {
            let lease = ambient.as_ref().ok_or_else(|| {
                anyhow!("presenting a parent Ask requires the active parent Work Run")
            })?;
            store
                .mark_presented_by_target(&ControlCtx::Run(lease), ask_id, invocation_id)
                .await?
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&invocation)?);
    }
    Ok(())
}

async fn settle_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    result: AskResult,
    json: bool,
) -> anyhow::Result<()> {
    let invocation_id = ambient_invocation_id()?;
    let ask = crate::ops::ask::settle(store, ask_id, &invocation_id, result).await?;
    publish_comments(store);
    print_ask_receipt(&ask, json)
}

async fn release_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    reason: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let invocation_id = ambient_invocation_id()?;
    let ask = store.release_ask(ask_id, &invocation_id, reason).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ask)?);
    } else {
        println!("session closed without resolution; {} requeued", ask.id);
    }
    Ok(())
}

async fn escalate_command(store: &Arc<Store>, ask_id: &AskId, json: bool) -> anyhow::Result<()> {
    let current = store.ask_by_id(ask_id).await?;
    let ambient_invocation = ambient_invocation_id_if_present()?;
    let active_invocation = ambient_invocation
        .as_ref()
        .filter(|invocation_id| Some(*invocation_id) == current.active_invocation_id.as_ref());
    let ask = if let Some(invocation_id) = active_invocation {
        store.escalate_ask(ask_id, invocation_id).await?
    } else {
        let run = crate::ops::required_run_lease(store)
            .await
            .map_err(|error| anyhow!(error.to_string()))?;
        store
            .escalate_queued_ask(&ControlCtx::Run(&run), ask_id)
            .await?
    };
    print_ask_receipt(&ask, json)
}

async fn cancel_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    reason: &str,
    json: bool,
) -> anyhow::Result<()> {
    let current = store.ask_by_id(ask_id).await?;
    let ambient = crate::ops::ambient_run_lease(store).await?;
    let request = AuthenticatedRequest::cli();
    let ask = match (&current.target, ambient.as_ref()) {
        (AskTarget::User, _) | (_, None) => {
            crate::ops::ask::cancel(store, &ControlCtx::User(&request), ask_id, reason).await?
        }
        (AskTarget::Parent(_), Some(lease)) => {
            crate::ops::ask::cancel(store, &ControlCtx::Run(lease), ask_id, reason).await?
        }
    };
    publish_comments(store);
    print_ask_receipt(&ask, json)
}

fn ambient_invocation_id() -> anyhow::Result<AgentInvocationId> {
    let value = std::env::var(crate::durable::AGENT_INVOCATION_ENV)
        .context("lf ask requires LF_AGENT_INVOCATION_ID from the active agent Turn")?;
    AgentInvocationId::parse(&value).map_err(Into::into)
}

fn ambient_invocation_id_if_present() -> anyhow::Result<Option<AgentInvocationId>> {
    std::env::var(crate::durable::AGENT_INVOCATION_ENV)
        .ok()
        .map(|value| AgentInvocationId::parse(&value).map_err(Into::into))
        .transpose()
}

fn required_text(args: &[String], label: &str) -> anyhow::Result<String> {
    let stdin = std::io::stdin();
    if args.is_empty() && stdin.is_terminal() {
        bail!("{label} cannot be empty");
    }
    let text = text_from_args_or_stdin(args, &mut stdin.lock())?;
    if text.is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(text)
}

fn optional_text(args: &[String], default: &str) -> String {
    let text = args.join(" ").trim().to_string();
    if text.is_empty() {
        default.to_string()
    } else {
        text
    }
}

fn text_from_args_or_stdin(args: &[String], stdin: &mut impl Read) -> anyhow::Result<String> {
    let joined = args.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    let mut buffer = String::new();
    stdin.read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn print_terminal_ask(ask: &Ask, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(ask)?);
    }
    match ask.result.as_ref() {
        Some(AskResult::Resolved { summary }) => {
            if !json {
                println!("{summary}");
            }
            Ok(())
        }
        Some(AskResult::Declined { reason }) => {
            if !json {
                eprintln!("Ask {} declined: {reason}", ask.id);
            }
            bail!("Ask {} declined", ask.id)
        }
        Some(AskResult::Cancelled { reason }) => {
            if !json {
                eprintln!("Ask {} cancelled: {reason}", ask.id);
            }
            bail!("Ask {} cancelled", ask.id)
        }
        None => bail!("terminal Ask {} has no typed result", ask.id),
    }
}

fn print_wait_selection(ask: &Ask, json: bool) {
    let message = format!("waiting on {}: {}", ask.id, ask.request);
    if json {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}

fn print_ask_receipt(ask: &Ask, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(ask)?);
    } else {
        println!("{}  {}", ask.id, ask.state.as_str());
    }
    Ok(())
}

fn publish_comments(store: &Arc<Store>) {
    let store = Arc::clone(store);
    tokio::spawn(async move {
        if let Err(error) = crate::ops::publish_pending_ask_comments(&store).await {
            tracing::warn!(%error, "Ask comment outbox publication failed");
        }
    });
}

async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    Ok(Arc::new(
        open_store(&config)
            .await
            .context("open the shared Loopflow store")?,
    ))
}

fn present_in_external_terminal(surface: &InvocationSurface) -> anyhow::Result<()> {
    let attach = exact_attach_argv(surface)?;
    let terminal = std::env::var("LF_EXTERNAL_TERMINAL")
        .ok()
        .or_else(|| {
            crate::engine::config::load_global_config()
                .ok()
                .flatten()
                .and_then(|config| config.session.terminal)
        })
        .unwrap_or_else(default_external_terminal);
    let presentation = external_terminal_command(&terminal, &attach)?;
    run_presentation(&presentation)
}

fn run_presentation(presentation: &PresentationCommand) -> anyhow::Result<()> {
    let status = Command::new(&presentation.program)
        .args(&presentation.args)
        .status()
        .with_context(|| format!("launch external terminal {:?}", presentation.program))?;
    if !status.success() {
        if let Some(path) = presentation.cleanup_on_failure.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        bail!("external terminal presentation failed with {status}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PresentationCommand {
    program: String,
    args: Vec<String>,
    cleanup_on_failure: Option<PathBuf>,
}

fn external_terminal_command(
    terminal: &str,
    attach_argv: &[String],
) -> anyhow::Result<PresentationCommand> {
    if cfg!(target_os = "macos") {
        let launcher = write_terminal_launcher(attach_argv)?;
        Ok(PresentationCommand {
            program: "open".to_string(),
            args: vec![
                "-na".to_string(),
                terminal.to_string(),
                launcher.display().to_string(),
            ],
            cleanup_on_failure: Some(launcher),
        })
    } else {
        let mut args = vec!["-e".to_string()];
        args.extend_from_slice(attach_argv);
        Ok(PresentationCommand {
            program: terminal.to_string(),
            args,
            cleanup_on_failure: None,
        })
    }
}

fn write_terminal_launcher(argv: &[String]) -> anyhow::Result<PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let path = std::env::temp_dir().join(format!(
        "loopflow-ask-{}.command",
        uuid::Uuid::new_v4().simple()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)?;
    let command = argv
        .iter()
        .map(|arg| crate::engine::process::shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    writeln!(
        file,
        "#!/bin/zsh\nlauncher=$0\nrm -f -- \"$launcher\"\nexec {command}"
    )?;
    file.sync_all()?;
    Ok(path)
}

fn exact_attach_argv(surface: &InvocationSurface) -> anyhow::Result<Vec<String>> {
    let attach = surface
        .attach_argv
        .as_ref()
        .ok_or_else(|| anyhow!("Invocation {} has no attach route", surface.invocation.id))?;
    let home = HomeRoute::parse(&surface.home_route)
        .ok_or_else(|| anyhow!("invalid Home route {:?}", surface.home_route))?;
    if let Some(destination) = home.ssh_destination() {
        let mut argv = vec!["ssh".to_string()];
        if let Some(port) = home.ssh_port() {
            argv.extend(["-p".to_string(), port.to_string()]);
        }
        argv.push(destination.to_string());
        argv.push("--".to_string());
        argv.extend(attach.iter().cloned());
        Ok(argv)
    } else {
        Ok(attach.clone())
    }
}

fn default_external_terminal() -> String {
    if cfg!(target_os = "macos") {
        "Terminal".to_string()
    } else {
        "x-terminal-emulator".to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::durable::{
        AgentInvocationId, Ask, AskBody, AskId, AskOrigin, AskResult, AskState, AskTarget,
        AuthenticatedRequest, Containment, ControlCtx, HomeId, InvocationRoute, RunAdvance, RunId,
        RunTrigger, WorkRef,
    };
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig};
    use crate::wave::Wave;

    #[test]
    fn external_terminal_launcher_execs_the_exact_attach_route() {
        let attach = vec![
            "ssh".to_string(),
            "jack@mini".to_string(),
            "--".to_string(),
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            "lf-ask-proof".to_string(),
        ];
        let command = super::external_terminal_command("Terminal", &attach).unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(command.program, "open");
            let launcher = std::path::Path::new(command.args.last().unwrap());
            let contents = std::fs::read_to_string(launcher).unwrap();
            assert!(contents.contains(
                "exec 'ssh' 'jack@mini' '--' 'tmux' 'attach-session' '-t' 'lf-ask-proof'"
            ));
            std::fs::remove_file(launcher).unwrap();
        } else {
            assert_eq!(command.args[0], "-e");
            assert_eq!(&command.args[1..], attach);
        }
    }

    #[test]
    fn failed_terminal_presentation_is_reported_without_a_fallback() {
        let cleanup = tempfile::NamedTempFile::new().unwrap().into_temp_path();
        let cleanup = cleanup.keep().unwrap();
        let command = super::PresentationCommand {
            program: "/usr/bin/false".to_string(),
            args: Vec::new(),
            cleanup_on_failure: Some(cleanup.clone()),
        };

        let error = super::run_presentation(&command).unwrap_err();

        assert!(error.to_string().contains("presentation failed"));
        assert!(!cleanup.exists());
    }

    #[test]
    fn default_wait_prefers_invocation_then_run_then_work() {
        fn outgoing(run_id: RunId, invocation_id: AgentInvocationId) -> Ask {
            Ask {
                id: AskId::new(),
                origin: AskOrigin {
                    work: WorkRef::Wave(WaveId::new()),
                    run_id,
                    turn_id: None,
                    invocation_id: Some(invocation_id),
                    home_id: HomeId::new(),
                    cwd: PathBuf::from("/repo"),
                },
                target: AskTarget::User,
                request: AskBody::Intervention {
                    prompt: "Which proof?".to_string(),
                },
                state: AskState::Queued,
                active_invocation_id: None,
                result: None,
                terminal_author: None,
                asked_at: time::OffsetDateTime::now_utc(),
                terminal_at: None,
            }
        }

        let run_id = RunId::new();
        let invocation_id = AgentInvocationId::new();
        let work_ask = outgoing(RunId::new(), AgentInvocationId::new());
        let run_ask = outgoing(run_id.clone(), AgentInvocationId::new());
        let invocation_ask = outgoing(run_id.clone(), invocation_id.clone());
        let asks = vec![work_ask.clone(), run_ask.clone(), invocation_ask.clone()];

        assert_eq!(
            super::select_default_wait(asks.clone(), &run_id, Some(&invocation_id))
                .unwrap()
                .id,
            invocation_ask.id
        );
        assert_eq!(
            super::select_default_wait(
                vec![work_ask.clone(), run_ask.clone()],
                &run_id,
                Some(&invocation_id),
            )
            .unwrap()
            .id,
            run_ask.id
        );
        assert_eq!(
            super::select_default_wait(vec![work_ask.clone()], &run_id, Some(&invocation_id))
                .unwrap()
                .id,
            work_ask.id
        );
    }

    #[tokio::test]
    async fn waiter_rejoins_without_ending_its_invocation_and_terminal_results_are_typed() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            "ask-cli".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let work = WorkRef::Wave(wave.id().clone());
        let (_, run_lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &run_lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "lf-ask-cli".to_string(),
                    },
                    cwd: PathBuf::from("/tmp/ask-cli"),
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &run_lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        store
            .advance_run(
                &run_lease,
                RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let ask = store
            .request_intervention(&run_lease, &invocation.id, "Connect the account", true)
            .await
            .unwrap();

        let wait_store = Arc::clone(&store);
        let wait_lease = run_lease.clone();
        let wait_ask = ask.clone();
        let abandoned_wait = tokio::spawn(async move {
            super::wait_for_terminal(&wait_store, &wait_lease, wait_ask, false).await
        });
        tokio::time::sleep(super::WAIT_INTERVAL).await;
        abandoned_wait.abort();
        assert_eq!(
            store.ask_by_id(&ask.id).await.unwrap().state,
            AskState::Queued
        );

        let wait_store = Arc::clone(&store);
        let wait_lease = run_lease.clone();
        let wait_ask = ask.clone();
        let resumed_wait = tokio::spawn(async move {
            super::wait_for_terminal(&wait_store, &wait_lease, wait_ask, false).await
        });
        let user = AuthenticatedRequest::cli();
        let claim = store
            .claim_test_ask(&ControlCtx::User(&user), &ask.id)
            .await
            .unwrap();
        assert!(claim.needs_launch);
        let mismatched = store
            .request_intervention(&run_lease, &invocation.id, "Different Ask", true)
            .await
            .unwrap();
        let mismatch = store
            .settle_ask(
                &mismatched.id,
                &claim.invocation_id,
                AskResult::Resolved {
                    summary: "Wrong Ask".to_string(),
                },
            )
            .await
            .unwrap_err();
        assert!(mismatch.to_string().contains("does not belong to Ask"));
        let ask_invocation = store
            .ask_invocations(&ask.id)
            .await
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == claim.invocation_id)
            .unwrap();
        let surface = store
            .invocation_surface(&ask_invocation.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(surface.run.cwd, Some(PathBuf::from("/tmp/ask-cli")));
        assert_eq!(
            surface.attach_argv,
            Some(vec![
                "tmux".to_string(),
                "attach-session".to_string(),
                "-t".to_string(),
                crate::ops::ask::session_name(&ask_invocation.id),
            ])
        );
        store
            .mark_ask_ready(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        let failed_presentation = super::PresentationCommand {
            program: "/usr/bin/false".to_string(),
            args: Vec::new(),
            cleanup_on_failure: None,
        };
        super::run_presentation(&failed_presentation).unwrap_err();
        assert_eq!(
            store.ask_presentation(&ask_invocation.id).await.unwrap(),
            (true, false)
        );
        assert_eq!(
            store.ask_by_id(&ask.id).await.unwrap().state,
            AskState::Claimed
        );
        let reopened = store
            .claim_test_ask(&ControlCtx::User(&user), &ask.id)
            .await
            .unwrap();
        assert_eq!(reopened.invocation_id, claim.invocation_id);
        assert!(!reopened.needs_launch);
        assert_eq!(
            store
                .invocation_surface(&claim.invocation_id)
                .await
                .unwrap()
                .unwrap()
                .invocation
                .id,
            ask_invocation.id
        );
        let stale_invocation_id = AgentInvocationId::new();
        let stale_error = store
            .mark_presented_by_target(&ControlCtx::User(&user), &ask.id, &stale_invocation_id)
            .await
            .unwrap_err();
        assert!(stale_error.to_string().contains("not"));
        assert_eq!(
            store.ask_presentation(&ask_invocation.id).await.unwrap(),
            (true, false)
        );
        store
            .mark_presented_by_target(&ControlCtx::User(&user), &ask.id, &ask_invocation.id)
            .await
            .unwrap();
        store
            .settle_ask(
                &ask.id,
                &claim.invocation_id,
                AskResult::Resolved {
                    summary: "Account connected".to_string(),
                },
            )
            .await
            .unwrap();
        resumed_wait.await.unwrap().unwrap();

        let invocation = store
            .invocations_for_run(&run_lease.run_id)
            .await
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == invocation.id)
            .unwrap();
        assert!(invocation.ended_at.is_none());

        let declined = store
            .request_intervention(&run_lease, &invocation.id, "Unsafe action", true)
            .await
            .unwrap();
        let claim = store
            .claim_test_ask(&ControlCtx::User(&user), &declined.id)
            .await
            .unwrap();
        store
            .mark_ask_ready(&declined.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .mark_ask_presented(&declined.id, &claim.invocation_id)
            .await
            .unwrap();
        let declined = store
            .settle_ask(
                &declined.id,
                &claim.invocation_id,
                AskResult::Declined {
                    reason: "Unsafe".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(super::print_terminal_ask(&declined, true)
            .unwrap_err()
            .to_string()
            .contains("declined"));

        let cancelled = store
            .request_intervention(&run_lease, &invocation.id, "No longer needed", true)
            .await
            .unwrap();
        let cancelled = store
            .cancel_ask(&ControlCtx::Run(&run_lease), &cancelled.id, "Withdrawn")
            .await
            .unwrap();
        assert!(super::print_terminal_ask(&cancelled, true)
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
    }
}
