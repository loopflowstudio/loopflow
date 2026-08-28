use std::path::Path;

use clap::Parser;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::engine::flow::Op;
use crate::engine::git::get_default_branch;
use crate::lf::{Cli, Commands, PrCommand, ReleaseCommand};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::{
    abandon_branch, arm, commit_workflow, create_or_update_pr, rebase_with_recovery, release_bump,
    release_check, release_notes, release_publish, release_run, release_status, release_tag,
    submit, AbandonOptions, CommitOptions, LandOptions, PrOptions, RebaseOptions,
};

pub fn execute_flow_ops(repo: &Path, item: &Op, progress: &impl Progress) -> OpsResult<()> {
    let mut argv = vec!["lf".to_string(), item.command.clone()];
    argv.extend(item.args.iter().cloned());

    let cli = Cli::try_parse_from(argv)
        .map_err(|err| OpsError::Message(format!("invalid op item: {err}")))?;

    match cli.command {
        Some(Commands::Pr { cmd: Some(pr) }) => execute_pr(repo, pr, progress),
        Some(Commands::Rebase {
            plan,
            manual,
            continue_rebase,
            abort,
            adopt,
            onto,
        }) => {
            if manual || continue_rebase || abort || adopt {
                return Err(OpsError::Message(
                    "manual rebase recovery is only available from the CLI".to_string(),
                ));
            }
            if plan {
                return Ok(());
            }
            let base = get_default_branch(repo)?;
            let onto_ref = onto.unwrap_or_else(|| format!("origin/{base}"));
            rebase_with_recovery(
                repo,
                &RebaseOptions {
                    onto: onto_ref,
                    push: true,
                    fork_base: None,
                },
                progress,
            )?;
            Ok(())
        }
        Some(Commands::Commit {
            message,
            push,
            no_add,
        }) => {
            crate::ops::task::guard_task_mutation(repo)?;
            commit_workflow(
                repo,
                &CommitOptions {
                    add: !no_add,
                    push,
                    create_draft_pr: true,
                    message,
                    ..CommitOptions::for_task("commit")
                },
                progress,
            )?;
            Ok(())
        }
        Some(Commands::Release { cmd }) => execute_release(repo, cmd, progress),
        Some(Commands::Doctor { json }) => crate::lf::commands::doctor::run(json)
            .map_err(|error| OpsError::Message(error.to_string())),
        Some(Commands::TelemetryScorecard { json }) => run_telemetry_scorecard(repo, json),
        _ => Err(unsupported()),
    }
}

fn run_telemetry_scorecard(repo: &Path, json: bool) -> OpsResult<()> {
    let script = repo.join("scripts/lifecycle_scorecard.py");
    if !script.is_file() {
        return Err(OpsError::Message(format!(
            "telemetry scorecard generator not found: {}",
            script.display()
        )));
    }
    let database = crate::store::database_path_from_env()
        .map_err(|error| OpsError::Message(format!("resolve telemetry database: {error}")))?;
    let mut command = std::process::Command::new("python3");
    command
        .arg(script)
        .arg("--repo")
        .arg(repo)
        .arg("--database")
        .arg(database)
        .arg("--envelope");
    let output = command
        .output()
        .map_err(|error| OpsError::Message(format!("launch telemetry scorecard: {error}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(OpsError::Message(format!(
            "telemetry scorecard exited with {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        )));
    }
    let envelope: TelemetryScorecardEnvelope = serde_json::from_slice(&output.stdout)
        .map_err(|error| OpsError::Message(format!("decode telemetry scorecard: {error}")))?;
    for result in persist_metric_observations(repo, envelope.metric_observations)? {
        eprintln!("metric observation · {result}");
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope.report)
                .expect("scorecard report JSON always re-serializes")
        );
    } else {
        print!("{}", envelope.text);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct TelemetryScorecardEnvelope {
    report: serde_json::Value,
    metric_observations: Vec<crate::ops::metrics::MetricProducerObservation>,
    text: String,
}

fn persist_metric_observations(
    repo: &Path,
    observations: Vec<crate::ops::metrics::MetricProducerObservation>,
) -> OpsResult<Vec<String>> {
    if observations.is_empty() {
        return Ok(Vec::new());
    }
    let repo = repo.to_path_buf();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .map_err(|error| OpsError::Message(format!("start metric writer: {error}")))?
            .block_on(async move {
                let Some(store) = crate::store::open_existing_store().await else {
                    return Ok(vec![
                        "no local Loopflow registry; observation was not persisted".to_string(),
                    ]);
                };
                crate::ops::metrics::publish_metric_observations(
                    &store,
                    &repo,
                    observations,
                    OffsetDateTime::now_utc(),
                )
                .await
                .map_err(|error| OpsError::Message(format!("publish metric observation: {error}")))
            })
    })
    .join()
    .map_err(|_| OpsError::Message("metric writer thread panicked".to_string()))?
}

