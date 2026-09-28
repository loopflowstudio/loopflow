//! Local persistence evidence, deliberately narrower than Claude's private loader.
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;

use crate::run_record::ProviderSessionRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaudeHistory {
    Absent,
    Persisted,
}

pub(crate) fn history_root(dir: &Path, session: &ProviderSessionRef) -> Result<PathBuf> {
    if let Some(root) = &session.history_root {
        ensure!(root.is_absolute(), "Claude history root is not absolute");
        return Ok(root.clone());
    }
    let account = session
        .account_id
        .as_ref()
        .context("Claude history location was not recorded; cannot establish absence")?;
    let home = dir.ancestors().nth(3).context("Run Home is unavailable")?;
    Ok(home.join("accounts/claude").join(account.as_str()))
}

pub(crate) fn inspect(dir: &Path, session: &ProviderSessionRef) -> Result<ClaudeHistory> {
    inspect_root(&history_root(dir, session)?, &session.provider_session_id)
}

fn inspect_root(root: &Path, id: &str) -> Result<ClaudeHistory> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "invalid Claude conversation ID"
    );
    // read_dir proves access even when projects does not exist. Never manufacture
    // an account directory while deciding whether history can be replaced.
    fs::read_dir(root).context("cannot inspect Claude history root")?;
    let projects = match fs::read_dir(root.join("projects")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ClaudeHistory::Absent);
        }
        Err(error) => return Err(error).context("cannot inspect Claude projects"),
    };
    let mut persisted = false;
    for project in projects {
        let project = project.context("cannot enumerate Claude projects")?;
        let kind = project.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "ambiguous linked Claude project directory"
        );
        if !kind.is_dir() {
            continue;
        }
        // Enumerate rather than treating a failed stat/open as missing history.
        for entry in fs::read_dir(project.path()).context("cannot inspect Claude project")? {
            let entry = entry?;
            if entry.file_name() != format!("{id}.jsonl").as_str() {
                continue;
            }
            ensure!(
                entry.file_type()?.is_file(),
                "ambiguous Claude transcript location"
            );
            let file = File::open(entry.path()).context("cannot read Claude transcript")?;
            if file.metadata()?.len() == 0 {
                continue;
            }
            let mut recognized = false;
            for line in BufReader::new(file).lines() {
                let line = line.context("cannot read Claude transcript record")?;
                if let Ok(record) = serde_json::from_str::<Value>(&line) {
                    recognized |= conversation_record(&record, id);
                }
            }
            if !recognized {
                bail!("Claude transcript contains unresolved history; retaining this Session");
            }
            persisted = true;
        }
    }
    Ok(if persisted {
        ClaudeHistory::Persisted
    } else {
        ClaudeHistory::Absent
    })
}

fn conversation_record(record: &Value, id: &str) -> bool {
    let text = |key| record.get(key).and_then(Value::as_str);
    if text("sessionId") != Some(id)
        || text("uuid").is_none_or(|value| uuid::Uuid::parse_str(value).is_err())
        || text("timestamp").is_none_or(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                .is_err()
        })
    {
        return false;
    }
    let content = match text("type") {
        Some("user" | "assistant") => record.pointer("/message/content"),
        Some("system") if text("subtype") == Some("local_command") => record.get("content"),
        _ => None,
    };
    content.is_some_and(|value| value.is_string() || value.is_array())
}

#[cfg(test)]
mod tests {
    use super::{inspect_root, ClaudeHistory};
    use std::fs;

    #[test]
    fn absence_requires_access_and_unknown_history_survives() {
        let root = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        assert!(inspect_root(&root.path().join("missing"), &id).is_err());
        assert_eq!(
            inspect_root(root.path(), &id).unwrap(),
            ClaudeHistory::Absent
        );
        let project = root.path().join("projects/another-project");
        fs::create_dir_all(&project).unwrap();
        let path = project.join(format!("{id}.jsonl"));
        fs::write(&path, "").unwrap();
        assert_eq!(
            inspect_root(root.path(), &id).unwrap(),
            ClaudeHistory::Absent
        );
        for content in ["{", "{\"type\":\"file-history-snapshot\"}"] {
            fs::write(&path, content).unwrap();
            assert!(inspect_root(root.path(), &id).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
        let record = serde_json::json!({"type":"user", "sessionId":id,
            "uuid":uuid::Uuid::new_v4().to_string(), "timestamp":"2026-09-28T12:00:00Z",
            "message":{"role":"user","content":"saved seed"}});
        fs::write(&path, format!("{record}\n{{")).unwrap();
        assert_eq!(
            inspect_root(root.path(), &id).unwrap(),
            ClaudeHistory::Persisted
        );
        // Deterministic I/O failure, including when tests run as root.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(inspect_root(root.path(), &id).is_err());
    }
}
