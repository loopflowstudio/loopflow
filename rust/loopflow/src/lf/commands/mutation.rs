//! Ordinary mutations shared by CLI dispatch and server requests.
use loopflow_execution::command::CommandIo;
use std::io::Read;
use std::path::Path;

use crate::lf::{Cli, Commands, TaskCommand, WaveCommand};
use anyhow::{Context, Result};

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
                | TaskCommand::Auto {
                    cmd: crate::lf::AutoCommand::Off { .. }
                }
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
    let result = loopflow_execution::journal::request::diagnostics(&mut diagnostics, || {
        execute(cli, args, directory, io)
    });
    io.error.write_all(&diagnostics)?;
    result
}

fn execute(cli: &Cli, args: &[String], directory: &Path, io: &mut CommandIo<'_>) -> Result<()> {
    let _settings = loopflow_execution::config::settings::select_invocation(cli.setting_values()?);
    if let Some(forward) = super::work_route::forward(cli, args, directory)? {
        return forward.run(cli, directory, io);
    }
    // Session, auto and arrangement select their own Work. Other
    // commands retain the CLI's explicit Task placement without changing
    // process cwd.
    let binding = if !matches!(
        cli.command,
        Some(
            Commands::Session { .. }
                | Commands::Desktop { .. }
                | Commands::Task {
                    cmd: TaskCommand::Auto { .. }
                }
        )
    ) {
        cli.task
            .as_deref()
            .map(|task| {
                let repo = selected_task_repository(cli, directory, Some(task))?;
                loopflow_execution::runtime::block_on(async {
                    let store = loopflow_execution::store::open_existing_store()
                        .await?
                        .context("planning registry unavailable")?;
                    loopflow_execution::ops::resolve_work_binding(
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
                TaskCommand::Auto {
                    cmd:
                        crate::lf::AutoCommand::Off {
                            issue,
                            reason,
                            json,
                        },
                },
        } => {
            let issue = issue
                .as_deref()
                .or(cli.task.as_deref())
                .context("No Task selected")?;
            // A Task another Machine runs is set there.
            let super::write::WriteResult::TaskAutoOff(off) = super::write::Write::TaskAutoOff {
                directory: directory.to_path_buf(),
                issue: issue.to_owned(),
                reason: reason.clone(),
            }
            .run()?
            else {
                unreachable!("task.auto.off answers with what it turned off")
            };
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&off)?)?;
            } else {
                writeln!(io.output, "{}", off.render())?;
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
                super::waves::status_json(directory, refreshed.as_deref().or(wave), false)?
            )?;
            Ok(())
        }
        Commands::Wave { cmd } => wave(
            &loopflow_execution::repo::require_repo_root(directory, "this command")?,
            cmd,
            io,
        ),
        Commands::Session { cmd } => super::session::mutate(cli, directory, cmd, io),
        Commands::Desktop {
            cmd: crate::lf::DesktopCommand::Portfolio { cmd, json },
        } => super::desktop::write_portfolio(cmd.as_ref(), *json, directory, io),
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
        TaskCommand::Create { .. } => super::task_create::create(repo, command, io, false, None),
        TaskCommand::Claim { issue, json } | TaskCommand::Unassign { issue, json } => {
            use super::write::{Write, WriteResult};
            use loopflow_execution::ops::task_ownership::render;
            let (directory, issue) = (repo.to_path_buf(), issue.clone());
            let write = match command {
                TaskCommand::Claim { .. } => Write::TaskClaim { directory, issue },
                _ => Write::TaskUnassign { directory, issue },
            };
            // A Task whose ownership another Machine writes is changed there.
            let (WriteResult::TaskClaim(ownership) | WriteResult::TaskUnassign(ownership)) =
                write.run()?
            else {
                unreachable!("an ownership write answers with ownership")
            };
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
            let task = loopflow_execution::ops::task::task_reopen(issue, reason.as_deref())?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&task)?)?;
            } else {
                writeln!(io.output, "{issue}: reopened; its Graph unchanged")?;
            }
            Ok(())
        }
        TaskCommand::Complete {
            issue,
            children,
            reason,
            json,
        } => {
            let completion = loopflow_execution::ops::task::task_complete(
                repo,
                issue,
                reason.as_deref(),
                *children,
            )?;
            if *json {
                let completed = super::waves::task_completed(completion)?;
                writeln!(io.output, "{}", serde_json::to_string_pretty(&completed)?)?;
            } else {
                writeln!(io.output, "{} done", completion.task.identifier)?;
                let destination = completion
                    .rehomed_to
                    .as_ref()
                    .map(|task| task.identifier.as_str());
                write_child_completions(io.output, &completion.children, destination, 1)?;
            }
            Ok(())
        }
        TaskCommand::Save {
            issue,
            path,
            revision,
            json,
        } => {
            // Saved on the Machine that runs the Task.
            let saved = match (super::write::Write::TaskSave {
                directory: repo.to_owned(),
                issue: issue.clone(),
                path: path.clone(),
                revision: revision.clone(),
                content: read_task_draft(io.input)?,
            })
            .run()?
            {
                super::write::WriteResult::TaskSave(saved) => *saved,
                _ => unreachable!("task.save answers a save"),
            };
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&saved)?)?;
            } else {
                writeln!(io.output, "{}", saved.message)?;
            }
            Ok(())
        }
        TaskCommand::Abandon { issue, force, json } => {
            let identifier =
                loopflow_execution::ops::task::task_abandon(repo, issue.as_deref(), *force)?;
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
            parent,
            wave,
            json,
        } => {
            let result = loopflow_execution::ops::task::task_edit(
                repo,
                issue,
                wave.as_deref(),
                task_edit_update(title, notes, *rank, assignee),
                parent.as_deref(),
            )?;
            print_edit(&result, *json, "updated", io)
        }
        TaskCommand::Move {
            issue,
            machine: Some(machine),
            json,
            ..
        } => {
            let ownership = loopflow_execution::ops::session_move::move_task(
                issue,
                machine,
                None,
                super::write::run_task,
            )?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string(&ownership)?)?;
            }
            Ok(())
        }
        TaskCommand::Move {
            issue, wave, json, ..
        } => {
            let result =
                loopflow_execution::ops::task::task_move_membership(repo, issue, wave.as_deref())?;
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
            let added = loopflow_execution::runtime::block_on(
                loopflow_execution::ops::wave_lifecycle::add(repo, wave, objective.as_deref()),
            )?;
            if *json {
                writeln!(io.output, "{}", serde_json::to_string_pretty(&added)?)?;
            } else {
                writeln!(
                    io.output,
                    "{}",
                    loopflow_execution::ops::wave_lifecycle::render_added(&added)
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
) -> loopflow_storage::work::task::TaskUpdate {
    loopflow_storage::work::task::TaskUpdate {
        title: title.clone(),
        description: notes.clone(),
        rank,
        assignee: assignee.clone().map(Some),
        parent: None,
    }
}

pub fn selected_task_repository(
    cli: &Cli,
    directory: &Path,
    selector: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    let repo = loopflow_execution::ops::task::task_repository(directory, selector)?;
    if cli.repo.is_some() {
        anyhow::ensure!(
            loopflow_execution::repository::discover(directory)?
                == loopflow_execution::repository::discover(&repo)?,
            "Task {} does not belong to selected repository {}",
            selector.unwrap_or("in this checkout"),
            directory.display()
        );
    }
    Ok(repo)
}

/// `--json` prints the edit; text prints `<id>: <done>` and its sync on stderr.
fn print_edit(
    edit: &loopflow_execution::ops::task::TaskEdit,
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
    let limit = loopflow_execution::ops::task::MAX_FILE_BYTES;
    let mut bytes = Vec::new();
    input.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "Draft exceeds 1 MB");
    Ok(String::from_utf8(bytes)?)
}

/// One line per child, each child's own children indented under it.
fn write_child_completions(
    output: &mut dyn std::io::Write,
    children: &[loopflow_storage::sqlite::children::ChildCompletion],
    destination: Option<&str>,
    depth: usize,
) -> Result<()> {
    for child in children {
        writeln!(
            output,
            "{}{}",
            "  ".repeat(depth),
            child_completion_line(child, destination)
        )?;
        write_child_completions(output, &child.children, destination, depth + 1)?;
    }
    Ok(())
}

fn child_completion_line(
    child: &loopflow_storage::sqlite::children::ChildCompletion,
    destination: Option<&str>,
) -> String {
    use loopflow_storage::sqlite::children::ChildOutcome;

    let id = &child.task.identifier;
    let pr = child.pr.as_ref().and_then(|pr| {
        pr.github()
            .map(|github| format!("#{} {}", github.number, pr.phase().as_str()))
    });
    match child.outcome {
        ChildOutcome::Done => {
            format!("{id} done: {}", pr.as_deref().unwrap_or("PR merged"))
        }
        ChildOutcome::Rehomed => match destination {
            Some(destination) => format!("{id} moved up to {destination}"),
            None => format!("{id} moved to top level"),
        },
        ChildOutcome::Abandoned => {
            let step = child.step.as_ref().map(|step| format!("at {step}"));
            let reasons: Vec<String> = pr.into_iter().chain(step).collect();
            if reasons.is_empty() {
                format!("{id} abandoned: no merged PR")
            } else {
                format!("{id} abandoned: {}", reasons.join(", "))
            }
        }
    }
}
