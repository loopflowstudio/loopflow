//! Present files through the caller's existing host. Never launch a host or a conversation.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use super::{OpenArgs, OpenHost};

#[derive(Debug, Serialize)]
struct FilePresentation {
    host: Option<OpenHost>,
    paths: Vec<PathBuf>,
    opening: bool,
}

/// Unlike Work opening, file presentation has no installed-app or terminal fallback.
fn host(requested: OpenHost) -> Option<OpenHost> {
    if requested != OpenHost::Auto {
        return (requested != OpenHost::Terminal).then_some(requested);
    }
    super::viewed_placement().map(|(host, _)| host).or_else(|| {
        [
            (super::in_rex(), OpenHost::Rex),
            (super::in_herdr(), OpenHost::Herdr),
            (super::in_tmux(), OpenHost::Tmux),
            (super::in_cmux(), OpenHost::Cmux),
            (super::in_desktop(), OpenHost::Desktop),
        ]
        .into_iter()
        .find_map(|(present, host)| present.then_some(host))
    })
}

pub(super) fn run(cli: &crate::lf::Cli, args: &OpenArgs, explain: bool) -> Result<()> {
    let current = host(args.host);
    let (targets, _) = match super::targets(cli, args) {
        Ok(targets) => targets,
        Err(_) if current.is_none() => return report(cli, args, None, args.files.clone(), explain),
        Err(error) => return Err(error),
    };
    let target = targets
        .first()
        .context("File presentation has no selected Work")?;
    // A background Graph step presents to the Task's currently viewed conversation.
    let viewed = super::viewed_placement()
        .or_else(|| target.primary.as_deref().and_then(super::session_placement));
    let selected = if args.host == OpenHost::Auto {
        viewed.as_ref().map(|(host, _)| *host).or(current)
    } else {
        current
    };
    let Some(host) = selected else {
        return report(cli, args, None, args.files.clone(), explain);
    };
    let placement = viewed
        .as_ref()
        .filter(|(viewed, _)| *viewed == host)
        .map(|(_, id)| id.as_str());
    let directory = std::env::current_dir()?;
    let checkout = crate::repo::discover_repo_root(&directory)?.unwrap_or(directory);
    let cwd = match &target.task {
        Some(task) => task.worktree()?.as_path(),
        None if args.url.is_none() => &checkout,
        None => target.cwd(),
    };
    let mut paths = Vec::new();
    for path in &args.files {
        let path = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };
        // Desktop reads on the recorded Machine through its existing Files transport.
        let path = if host == OpenHost::Desktop && target.execution_machine.is_some() {
            path
        } else {
            anyhow::ensure!(
                target.execution_machine.is_none() && target.execution_unavailable.is_none(),
                "Present files on the Task's recorded Machine; no local editor was opened"
            );
            path.canonicalize()
                .with_context(|| format!("Cannot open {}", path.display()))?
        };
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if explain {
        return report(cli, args, Some(host), paths, true);
    }
    for path in &paths {
        if host == OpenHost::Desktop {
            let relative = path
                .strip_prefix(cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf()))
                .context("Desktop Files target is outside this checkout")?;
            let context = super::work_context(
                cwd,
                target.task.as_ref().map(|task| task.identifier.as_str()),
                target.wave.as_deref(),
                None,
            )?;
            if !super::super::desktop::present_file(&context, relative)? {
                return report(cli, args, None, paths.clone(), explain);
            }
            continue;
        }
        let argv = file_argv(target, cwd, path)?;
        match host {
            OpenHost::Rex => super::workspace::present_file(args, cwd, path, argv, placement)?,
            OpenHost::Herdr => super::workspace::present_herdr_file(cwd, path, argv, placement)?,
            OpenHost::Tmux | OpenHost::Cmux => terminal_file(host, cwd, &argv, placement)?,
            OpenHost::Auto | OpenHost::Desktop | OpenHost::Terminal => {
                unreachable!("resolved file host")
            }
        }
    }
    report(cli, args, Some(host), paths, explain)
}

fn file_argv(target: &super::Resolved, cwd: &Path, path: &Path) -> Result<Vec<String>> {
    if let Some(task) = &target.task {
        let root = cwd.canonicalize()?;
        let relative = path
            .strip_prefix(&root)
            .context("File is outside the Task checkout")?;
        return super::workspace::viewer_argv(
            cwd,
            &[
                "task",
                "tool",
                &task.identifier,
                "files",
                "--checkout",
                cwd.to_str().context("Checkout is not UTF-8")?,
                "--path",
                relative.to_str().context("File path is not UTF-8")?,
            ],
        );
    }
    // Taskless callers use the same editor policy without inventing a Task.
    let command = super::super::task_tool::file_editor(path);
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| {
            arg.to_str()
                .map(str::to_owned)
                .context("Editor argument is not UTF-8")
        })
        .collect()
}

fn terminal_file(
    host: OpenHost,
    cwd: &Path,
    argv: &[String],
    placement: Option<&str>,
) -> Result<()> {
    let shell = shlex::try_join(argv.iter().map(String::as_str))?;
    let placement = placement.map(str::to_owned);
    let mut command = match host {
        OpenHost::Tmux => {
            let pane = placement
                .or_else(|| std::env::var("TMUX_PANE").ok())
                .context("Current tmux pane is unavailable")?;
            let mut command = Command::new("tmux");
            command
                .args(["split-window", "-d", "-t", &pane, "-c"])
                .arg(cwd)
                .arg(&shell);
            command
        }
        OpenHost::Cmux => {
            let workspace = placement
                .or_else(|| std::env::var("CMUX_WORKSPACE_ID").ok())
                .context("Current cmux workspace is unavailable")?;
            let mut command = Command::new("cmux");
            // Only the newly created pane receives the command.
            let shell = format!(
                "cd {} && exec {shell}",
                shlex::try_quote(&cwd.to_string_lossy())?
            );
            command.env("CMUX_QUIET", "1").args([
                "new-split",
                "right",
                "--workspace",
                &workspace,
                "--focus",
                "false",
                "--command",
                &shell,
            ]);
            command
        }
        _ => unreachable!("terminal file host"),
    };
    command.current_dir(cwd);
    super::run_host(command, host.label())
        .map_err(|error| anyhow::anyhow!("{}", error.reason()))?;
    Ok(())
}

fn report(
    cli: &crate::lf::Cli,
    args: &OpenArgs,
    host: Option<OpenHost>,
    paths: Vec<PathBuf>,
    explain: bool,
) -> Result<()> {
    let result = FilePresentation {
        opening: host.is_some() && !explain,
        host,
        paths,
    };
    if cli.json || args.json {
        println!("{}", serde_json::to_string(&result)?);
    } else if let Some(host) = host {
        for path in result.paths {
            println!(
                "{} {} in {}.",
                if explain { "Would open" } else { "Opening" },
                path.display(),
                host.label()
            );
        }
    } else {
        println!("No file-viewing host attached; nothing opened.");
    }
    Ok(())
}
