//! Offline dependency and billing inspections. Only explicit imports write the Store.
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde::de::DeserializeOwned;

use crate::spend::{DependencyId, Inventory, Invoice};
use crate::store::{open_read_only_store, open_store, storage_config_from_env};

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Inspect or explicitly verify environment access
    Access {
        #[command(subcommand)]
        cmd: AccessCommand,
    },
    /// Import non-secret dependency metadata
    Inventory {
        #[command(subcommand)]
        cmd: InventoryCommand,
    },
    /// Inspect dependency consumers, access and linked billed evidence
    Dependency {
        #[command(subcommand)]
        cmd: DependencyCommand,
    },
    /// Import a complete normalized billing export
    Source {
        #[command(subcommand)]
        cmd: SourceCommand,
    },
    /// Report billed evidence without fetching credentials or contacting providers
    Report {
        #[arg(long)]
        period: String,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        wave: Option<crate::id::WaveId>,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum AccessCommand {
    Show {
        environment: String,
        #[arg(long)]
        json: bool,
    },
    Verify {
        environment: String,
        #[arg(long)]
        period: String,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum InventoryCommand {
    Import { file: PathBuf },
}
#[derive(Debug, Subcommand)]
pub enum DependencyCommand {
    Show {
        id: String,
        #[arg(long)]
        period: String,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    Import {
        source: String,
        #[arg(long)]
        period: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
fn read<T: DeserializeOwned>(path: &PathBuf) -> Result<T> {
    // Do not echo input or serde errors, which can contain a mistakenly supplied value.
    let contents = fs::read(path).context("could not read import document")?;
    serde_json::from_slice(&contents).map_err(|_| {
        anyhow::anyhow!(
            "invalid import document; expected non-secret inventory or normalized invoice JSON"
        )
    })
}
pub fn run(command: &AuthCommand) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(command))
}
async fn run_async(command: &AuthCommand) -> Result<()> {
    let config = storage_config_from_env()?;
    match command {
        AuthCommand::Access { cmd } => {
            let (inspection, json) = match cmd {
                AccessCommand::Show { environment, json } => (
                    open_read_only_store(&config)
                        .await?
                        .spend_access(crate::spend::EnvironmentId(environment.clone()))
                        .await?
                        .context("access environment not found")?,
                    json,
                ),
                AccessCommand::Verify {
                    environment,
                    period,
                    json,
                } => (
                    open_store(&config)
                        .await?
                        .verify_spend_access(
                            crate::spend::EnvironmentId(environment.clone()),
                            period.clone(),
                        )
                        .await?,
                    json,
                ),
            };
            if *json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("{}", inspection.environment.name);
                for requirement in inspection.requirements {
                    println!(
                        "{} · {} · permissions {}",
                        requirement.id.0,
                        requirement.purpose,
                        requirement.permissions.join(", ")
                    );
                }
                for observation in inspection.observations {
                    println!(
                        "{} · {:?} · {} · {:?}",
                        observation.requirement.0,
                        observation.outcome,
                        observation.observed_at,
                        observation.gap
                    );
                }
                for gap in inspection.coverage {
                    println!("Gap: {gap}");
                }
            }
        }

        AuthCommand::Inventory {
            cmd: InventoryCommand::Import { file },
        } => {
            let inventory: Inventory = read(file)?;
            open_store(&config)
                .await?
                .import_spend_inventory(inventory)
                .await?;
            println!("Imported dependency inventory");
        }
        AuthCommand::Source {
            cmd:
                SourceCommand::Import {
                    source,
                    period,
                    file,
                    json,
                },
        } => {
            let invoice: Invoice = read(file)?;
            if invoice.source.0 != *source || invoice.period != *period {
                bail!("export source or period does not match the requested import");
            }
            let revision = open_store(&config)
                .await?
                .import_spend_invoice(invoice)
                .await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&revision)?);
            } else {
                println!(
                    "{} {} {} · revision {} · difference {}",
                    revision.invoice.document_id,
                    revision.billed.0,
                    revision.invoice.currency,
                    revision.revision,
                    revision.difference.0
                );
            }
        }
        AuthCommand::Report {
            period,
            repo,
            wave,
            json,
        } => {
            let report = open_read_only_store(&config)
                .await?
                .spend_report(period.clone(), repo.clone(), wave.clone())
                .await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Billed evidence · {period}");
                for total in &report.totals {
                    println!(
                        "{} {} · direct {} · allocated {} · shared {} · unassigned {}",
                        total.currency,
                        total.billed.0,
                        total.direct.0,
                        total.allocated.0,
                        total.shared.0,
                        total.unassigned.0
                    );
                }
                for invoice in &report.invoices {
                    println!(
                        "Source {} / {} · revision {} · fetched {} · generated {:?}",
                        invoice.invoice.source.0,
                        invoice.invoice.document_id,
                        invoice.revision,
                        invoice.invoice.fetched_at,
                        invoice.invoice.generated_at
                    );
                }
                for gap in report.coverage {
                    println!("Gap: {gap}");
                }
            }
        }
        AuthCommand::Dependency {
            cmd: DependencyCommand::Show { id, period, json },
        } => {
            let dependency = open_read_only_store(&config)
                .await?
                .spend_dependency(DependencyId(id.clone()), period.clone())
                .await?
                .context("dependency not found")?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&dependency)?);
            } else {
                println!(
                    "{} · {}",
                    dependency.dependency.name, dependency.dependency.id.0
                );
                for consumer in &dependency.dependency.consumers {
                    println!("Consumer: {} · Wave {:?}", consumer.repo, consumer.wave_id);
                }
                for requirement in &dependency.requirements {
                    println!(
                        "Access: {} · {} · {} · permissions {} · unverified",
                        requirement.environment.0,
                        requirement.id.0,
                        requirement.purpose,
                        requirement.permissions.join(", ")
                    );
                }
                if let Some(totals) = dependency.attributed_cost {
                    for total in totals {
                        println!("Attributed: {} {}", total.billed.0, total.currency);
                    }
                } else {
                    println!("Attributed cost: unknown");
                }
                for invoice in &dependency.linked_invoices {
                    println!(
                        "Linked account evidence: {} · {} {} · {}",
                        invoice.invoice.account.0,
                        invoice.billed.0,
                        invoice.invoice.currency,
                        invoice.invoice.document_id
                    );
                }
                for gap in dependency.coverage {
                    println!("Gap: {gap}");
                }
            }
        }
    }
    Ok(())
}
