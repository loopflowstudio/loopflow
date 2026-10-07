//! The internal telemetry scorecard command.
use std::path::Path;
use std::process::Stdio;

use serde::Deserialize;
use time::OffsetDateTime;

use crate::engine::process::ProcessGroupGuard;
use crate::ops::error::{OpsError, OpsResult};

pub fn run_telemetry_scorecard(repo: &Path, json: bool) -> OpsResult<()> {
    let script = repo.join("scripts/lifecycle_scorecard.py");
    if !script.is_file() {
        return Err(OpsError::Message(format!(
            "telemetry scorecard generator not found: {}",
            script.display()
        )));
    }
    let database = crate::store::database_path_from_env()
        .map_err(|error| OpsError::Message(format!("resolve telemetry database: {error}")))?;
    // Earlier and unfinished Sessions can contain the first attempt on a PR merged
    // inside the window. Python windows Session statistics and PR intervals separately.
    let history = crate::lf::commands::session_history::collect_history(
        crate::lf::commands::WorkFilter {
            wave: None,
            project: None,
            task: None,
        },
        None,
        0,
    )
    .map_err(|error| OpsError::Message(format!("read telemetry Session history: {error}")))?;
    let mut history_input = tempfile::NamedTempFile::new()
        .map_err(|error| OpsError::Message(format!("create telemetry input: {error}")))?;
    serde_json::to_writer(history_input.as_file_mut(), &history).map_err(|error| {
        OpsError::Message(format!("serialize telemetry Session history: {error}"))
    })?;
    let mut command = std::process::Command::new("python3");
    command
        .arg(script)
        .arg("--repo")
        .arg(repo)
        .arg("--database")
        .arg(database)
        .arg("--history")
        .arg(history_input.path())
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

#[cfg(test)]
mod tests {

    use time::OffsetDateTime;

    use super::{run_telemetry_scorecard, TelemetryScorecardEnvelope};
    use crate::engine::stream::StreamEvent;
    use crate::id::WaveId;
    use crate::session_record::{CaptureHandle, SessionCaptureSpec, SessionFlowMembership};
    use crate::store::{open_store, storage_config_from_env};
    use crate::work::wave::metrics::MetricEvidenceDto;
    use crate::work::wave::Wave;

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
        let store = crate::store::sqlite::SqliteStore::open_processes_read_only(
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
runs = json.loads(pathlib.Path(sys.argv[sys.argv.index("--history") + 1]).read_text())
repo.joinpath("scorecard-ran").write_text(json.dumps(runs))
print(json.dumps({"report": {"ok": True}, "metric_observations": [], "text": "scorecard text\n"}))
"#,
        )
        .expect("write scorecard fixture");
        run_telemetry_scorecard(repo.path(), true).expect("run telemetry scorecard");

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

        run_telemetry_scorecard(repo.path(), false).expect("publish telemetry metric");

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
