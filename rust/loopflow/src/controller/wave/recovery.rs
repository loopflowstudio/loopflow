//! Retire the historical Wave Flow once, retaining its journal as evidence.

use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

use crate::controller::wave::journal::{
    fold_thread, journal_path, read_events_with_state, Event, EventKind, Journal,
    ReadOnlyJournalState,
};
use crate::controller::wave::playhead::Playhead;
use crate::controller::wave::relocate::WaveLocatorLock;
use crate::engine::{ConcreteStep, OccurrencePolicy};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum FlowDisposition {
    Retired,
    Unresolved { reason: String },
    Cancelled { reason: String },
}

/// The journal boundary and identities covered by one explicit disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedDisposition {
    pub source_seq: u64,
    pub invocation_ids: Vec<String>,
    pub disposition: FlowDisposition,
}

#[derive(Debug, Serialize)]
pub struct FlowRecovery {
    pub journal: PathBuf,
    pub record: Option<RecordedDisposition>,
    /// Original stored snapshot, including definitions an old reader could not execute.
    pub source: Option<serde_json::Value>,
}

fn classify(snapshot: &Playhead, open_turn: bool) -> FlowDisposition {
    if snapshot.active.is_some() || open_turn {
        return FlowDisposition::Unresolved {
            reason: "An attempt has no recorded termination; provider liveness is unknown.".into(),
        };
    }
    let default_root = snapshot.stack.len() == 1
        && snapshot.stack[0].flow == "wave"
        && snapshot.stack[0].cursor == 0
        && snapshot.stack[0].queue.is_empty()
        && matches!(snapshot.stack[0].steps.as_slice(), [ConcreteStep::Skill(skill)]
            if skill.skill.name == "wave/operate"
                && skill.skill.content.is_some()
                && skill.policy == OccurrencePolicy::default());
    if default_root {
        FlowDisposition::Retired
    } else {
        FlowDisposition::Unresolved {
            reason:
                "Saved custom, nested, queued, or uncaptured Flow work needs explicit disposition."
                    .into(),
        }
    }
}

fn find_disposition(events: &[Event]) -> Option<RecordedDisposition> {
    for event in events.iter().rev() {
        match &event.kind {
            EventKind::WaveFlowDisposition { record } => return Some(record.clone()),
            EventKind::PlayheadChanged { .. } => break,
            _ => {}
        }
    }
    let fold = fold_thread(events);
    let snapshot = fold.playhead?;
    Some(RecordedDisposition {
        source_seq: events.last().expect("a snapshot belongs to an event").seq,
        invocation_ids: snapshot
            .stack
            .iter()
            .flat_map(|frame| {
                std::iter::once(frame.id.clone())
                    .chain(frame.queue.iter().map(|queued| queued.id.clone()))
            })
            .collect(),
        disposition: classify(&snapshot, !fold.open.is_empty()),
    })
}

pub(super) fn prepare(journal: &mut Journal, events: &mut Vec<Event>, wave: &str) -> Result<()> {
    let Some(record) = find_disposition(events) else {
        return Ok(());
    };
    if !events
        .iter()
        .any(|event| matches!(&event.kind, EventKind::WaveFlowDisposition { record: saved } if saved == &record))
    {
        events.push(journal.try_append(|_| EventKind::WaveFlowDisposition {
            record: record.clone(),
        })?);
    }
    if let FlowDisposition::Unresolved { reason } = record.disposition {
        bail!(
            "Wave '{wave}' has unresolved historical Flow work at journal sequence {}: {reason} \
             Inspect it with `lf wave recover {wave}`. No continuation was started or discarded.",
            record.source_seq
        );
    }
    Ok(())
}

pub fn inspect(repo: &Path, wave: &str) -> Result<FlowRecovery> {
    let path = journal_path(repo, wave);
    let read = read_events_with_state(&path);
    ensure!(
        matches!(
            read.state,
            ReadOnlyJournalState::Available | ReadOnlyJournalState::Missing
        ),
        "{}: {}",
        path.display(),
        read.detail
            .as_deref()
            .unwrap_or("journal evidence is incomplete")
    );
    let record = find_disposition(&read.events);
    let source = if let Some(record) = &record {
        // Decode the original JSON, not a lossy historical execution projection.
        let raw = std::fs::read_to_string(&path)?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .rev()
            .find(|row| {
                row["seq"]
                    .as_u64()
                    .is_some_and(|seq| seq <= record.source_seq)
                    && row["kind"]["type"] == "playhead_changed"
            })
    } else {
        None
    };
    Ok(FlowRecovery {
        journal: path,
        record,
        source,
    })
}

/// Cancel an exact saved continuation without executing or recompiling it.
pub fn cancel(repo: &Path, wave: &str, source_seq: u64, reason: &str) -> Result<FlowRecovery> {
    ensure!(!reason.trim().is_empty(), "cancellation requires a reason");
    let _lock = WaveLocatorLock::acquire(repo, wave)?;
    let report = inspect(repo, wave)?;
    let mut record = report.record.context("no historical Flow to cancel")?;
    ensure!(
        record.source_seq == source_seq,
        "historical Flow changed; inspect it again"
    );
    match &record.disposition {
        FlowDisposition::Cancelled { .. } => return inspect(repo, wave),
        FlowDisposition::Retired => {
            bail!("default governance is already retired; nothing to cancel")
        }
        FlowDisposition::Unresolved { .. } => {}
    }
    let (mut journal, events) = Journal::open(&journal_path(repo, wave))?;
    let fold = fold_thread(&events);
    ensure!(
        fold.open.is_empty() && fold.playhead.as_ref().is_none_or(|state| state.active.is_none()),
        "attempt termination is unproven; cancellation cannot establish provider death or authorize a replacement"
    );
    record.disposition = FlowDisposition::Cancelled {
        reason: reason.trim().into(),
    };
    journal.try_append(|_| EventKind::WaveFlowDisposition { record })?;
    inspect(repo, wave)
}

#[cfg(test)]
mod tests {
    use crate::controller::wave::journal::{
        journal_path, read_events, EventKind, Journal, JournalAppendStage,
    };
    use crate::controller::wave::recovery::prepare;

    #[test]
    fn failed_cutover_append_leaves_the_original_history_retryable() {
        let saved = include_str!("../../../../../tests/fixtures/wave/queued-after-skip.jsonl");
        for failure in [JournalAppendStage::Write, JournalAppendStage::Flush] {
            let tmp = tempfile::tempdir().unwrap();
            let path = journal_path(tmp.path(), "ship");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, saved).unwrap();
            let (mut journal, mut events) = Journal::open(&path).unwrap();
            journal.fail_next_append(failure);
            let error = prepare(&mut journal, &mut events, "ship").unwrap_err();
            assert!(error.to_string().contains("failed during"));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
            drop(journal);

            for _ in 0..2 {
                let (mut journal, mut events) = Journal::open(&path).unwrap();
                assert!(prepare(&mut journal, &mut events, "ship")
                    .unwrap_err()
                    .to_string()
                    .contains("unresolved historical"));
            }
            assert_eq!(
                read_events(&path)
                    .iter()
                    .filter(|event| matches!(event.kind, EventKind::WaveFlowDisposition { .. }))
                    .count(),
                1
            );
            assert!(std::fs::read_to_string(path).unwrap().starts_with(saved));
        }
    }
}
