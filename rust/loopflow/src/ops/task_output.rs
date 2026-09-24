//! Passive Task output over Run journals and provider-owned native history.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

use crate::run_record::output::{gap, read_source, resolve_native, tail_cursor, OutputCursor};
use crate::run_record::{read_provider_session, RunManifest};
use crate::store::SharedStore;
use crate::work::task::flow_history::{TaskFlowEvent, TaskFlowStage};
use crate::work::task::{Task, TaskEventKind};

pub use crate::run_record::output::{OutputGap, OutputRecord, OutputSource};

pub const MAX_CURSOR_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputSource {
    pub run_id: String,
    pub provider: String,
    pub stage: Option<TaskFlowStage>,
    pub records: Vec<OutputRecord>,
    pub source: OutputSource,
    pub available: bool,
    pub has_more: bool,
    pub reset: bool,
    pub gaps: Vec<OutputGap>,
}

/// Sources are in observation order; each source's records retain source order.
/// Labels and availability accompany even an empty or unavailable source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputPage {
    pub task_id: String,
    pub sources: Vec<TaskOutputSource>,
    pub gaps: Vec<OutputGap>,
    pub next_cursor: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    version: u32,
    task_id: String,
    sources: BTreeMap<String, OutputCursor>,
    next_run: usize,
}

/// Each request observes a bounded round-robin group of sources. Discovery reads
/// immutable manifests only; it never reduces entire transcripts as `lf runs` does.
pub async fn read_task_output(
    store: &SharedStore,
    task: &Task,
    home: &Path,
    cursor: Option<&str>,
    tail: bool,
) -> Result<TaskOutputPage> {
    if tail && cursor.is_some() {
        return Err(anyhow!("Start live output without a continuation cursor"));
    }
    let mut cursor = match cursor {
        Some(encoded) => {
            if encoded.len() > MAX_CURSOR_BYTES {
                return Err(anyhow!("Task output cursor is too large"));
            }
            let decoded: Cursor = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded)?)?;
            if decoded.version != 3 || decoded.task_id != task.id.as_str() {
                return Err(anyhow!(
                    "Task output cursor belongs to another Task or version; reload output"
                ));
            }
            decoded
        }
        None => Cursor {
            version: 3,
            task_id: task.id.to_string(),
            sources: BTreeMap::new(),
            next_run: 0,
        },
    };
    let bindings = store
        .task_events_after(&task.id, 0)
        .await?
        .into_iter()
        .filter_map(|event| match event.kind {
            TaskEventKind::Flow { event } => match *event {
                TaskFlowEvent::RunBound { stage, run_id } => Some((run_id.to_string(), stage)),
                _ => None,
            },
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let discovery = crate::run_record::task_run_manifests(home, task)?;
    let runs = discovery.runs;
    let mut page = TaskOutputPage {
        task_id: task.id.to_string(),
        sources: Vec::new(),
        gaps: discovery.gaps,
        next_cursor: String::new(),
    };
    if tail {
        // Seed the whole discovered inventory once. A later round-robin visit
        // must not skip output written after this initial request. New Runs
        // have no cursor and are read from their beginning.
        for (dir, manifest) in &runs {
            let source = output_source(manifest);
            match resolve_source(store, dir, manifest, source)
                .await
                .and_then(|(path, session)| {
                    tail_cursor(&path, source, session.as_deref()).map_err(Into::into)
                }) {
                Ok(position) => {
                    cursor.sources.insert(manifest.run_id.to_string(), position);
                }
                Err(error) => page.gaps.push(gap(
                    "tail_unavailable",
                    format!(
                        "{}: {error}; retained output will be read from the beginning",
                        manifest.run_id
                    ),
                )),
            }
        }
    }
    let total = runs.len();
    let start = if total == 0 {
        0
    } else {
        cursor.next_run % total
    };
    let count = total.min(8);
    for index in 0..count {
        let (dir, manifest) = &runs[(start + index) % total];
        let run_id = manifest.run_id.to_string();
        let source = output_source(manifest);
        let resolved = resolve_source(store, dir, manifest, source).await;
        let result = resolved.and_then(|(path, session)| {
            read_source(
                &path,
                source,
                session.as_deref(),
                cursor.sources.get(&run_id),
            )
            .map_err(Into::into)
        });
        match result {
            Ok(source_page) => {
                cursor.sources.insert(run_id.clone(), source_page.cursor);
                page.sources.push(TaskOutputSource {
                    provider: manifest.harness.clone(),
                    stage: bindings.get(&run_id).cloned(),
                    records: source_page.records,
                    run_id,
                    source,
                    available: true,
                    has_more: source_page.has_more,
                    reset: source_page.reset,
                    gaps: source_page.gaps,
                });
            }
            Err(error) => page.sources.push(TaskOutputSource {
                provider: manifest.harness.clone(),
                stage: bindings.get(&run_id).cloned(),
                records: Vec::new(),
                run_id,
                source,
                available: false,
                has_more: false,
                reset: false,
                gaps: vec![gap("source_unavailable", error.to_string())],
            }),
        }
    }
    cursor.next_run = if total == 0 {
        0
    } else {
        (start + count) % total
    };
    page.next_cursor = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor)?);
    if page.next_cursor.len() > MAX_CURSOR_BYTES {
        return Err(anyhow!(
            "Task output continuation exceeds the cursor size limit"
        ));
    }
    Ok(page)
}

