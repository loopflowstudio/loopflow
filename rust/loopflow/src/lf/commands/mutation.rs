//! Ordinary mutations shared by CLI dispatch and server requests.
use std::io::{Read, Write};
use std::path::Path;

use crate::lf::{Cli, Commands, TaskCommand, WaveCommand};
use anyhow::{Context, Result};

/// A command's streams belong to its caller, never to the server process.
pub struct CommandIo<'a> {
    pub input: &'a mut dyn Read,
    pub output: &'a mut dyn Write,
    pub error: &'a mut dyn Write,
}

impl std::fmt::Debug for CommandIo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandIo").finish_non_exhaustive()
    }
}

pub fn accepts(cli: &Cli) -> bool {
    match &cli.command {
        Some(Commands::Task { cmd }) => matches!(
            cmd,
            TaskCommand::Create {
                session: Some(_),
                run: false,
                ..
            } | TaskCommand::Claim { .. }
                | TaskCommand::Unassign { .. }
                | TaskCommand::Reopen { .. }
                | TaskCommand::Complete { .. }
                | TaskCommand::Save { .. }
                | TaskCommand::Abandon { .. }
                | TaskCommand::Update { .. }
                | TaskCommand::Move { .. }
                | TaskCommand::Hold { .. }
        ),
        Some(Commands::Wave { cmd }) => matches!(
            cmd,
            WaveCommand::Create { .. } | WaveCommand::Sync { json: true, .. }
        ),
        Some(Commands::Session { cmd }) => super::session::is_mutation(cmd),
        Some(Commands::Graph {
            cmd: crate::lf::GraphCommand::Customize { .. },
        }) => true,
        Some(Commands::Desktop { cmd }) => super::desktop::is_saved_mutation(cmd),
        _ => false,
    }
}

/// Routing, selection, defaults, effects and output for one admitted command.
/// The CLI owns its Process; a served request owns no execution lifetime.
pub fn run(cli: &Cli, args: &[String], directory: &Path, io: &mut CommandIo<'_>) -> Result<()> {
    let mut diagnostics = Vec::new();
    let result = crate::journal::request::diagnostics(&mut diagnostics, || {
        execute(cli, args, directory, io)
    });
    io.error.write_all(&diagnostics)?;
    result
}

fn execute(cli: &Cli, args: &[String], directory: &Path, io: &mut CommandIo<'_>) -> Result<()> {
    let _settings = crate::config::settings::select_invocation(cli.setting_values()?);
    if let Some(forward) = super::work_route::forward(cli, args, directory)? {
        return forward.run(cli, directory, io);
    }
    // Session, shipping, holding and arrangement select their own Work. Other
    // commands retain the CLI's explicit Task placement without changing
    // process cwd.
    let binding = if !matches!(
        cli.command,
        Some(
            Commands::Session { .. }
                | Commands::Ship { .. }
                | Commands::Desktop { .. }
                | Commands::Task {
                    cmd: TaskCommand::Hold { .. }
                }
        )
    ) {
        cli.task
            .as_deref()
            .map(|task| {
                let repo = selected_task_repository(cli, directory, Some(task))?;
                tokio::runtime::Runtime::new()?.block_on(async {
                    let store = crate::store::open_existing_store()
                        .await?
                        .context("planning registry unavailable")?;
                    crate::ops::resolve_work_binding(
                        &std::sync::Arc::new(store),
                        &repo,
                        &format!("task:{task}"),
                    )
                    .await
                    .map_err(anyhow::Error::from)
                })
            })
            .transpose()?
    } else {
        None
    };
    let directory = binding
        .as_ref()
        .map(|binding| cli.cwd.as_deref().unwrap_or(&binding.cwd))
        .unwrap_or(directory);
    let wave = cli.wave.as_deref().or_else(|| {
        binding
            .as_ref()
            .and_then(|binding| binding.wave_name.as_deref())
    });
    local(cli, directory, wave, io)
}

fn local(
    cli: &Cli,
    directory: &Path,
    selected_wave: Option<&str>,
    io: &mut CommandIo<'_>,
) -> Result<()> {
    match cli.command.as_ref().context("missing mutation")? {
        Commands::Task {
            cmd:
                TaskCommand::Hold {
                    issue,
                    reason,
                    json,
                },
        } => {
            let issue = issue
                .as_deref()
                .or(cli.task.as_deref())
                .context("No Task selected")?;
            let held = crate::engine::task::hold(directory, issue, reason.as_deref())?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&held)?)?;
            } else {
                writeln!(io.output, "{}", held.render())?;
            }
            Ok(())
        }
        Commands::Task { cmd } => {
            let repo = selected_task_repository(cli, directory, cmd.selector())?;
            task(&repo, cmd, io)
        }
        Commands::Wave {
            cmd: WaveCommand::Sync { wave, .. },
        } => {
            let wave = wave.as_deref().or(selected_wave);
            let refreshed = super::ops::refresh_status(directory, wave)?;
            writeln!(
                io.output,
                "{}",
                super::waves::status_json(
                    directory,
                    refreshed.as_deref().or(wave),
                    super::waves::Shape::Tasks
                )?
            )?;
            Ok(())
        }
        Commands::Wave { cmd } => wave(
            &crate::repo::require_repo_root(directory, "this command")?,
            cmd,
            io,
        ),
        Commands::Session { cmd } => super::session::mutate(cli, directory, cmd, io),
        Commands::Desktop {
            cmd: crate::lf::DesktopCommand::Portfolio { cmd, json },
        } => crate::portfolio::write(cmd.as_ref(), *json, directory, io),
        Commands::Graph {
            cmd: crate::lf::GraphCommand::Customize { name, json },
        } => {
            let customized = super::graph::customize(name, directory)?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&customized)?)?;
            } else {
                writeln!(io.output, "{}", customized.path.display())?;
            }
            Ok(())
        }
        Commands::Desktop { cmd } => {
            let saved =
                super::desktop::save_arrangement(cli.repository.as_ref(), directory, cmd, io.input)
                    .map_err(super::desktop::exit_status)?;
            writeln!(io.output, "{}", serde_json::to_string(&saved)?)?;
            Ok(())
        }
        _ => anyhow::bail!("not a shared mutation"),
    }
}

