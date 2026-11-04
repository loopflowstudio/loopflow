//! Present files through the caller's existing host. Never launch a host or a conversation.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use super::{OpenArgs, OpenHost};

#[derive(Debug, Serialize)]
struct FilePresentation<'a> {
    host: Option<OpenHost>,
    paths: &'a [PathBuf],
    opening: bool,
}

/// Unlike Work opening, file presentation has no installed-app or terminal fallback.
fn host(requested: OpenHost, viewed: Option<OpenHost>) -> Option<OpenHost> {
    if requested != OpenHost::Auto {
        return (requested != OpenHost::Terminal).then_some(requested);
    }
    viewed.or_else(|| {
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
    let viewed = super::viewed_placement();
    let current = host(args.host, viewed.as_ref().map(|(host, _)| *host));
    let (targets, _) = match super::targets(cli, args) {
        Ok(targets) => targets,
        Err(_) if current.is_none() => return report(cli, args, None, &args.files, explain),
        Err(error) => return Err(error),
    };
    let target = targets
        .first()
        .context("File presentation has no selected Work")?;
    // A background Graph step presents to the Task's currently viewed conversation.
    let viewed = viewed.or_else(|| target.conversation().and_then(super::session_placement));
    let selected = if args.host == OpenHost::Auto {
        viewed.as_ref().map(|(host, _)| *host).or(current)
    } else {
        current
    };
    let Some(host) = selected else {
        return report(cli, args, None, &args.files, explain);
    };
    let placement = viewed
        .as_ref()
        .filter(|(viewed, _)| *viewed == host)
        .map(|(_, id)| id.as_str());
    let cwd = match &target.task {
        Some(task) => task.worktree()?.clone(),
        None if args.url.is_none() => {
            let directory = std::env::current_dir()?;
            crate::repo::discover_repo_root(&directory)?.unwrap_or(directory)
        }
        None => target.cwd().to_path_buf(),
    };
    // Desktop reads remote paths through Files; native editors need a local checkout.
    let remote = host == OpenHost::Desktop && target.execution_machine.is_some();
    if !remote {
        anyhow::ensure!(
            target.execution_machine.is_none() && target.execution_unavailable.is_none(),
            "Present files on the Task's recorded Machine; no local editor was opened"
        );
    }
    // Compare canonical paths, but keep the recorded checkout spelling for launch.
    let root = if remote {
        cwd.clone()
    } else {
        cwd.canonicalize()?
    };
    let mut paths = Vec::new();
    for path in &args.files {
        let path = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };
        let path = if remote {
            path
        } else {
            path.canonicalize()
                .with_context(|| format!("Cannot open {}", path.display()))?
        };
        if target.task.is_some() && !remote {
            anyhow::ensure!(path.starts_with(&root), "File is outside the Task checkout");
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if explain {
        return report(cli, args, Some(host), &paths, true);
    }
    if host == OpenHost::Desktop {
        let relative = paths
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .context("Desktop Files target is outside this checkout")
            })
            .collect::<Result<Vec<_>>>()?;
        let context = super::work_context(
            &cwd,
            target.task.as_ref().map(|task| task.identifier.as_str()),
            target.wave.as_deref(),
            None,
        )?;
        let opened = super::super::desktop::present_files(&context, &relative)?;
        return report(cli, args, opened.then_some(host), &paths, false);
    }
    for path in &paths {
        let argv = file_argv(path)?;
        match host {
            OpenHost::Rex => super::workspace::present_file(args, &cwd, path, argv, placement)?,
            OpenHost::Herdr => super::workspace::present_herdr_file(&cwd, path, argv, placement)?,
            OpenHost::Tmux | OpenHost::Cmux => terminal_file(host, &cwd, &argv, placement)?,
            OpenHost::Auto | OpenHost::Desktop | OpenHost::Terminal => {
                unreachable!("resolved file host")
            }
        }
    }
    report(cli, args, Some(host), &paths, false)
}

fn file_argv(path: &Path) -> Result<Vec<String>> {
    // Files are already resolved on this Machine. Use the shared editor policy
    // directly, without asking another installed lf to interpret this build's CLI.
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
    paths: &[PathBuf],
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
