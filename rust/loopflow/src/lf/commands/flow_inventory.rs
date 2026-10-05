use std::num::NonZeroU32;

use anyhow::{Context, Result};
use clap::Args;

use crate::durable::FlowFilter;
use crate::repository::CanonicalRepo;
use crate::session::FlowSummaryState;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, Default, Args)]
pub struct FlowInventoryArgs {
    /// List or show saved FlowSessions instead of reusable templates
    #[arg(long)]
    pub sessions: bool,
    /// Include every repository and flows with unknown repository evidence
    #[arg(long)]
    pub all: bool,
    /// FlowSession page size (default 100)
    #[arg(long)]
    pub limit: Option<NonZeroU32>,
    /// Previous page's next identity; retain the same filters
    #[arg(long)]
    pub after: Option<String>,
    /// Literal name or identity containment
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long, value_parser = ["current", "completed", "replaced"])]
    pub state: Option<String>,
    /// Retained Task ID or issue identifier, including completed Tasks
    #[arg(long, conflicts_with = "taskless")]
    pub for_task: Option<String>,
    /// Retained Wave ID or name
    #[arg(long)]
    pub for_wave: Option<String>,
    #[arg(long)]
    pub taskless: bool,
    /// Whether the Task currently selects this FlowSession
    #[arg(long, action = clap::ArgAction::Set)]
    pub managed: Option<bool>,
}

impl FlowInventoryArgs {
    pub fn is_empty(&self) -> bool {
        !self.sessions
            && !self.all
            && self.limit.is_none()
            && self.after.is_none()
            && self.search.is_none()
            && self.state.is_none()
            && self.for_task.is_none()
            && self.for_wave.is_none()
            && !self.taskless
            && self.managed.is_none()
    }
}

pub fn list(args: &FlowInventoryArgs, json: bool) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        let repo = if args.all {
            None
        } else {
            CanonicalRepo::current()?.map(|repo| repo.to_string())
        };
        let filter = FlowFilter {
            task_id: args
                .for_task
                .as_deref()
                .map(|task| {
                    store
                        .sqlite
                        .resolve_task_id(task, repo.as_deref())
                        .map_err(anyhow::Error::from)?
                        .context("Task was not found")
                })
                .transpose()?,
            wave_id: args
                .for_wave
                .as_deref()
                .map(|wave| {
                    store
                        .sqlite
                        .resolve_wave_id(wave, repo.as_deref())
                        .map_err(anyhow::Error::from)?
                        .context("Wave was not found")
                })
                .transpose()?,
            state: args.state.as_deref().map(|state| match state {
                "completed" => FlowSummaryState::Completed,
                "replaced" => FlowSummaryState::Replaced,
                _ => FlowSummaryState::Current,
            }),
            repo,
            taskless: args.taskless,
            managed: args.managed,
            search: args.search.clone(),
        };
        let page = store
            .flow_inventory(
                &filter,
                args.after.as_deref(),
                args.limit
                    .unwrap_or(NonZeroU32::new(100).expect("100 is nonzero")),
            )
            .await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&page)?);
        } else {
            for entry in page.entries {
                println!(
                    "{}  {:?}  {}{}",
                    entry.summary.id,
                    entry.summary.state,
                    entry.summary.name.as_deref().unwrap_or("unknown template"),
                    if entry.managed { "  [managed]" } else { "" }
                );
            }
            if let Some(next) = page.next {
                println!("Continue with --after '{}' and the same filters", next);
            }
        }
        Ok(())
    })
}

pub fn inspect(selector: &str, json: bool) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        let detail = store
            .flow_detail(selector)
            .await?
            .context("FlowSession was not found")?;
        if !json {
            println!("Flow invocation {}", detail.entry.summary.id);
        }
        println!("{}", serde_json::to_string_pretty(&detail)?);
        Ok(())
    })
}
