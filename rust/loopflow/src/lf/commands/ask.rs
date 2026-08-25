use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use crate::durable::{Ask, AskId, AskOrigin, AskResult, AskSession, AskTarget, RunId, WorkRef};
use crate::engine::wave_home::HomeRoute;
use crate::lf::{AskArgs, AskCommand};
use crate::store::{open_store, storage_config_from_env, Store};
use anyhow::{anyhow, bail, Context};
use fs2::FileExt;

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
            all,
        }) => list_command(&store, *user, *outgoing, *json, *all).await,
        Some(AskCommand::Open {
            ask_id,
            prepare,
            json,
        }) => open_command(&store, ask_id, *prepare, *json).await,
        Some(AskCommand::Presented {
            ask_id,
            run_id,
            json,
        }) => presented_command(&store, ask_id, run_id, *json).await,
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
        Some(AskCommand::Serve {
            ask_id,
            run_id,
            headless,
        }) => crate::ops::ask::serve(store, ask_id.clone(), run_id.clone(), *headless).await,
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
    let origin = ambient_ask_origin(store).await?;
    let work = origin.work.clone();
    let ask = crate::ops::ask::request_intervention(store, origin, &prompt, args.user).await?;
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
    wait_for_terminal(store, &work, ask, args.json).await
}

async fn wait_command(
    store: &Arc<Store>,
    ask_id: Option<&AskId>,
    json: bool,
) -> anyhow::Result<()> {
    let origin = ambient_ask_origin(store).await?;
    let ask = ask_for_wait(store, &origin.work, origin.source_run_id.as_ref(), ask_id).await?;
    print_wait_selection(&ask, json);
    crate::ops::ask::wake(store, &ask.target).await;
    wait_for_terminal(store, &origin.work, ask, json).await
}

async fn wait_for_terminal(
    store: &Arc<Store>,
    work: &WorkRef,
    mut ask: Ask,
    json: bool,
) -> anyhow::Result<()> {
    let mut next_wake = tokio::time::Instant::now() + TARGET_WAKE_INTERVAL;
    loop {
        if ask.state.is_terminal() {
            return print_terminal_ask(&ask, json);
        }
        tokio::time::sleep(WAIT_INTERVAL).await;
        ask = ask_for_wait(store, work, None, Some(&ask.id)).await?;
        if tokio::time::Instant::now() >= next_wake {
            crate::ops::ask::wake(store, &ask.target).await;
            next_wake = tokio::time::Instant::now() + TARGET_WAKE_INTERVAL;
        }
    }
}

async fn ask_for_wait(
    store: &Arc<Store>,
    work: &WorkRef,
    source_run_id: Option<&RunId>,
    ask_id: Option<&AskId>,
) -> anyhow::Result<Ask> {
    let asks = store.asks_for_work(work).await?;
    match ask_id {
        Some(ask_id) => asks
            .into_iter()
            .find(|ask| &ask.id == ask_id)
            .ok_or_else(|| anyhow!("Ask {ask_id} does not belong to this Work")),
        None => select_default_wait(asks, source_run_id)
            .ok_or_else(|| anyhow!("this Work has no unresolved outgoing Ask")),
    }
}

fn select_default_wait(asks: Vec<Ask>, source_run_id: Option<&RunId>) -> Option<Ask> {
    let unresolved = |ask: &&Ask| !ask.state.is_terminal();
    if let Some(run_id) = source_run_id {
        if let Some(ask) = asks
            .iter()
            .filter(unresolved)
            .find(|ask| ask.origin.source_run_id.as_ref() == Some(run_id))
        {
            return Some(ask.clone());
        }
    }
    asks.iter().find(unresolved).cloned()
}

/// Keep only Asks whose origin cwd resolves to the current repository,
/// collapsing worktrees to their main checkout. `all` (or a cwd outside any git
/// repo, where there is nothing to scope to) returns every Ask unchanged.
fn scope_attention_to_repo(
    attention: Vec<crate::ops::ask::AskAttention>,
    all: bool,
) -> Vec<crate::ops::ask::AskAttention> {
    if all {
        return attention;
    }
    let Some(scope) = crate::repository::CanonicalRepo::current() else {
        return attention;
    };
    attention
        .into_iter()
        .filter(|item| scope.contains(&item.ask.origin.cwd))
        .collect()
}

