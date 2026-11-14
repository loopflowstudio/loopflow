//! Present files in an attached Desktop Files view. Never launch a host or a conversation.
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

use super::{OpenArgs, OpenHost};

#[derive(Debug, Serialize)]
struct FilePresentation<'a> {
    host: Option<OpenHost>,
    paths: &'a [PathBuf],
    opening: bool,
}

pub(super) fn run(cli: &crate::lf::Cli, args: &OpenArgs, explain: bool) -> Result<()> {
    anyhow::ensure!(
        matches!(args.selection.host, OpenHost::Auto | OpenHost::Desktop),
        "File presentation uses an attached Desktop Files view; no host was opened"
    );
    let target = match super::target(cli, &args.selection) {
        Ok(target) => target,
        Err(_)
            if args.selection.host == OpenHost::Auto
                && args.selection.url.is_none()
                && cli.task.is_none()
                && cli.wave.is_none()
                && !cli.selects_repository()
                && crate::repo::discover_repo_root(&std::env::current_dir()?)?.is_none() =>
        {
            return report(cli, args, None, &args.files, explain)
        }
        Err(error) => return Err(error),
    };
    let cwd = match &target.task {
        Some(task) => task.worktree()?.clone(),
        None if args.selection.url.is_none() => {
            let directory = std::env::current_dir()?;
            crate::repo::discover_repo_root(&directory)?.unwrap_or(directory)
        }
        None => target.cwd().to_path_buf(),
    };
    // Desktop reads remote paths through its existing Files transport.
    let remote = target.execution_machine.is_some();
    anyhow::ensure!(
        remote || target.execution_unavailable.is_none(),
        "Present files on the Task's recorded Machine; nothing opened"
    );
    // Canonicalize local paths; remote paths belong to Desktop's Files transport.
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
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    let relative = paths
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .context("Desktop Files target is outside this checkout")
        })
        .collect::<Result<Vec<_>>>()?;
    if explain {
        return report(cli, args, Some(OpenHost::Desktop), &paths, true);
    }
    let context = super::work_context(
        &cwd,
        target.task.as_ref().map(|task| task.identifier.as_str()),
        target.wave.as_deref(),
        None,
    )?;
    let opened = super::super::desktop::present_files(&context, &relative)?;
    report(
        cli,
        args,
        opened.then_some(OpenHost::Desktop),
        &paths,
        false,
    )
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
    if cli.json || args.selection.json {
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
        println!("No Desktop Files view attached; nothing opened.");
        for path in paths {
            println!("{}", path.display());
        }
    }
    Ok(())
}