fn task(repo: &Path, command: &TaskCommand, io: &mut CommandIo<'_>) -> Result<()> {
    match command {
        TaskCommand::Create { .. } => super::task_create::create(repo, command, io).map(drop),
        TaskCommand::Claim { issue, json } | TaskCommand::Unassign { issue, json } => {
            use crate::ops::task_ownership::{render, OwnershipChange};
            let requested = match command {
                TaskCommand::Claim { .. } => OwnershipChange::Claim,
                _ => OwnershipChange::Unassign,
            };
            let ownership = crate::engine::task::ownership(repo, issue, requested)?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&ownership)?)?;
            } else {
                writeln!(io.output, "{}", render(&ownership))?;
            }
            Ok(())
        }
        TaskCommand::Reopen {
            issue,
            reason,
            json,
        } => {
            let task = crate::ops::task::task_reopen(issue, reason.as_deref())?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&task)?)?;
            } else {
                writeln!(io.output, "{issue}: reopened; its Graph unchanged")?;
            }
            Ok(())
        }
        TaskCommand::Complete {
            issue,
            reason,
            json,
        } => {
            let task = crate::ops::task::task_complete(repo, issue, reason.as_deref())?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&task)?)?;
            } else {
                writeln!(io.output, "{issue}: completed")?;
            }
            Ok(())
        }
        TaskCommand::Save {
            issue,
            path,
            revision,
            json,
        } => {
            let content = read_task_draft(io.input)?;
            let saved = crate::ops::task::task_save(issue, path, revision, &content)?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&saved)?)?;
            } else {
                writeln!(io.output, "{}", saved.message)?;
            }
            Ok(())
        }
        TaskCommand::Abandon { issue, force, json } => {
            let identifier = crate::ops::task::task_abandon(repo, issue.as_deref(), *force)?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&identifier)?)?;
            } else {
                writeln!(
                    io.output,
                    "{identifier}: canceled; PRs and branches removed"
                )?;
            }
            Ok(())
        }
        TaskCommand::Update {
            issue,
            title,
            notes,
            rank,
            assignee,
            wave,
            json,
        } => {
            let result = crate::ops::task::task_edit(
                repo,
                issue,
                wave.as_deref(),
                task_edit_update(title, notes, *rank, assignee),
            )?;
            print_edit(&result, *json, "updated", io)
        }
        TaskCommand::Move {
            issue,
            machine: Some(machine),
            json,
            ..
        } => {
            let ownership = crate::lf::commands::session_move::move_task(issue, machine)?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&ownership)?)?;
            }
            Ok(())
        }
        TaskCommand::Move {
            issue, wave, json, ..
        } => {
            let result = crate::ops::task::task_move_membership(repo, issue, wave.as_deref())?;
            print_edit(&result, *json, "membership saved", io)
        }
        _ => unreachable!("accepts selects Task mutations"),
    }
}

fn wave(repo: &Path, command: &WaveCommand, io: &mut CommandIo<'_>) -> Result<()> {
    match command {
        WaveCommand::Create {
            wave,
            objective,
            json,
        } => {
            let added = tokio::runtime::Runtime::new()?.block_on(
                crate::ops::wave_lifecycle::add(repo, wave, objective.as_deref()),
            )?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&added)?)?;
            } else {
                writeln!(
                    io.output,
                    "{}",
                    crate::ops::wave_lifecycle::render_added(&added)
                )?;
            }
            Ok(())
        }
        _ => unreachable!("accepts selects Wave mutations"),
    }
}

pub fn task_edit_update(
    title: &Option<String>,
    notes: &Option<String>,
    rank: Option<u32>,
    assignee: &Option<String>,
) -> crate::work::task::TaskUpdate {
    crate::work::task::TaskUpdate {
        title: title.clone(),
        description: notes.clone(),
        rank,
        assignee: assignee.clone().map(Some),
    }
}

pub fn selected_task_repository(
    cli: &Cli,
    directory: &Path,
    selector: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    let repo = crate::ops::task::task_repository(directory, selector)?;
    if cli.repo.is_some() {
        anyhow::ensure!(
            crate::repository::CanonicalRepo::discover(directory)?
                == crate::repository::CanonicalRepo::discover(&repo)?,
            "Task {} does not belong to selected repository {}",
            selector.unwrap_or("in this checkout"),
            directory.display()
        );
    }
    Ok(repo)
}

/// `--json` prints the edit; text prints `<id>: <done>` and its sync on stderr.
fn print_edit(
    edit: &crate::ops::task::TaskEdit,
    json: bool,
    done: &str,
    io: &mut CommandIo<'_>,
) -> Result<()> {
    if json {
        writeln!(io.output, "{}", serde_json::to_string_pretty(edit)?)?;
        return Ok(());
    }
    writeln!(io.output, "{}: {done}", edit.id)?;
    for line in edit.sync.lines() {
        writeln!(io.error, "{line}")?;
    }
    Ok(())
}

pub fn read_task_draft(input: &mut dyn Read) -> Result<String> {
    let limit = crate::ops::task::MAX_FILE_BYTES;
    let mut bytes = Vec::new();
    input.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "Draft exceeds 1 MB");
    Ok(String::from_utf8(bytes)?)
}
