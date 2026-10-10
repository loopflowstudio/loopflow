//! Inspect memory and scratch size targets without restricting launch input.

use anyhow::Result;
use serde::Serialize;

use crate::engine::config::load_config;
use crate::engine::context_budget::{measure_context, ContextBudgetReport, ContextBudgets};
use crate::engine::prompt::{gather_documents, GatherSpec};

#[derive(Debug, Serialize)]
struct ContextReport {
    checkout: std::path::PathBuf,
    wave: Option<String>,
    task: Option<String>,
    context: ContextBudgetReport,
}

pub fn run(json: bool, wave: Option<&str>, task: Option<&str>) -> Result<()> {
    let mut repo = crate::repo::require_repo_root(&std::env::current_dir()?, "lf context")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let binding = runtime.block_on(async {
        let Some(store) = crate::store::open_existing_store().await else {
            anyhow::ensure!(task.is_none(), "Task usage unavailable: no local registry");
            return Ok::<_, anyhow::Error>(None);
        };
        let store = std::sync::Arc::new(store);
        if let Some(task) = task {
            Ok(Some(
                crate::ops::resolve_work_binding(&store, &repo, &format!("task:{task}")).await?,
            ))
        } else if wave.is_none() {
            Ok(crate::ops::resolve_execution_binding(&store, &repo).await?)
        } else {
            Ok(None)
        }
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
    let config = load_config(Some(&repo))?.unwrap_or_default();
    let budgets = ContextBudgets::resolve(&config, &repo, wave.as_deref())?;
    // Size targets concern authored memory and scratch, not a prospective launch.
    let docs = gather_documents(&GatherSpec {
        repo_root: repo.clone(),
        wave: wave.clone(),
        ..Default::default()
    })?;
    let report = ContextReport {
        checkout: repo,
        wave,
        task,
        context: measure_context(&docs, budgets),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Checkout: {}\nWave: {}\nTask: {}\n{}",
            report.checkout.display(),
            report.wave.as_deref().unwrap_or("none"),
            report.task.as_deref().unwrap_or("none"),
            report.context.render()
        );
    }
    Ok(())
}

/// A hook emits only the provider's JSON envelope; no launch or store mutation.
pub fn emit_block(
    delivery: &std::path::Path,
    moment: crate::engine::context_block::ContextMoment,
) -> Result<()> {
    let delivery: crate::engine::context_block::ContextDelivery =
        serde_json::from_slice(&std::fs::read(delivery)?)?;
    let block = delivery.block(moment)?;
    println!(
        "{}",
        serde_json::json!({"hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": block.text,
        }})
    );
    Ok(())
}
