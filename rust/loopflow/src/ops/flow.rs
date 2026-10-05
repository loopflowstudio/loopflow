use std::path::Path;
use std::process::Stdio;

use clap::Parser;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::engine::flow::Command as FlowCommand;
use crate::engine::process::ProcessGroupGuard;
use crate::lf::{Cli, Commands, PrCommand, ReleaseCommand, RepoCommand, SyncArgs};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::{
    abandon_branch, arm, commit_workflow, create_or_update_pr, release_bump, release_check,
    release_notes, release_publish, release_status, release_tag, submit, AbandonOptions,
    CommitOptions, LandOptions, PrOptions,
};

pub fn execute_flow_command(
    repo: &Path,
    item: &FlowCommand,
    progress: &impl Progress,
) -> OpsResult<Option<crate::pr_landing::PrLandingId>> {
    execute_flow_command_with_cron(repo, item, progress, None)
}

pub(crate) fn execute_flow_command_with_cron(
    repo: &Path,
    item: &FlowCommand,
    progress: &impl Progress,
    cron_receipt: Option<&crate::ops::cron::accounting::CronExecution>,
) -> OpsResult<Option<crate::pr_landing::PrLandingId>> {
    let argv = crate::lf::navigation::normalize_args(item.argv())
        .map_err(|err| OpsError::Message(format!("invalid cmd item: {err}")))?;
    let cli = Cli::try_parse_from(argv)
        .map_err(|err| OpsError::Message(format!("invalid cmd item: {err}")))?;

    let result = match cli.command {
        Some(Commands::Pr { cmd: Some(pr) }) => return execute_pr(repo, pr, progress),
        Some(Commands::Sync(SyncArgs {
            plan,
            manual,
            continue_sync,
            abort,
            adopt,
            onto,
        })) => {
            if manual || continue_sync || abort || adopt {
                return Err(OpsError::Message(
                    "manual sync recovery is only available from the CLI".to_string(),
                ));
            }
            crate::lf::commands::ops::run_sync_in(
                repo,
                onto.as_deref(),
                plan,
                false,
                false,
                false,
                false,
            )
            .map_err(|error| OpsError::Message(error.to_string()))
        }

        Some(Commands::Commit {
            message,
            no_add,
            paths,
        }) => {
            crate::ops::task::guard_task_mutation(repo)?;
            if !paths.is_empty() {
                crate::ops::commit::commit_selected(repo, &paths, message.as_deref())?;
                return Ok(None);
            }
            commit_workflow(
                repo,
                &CommitOptions {
                    add: !no_add,
                    message,
                    ..CommitOptions::for_task("commit")
                },
                progress,
                &|_| {},
            )?;
            Ok(())
        }
        Some(Commands::Repo {
            cmd: RepoCommand::Release { cmd },
        }) => execute_release(repo, cmd, progress, cron_receipt),
        Some(Commands::Home {
            cmd:
                crate::lf::HomeCommand::Doctor {
                    json,
                    planning: false,
                },
        }) => crate::lf::commands::doctor::run(json)
            .map_err(|error| OpsError::Message(error.to_string())),
        Some(Commands::TelemetryScorecard { json }) => run_telemetry_scorecard(repo, json),
        Some(Commands::Monitor {
            cmd:
                Some(
                    cmd @ crate::lf::commands::monitor::MonitorCommand::Usage {
                        weekly: true, ..
                    },
                ),
            ..
        }) => crate::lf::commands::monitor::run(&cmd)
            .map_err(|error| OpsError::Message(error.to_string())),
        _ => Err(unsupported()),
    };
    result.map(|()| None)
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
    // Earlier and unfinished Runs can contain the first attempt on a PR merged
    // inside the window. Python windows Run statistics and PR intervals separately.
    let runs = crate::lf::commands::session_history::collect_history(
        crate::lf::commands::WorkFilter {
            wave: None,
            project: None,
            task: None,
        },
        None,
        0,
    )
    .map_err(|error| OpsError::Message(format!("read telemetry Runs: {error}")))?;
    let mut run_input = tempfile::NamedTempFile::new()
        .map_err(|error| OpsError::Message(format!("create telemetry input: {error}")))?;
    serde_json::to_writer(run_input.as_file_mut(), &runs)
        .map_err(|error| OpsError::Message(format!("serialize telemetry Runs: {error}")))?;
    let mut command = std::process::Command::new("python3");
    command
        .arg(script)
        .arg("--repo")
        .arg(repo)
        .arg("--database")
        .arg(database)
        .arg("--runs")
        .arg(run_input.path())
        .arg("--envelope");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|error| OpsError::Message(format!("launch telemetry scorecard: {error}")))?;
    let process_group = ProcessGroupGuard::new(child.id());
    let output = child
        .wait_with_output()
        .map_err(|error| OpsError::Message(format!("wait for telemetry scorecard: {error}")))?;
    process_group.disarm();
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

