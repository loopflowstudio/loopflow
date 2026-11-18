//! Records one Machine copies from another before the other is deleted.
//!
//! `lf history export` writes a Machine's Sessions, session events and
//! capture files as JSON lines, origin first. The Machine
//! that rented it imports them into `machine_records` and
//! `<home>/machines/<origin>/runs`, apart from its own rows and captures, so a
//! copy is readable history and never local authority.

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
        machine_id: crate::store::machines::local_machine(sqlite)?.id,
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
        ExportLine::Origin { machine_id } if &machine_id == expected => {}
        ExportLine::Origin { machine_id } => {
            bail!("the export came from {machine_id}, not {expected}; nothing was copied")
        }
        _ => bail!("the export does not start with its origin; nothing was copied"),
    }
    let mut records = Vec::new();
    let mut captures = Vec::new();
    for line in lines {
        let line: serde_json::Value =
            serde_json::from_str(&line?).context("read an export line")?;
        // A released exporter also wrote Process and Graph rows; no copy keeps them.
        if line["line"] == "row"
            && !matches!(line["kind"].as_str(), Some("session" | "session_event"))
        {
            continue;
        }
        match serde_json::from_value::<ExportLine>(line).context("read an export line")? {
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
