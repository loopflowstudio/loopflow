use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::debug;

use crate::engine::worktrees::{main_repo_root, wave_name_from_worktree_and_main};
use crate::lfd::id::LfdId;

const JOURNAL_ROOT: &str = ".lf/journal/runs";
const JOURNAL_EXCLUDE_ENTRY: &str = ".lf/journal/";

thread_local! {
    static RUN_CONTEXT: RefCell<Option<RunContext>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone)]
struct RunContext {
    run_id: LfdId,
    run_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfNode {
    Run,
    Flow,
    Step,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LfEventType {
    Started,
    Completed,
    Errored,
    Escalated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LfEventFields {
    pub wave_name: Option<String>,
    pub worktree: Option<String>,
    pub command: Option<Vec<String>>,
    pub flow: Option<String>,
    pub step: Option<String>,
    pub index: Option<u32>,
    pub error: Option<String>,
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LfEvent {
    pub run_id: LfdId,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub node: LfNode,
    pub event: LfEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

pub fn emit(repo_root: &Path, node: LfNode, event: LfEventType, fields: LfEventFields) {
    if let Err(err) = try_emit(repo_root, node, event, fields) {
        debug!(
            error = %err,
            repo = %repo_root.display(),
            ?node,
            ?event,
            "journal append failed"
        );
    }
}

pub fn runs_root(worktree: &Path) -> PathBuf {
    worktree.join(JOURNAL_ROOT)
}

pub fn events_path(run_dir: &Path) -> PathBuf {
    run_dir.join("events.jsonl")
}

pub fn read_events(run_dir: &Path) -> Result<Vec<LfEvent>, std::io::Error> {
    let path = events_path(run_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path)?;
    let mut events = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str(&line).map_err(std::io::Error::other)?;
        events.push(event);
    }
    Ok(events)
}

fn try_emit(
    repo_root: &Path,
    node: LfNode,
    event: LfEventType,
    fields: LfEventFields,
) -> Result<(), std::io::Error> {
    let is_run_started = matches!((node, event), (LfNode::Run, LfEventType::Started));
    let maybe_context = if is_run_started {
        ensure_run_context(repo_root, &fields)?
    } else {
        current_context()
    };

    let Some(context) = maybe_context else {
        return Ok(());
    };

    let event = LfEvent {
        run_id: context.run_id.clone(),
        ts: OffsetDateTime::now_utc(),
        node,
        event,
        wave_name: fields.wave_name,
        worktree: fields.worktree,
        command: fields.command,
        flow: fields.flow,
        step: fields.step,
        index: fields.index,
        error: fields.error,
        signal: fields.signal,
    };
    append_event(&context.run_dir, &event)?;

    if matches!(node, LfNode::Run)
        && matches!(
            event.event,
            LfEventType::Completed | LfEventType::Errored | LfEventType::Escalated
        )
    {
        clear_context();
    }

    Ok(())
}

fn ensure_run_context(
    repo_root: &Path,
    fields: &LfEventFields,
) -> Result<Option<RunContext>, std::io::Error> {
    if let Some(context) = current_context() {
        return Ok(Some(context));
    }

    let Some(main_repo) = main_repo_root(repo_root).ok() else {
        return Ok(None);
    };
    let Some(wave_name) = wave_name_from_worktree_and_main(repo_root, &main_repo) else {
        return Ok(None);
    };

    ensure_journal_ignored(repo_root)?;
    let run_id = LfdId::new();
    let run_dir = runs_root(repo_root).join(run_id.as_str());
    fs::create_dir_all(&run_dir)?;

    let context = RunContext { run_id, run_dir };
    set_context(context.clone());

    if fields.wave_name.as_deref() != Some(wave_name.as_str()) {
        debug!(
            expected_wave = %wave_name,
            observed_wave = ?fields.wave_name,
            repo = %repo_root.display(),
            "journal run start received mismatched wave metadata"
        );
    }

    Ok(Some(context))
}

fn append_event(run_dir: &Path, event: &LfEvent) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(run_dir))?;
    serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn current_context() -> Option<RunContext> {
    RUN_CONTEXT.with(|cell| cell.borrow().clone())
}

fn set_context(context: RunContext) {
    RUN_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(context);
    });
}

fn clear_context() {
    RUN_CONTEXT.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn ensure_journal_ignored(repo_root: &Path) -> Result<(), std::io::Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--git-path", "info/exclude"])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "git rev-parse --git-path info/exclude failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let exclude_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim() == JOURNAL_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(JOURNAL_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)
}

#[cfg(test)]
mod tests {
    use super::{emit, read_events, runs_root, LfEventFields, LfEventType, LfNode};
    use crate::engine::git::is_clean;
    use loopflow_test_support::TestRepo;

    fn started_fields(
        command: &[String],
        worktree: &std::path::Path,
        wave_name: &str,
    ) -> LfEventFields {
        LfEventFields {
            wave_name: Some(wave_name.to_string()),
            worktree: Some(worktree.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        }
    }

    fn only_run_dir(worktree: &std::path::Path) -> std::path::PathBuf {
        let mut entries = std::fs::read_dir(runs_root(worktree))
            .expect("read runs")
            .map(|entry| entry.expect("run dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "expected a single journal run dir");
        entries.pop().expect("run dir")
    }

    #[test]
    fn journal_is_disabled_in_main_repo() {
        let repo = TestRepo::new();
        let command = vec!["lf".to_string(), "implement".to_string()];

        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, repo.path(), "main"),
        );
        emit(
            repo.path(),
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        assert!(!runs_root(repo.path()).exists());
    }

    #[test]
    fn journal_writes_run_flow_and_step_events_in_wave_worktree() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Flow,
            LfEventType::Started,
            LfEventFields {
                flow: Some("build".to_string()),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Step,
            LfEventType::Started,
            LfEventFields {
                step: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Step,
            LfEventType::Completed,
            LfEventFields {
                step: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Flow,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let run_dir = only_run_dir(&worktree);
        let events = read_events(&run_dir).expect("read events");
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].node, LfNode::Run);
        assert_eq!(events[0].event, LfEventType::Started);
        assert_eq!(events[0].wave_name.as_deref(), Some("runtime"));
        assert_eq!(events[1].node, LfNode::Flow);
        assert_eq!(events[1].flow.as_deref(), Some("build"));
        assert_eq!(events[2].node, LfNode::Step);
        assert_eq!(events[2].step.as_deref(), Some("implement"));
        assert_eq!(events[2].index, Some(0));
        assert_eq!(events[3].event, LfEventType::Completed);
        assert_eq!(events[4].node, LfNode::Flow);
        assert_eq!(events[5].node, LfNode::Run);
        assert_eq!(events[5].event, LfEventType::Completed);
    }

    #[test]
    fn journal_keeps_worktree_clean() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );

        assert!(is_clean(&worktree).expect("worktree should stay clean"));
    }

    #[test]
    fn terminal_run_events_clear_context_for_the_next_run() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "runtime"),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let entries = std::fs::read_dir(runs_root(&worktree))
            .expect("read runs")
            .count();
        assert_eq!(entries, 2);
    }
}
