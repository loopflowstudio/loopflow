//! Sessions collected from a rental while it runs and before give-back.
//!
//! `lf session history export` writes Session state, history, provider turns and
//! capture files as JSON lines, origin first. The Machine
//! that rented it imports them into `machine_records` and
//! `<home>/machines/<origin>/runs`, apart from its own rows and captures, so a
//! copy is readable history and never local authority.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use base64::Engine as _;

use loopflow_model::durable::MachineId;
use loopflow_storage::sqlite::SqliteStore;

pub use loopflow_storage::machine_records::*;

/// Where an event's copied capture lives here, when it was copied.
fn capture_dir(history: &CopiedHistory, lf_home: &Path, event: &CopiedEvent) -> Option<PathBuf> {
    let key = event.artifact_key.as_deref()?;
    crate::session_record::record_dir(&copies_home(lf_home, &history.session.origin), key)
        .filter(|dir| dir.is_dir())
}

/// The text `lf history show` prints for a copied Session.
pub fn render(history: &CopiedHistory, lf_home: &Path) -> String {
    let session = &history.session;
    let mut lines = vec![
        format!(
            "Session {} · from {} (copied {})",
            session.id,
            session.origin_name(),
            when(Some(history.copied_at))
        ),
        format!("Title: {}", session.title),
    ];
    if let Some(task) = &history.task_id {
        lines.push(format!("Task: {task}"));
    }
    lines.push(format!(
        "Working directory on {}: {}",
        session.origin_name(),
        history.cwd.as_deref().unwrap_or("unknown")
    ));
    lines.push(format!("Created: {}", when(history.created_at)));
    lines.push(match session.completed_at {
        Some(at) => format!("Completed: {}", when(Some(at))),
        None => "Completed: not recorded before the copy".to_string(),
    });
    lines.push(format!("Events: {}", history.events.len()));
    for event in &history.events {
        let capture = match (&event.artifact_key, capture_dir(history, lf_home, event)) {
            (_, Some(dir)) => format!(" {}", dir.display()),
            (Some(key), None) => format!(" {key} (capture not copied)"),
            (None, None) => String::new(),
        };
        lines.push(format!(
            "  {} {}{capture}",
            when(event.observed_at),
            event.kind
        ));
    }
    lines.join("\n")
}