fn execute_pr(
    repo: &Path,
    cmd: PrCommand,
    progress: &impl Progress,
) -> OpsResult<Option<crate::pr_landing::PrLandingId>> {
    let draft = matches!(&cmd, PrCommand::Open { .. });
    let result = match cmd {
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
                return Ok(None);
            };
            let landing = crate::ops::pr_landing::record_armed_pr(repo, &options, &pr)?;
            return Ok(Some(landing.id));
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
        // Headless operations preserve the command's draft/ready policy but
        // leave browser presentation to an explicitly requested CLI action.
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
                    draft,
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
        PrCommand::Checks { .. } | PrCommand::Reconcile => Err(unsupported()),
    };
    result.map(|()| None)
}

fn execute_release(
    repo: &Path,
    cmd: ReleaseCommand,
    progress: &impl Progress,
    cron_receipt: Option<&crate::ops::cron::accounting::CronExecution>,
) -> OpsResult<()> {
    match cmd {
        ReleaseCommand::History { .. } => Err(OpsError::Message(
            "release history is a read-only CLI operation".into(),
        )),
        ReleaseCommand::Run { version, target } => {
            if cron_receipt.is_none() {
                crate::ops::release_run(
                    repo,
                    version.as_deref().unwrap_or("patch"),
                    target.as_deref(),
                    progress,
                )?;
                return Ok(());
            }
            crate::ops::release::release_run_with_cron(
                repo,
                version.as_deref().unwrap_or("patch"),
                target.as_deref(),
                progress,
                cron_receipt,
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
            preview,
            target,
        } => {
            let generate = if preview {
                crate::ops::preview_release_notes
            } else {
                release_notes
            };
            let notes = generate(
                repo,
                &version,
                prev_tag.as_deref(),
                target.as_deref(),
                progress,
            )?;
            if preview {
                println!("{notes}");
            }
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

/// Flow `cmd:` items drive the mechanical verbs only; anything that launches an
/// agent, reads interactively, or manages waves has no place in a flow step.
fn unsupported() -> OpsError {
    OpsError::Message(
        "cmd item must be one of pr open, pr submit, pr arm, pr land, pr abandon, sync, commit, release, doctor, or the internal telemetry scorecard"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use time::OffsetDateTime;

    use super::{execute_flow_command, TelemetryScorecardEnvelope};
    use crate::engine::flow::Command as FlowCommand;
    use crate::engine::stream::StreamEvent;
    use crate::id::WaveId;
    use crate::ops::NullProgress;
    use crate::session_record::{CaptureHandle, SessionCaptureSpec, SessionFlowMembership};
    use crate::store::{open_store, storage_config_from_env};
    use crate::work::wave::metrics::MetricEvidenceDto;
    use crate::work::wave::Wave;

    #[test]
    fn authored_flow_cannot_dispatch_evidence_receipt_command() {
        let item = FlowCommand {
            command: "receipt".to_string(),
            args: vec!["show".to_string(), "chat_turn:turn-3".to_string()],
        };

        let error = execute_flow_command(Path::new("."), &item, &NullProgress)
            .expect_err("removed evidence command must not dispatch");
        assert!(error.to_string().contains("cmd item must be one of"));
    }

    #[test]
    fn telemetry_flow_op_runs_internal_scorecard() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let repo = tempfile::tempdir().expect("temp repo");
        let capture = CaptureHandle::begin_at(
            ledger.home(),
            SessionCaptureSpec {
                harness: "codex".to_string(),
                model: None,
                surface: "headless".to_string(),
                cwd: repo.path().to_path_buf(),
                repo: Some(repo.path().to_path_buf()),
                worktree: Some(repo.path().to_path_buf()),
                skill: Some("implement".to_string()),
                subjects: Vec::new(),
                work: None,
                flow: SessionFlowMembership::Independent,
            },
        )
        .unwrap();
        capture.record_stream_event(&StreamEvent::Usage {
            input_tokens: Some(12),
            output_tokens: None,
            cache_read_tokens: None,
        });
        capture.finish("completed").unwrap();
        let store = crate::store::sqlite::SqliteStore::open_execs_read_only(
            &ledger.home().join("loopflow.db"),
        )
        .unwrap();
        store.assert_no_historical_runs();
        std::fs::remove_dir_all(capture.artifact_dir()).unwrap();
        let scripts = repo.path().join("scripts");
        std::fs::create_dir(&scripts).expect("create scripts directory");
        std::fs::write(
            scripts.join("lifecycle_scorecard.py"),
            r#"import json
import pathlib
import sys

repo = pathlib.Path(sys.argv[2])
runs = json.loads(pathlib.Path(sys.argv[sys.argv.index("--runs") + 1]).read_text())
repo.joinpath("scorecard-ran").write_text(json.dumps(runs))
print(json.dumps({"report": {"ok": True}, "metric_observations": [], "text": "scorecard text\n"}))
"#,
        )
        .expect("write scorecard fixture");
        let item = FlowCommand {
            command: "__telemetry-scorecard".to_string(),
            args: vec!["--json".to_string()],
        };

        execute_flow_command(repo.path(), &item, &NullProgress).expect("run telemetry scorecard");

        let runs: Vec<crate::session_record::SessionHistory> = serde_json::from_str(
            &std::fs::read_to_string(repo.path().join("scorecard-ran")).unwrap(),
        )
        .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].usage.input_tokens, Some(12));
        assert_eq!(runs[0].usage.cost_usd, None);
        assert_eq!(runs[0].usage.final_streams, 0);
        assert_eq!(runs[0].recorded_outcome.as_deref(), Some("completed"));
    }

    #[test]
    fn telemetry_envelope_decodes_project_metric_observation() {
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
            "---\nschema: 1\nid: task-loop-trust\nstage: installed\ninstrument: lifecycle-scorecard\nunit: ratio\nwindow: 7d\nfreshness: 30h\n---\n\n# Task loops earn trust\n\nCount settled Task loops.\n",
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
        "complete": True,
        "eligible": 1,
        "successful": 1
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

        execute_flow_command(
            repo.path(),
            &FlowCommand {
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
        let portfolio = runtime
            .block_on(crate::ops::metrics::wave_metric_portfolio(
                &store,
                &wave,
                OffsetDateTime::now_utc(),
            ))
            .unwrap();
        assert!(portfolio.metrics[0].instrumented);
        assert!(matches!(
            portfolio.metrics[0].evidence,
            MetricEvidenceDto::Unknown {
                cause: crate::work::wave::metrics::MetricUnknownCauseDto::TargetUnavailable {
                    value: 1.0,
                    ..
                }
            }
        ));
        let prompt = crate::ops::metrics::metric_prompt_section("wave-metrics", Ok(portfolio));
        assert!(prompt.contains("\"kind\":\"target_unavailable\",\"value\":1.0"));
    }
}