async fn list_command(
    store: &Arc<Store>,
    user: bool,
    outgoing: bool,
    json: bool,
    all: bool,
) -> anyhow::Result<()> {
    if outgoing {
        let origin = ambient_ask_origin(store).await?;
        let asks = store
            .asks_for_work(&origin.work)
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
                    "{}  {:<8} to={:<32} from={}:{} source_run={}  {}",
                    ask.id,
                    ask.state.as_str(),
                    ask.target,
                    ask.origin.work.kind(),
                    ask.origin.work.id(),
                    ask.origin.source_run_id.as_ref().map_or("-", RunId::as_str),
                    ask.request,
                );
            }
        }
        return Ok(());
    }
    let attention = if user {
        let attention = crate::ops::ask::pending_attention(store, &AskTarget::User).await?;
        scope_attention_to_repo(attention, all)
    } else {
        let origin = ambient_ask_origin(store).await.map_err(|_| {
            anyhow!("parent Ask listing requires ambient Work; use `lf ask list --user` for User attention")
        })?;
        crate::ops::ask::pending_attention(store, &AskTarget::Parent(origin.work)).await?
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
    let surface = crate::ops::ask::prepare_open(store, ask_id).await?;
    if !prepare {
        present_in_external_terminal(&surface)?;
        store.mark_ask_presented(ask_id, &surface.run_id).await?;
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
    run_id: &RunId,
    json: bool,
) -> anyhow::Result<()> {
    let ask = store.mark_ask_presented(ask_id, run_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ask)?);
    }
    Ok(())
}

async fn settle_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    result: AskResult,
    json: bool,
) -> anyhow::Result<()> {
    let run_id = ambient_run_id()?;
    let ask = crate::ops::ask::settle(store, ask_id, &run_id, result).await?;
    publish_comments(store);
    print_ask_receipt(&ask, json)
}

async fn release_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    reason: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let run_id = ambient_run_id()?;
    let ask = store.release_ask(ask_id, &run_id, reason).await?;
    crate::ops::ask::checkpoint_origin_task(store, &ask, "release").await;
    if json {
        println!("{}", serde_json::to_string_pretty(&ask)?);
    } else {
        println!("session closed without resolution; {} requeued", ask.id);
    }
    Ok(())
}

async fn escalate_command(store: &Arc<Store>, ask_id: &AskId, json: bool) -> anyhow::Result<()> {
    let current = store.ask_by_id(ask_id).await?;
    let ambient_run = ambient_run_id_if_present()?;
    let active_run = ambient_run
        .as_ref()
        .filter(|run_id| Some(*run_id) == current.active_run_id.as_ref());
    let ask = if let Some(run_id) = active_run {
        store.escalate_ask(ask_id, run_id).await?
    } else {
        store.escalate_queued_ask(ask_id).await?
    };
    print_ask_receipt(&ask, json)
}

async fn cancel_command(
    store: &Arc<Store>,
    ask_id: &AskId,
    reason: &str,
    json: bool,
) -> anyhow::Result<()> {
    let ask = crate::ops::ask::cancel(store, ask_id, reason).await?;
    publish_comments(store);
    print_ask_receipt(&ask, json)
}

fn ambient_run_id() -> anyhow::Result<RunId> {
    ambient_run_id_if_present()?
        .ok_or_else(|| anyhow!("lf ask settlement requires LF_RUN_ID from the active generic Run"))
}

fn ambient_run_id_if_present() -> anyhow::Result<Option<RunId>> {
    std::env::var(crate::durable::RUN_ID_ENV)
        .ok()
        .map(|value| RunId::parse(&value).map_err(Into::into))
        .transpose()
}