fn execute_pr(repo: &Path, cmd: PrCommand, progress: &impl Progress) -> OpsResult<()> {
    match cmd {
        PrCommand::Arm {
            strict,
            local,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        } => {
            arm(
                repo,
                &LandOptions {
                    strict,
                    local,
                    create_pr: true,
                    complete,
                    next_slug: next,
                    worktree,
                    commit_message: message,
                    pr_title: title,
                    pr_body: body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        PrCommand::Land {
            strict,
            local,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        } => {
            let options = LandOptions {
                strict,
                local,
                create_pr: true,
                complete,
                next_slug: next,
                worktree,
                commit_message: message,
                pr_title: title,
                pr_body: body,
                agent: None,
            };
            let Some(pr) = arm(repo, &options, progress)? else {
                return Ok(());
            };
            crate::ops::pr_landing::watch_armed_pr(repo, &options, pr, progress)?;
            Ok(())
        }
        PrCommand::Submit {
            strict,
            create_pr,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        } => {
            submit(
                repo,
                &LandOptions {
                    strict,
                    local: false,
                    create_pr,
                    complete,
                    next_slug: next,
                    worktree,
                    commit_message: message,
                    pr_title: title,
                    pr_body: body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        // A flow `op:` runs headless, so both publish and open only publish —
        // presentation is a human-initiated CLI concern, never an automation step.
        PrCommand::Publish {
            model: _,
            title,
            body,
        }
        | PrCommand::Open {
            model: _,
            title,
            body,
        } => {
            create_or_update_pr(
                repo,
                &PrOptions {
                    title,
                    body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        PrCommand::Abandon { force, branch } => {
            abandon_branch(repo, &AbandonOptions { branch, force }, progress)?;
            Ok(())
        }
        PrCommand::Next { slug } => {
            crate::ops::task::pr_next(repo, slug.as_deref())?;
            Ok(())
        }
        PrCommand::Status => Err(unsupported()),
    }
}

fn execute_release(repo: &Path, cmd: ReleaseCommand, progress: &impl Progress) -> OpsResult<()> {
    match cmd {
        ReleaseCommand::Run { version, target } => {
            release_run(
                repo,
                version.as_deref().unwrap_or("patch"),
                target.as_deref(),
                progress,
            )?;
            Ok(())
        }
        ReleaseCommand::Check { target } => {
            release_check(repo, target.as_deref())?;
            Ok(())
        }
        ReleaseCommand::Notes {
            version,
            prev_tag,
            target,
        } => {
            release_notes(
                repo,
                &version,
                prev_tag.as_deref(),
                target.as_deref(),
                progress,
            )?;
            Ok(())
        }
        ReleaseCommand::Bump { version, target } => {
            release_bump(repo, &version, target.as_deref(), progress)
        }
        ReleaseCommand::Tag { version, target } => {
            release_tag(repo, &version, target.as_deref())?;
            Ok(())
        }
        ReleaseCommand::Publish {
            tag,
            notes,
            assets,
            finalize,
        } => release_publish(repo, &tag, notes.as_deref(), &assets, finalize),
        ReleaseCommand::Status { target } => {
            release_status(repo, target.as_deref())?;
            Ok(())
        }
    }
}

/// Flow `op:` items drive the mechanical verbs only; anything that launches an
/// agent, reads interactively, or manages waves has no place in a flow step.
fn unsupported() -> OpsError {
    OpsError::Message(
        "op item must be one of pr open, pr submit, pr arm, pr land, pr abandon, rebase, commit, release, doctor, or the internal telemetry scorecard"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::wave::metrics::MetricEvidenceDto;
    use crate::id::WaveId;
    use crate::ops::NullProgress;
    use crate::pm::{PmKr, PmProject, ProjectFlowPlan};
    use crate::store::{open_store, storage_config_from_env};
    use crate::work::wave::Wave;

    #[test]
    fn authored_flow_cannot_dispatch_evidence_receipt_command() {
        let item = Op {
            command: "receipt".to_string(),
            args: vec!["show".to_string(), "chat_turn:turn-3".to_string()],
        };

        let error = execute_flow_ops(Path::new("."), &item, &NullProgress)
            .expect_err("removed evidence command must not dispatch");
        assert!(error.to_string().contains("op item must be one of"));
    }

    #[test]
    fn telemetry_flow_op_runs_internal_scorecard() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let repo = tempfile::tempdir().expect("temp repo");
        let scripts = repo.path().join("scripts");
        std::fs::create_dir(&scripts).expect("create scripts directory");
        std::fs::write(
            scripts.join("lifecycle_scorecard.py"),
            r#"import json
import pathlib
import sys

repo = pathlib.Path(sys.argv[2])
repo.joinpath("scorecard-ran").write_text("envelope" if "--envelope" in sys.argv else "direct")
print(json.dumps({"report": {"ok": True}, "metric_observations": [], "text": "scorecard text\n"}))
"#,
        )
        .expect("write scorecard fixture");
        let item = Op {
            command: "__telemetry-scorecard".to_string(),
            args: vec!["--json".to_string()],
        };

        execute_flow_ops(repo.path(), &item, &NullProgress).expect("run telemetry scorecard");

        assert_eq!(
            std::fs::read_to_string(repo.path().join("scorecard-ran"))
                .expect("read scorecard receipt"),
            "envelope"
        );
    }

    #[test]
    fn telemetry_envelope_accepts_older_producer_annotations() {
        let envelope: TelemetryScorecardEnvelope = serde_json::from_str(
            r#"{
                "report":{"schema_version":1},
                "metric_observations":[{
                    "wave":"product",
                    "metric_id":"task-loop-trust",
                    "instrument":"lifecycle-scorecard",
                    "kind":"observed",
                    "value":0.5,
                    "source_window_start":"2026-08-14T09:00:00Z",
                    "source_window_end":"2026-08-21T09:00:00Z",
                    "complete":true,
                    "eligible":4,
                    "successful":2
                }],
                "text":"Lifecycle scorecard"
            }"#,
        )
        .unwrap();

        assert_eq!(envelope.metric_observations.len(), 1);
    }

    #[test]
    fn telemetry_flow_persists_the_portfolio_reading() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let repo = tempfile::tempdir().expect("temp repo");
        let scripts = repo.path().join("scripts");
        let metrics = repo.path().join("wave/product/metrics");
        std::fs::create_dir(&scripts).expect("create scripts directory");
        std::fs::create_dir_all(&metrics).expect("create metrics directory");
        std::fs::write(
            metrics.join("task-loop-trust.md"),
            "---\nschema: 1\nid: task-loop-trust\nproject_id: project-api\nstage: installed\ninstrument: lifecycle-scorecard\nunit: ratio\ntarget:\n  at_least: 1\nwindow: 7d\nfreshness: 30h\n---\n\n# Task loops earn trust\n\nCount settled Task loops.\n",
        )
        .expect("write metric contract");
        std::fs::write(
            scripts.join("lifecycle_scorecard.py"),
            r#"import json
from datetime import datetime, timedelta, timezone

end = datetime.now(timezone.utc)
start = end - timedelta(days=7)
print(json.dumps({
    "report": {"ok": True},
    "metric_observations": [{
        "wave": "product",
        "metric_id": "task-loop-trust",
        "instrument": "lifecycle-scorecard",
        "kind": "observed",
        "value": 1.0,
        "source_window_start": start.isoformat(),
        "source_window_end": end.isoformat(),
        "complete": True
    }],
    "text": "scorecard text\n"
}))
"#,
        )
        .expect("write scorecard fixture");
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.path().display().to_string(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime
            .block_on(open_store(&storage_config_from_env().unwrap()))
            .unwrap();
        store
            .apply_migration_for_test("project_metric_observations")
            .unwrap();
        runtime.block_on(store.create_wave(&wave)).unwrap();
        drop(store);
        drop(runtime);

        execute_flow_ops(
            repo.path(),
            &Op {
                command: "__telemetry-scorecard".to_string(),
                args: Vec::new(),
            },
            &NullProgress,
        )
        .expect("publish telemetry metric");

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = runtime
            .block_on(open_store(&storage_config_from_env().unwrap()))
            .unwrap();
        let projects = vec![PmProject {
            id: "project-api".to_string(),
            slug: "loopflow-api".to_string(),
            name: "Loopflow API".to_string(),
            summary: String::new(),
            definition: "Make Task loops trustworthy.".to_string(),
            flows: Some(ProjectFlowPlan::empty()),
            krs: vec![PmKr {
                text: "Task loops settle without repair.".to_string(),
                holds: false,
            }],
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: vec!["team-1".to_string()],
        }];
        let portfolio = runtime
            .block_on(crate::ops::metrics::wave_metric_portfolio(
                &store,
                &wave,
                &projects,
                OffsetDateTime::now_utc(),
            ))
            .unwrap();
        let project_portfolio = runtime
            .block_on(crate::ops::metrics::project_metric_portfolio(
                &store,
                &wave,
                &projects,
                "project-api",
                OffsetDateTime::now_utc(),
            ))
            .unwrap();
        let prompt = crate::ops::metrics::metric_prompt_section(
            "project-owned-metrics",
            Ok(project_portfolio),
        );

        assert!(portfolio.metrics[0].instrumented);
        assert!(matches!(
            portfolio.metrics[0].evidence,
            MetricEvidenceDto::Met { value: 1.0, .. }
        ));
        assert!(prompt.contains("\"kind\":\"met\",\"value\":1.0"));
    }
}
