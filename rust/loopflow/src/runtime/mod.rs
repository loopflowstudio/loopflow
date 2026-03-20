use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::debug;

use crate::engine::git::current_branch;
use crate::engine::worktrees::{main_repo_root, wave_name_from_worktree_and_main};
use crate::lfd::id::LfdId;

const RUNTIME_ROOT: &str = ".lf/runtime/runs";
const RUNTIME_EXCLUDE_ENTRY: &str = ".lf/runtime/";

#[derive(Debug, Clone, Default)]
pub enum RunTarget {
    #[default]
    Untargeted,
    Flow(String),
    Step(String),
}

impl RunTarget {
    pub fn flow(name: impl Into<String>) -> Self {
        Self::Flow(name.into())
    }

    pub fn step(name: impl Into<String>) -> Self {
        Self::Step(name.into())
    }

    fn into_parts(self) -> (Option<String>, Option<String>) {
        match self {
            Self::Untargeted => (None, None),
            Self::Flow(flow) => (Some(flow), None),
            Self::Step(step) => (None, Some(step)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRunMeta {
    pub run_id: LfdId,
    pub wave_name: String,
    pub repo: String,
    pub worktree: String,
    pub command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum RuntimeEvent {
    #[serde(rename = "run.started")]
    RunStarted {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        command: Vec<String>,
        worktree: String,
        wave_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        flow: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<String>,
    },
    #[serde(rename = "step.started")]
    StepStarted {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        step: String,
        index: u32,
    },
    #[serde(rename = "step.completed")]
    StepCompleted {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        step: String,
        index: u32,
        exit_code: i32,
    },
    #[serde(rename = "run.waiting")]
    RunWaiting {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        step: String,
    },
    #[serde(rename = "run.completed")]
    RunCompleted {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        exit_code: i32,
    },
    #[serde(rename = "run.failed")]
    RunFailed {
        run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
        exit_code: i32,
        error: String,
    },
}

#[derive(Debug)]
pub struct RuntimeRun {
    meta: RuntimeRunMeta,
    run_dir: PathBuf,
    finished: Cell<bool>,
}

impl RuntimeRun {
    pub fn maybe_start(repo_root: &Path, command: &[String], target: RunTarget) -> Option<Self> {
        let main_repo = main_repo_root(repo_root).ok()?;
        let wave_name = wave_name_from_worktree_and_main(repo_root, &main_repo)?;
        match Self::start(repo_root, &main_repo, wave_name, command, target) {
            Ok(run) => Some(run),
            Err(err) => {
                debug!(error = %err, repo = %repo_root.display(), "runtime journal unavailable");
                None
            }
        }
    }

    fn start(
        repo_root: &Path,
        main_repo: &Path,
        wave_name: String,
        command: &[String],
        target: RunTarget,
    ) -> Result<Self, std::io::Error> {
        ensure_runtime_ignored(repo_root)?;
        let run_id = LfdId::new();
        let run_dir = runs_root(repo_root).join(run_id.as_str());
        fs::create_dir_all(&run_dir)?;
        let started_at = OffsetDateTime::now_utc();
        let worktree = repo_root.to_string_lossy().to_string();
        let command = command.to_vec();
        let (flow, step) = target.into_parts();

        let meta = RuntimeRunMeta {
            run_id: run_id.clone(),
            wave_name: wave_name.clone(),
            repo: main_repo.to_string_lossy().to_string(),
            worktree: worktree.clone(),
            command: command.clone(),
            flow: flow.clone(),
            step: step.clone(),
            started_at,
            target_branch: current_branch(repo_root).ok().flatten(),
        };

        fs::write(
            meta_path(&run_dir),
            serde_json::to_vec_pretty(&meta).map_err(std::io::Error::other)?,
        )?;
        let run = Self {
            meta,
            run_dir,
            finished: Cell::new(false),
        };
        run.append_event(&RuntimeEvent::RunStarted {
            run_id,
            timestamp: started_at,
            command,
            worktree,
            wave_name,
            flow,
            step,
        })?;
        Ok(run)
    }

    pub fn emit_step_started(&self, step: &str, index: u32) {
        self.emit(RuntimeEvent::StepStarted {
            run_id: self.meta.run_id.clone(),
            timestamp: OffsetDateTime::now_utc(),
            step: step.to_string(),
            index,
        });
    }

    pub fn emit_step_completed(&self, step: &str, index: u32, exit_code: i32) {
        self.emit(RuntimeEvent::StepCompleted {
            run_id: self.meta.run_id.clone(),
            timestamp: OffsetDateTime::now_utc(),
            step: step.to_string(),
            index,
            exit_code,
        });
    }

    pub fn emit_waiting(&self, step: &str) {
        self.emit(RuntimeEvent::RunWaiting {
            run_id: self.meta.run_id.clone(),
            timestamp: OffsetDateTime::now_utc(),
            step: step.to_string(),
        });
    }

    pub fn complete_success(&self) {
        if self.finished.replace(true) {
            return;
        }
        self.emit(RuntimeEvent::RunCompleted {
            run_id: self.meta.run_id.clone(),
            timestamp: OffsetDateTime::now_utc(),
            exit_code: 0,
        });
    }

    pub fn complete_failure(&self, error: &str) {
        if self.finished.replace(true) {
            return;
        }
        self.emit(RuntimeEvent::RunFailed {
            run_id: self.meta.run_id.clone(),
            timestamp: OffsetDateTime::now_utc(),
            exit_code: 1,
            error: error.to_string(),
        });
    }

    fn emit(&self, event: RuntimeEvent) {
        if let Err(err) = self.append_event(&event) {
            debug!(
                error = %err,
                run_id = %self.meta.run_id,
                "runtime journal append failed"
            );
        }
    }

    fn append_event(&self, event: &RuntimeEvent) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(events_path(&self.run_dir))?;
        serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

pub fn runs_root(worktree: &Path) -> PathBuf {
    worktree.join(RUNTIME_ROOT)
}

pub fn meta_path(run_dir: &Path) -> PathBuf {
    run_dir.join("meta.json")
}

pub fn events_path(run_dir: &Path) -> PathBuf {
    run_dir.join("events.jsonl")
}

pub fn read_meta(run_dir: &Path) -> Result<RuntimeRunMeta, std::io::Error> {
    let bytes = fs::read(meta_path(run_dir))?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}

pub fn read_events(run_dir: &Path) -> Result<Vec<RuntimeEvent>, std::io::Error> {
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

fn ensure_runtime_ignored(repo_root: &Path) -> Result<(), std::io::Error> {
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
        .any(|line| line.trim() == RUNTIME_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(RUNTIME_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)
}

#[cfg(test)]
mod tests {
    use super::{read_events, read_meta, RunTarget, RuntimeEvent, RuntimeRun};
    use crate::engine::git::is_clean;
    use loopflow_test_support::TestRepo;

    #[test]
    fn runtime_run_is_disabled_in_main_repo() {
        let repo = TestRepo::new();
        let command = vec!["lf".to_string(), "implement".to_string()];
        assert!(
            RuntimeRun::maybe_start(repo.path(), &command, RunTarget::step("implement")).is_none()
        );
    }

    #[test]
    fn runtime_run_writes_meta_and_events_in_wave_worktree() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];
        let run = RuntimeRun::maybe_start(&worktree, &command, RunTarget::flow("build"))
            .expect("runtime run");

        run.emit_step_started("implement", 0);
        run.emit_waiting("review-design");
        run.emit_step_completed("implement", 0, 0);
        run.complete_success();

        let meta = read_meta(&run.run_dir).expect("read meta");
        assert_eq!(meta.wave_name, "runtime");
        assert_eq!(meta.command, command);
        assert_eq!(meta.flow.as_deref(), Some("build"));

        let events = read_events(&run.run_dir).expect("read events");
        assert!(matches!(events[0], RuntimeEvent::RunStarted { .. }));
        assert!(matches!(events[1], RuntimeEvent::StepStarted { .. }));
        assert!(matches!(events[2], RuntimeEvent::RunWaiting { .. }));
        assert!(matches!(events[3], RuntimeEvent::StepCompleted { .. }));
        assert!(matches!(events[4], RuntimeEvent::RunCompleted { .. }));
    }

    #[test]
    fn runtime_run_keeps_worktree_clean() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("runtime");
        let command = vec!["lf".to_string(), "build".to_string()];

        let _run = RuntimeRun::maybe_start(&worktree, &command, RunTarget::flow("build"))
            .expect("runtime run");

        assert!(is_clean(&worktree).expect("worktree should stay clean"));
    }
}