async fn ambient_ask_origin(store: &Arc<Store>) -> anyhow::Result<AskOrigin> {
    let cwd = std::env::current_dir().context("resolve Ask cwd")?;
    let work = ambient_work(store, &cwd).await?;
    let placement = store.placement(&work).await?;
    Ok(AskOrigin {
        work,
        source_run_id: ambient_run_id_if_present()?,
        home_id: placement.home_id,
        cwd,
    })
}

async fn ambient_work(store: &Arc<Store>, cwd: &std::path::Path) -> anyhow::Result<WorkRef> {
    if let Ok(run_dir) = std::env::var(crate::run_record::RUN_DIR_ENV) {
        let manifest = std::fs::read(std::path::Path::new(&run_dir).join("manifest.json"))
            .context("read ambient Run manifest")?;
        let manifest: crate::run_record::RunManifest =
            serde_json::from_slice(&manifest).context("parse ambient Run manifest")?;
        let repo = manifest.repo.as_deref().unwrap_or(cwd);
        for subject in manifest.subjects {
            if let Ok(binding) =
                crate::ops::resolve_work_binding(store, repo, &subject.selector).await
            {
                return Ok(binding.work);
            }
        }
    }
    if let Some(task) = store
        .get_task_by_worktree(&cwd.display().to_string())
        .await?
    {
        return Ok(WorkRef::Task(task.id));
    }
    bail!("cannot resolve ambient Work from the generic Run or current worktree")
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

fn present_in_external_terminal(surface: &AskSession) -> anyhow::Result<()> {
    let attach = exact_attach_argv(surface)?;
    let terminal = resolve_external_terminal();
    if cfg!(target_os = "macos") && is_ghostty_terminal(&terminal) {
        if let Some(ask_session) = local_tmux_attach_session(&attach) {
            return present_in_ghostty(&attach, &ask_session);
        }
    }
    let presentation = external_terminal_command(&terminal, &attach)?;
    run_presentation(&presentation)
}

fn resolve_external_terminal() -> String {
    std::env::var("LF_EXTERNAL_TERMINAL")
        .ok()
        .or_else(|| {
            crate::engine::config::load_global_config()
                .ok()
                .flatten()
                .and_then(|config| config.session.terminal)
        })
        .unwrap_or_else(default_external_terminal)
}

fn local_tmux_attach_session(attach_argv: &[String]) -> Option<String> {
    let program = std::path::Path::new(attach_argv.first()?)
        .file_name()?
        .to_str()?;
    if program != "tmux" {
        return None;
    }
    if !matches!(
        attach_argv.get(1)?.as_str(),
        "attach-session" | "attach" | "a"
    ) {
        return None;
    }
    let mut args = attach_argv[2..].iter();
    while let Some(arg) = args.next() {
        if arg == "-t" {
            return args.next().cloned();
        }
    }
    None
}

fn is_ghostty_terminal(terminal: &str) -> bool {
    std::path::Path::new(terminal)
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("ghostty"))
}

const GHOSTTY_ASK_SCRIPT: &str = r#"
on run argv
    set launcherPath to item 1 of argv
    set tabTitle to item 2 of argv
    set statePath to item 3 of argv
    set savedWindowId to ""
    set removeLauncher to false

    try
        set savedWindowId to do shell script "/bin/cat " & quoted form of statePath
    end try

    tell application "Ghostty"
        set askWindow to missing value
        if savedWindowId is not "" then
            repeat with candidateWindow in windows
                if (id of candidateWindow as text) is savedWindowId then
                    set askWindow to candidateWindow
                    exit repeat
                end if
            end repeat
        end if

        set askTab to missing value
        if askWindow is not missing value then
            repeat with candidateTab in tabs of askWindow
                if (name of candidateTab as text) is tabTitle then
                    set askTab to candidateTab
                    exit repeat
                end if
            end repeat
        end if

        if askTab is missing value then
            set surfaceConfig to new surface configuration from {command:launcherPath, wait after command:true}
            if askWindow is missing value then
                set askWindow to new window with configuration surfaceConfig
                set askTab to selected tab of askWindow
            else
                set askTab to new tab in askWindow with configuration surfaceConfig
            end if
            perform action ("set_tab_title:" & tabTitle) on focused terminal of askTab
        else
            set removeLauncher to true
        end if

        select tab askTab
        activate window askWindow
        set askWindowId to id of askWindow as text
    end tell

    do shell script "/usr/bin/printf %s " & quoted form of askWindowId & " > " & quoted form of statePath
    if removeLauncher then
        do shell script "/bin/rm -f -- " & quoted form of launcherPath
    end if
