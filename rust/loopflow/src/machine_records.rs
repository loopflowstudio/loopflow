//! Records one Machine copies from another before the other is deleted.
//!
//! `lf history export` writes a Machine's Sessions, Processes, Graph rows,
//! session events and capture files as JSON lines, origin first. The Machine
//! that rented it imports them into `machine_records` and
//! `<home>/machines/<origin>/runs`, apart from its own rows and captures, so a
//! copy is readable history and never local authority.

use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::durable::{MachineId, TaskId};
use crate::store::sqlite::SqliteStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Process,
    Session,
    SessionEvent,
    GraphProcess,
    GraphProcessStep,
}

impl RecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Session => "session",
            Self::SessionEvent => "session_event",
            Self::GraphProcess => "graph_process",
            Self::GraphProcessStep => "graph_process_step",
        }
    }
}

/// One copied row: its table's kind, its key there, and its columns as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: RecordKind,
    pub key: String,
    pub row: serde_json::Value,
}

/// One line of `lf history export`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum ExportLine {
    /// Always first: which Machine wrote these, so a reused address cannot
    /// file another Machine's records under this one.
    Origin {
        machine_id: MachineId,
        exported_at: i64,
    },
    Row(Record),
    /// One file of a Session capture, base64.
    Capture {
        artifact_key: String,
        path: String,
        content: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CopyCounts {
    pub sessions: usize,
    pub processes: usize,
}

impl std::fmt::Display for CopyCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} Session(s), {} Process(es)",
            self.sessions, self.processes
        )
    }
}

/// A Session as its origin Machine recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CopiedSession {
    pub origin: MachineId,
    /// The origin's label while it is still added here.
    pub origin_label: Option<String>,
    pub id: String,
    pub title: String,
    pub task_id: Option<TaskId>,
    pub repo: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    /// When it was copied.
    pub observed_at: i64,
}

impl CopiedSession {
    pub fn origin_name(&self) -> &str {
        self.origin_label.as_deref().unwrap_or(self.origin.as_str())
    }
}

/// Write this Machine's records since `since` (unix seconds).
///
/// # Errors
/// When the store cannot be read, a capture file cannot be read, or `out` fails.
pub fn export(
    sqlite: &SqliteStore,
    lf_home: &Path,
    since: i64,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let origin = ExportLine::Origin {
        machine_id: sqlite.local_machine()?.id,
        exported_at: time::OffsetDateTime::now_utc().unix_timestamp(),
    };
    writeln!(out, "{}", serde_json::to_string(&origin)?)?;
    let (records, artifact_keys) = sqlite.export_records(since)?;
    for record in records {
        writeln!(out, "{}", serde_json::to_string(&ExportLine::Row(record))?)?;
    }
    for key in artifact_keys {
        let Some(dir) = crate::session_record::record_dir(lf_home, &key) else {
            continue;
        };
        for (path, file) in files_under(&dir)? {
            let content =
                std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
            let line = ExportLine::Capture {
                artifact_key: key.clone(),
                path,
                content: base64::engine::general_purpose::STANDARD.encode(content),
            };
            writeln!(out, "{}", serde_json::to_string(&line)?)?;
        }
    }
    Ok(())
}

/// Read an export from `expected` and keep it here. Rows replace earlier
/// copies of the same rows, so copying again only brings the record up to date.
///
/// # Errors
/// When the export is malformed, comes from another Machine, or cannot be kept.
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
        ExportLine::Origin { machine_id, .. } if &machine_id == expected => {}
        ExportLine::Origin { machine_id, .. } => {
            bail!("the export came from {machine_id}, not {expected}; nothing was copied")
        }
        _ => bail!("the export does not start with its origin; nothing was copied"),
    }
    let mut records = Vec::new();
    let mut captures = Vec::new();
    for line in lines {
        match serde_json::from_str::<ExportLine>(&line?).context("read an export line")? {
            ExportLine::Row(record) => records.push(record),
            ExportLine::Capture {
                artifact_key,
                path,
                content,
            } => captures.push((artifact_key, path, content)),
            ExportLine::Origin { .. } => bail!("the export names its origin twice"),
        }
    }
    let root = copies_home(lf_home, expected);
    for (key, path, content) in captures {
        let dir = crate::session_record::record_dir(&root, &key)
            .ok_or_else(|| anyhow!("capture {key:?} is not an artifact key"))?;
        let file = dir.join(relative(&path)?);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = base64::engine::general_purpose::STANDARD.decode(content)?;
        std::fs::write(&file, bytes).with_context(|| format!("write {}", file.display()))?;
    }
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
