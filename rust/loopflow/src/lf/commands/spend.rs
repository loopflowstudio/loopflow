//! Dependency, access and billing operations. Inspections never fetch credentials.
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
    /// Export only the designated repository/Wave amounts for a restricted consumer
    Export {
        #[arg(long)]
        period: String,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        wave: Option<crate::id::WaveId>,
        #[arg(short, long)]
        output: PathBuf,
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
    /// Consume a designated export in isolation, without opening the administrative Store
    Consume {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        wave: Option<crate::id::WaveId>,
        #[arg(long)]
        period: String,
    },
    /// Record a candidate; no provider credentials are created or revoked
    Rotate {
        credential: String,
        #[arg(long)]
        replacement: String,
        #[arg(long)]
        consumer_inventory_evidence: Option<String>,
    },
    /// Inspect or advance an administrative rotation using explicit evidence
    Rotation {
        #[command(subcommand)]
        cmd: RotationCommand,
    },
    Show {
        environment: String,
        #[arg(long)]
        json: bool,
    },
    Verify {
        environment: String,
        #[arg(long)]
        export: Option<PathBuf>,
        #[arg(long)]
        period: String,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum RotationCommand {
    /// Refresh changed consumers or candidate metadata; requires fresh receipts
    Reconcile {
        id: String,
        #[arg(long)]
        consumer_inventory_evidence: Option<String>,
    },
    /// Discard a candidate before activation; does not revoke its provider key
    Cancel {
        id: String,
    },
    Show {
        id: String,
    },
    /// Switch the active reference after every required candidate read succeeds
    Activate {
        id: String,
    },
    /// Record non-secret provider/operator evidence; this does not execute revocation
    Receipt {
        id: String,
        file: PathBuf,
    },
}
#[derive(Debug, Subcommand)]
pub enum InventoryCommand {
    Import { file: PathBuf },
}
#[derive(Debug, Subcommand)]
pub enum DependencyCommand {
    /// Inspect effective intervals of recorded dependency relationships
    History {
        id: String,
        #[arg(long)]
        json: bool,
    },
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
    /// Import a finalized daily/monthly AWS CUR assembly with an invoice control total
    ImportAwsCur {
        source: String,
        #[arg(long)]
        period: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
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
    if let AuthCommand::Access {
        cmd:
            AccessCommand::Consume {
                file,
                repo,
                wave,
                period,
            },
    } = command
    {
        let (report, _) = crate::spend::consumer::consume(
            file,
            &crate::spend::Consumer {
                repo: repo.clone(),
                wave_id: wave.clone(),
            },
            period,
        )
        .await?;
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    let config = storage_config_from_env()?;
    match command {
        AuthCommand::Access {
            cmd: AccessCommand::Consume { .. },
        } => unreachable!("handled without Store access"),
        AuthCommand::Access {
            cmd:
                AccessCommand::Rotate {
                    credential,
                    replacement,
                    consumer_inventory_evidence,
                },
        } => {
            let rotation = open_store(&config)
                .await?
                .begin_spend_rotation(
                    crate::spend::CredentialId(credential.clone()),
                    crate::spend::CredentialId(replacement.clone()),
                    consumer_inventory_evidence.clone(),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&rotation)?);
        }
        AuthCommand::Access {
            cmd: AccessCommand::Rotation { cmd },
        } => {
            let rotation = match cmd {
                RotationCommand::Reconcile {
                    id,
                    consumer_inventory_evidence,
                } => {
                    open_store(&config)
                        .await?
                        .reconcile_spend_rotation(
                            crate::spend::rotation::RotationId(id.clone()),
                            consumer_inventory_evidence.clone(),
                        )
                        .await?
                }
                RotationCommand::Cancel { id } => {
                    open_store(&config)
                        .await?
                        .cancel_spend_rotation(crate::spend::rotation::RotationId(id.clone()))
                        .await?
                }
                RotationCommand::Show { id } => open_read_only_store(&config)
                    .await?
                    .spend_rotation(crate::spend::rotation::RotationId(id.clone()))
                    .await?
                    .context("rotation not found")?,
                RotationCommand::Activate { id } => {
                    open_store(&config)
                        .await?
                        .activate_spend_rotation(crate::spend::rotation::RotationId(id.clone()))
                        .await?
                }
                RotationCommand::Receipt { id, file } => {
                    open_store(&config)
                        .await?
                        .record_spend_rotation(
                            crate::spend::rotation::RotationId(id.clone()),
                            read(file)?,
                        )
                        .await?
                }
            };
            println!("{}", serde_json::to_string_pretty(&rotation)?);
        }
        AuthCommand::Access {
            cmd: AccessCommand::Show { environment, json },
        } => {
            let inspection = open_read_only_store(&config)
                .await?
                .spend_access(crate::spend::EnvironmentId(environment.clone()))
                .await?
                .context("access environment not found")?;
            print_access_output(&inspection, *json)?;
        }
        AuthCommand::Access {
            cmd:
                AccessCommand::Verify {
                    environment,
                    export,
                    period,
                    json,
                },
        } => {
            let inspection = open_store(&config)
                .await?
                .verify_spend_access(
                    crate::spend::EnvironmentId(environment.clone()),
                    period.clone(),
                    export.clone(),
                )
                .await?;
            print_access_output(&inspection, *json)?;
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
                cmd @ (SourceCommand::ImportAwsCur {
                    source,
                    period,
                    file,
                    json,
                }
                | SourceCommand::Import {
                    source,
                    period,
                    file,
                    json,
                }),
        } => {
            let store = open_store(&config).await?;
            let result: Result<_> = async {
                match cmd {
                    SourceCommand::ImportAwsCur { .. } => {
                        let mut export: crate::spend::aws_cur::AwsCurExport = read(file)?;
                        let parent = file.parent().context("export descriptor has no parent")?;
                        export.manifest = parent.join(export.manifest);
                        export.directory = parent.join(export.directory);
                        Ok(store
                            .import_spend_aws_cur(
                                export,
                                crate::spend::SourceId(source.clone()),
                                period.clone(),
                            )
                            .await?)
                    }
                    SourceCommand::Import { .. } => {
                        let invoice: Invoice = read(file)?;
                        if invoice.source.0 != *source || invoice.period != *period {
                            bail!("export source or period does not match the requested import");
                        }
                        Ok(store.import_spend_invoice(invoice).await?)
                    }
                }
            }
            .await;
            // Record every completed attempt, including parse errors, without raw error text.
            store
                .record_spend_import(crate::spend::ImportObservation {
                    source: crate::spend::SourceId(source.clone()),
                    period: period.clone(),
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    succeeded: result.is_ok(),
                })
                .await
                .context("could not record import outcome")?;
            let revision = result?;
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
        AuthCommand::Export {
            period,
            repo,
            wave,
            output,
        } => {
            let report = open_read_only_store(&config)
                .await?
                .spend_report(period.clone(), None, None)
                .await?;
            let export = report.export(repo.clone(), wave.clone())?;
            fs::write(output, serde_json::to_vec_pretty(&export)?)
                .context("could not write designated report export")?;
            println!("Exported designated report");
        }
        AuthCommand::Dependency {
            cmd: DependencyCommand::History { id, json },
        } => {
            let history = open_read_only_store(&config)
                .await?
                .spend_dependency_history(DependencyId(id.clone()))
                .await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                for version in history {
                    println!(
                        "{} · {} until {} · {}",
                        version.dependency.name,
                        version.effective_from,
                        version.effective_to.as_deref().unwrap_or("open"),
                        version.provenance
                    );
                    println!("{}", serde_json::to_string_pretty(&version)?);
                }
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
                for usage in &report.session_usage {
                    println!(
                        "Session {} · input {} · estimated USD {:?} · evidence gaps {}",
                        usage.session_id,
                        usage.artifact_key.as_deref().unwrap_or("unknown"),
                        usage.usage.cost_usd,
                        usage.evidence_gaps
                    );
                }
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
                for access in &dependency.access {
                    print_access(access);
                }
                if let Some(estimate) = &dependency.dependency.recurring_estimate {
                    println!(
                        "Estimate: {} {} / {} · {}",
                        estimate.amount.0, estimate.currency, estimate.cadence, estimate.provenance
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

fn print_access_output(inspection: &crate::spend::AccessInspection, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(inspection)?);
    } else {
        print_access(inspection);
    }
    Ok(())
}

fn print_access(inspection: &crate::spend::AccessInspection) {
    println!("Environment: {}", inspection.environment.name);
    for requirement in &inspection.requirements {
        println!(
            "Access: {} · {} · permissions {}",
            requirement.id.0,
            requirement.purpose,
            requirement.permissions.join(", ")
        );
        if let Some(consumer) = &requirement.report_consumer {
            println!(
                "Designated recipient: {} · Wave {:?}",
                consumer.repo, consumer.wave_id
            );
        }
        match inspection.current_observation(requirement) {
            Some(observation) => println!(
                "Current read: {:?} · {} · scope evidence {:?}",
                observation.outcome, observation.observed_at, observation.scope_evidence
            ),
            None => println!("Current read: unverified"),
        }
    }
    for credential in &inspection.credentials {
        println!(
            "Credential: {} · {}/{}/{} · version {:?} · expires {:?} · rotation {:?}",
            credential.id.0,
            credential.reference.project,
            credential.reference.config,
            credential.reference.name,
            credential.version,
            credential.expires_at,
            credential.rotation_evidence
        );
    }
    for observation in &inspection.observations {
        if let Some(receipt) = &observation.report_receipt {
            println!(
                "Container receipt: {} · {} · {} · SHA256 {}",
                receipt.invocation, receipt.consumer.repo, receipt.period, receipt.export_sha256
            );
        }
        println!(
            "History: {} · {:?} · {} · Home {} · {:?}",
            observation.requirement.0,
            observation.outcome,
            observation.observed_at,
            observation.executed_home,
            observation.gap
        );
    }
    for gap in &inspection.coverage {
        println!("Gap: {gap}");
    }
}