async fn resolve_source(
    store: &SharedStore,
    dir: &Path,
    manifest: &RunManifest,
    source: OutputSource,
) -> Result<(std::path::PathBuf, Option<String>)> {
    if source == OutputSource::Journal {
        if manifest.surface == "tui" {
            return Err(anyhow!("Native output is unsupported for this provider"));
        }
        return Ok((dir.join("events.jsonl"), None));
    }
    if !dir.join("provider-session.json").is_file() {
        return Err(anyhow!("Run has no native Session receipt"));
    }
    let session =
        read_provider_session(dir)?.ok_or_else(|| anyhow!("Run has no native Session receipt"))?;
    let native = resolve_native(store, manifest, &session)
        .await?
        .ok_or_else(|| anyhow!("Run has no recorded native location or exact account source"))?;
    if matches!(
        (&native, source),
        (
            crate::run_record::native_source::NativeSource::OpenCode { .. },
            OutputSource::Claude | OutputSource::Codex
        ) | (
            crate::run_record::native_source::NativeSource::Jsonl { .. },
            OutputSource::OpenCode
        )
    ) {
        return Err(anyhow!(
            "Native location kind does not match its recorded provider"
        ));
    }
    Ok((
        native.path().to_path_buf(),
        Some(session.provider_session_id),
    ))
}

fn output_source(manifest: &RunManifest) -> OutputSource {
    if manifest.surface != "tui" {
        return OutputSource::Journal;
    }
    match manifest.harness.as_str() {
        "claude" => OutputSource::Claude,
        "codex" => OutputSource::Codex,
        "opencode" => OutputSource::OpenCode,
        _ => OutputSource::Journal,
    }
}

#[cfg(test)]
mod tests {
    use super::read_task_output;
    use crate::chat::types::ConversationEvent;
    use crate::planning::{LinearIssueId, TaskPlan};
    use crate::run_record::{CaptureHandle, RunSpec, SubjectAttribution};
    use crate::store::{open_ephemeral_store, StorageConfig};
    use crate::work::task::{Observation, PmWritebackState, Task};
    use std::sync::Arc;

