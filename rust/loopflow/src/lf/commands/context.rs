//! Inspect the same budgets and local source snapshot used at launch.

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::engine::config::load_config;
use crate::engine::context_budget::ContextBudgetReport;
use crate::engine::process_prompt::{preview_process_prompt, ProcessPromptInput};
use crate::engine::prompt::Surface;

#[derive(Debug, Serialize)]
struct ContextReport {
    checkout: std::path::PathBuf,
    wave: Option<String>,
    task: Option<String>,
    skill: String,
    goal_status: String,
    context: ContextBudgetReport,
}

pub fn run(json: bool, wave: Option<&str>, task: Option<&str>, skill: &str) -> Result<()> {
    let mut repo = crate::repo::require_repo_root(&std::env::current_dir()?, "lf context")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let (binding, message) = runtime.block_on(async {
        let Some(store) = crate::store::open_existing_store().await else {
            anyhow::ensure!(task.is_none(), "Task usage unavailable: no local registry");
            return Ok::<_, anyhow::Error>((None, None));
        };
        let store = std::sync::Arc::new(store);
        let binding = if let Some(task) = task {
            Some(crate::ops::resolve_work_binding(&store, &repo, &format!("task:{task}")).await?)
        } else if wave.is_none() {
            crate::ops::resolve_execution_binding(&store, &repo).await?
        } else {
            None
        };
        let message = if let Some(binding) = &binding {
            if let crate::durable::WorkRef::Task(id) = &binding.work {
                let task = store
                    .get_task(id)
                    .await?
                    .ok_or_else(|| anyhow!("Task {id} is missing"))?;
                Some(
                    crate::ops::task_input::read_seed(&store, &task, &binding.wave_name, 0)
                        .await?
                        .message,
                )
            } else {
                Some(binding.context.clone())
            }
        } else {
            None
        };
        Ok((binding, message))
    })?;
    if let Some(binding) = &binding {
        repo = binding.cwd.clone();
    }
    let wave = wave
        .map(str::to_owned)
        .or_else(|| binding.as_ref().map(|binding| binding.wave_name.clone()))
        .or_else(crate::work::wave::context::resolve_ambient_wave_name);
    let task = binding.as_ref().and_then(|binding| match &binding.work {
        crate::durable::WorkRef::Task(id) => Some(id.to_string()),
        _ => None,
    });
    let goal_status = if message.is_some() {
        "local Work seed, including all stored steers"
    } else {
        "no Work seed selected; arbitrary launch messages are not included"
    }
    .to_string();
    let config = load_config(Some(&repo))?.unwrap_or_default();
    let prepared = preview_process_prompt(
        &config,
        ProcessPromptInput {
            repo_root: repo.clone(),
            wave: wave.clone(),
            message,
            skill: Some(skill.to_owned()),
            surface: Surface::Headless,
            // A query never reads the desktop clipboard.
            source_overrides: crate::engine::process_prompt::ContextSourceOverrides {
                clipboard: Some(false),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let report = ContextReport {
        checkout: repo,
        wave,
        task,
        skill: skill.into(),
        goal_status,
        context: prepared.budget_report,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Checkout: {}\nWave: {}\nTask: {}\nSkill: {}\nGoal: {}\n{}",
            report.checkout.display(),
            report.wave.as_deref().unwrap_or("none"),
            report.task.as_deref().unwrap_or("none"),
            report.skill,
            report.goal_status,
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
    anyhow::ensure!(
        (wave.is_some() || task.is_some()) as usize
            + session.is_some() as usize
            + process.is_some() as usize
            <= 1,
        "select Work, a Session, or a Process, not several independent targets"
    );
    let cwd = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(async {
        use crate::ops::context::ContextExplanation;
        match crate::store::read_existing_registry() {
            Ok(Some(store)) => {
                crate::ops::context::explain_context(
                    &std::sync::Arc::new(store),
                    &cwd,
                    crate::ops::WorkSelection { task, wave },
                    session,
                    process,
                )
                .await
            }
            Ok(None) => ContextExplanation::unavailable("Local registry is absent"),
            Err(error) => {
                ContextExplanation::unavailable(format!("Local registry unavailable: {error}"))
            }
        }
    }))
}