/// Local date and time, as people read history.
fn when(at: Option<i64>) -> String {
    at.and_then(|at| chrono::DateTime::from_timestamp(at, 0))
        .map(|at| {
            at.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// One turn's provider text, in the order the provider presented it.
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptTurn {
    pub session_id: String,
    pub thread: String,
    pub harness: String,
    pub thread_position: usize,
    pub position: usize,
    pub turn: loopflow_harness::transcript::Turn,
}

fn write_line(out: &mut impl Write, line: ExportLine) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *out, &line)?;
    writeln!(out)?;
    Ok(())
}

/// Write changed Sessions and readable provider text, followed by the resume cursor.
pub fn export(
    sqlite: &SqliteStore,
    lf_home: &Path,
    since: i64,
    after: Option<CopyCursor>,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let origin = crate::store::machines::local_machine(sqlite)?.id;
    sqlite.prepare_history_export()?;
    let snapshot = sqlite.read_snapshot()?;
    let cursor = sqlite.export_cursor()?;
    // A replaced history file can start its positions over.
    let after = after.filter(|old| {
        old.state <= cursor.state
            && old.history <= cursor.history
            && old.started_at <= cursor.started_at
    });
    let changed: BTreeSet<_> = sqlite.export_sessions(since, after)?.into_iter().collect();
    let mut transcripts = changed.clone();
    if after.is_some() {
        transcripts.extend(sqlite.open_transcripts()?);
    }
    // Changes made while a slow SSH transfer runs remain past this cursor.
    // Release both WAL snapshots before writing to the network.
    drop(snapshot);
    write_line(out, ExportLine::Origin { machine_id: origin })?;
    for session in &changed {
        write_session(out, sqlite, lf_home, session)?;
    }
    for session in transcripts {
        let unchanged_since = after
            .filter(|_| !changed.contains(&session))
            .map(|cursor| cursor.started_at);
        write_transcript(out, sqlite, &session, unchanged_since)?;
    }
    write_line(
        out,
        ExportLine::Row(Record {
            kind: RecordKind::Checkpoint,
            key: "export".into(),
            row: serde_json::to_value(cursor)?,
        }),
    )
}

fn write_session(
    out: &mut impl Write,
    sqlite: &SqliteStore,
    lf_home: &Path,
    session: &str,
) -> anyhow::Result<()> {
    let (records, turns) = {
        let _snapshot = sqlite.read_snapshot()?;
        (
            sqlite.export_session_records(session)?,
            sqlite.session_turns(session)?,
        )
    };
    for record in records {
        if record.kind == RecordKind::SessionEvent && record.row["kind"] == "captured" {
            let payload: serde_json::Value = serde_json::from_str(
                record.row["payload"]
                    .as_str()
                    .context("capture has no payload")?,
            )?;
            if let Some(key) = payload["artifact_key"].as_str() {
                write_capture(out, lf_home, key)?;
            }
        }
        write_line(out, ExportLine::Row(record))?;
    }
    for turn in turns {
        write_line(
            out,
            ExportLine::Row(Record {
                kind: RecordKind::Turn,
                key: serde_json::to_string(&(session, &turn.thread, &turn.turn))?,
                row: serde_json::to_value(turn)?,
            }),
        )?;
    }
    Ok(())
}

fn write_transcript(
    out: &mut impl Write,
    sqlite: &SqliteStore,
    session: &str,
    unchanged_since: Option<i64>,
) -> anyhow::Result<()> {
    use loopflow_harness::transcript::{modified, read, Source};
    for (thread_position, thread) in crate::transcript::threads(sqlite, session)?
        .into_iter()
        .enumerate()
    {
        let Ok((harness, crate::transcript::Located::Found { source, .. })) = thread.located else {
            continue;
        };
        if let Source::File(path) = &source {
            if unchanged_since
                .zip(modified(path))
                .is_some_and(|(since, at)| at < since)
            {
                continue;
            }
        }
        for (position, turn) in read(harness, &source)?.into_iter().enumerate() {
            // An unnamed provider turn is identified by its position in the file.
            let identity = turn.id.as_ref().map_or_else(
                || position.into(),
                |id| serde_json::Value::String(id.clone()),
            );
            let key = serde_json::to_string(&(session, &thread.facts.thread, identity))?;
            let row = TranscriptTurn {
                session_id: session.to_owned(),
                thread: thread.facts.thread.to_string(),
                harness: harness.as_str().into(),
                thread_position,
                position,
                turn,
            };
            write_line(
                out,
                ExportLine::Row(Record {
                    kind: RecordKind::Transcript,
                    key,
                    row: serde_json::to_value(row)?,
                }),
            )?;
        }
    }
    Ok(())
}

fn write_capture(out: &mut impl Write, lf_home: &Path, key: &str) -> anyhow::Result<()> {
    let Some(dir) = crate::session_record::record_dir(lf_home, key) else {
        return Ok(());
    };
    for (path, file) in files_under(&dir)? {
        let content = std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
        write_line(
            out,
            ExportLine::Capture {
                artifact_key: key.to_owned(),
                path,
                content: base64::engine::general_purpose::STANDARD.encode(content),
            },
        )?;
    }
    Ok(())
}

/// Read a completed SSH export. Each small batch is repeatable. The final
/// checkpoint becomes visible only after every row and capture reached disk.
pub fn import(
    sqlite: &SqliteStore,
    lf_home: &Path,
    expected: &MachineId,
    input: impl BufRead,
) -> anyhow::Result<CopyCounts> {
    let mut lines = input.lines();
    let first = lines
        .next()
        .ok_or_else(|| anyhow!("the export was empty"))??;
    match serde_json::from_str::<ExportLine>(&first).context("read the export's origin")? {
        ExportLine::Origin { machine_id } if &machine_id == expected => {}
        ExportLine::Origin { machine_id } => {
            bail!("the export came from {machine_id}, not {expected}; nothing was copied")
        }
        _ => bail!("the export does not start with its origin; nothing was copied"),
    }
    let root = copies_home(lf_home, expected);
    let mut records = Vec::new();
    let mut checkpoint = None;
    for line in lines {
        let line: serde_json::Value =
            serde_json::from_str(&line?).context("read an export line")?;
        // Released exporters also wrote Process and Graph rows.
        if line["line"] == "row"
            && matches!(
                line["kind"].as_str(),
                Some("process" | "graph_process" | "graph_process_step")
            )
        {
            continue;
        }
        anyhow::ensure!(
            checkpoint.is_none(),
            "the export continues after its checkpoint"
        );
        match serde_json::from_value::<ExportLine>(line).context("read an export line")? {
            ExportLine::Row(record) if record.kind == RecordKind::Checkpoint => {
                serde_json::from_value::<CopyCursor>(record.row.clone())?;
                checkpoint = Some(record);
            }
            ExportLine::Row(record) => records.push(record),
            ExportLine::Capture {
                artifact_key,
                path,
                content,
            } => {
                let dir = crate::session_record::record_dir(&root, &artifact_key)
                    .ok_or_else(|| anyhow!("capture {artifact_key:?} is not an artifact key"))?;
                let path = dir.join(relative(&path)?);
                let parent = path.parent().context("capture has no directory")?;
                std::fs::create_dir_all(parent)?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(content)?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                file.write_all(&bytes)?;
                file.persist(path)?;
            }
            ExportLine::Origin { .. } => bail!("the export names its origin twice"),
        }
        if records.len() == 128 {
            sqlite.import_records(expected, &records)?;
            records.clear();
        }
    }
    records.extend(checkpoint);
    sqlite.import_records(expected, &records)?;
    Ok(sqlite.copy_counts(expected)?)
}

/// Where copies of `origin`'s captures live; the same layout as this Machine's own.
pub fn copies_home(lf_home: &Path, origin: &MachineId) -> PathBuf {
    lf_home.join("machines").join(origin.as_str())
}

fn relative(path: &str) -> anyhow::Result<PathBuf> {
    let candidate = Path::new(path);
    if path.is_empty()
        || !candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("capture path {path:?} leaves its directory");
    }
    Ok(candidate.to_path_buf())
}

fn files_under(dir: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let entries = match std::fs::read_dir(dir.join(&relative)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let name = relative.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(name);
            } else if kind.is_file() {
                let text = name
                    .to_str()
                    .ok_or_else(|| anyhow!("{} is not UTF-8", name.display()))?
                    .to_string();
                found.push((text, entry.path()));
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests;