    #[tokio::test]
    async fn output_discovers_old_and_new_auxiliary_runs_without_replaying_records() {
        let home = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(home.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let mut task = Task {
            id: crate::durable::TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-id").unwrap(),
                identifier: "TEST-293".into(),
                title: "Watch".into(),
                description: "Watch".into(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::durable::ProjectId::new(),
            worktree: home.path().join("absent-worktree"),
            workspace_slug: "watch".into(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let launch = |subject: String| {
            let capture = CaptureHandle::begin_at(
                home.path(),
                RunSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: home.path().to_path_buf(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![SubjectAttribution::declared(subject)],
                },
            )
            .unwrap();
            capture.record_conversation(ConversationEvent::TextDelta {
                turn_id: "turn".into(),
                content: "live text".into(),
            });
            capture.record_conversation(ConversationEvent::TextDelta {
                turn_id: "turn".into(),
                content: "later text".into(),
            });
            capture.finish("completed").unwrap();
            capture
        };
        let mut runs = Vec::new();
        for _ in 0..51 {
            let capture = launch(format!("task:{}", task.plan.identifier));
            let path = capture.artifact_dir().join("manifest.json");
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            manifest["created_at"] = "2020-01-01T00:00:00Z".into();
            std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            runs.push(capture);
        }
        let unrelated = launch("task:OTHER-1".into());
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..7 {
            let page = read_task_output(&store, &task, home.path(), cursor.as_deref(), false)
                .await
                .unwrap();
            assert!(page.sources.iter().all(|source| source.available));
            for source in page
                .sources
                .into_iter()
                .filter(|source| !source.records.is_empty())
            {
                assert!(seen.insert(source.run_id));
                assert!(source.stage.is_none());
                assert_eq!(source.provider, "codex");
                assert_eq!(source.records.len(), 2);
                assert!(
                    matches!(&source.records[0].event, ConversationEvent::TextDelta { content, .. } if content == "live text")
                );
                assert!(
                    matches!(&source.records[1].event, ConversationEvent::TextDelta { content, .. } if content == "later text")
                );
            }
            cursor = Some(page.next_cursor);
        }
        assert_eq!(seen.len(), 51);
        let unrelated_manifest = unrelated.artifact_dir().join("manifest.json");
        let original = std::fs::read(&unrelated_manifest).unwrap();
        std::fs::write(&unrelated_manifest, "{broken").unwrap();
        let new = launch(format!("task:{}", task.id));
        for _ in 0..7 {
            let page = read_task_output(&store, &task, home.path(), cursor.as_deref(), false)
                .await
                .unwrap();
            assert_eq!(page.gaps.len(), 1);
            assert_eq!(page.gaps[0].code, "discovery_incomplete");
            assert!(page.gaps[0]
                .message
                .contains(unrelated.artifact_dir().to_str().unwrap()));
            for source in page
                .sources
                .into_iter()
                .filter(|source| !source.records.is_empty())
            {
                assert!(seen.insert(source.run_id));
                assert_eq!(source.records.len(), 2);
            }
            cursor = Some(page.next_cursor);
        }
        assert_eq!(seen.len(), 52);
        // A Run directory can appear before its atomic manifest install, or
        // disappear during discovery. Neither hides healthy Task output.
        std::fs::remove_file(&unrelated_manifest).unwrap();
        let missing = read_task_output(&store, &task, home.path(), cursor.as_deref(), false)
            .await
            .unwrap();
        assert_eq!(missing.gaps.len(), 1);
        assert_eq!(missing.gaps[0].code, "discovery_incomplete");
        assert!(missing.sources.iter().all(|source| source.available));
        std::fs::write(&unrelated_manifest, original).unwrap();
        let recovered = read_task_output(
            &store,
            &task,
            home.path(),
            Some(&missing.next_cursor),
            false,
        )
        .await
        .unwrap();
        assert!(recovered.gaps.is_empty());
        assert!(recovered
            .sources
            .iter()
            .all(|source| source.records.is_empty()));
        let live = read_task_output(&store, &task, home.path(), None, true)
            .await
            .unwrap();
        assert!(live.gaps.is_empty());
        assert!(live.sources.iter().all(|source| source.records.is_empty()));
        // Every existing Run is seeded, including those outside the first
        // eight-source page. None may skip output before its next visit.
        for capture in &runs {
            capture.record_conversation(ConversationEvent::TextDelta {
                turn_id: "turn".into(),
                content: "after live start".into(),
            });
        }
        let concurrent = launch(format!("task:{}", task.id));
        let mut live_cursor = live.next_cursor;
        let mut arrivals = std::collections::BTreeSet::new();
        for _ in 0..8 {
            let page = read_task_output(&store, &task, home.path(), Some(&live_cursor), false)
                .await
                .unwrap();
            for source in page
                .sources
                .into_iter()
                .filter(|source| !source.records.is_empty())
            {
                assert!(arrivals.insert(source.run_id));
                if source.records.len() == 1 {
                    assert!(
                        matches!(&source.records[0].event, ConversationEvent::TextDelta {content, ..} if content == "after live start")
                    );
                } else {
                    assert_eq!(source.records.len(), 2);
                    assert!(
                        matches!(&source.records[0].event, ConversationEvent::TextDelta {content, ..} if content == "live text")
                    );
                }
            }
            live_cursor = page.next_cursor;
        }
        assert_eq!(arrivals.len(), 52);
        drop(concurrent);
        task.id = crate::durable::TaskId::new();
        assert!(
            read_task_output(&store, &task, home.path(), cursor.as_deref(), false)
                .await
                .is_err()
        );
        drop((runs, unrelated, new));
    }
}