end run
"#;

fn present_in_ghostty(attach_argv: &[String], ask_session: &str) -> anyhow::Result<()> {
    let lock_path = std::env::temp_dir().join("loopflow-cli-ghostty-asks.lock");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("open Ghostty Ask presentation lock {lock_path:?}"))?;
    FileExt::lock_exclusive(&lock).context("lock Ghostty Ask presentation")?;
    run_presentation(&ghostty_ask_command(attach_argv, ask_session)?)
}

fn ghostty_ask_command(
    attach_argv: &[String],
    ask_session: &str,
) -> anyhow::Result<PresentationCommand> {
    let launcher = write_terminal_launcher(attach_argv)?;
    let state = std::env::temp_dir().join("loopflow-cli-ghostty-asks-window-id");
    Ok(PresentationCommand {
        program: "osascript".to_string(),
        args: vec![
            "-e".to_string(),
            GHOSTTY_ASK_SCRIPT.to_string(),
            "--".to_string(),
            launcher.display().to_string(),
            ask_session.to_string(),
            state.display().to_string(),
        ],
        cleanup_on_failure: Some(launcher),
    })
}

fn run_presentation(presentation: &PresentationCommand) -> anyhow::Result<()> {
    let output = Command::new(&presentation.program)
        .args(&presentation.args)
        .output()
        .with_context(|| format!("launch external terminal {:?}", presentation.program))?;
    if !output.status.success() {
        if let Some(path) = presentation.cleanup_on_failure.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        bail!(
            "{}",
            _presentation_failure_message(
                &presentation.program,
                &output.status.to_string(),
                String::from_utf8_lossy(&output.stderr).trim(),
            )
        );
    }
    Ok(())
}

fn _presentation_failure_message(program: &str, status: &str, stderr: &str) -> String {
    if std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "osascript")
        && stderr.contains("-1743")
    {
        return "Ghostty automation is not authorized; allow your terminal to control Ghostty in System Settings > Privacy & Security > Automation, then retry"
            .to_string();
    }
    if stderr.is_empty() {
        format!("external terminal presentation failed with {status}")
    } else {
        format!("external terminal presentation failed with {status}: {stderr}")
    }
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
        // No -n: route the launcher into the running terminal instance so each
        // Ask presents as a window there instead of spawning a whole new app
        // instance (which multiplies Cmd-Tab entries and restored windows).
        Ok(PresentationCommand {
            program: "open".to_string(),
            args: vec![
                "-a".to_string(),
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

fn exact_attach_argv(surface: &AskSession) -> anyhow::Result<Vec<String>> {
    let attach = &surface.attach_argv;
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
    #[test]
    fn local_tmux_attach_preserves_the_ask_session() {
        let attach = vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            "lf-ask-proof".to_string(),
        ];
        assert_eq!(
            super::local_tmux_attach_session(&attach).as_deref(),
            Some("lf-ask-proof")
        );
    }

    #[test]
    fn remote_attach_route_is_preserved() {
        let surface = crate::durable::AskSession {
            ask_id: crate::durable::AskId::new(),
            run_id: crate::durable::RunId::new(),
            home_route: "ssh://jack@mini".to_string(),
            attach_argv: vec![
                "tmux".to_string(),
                "attach-session".to_string(),
                "-t".to_string(),
                "lf-ask-proof".to_string(),
            ],
        };
        let argv = super::exact_attach_argv(&surface).unwrap();
        assert_eq!(argv.first().map(String::as_str), Some("ssh"));
        assert_eq!(argv.last().map(String::as_str), Some("lf-ask-proof"));
    }
}
