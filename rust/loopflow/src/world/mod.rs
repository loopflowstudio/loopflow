//! Fake Loopflow worlds for demos and tests, built through production writers.
//!
//! A World is a directory holding its own `LF_HOME`, provider homes and git
//! repository. Recipes in [`recipes`] describe what goes in it; everything is
//! written by the same ops and store calls `lf` uses, so a migration never
//! leaves a recipe behind. Times are relative to the moment of building.

pub mod recipes;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use crate::flow::graph::FlowGraph;
use crate::flow::{ConcreteSkill, ConcreteStep, Skill};
use crate::id::{LfProcessId, TraceId};
use crate::process::{LfProcess, ProcessKind};
use crate::session::TitleSource;
use crate::store::{SharedStore, StorageConfig};
use crate::work::edit::{Disposition, WorkEdit, WorkValue};
use crate::work::project::ProjectKR;
use crate::work::task::Task;
use crate::work::wave::metrics::MetricTarget;
pub use crate::work::wave::metrics::Threshold;

const DAY: i64 = 24 * 60 * 60;

/// The environment a World's commands run under, written as `env.json`.
#[derive(Debug, Clone, Serialize)]
pub struct WorldEnv {
    #[serde(rename = "LF_HOME")]
    pub lf_home: PathBuf,
    #[serde(rename = "CLAUDE_CONFIG_DIR")]
    pub claude_config_dir: PathBuf,
    #[serde(rename = "CODEX_HOME")]
    pub codex_home: PathBuf,
}

impl WorldEnv {
    fn new(dir: &Path) -> Self {
        Self {
            lf_home: dir.join("home"),
            claude_config_dir: dir.join("claude"),
            codex_home: dir.join("codex"),
        }
    }

    fn vars(&self) -> [(&'static str, &Path); 3] {
        [
            ("LF_HOME", &self.lf_home),
            ("CLAUDE_CONFIG_DIR", &self.claude_config_dir),
            ("CODEX_HOME", &self.codex_home),
        ]
    }
}

/// One fake world under construction. The process environment points at it
/// for as long as it lives, so build one World per process.
#[derive(Debug)]
pub struct World {
    dir: PathBuf,
    env: WorldEnv,
    repo: PathBuf,
    now: i64,
    runtime: tokio::runtime::Runtime,
}

impl World {
    /// Create an empty world in `dir`, which must not exist yet.
    ///
    /// # Errors
    /// When `dir` exists, git is unavailable, or the store cannot open.
    pub fn create(dir: &Path) -> Result<Self> {
        if dir.exists() {
            bail!("{} already exists; worlds are built fresh", dir.display());
        }
        let env = WorldEnv::new(dir);
        for path in [&env.lf_home, &env.claude_config_dir, &env.codex_home] {
            std::fs::create_dir_all(path)?;
        }
        let dir = dir.canonicalize()?;
        let env = WorldEnv::new(&dir);
        for (name, value) in env.vars() {
            std::env::set_var(name, value);
        }
        let repo = dir.join("repo");
        create_repo(&repo, &dir.join("origin.git"))?;
        // Manual membership: folder mode would scan the person's own ~/src.
        crate::portfolio::change(&crate::portfolio::PortfolioCommand::Manual)?;
        crate::portfolio::change(&crate::portfolio::PortfolioCommand::Add {
            paths: vec![repo.clone()],
        })?;
        Ok(Self {
            dir,
            env,
            repo,
            now: crate::store::rows::now_unix(),
            runtime: tokio::runtime::Runtime::new()?,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Unix time `days` before this world was built.
    pub fn days_ago(&self, days: f64) -> i64 {
        self.now - (days * DAY as f64) as i64
    }

    /// Ensure a Wave and its current Project.
    pub fn wave(&self, name: &str) -> Result<()> {
        self.runtime
            .block_on(crate::ops::project::ensure(&self.repo, name))?;
        Ok(())
    }

    /// Set the current Project's KRs and metric targets, as `lf wave edit-plan`
    /// does. Targets name metrics already authored with [`Self::metric`].
    pub fn plan(
        &self,
        wave: &str,
        krs: &[(&str, bool)],
        targets: &[(&str, Threshold)],
    ) -> Result<()> {
        let krs: Vec<ProjectKR> = krs
            .iter()
            .map(|(text, holds)| ProjectKR {
                text: (*text).to_string(),
                holds: *holds,
            })
            .collect();
        let targets: Vec<MetricTarget> = targets
            .iter()
            .map(|(metric_id, target)| MetricTarget {
                metric_id: (*metric_id).to_string(),
                target: target.clone(),
            })
            .collect();
        self.runtime.block_on(crate::ops::project::update_plan(
            &self.repo,
            Some(wave),
            &krs,
            &targets,
        ))?;
        Ok(())
    }

    /// Author a weekly metric contract for `wave` and, with `value`, publish one
    /// complete observation for the week ending `ended_days_ago`, the way a
    /// metric producer does. `None` leaves the metric uninstrumented.
    pub fn metric(
        &self,
        wave: &str,
        id: &str,
        title: &str,
        stage: &str,
        value: Option<f64>,
        ended_days_ago: f64,
    ) -> Result<()> {
        let instrument = "world";
        let dir = self.repo.join(".lf/wave").join(wave).join("metrics");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(format!("{id}.md")),
            format!(
                "---\nschema: 1\nid: {id}\nstage: {stage}\ninstrument: {instrument}\n\
                 unit: count\nwindow: 7d\nfreshness: 8d\n---\n\n# {title}\n\n\
                 Counted over the latest complete week.\n"
            ),
        )?;
        let Some(value) = value else {
            return Ok(());
        };
        let end = time::OffsetDateTime::from_unix_timestamp(self.days_ago(ended_days_ago))?;
        let store = self.store()?;
        self.runtime
            .block_on(crate::ops::metrics::publish_metric_observations(
                &store,
                &self.repo,
                vec![crate::ops::metrics::MetricProducerObservation::Observed {
                    wave: wave.to_string(),
                    metric_id: id.to_string(),
                    instrument: instrument.to_string(),
                    value,
                    source_window_start: end - time::Duration::days(7),
                    source_window_end: end,
                    complete: true,
                }],
                time::OffsetDateTime::now_utc(),
            ))?;
        Ok(())
    }

    /// Create an unstarted Task in `wave`, or a standalone one without.
    pub fn task(&self, wave: Option<&str>, title: &str, report: &str) -> Result<Task> {
        Ok(crate::ops::task::task_create(
            &self.repo,
            wave,
            Some(title.to_string()),
            Some(report.to_string()),
        )?)
    }

    /// Place a Task in its own worktree the way `lf task checkout` does.
    pub fn checkout(&self, task: &Task) -> Result<Task> {
        Ok(crate::ops::task::task_checkout(
            &self.repo,
            &task.identifier,
            crate::ops::task::TaskCheckoutOptions::default(),
        )?)
    }

    /// Complete a Task the way `lf task complete` does, then date the
    /// completion `completed_days_ago` through the planning writer.
    pub fn complete(&self, task: &Task, note: &str, completed_days_ago: f64) -> Result<Task> {
        let task = crate::ops::task::task_complete(&self.repo, &task.identifier, Some(note))?;
        let at = time::OffsetDateTime::from_unix_timestamp(self.days_ago(completed_days_ago))?
            .format(&time::format_description::well_known::Rfc3339)?;
        self.dispose(
            &task,
            Disposition {
                planning_state: Some("completed".into()),
                planning_completed: 1,
                planning_completed_at: Some(at),
            },
        )?;
        Ok(task)
    }

    /// Pause a Task the way `lf task pause` does.
    pub fn pause(&self, task: &Task) -> Result<()> {
        Ok(crate::ops::task::task_pause(&task.identifier, true)?)
    }

    /// Abandon a Task the way `lf task abandon` does, removing its checkout.
    pub fn abandon(&self, task: &Task) -> Result<()> {
        crate::ops::task::task_abandon(&self.repo, Some(&task.identifier), true)?;
        Ok(())
    }

    /// Cancel a Task's planning the way an accepted Linear cancellation does,
    /// leaving its local runtime alone.
    pub fn cancel_upstream(&self, task: &Task) -> Result<()> {
        self.dispose(
            task,
            Disposition {
                planning_state: Some("canceled".into()),
                planning_completed: 0,
                planning_completed_at: None,
            },
        )
    }

    /// Delete a Task's checkout folder, leaving Git's worktree metadata, as a
    /// person removing the directory would.
    pub fn remove_checkout(&self, task: &Task) -> Result<()> {
        let worktree = task
            .worktree
            .as_ref()
            .ok_or_else(|| anyhow!("Task {} has no checkout", task.identifier))?;
        std::fs::remove_dir_all(worktree)?;
        Ok(())
    }

    fn dispose(&self, task: &Task, disposition: Disposition) -> Result<()> {
        let store = self.store()?;
        let mut conn = store.sqlite.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        crate::store::sqlite::planning_write::local(
            &tx,
            &WorkEdit::for_target(task.id.as_str(), &[WorkValue::Disposition(disposition)]),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Record a finished Flow run in `task`'s checkout (the repository when
    /// absent), one step Process per skill, each lasting `minutes` and the last
    /// ending `ended_days_ago`. A Flow in a checkout starts its Task.
    pub fn flow(
        &self,
        task: Option<&Task>,
        name: &str,
        skills: &[&str],
        ended_days_ago: f64,
        minutes: i64,
        outcome: &str,
    ) -> Result<LfProcessId> {
        let store = self.store()?;
        let steps: Vec<ConcreteStep> = skills
            .iter()
            .map(|skill| {
                ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named(skill),
                    id: None,
                    human: false,
                    returns: None,
                    sources: Vec::new(),
                })
            })
            .collect();
        let graph = FlowGraph::new(name, &steps);
        let trace = TraceId::new();
        let ended = self.days_ago(ended_days_ago);
        let started = ended - minutes * 60 * skills.len() as i64;
        let cwd = match task {
            Some(task) => task.worktree.clone().ok_or_else(|| {
                anyhow!(
                    "Task {} has no checkout; call checkout first",
                    task.identifier
                )
            })?,
            None => self.repo.clone(),
        };
        let flow = LfProcessId::new();
        let mut flow_process = self.process(&trace, None, &cwd, vec!["lf", "flow", name], started);
        flow_process.id = flow.clone();
        flow_process.completed_at = Some(ended);
        flow_process.outcome = Some(outcome.to_string());
        flow_process.exit_code = Some(i32::from(outcome != "succeeded"));
        store.sqlite.record_process(&flow_process)?;
        store.sqlite.record_flow_process(&flow, &graph, None)?;
        for (index, skill) in skills.iter().enumerate() {
            let begin = started + minutes * 60 * index as i64;
            let mut step = self.process(
                &trace,
                Some(&flow),
                &cwd,
                vec!["lf", "-b", "skill", skill],
                begin,
            );
            step.completed_at = Some(begin + minutes * 60);
            let last = index + 1 == skills.len();
            step.outcome = Some(if last { outcome } else { "succeeded" }.to_string());
            step.exit_code = Some(i32::from(last && outcome != "succeeded"));
            store.sqlite.record_process(&step)?;
            store
                .sqlite
                .record_flow_step(&flow, &step.id, index as u32, &[Vec::new()])?;
        }
        Ok(flow)
    }

    /// Write a Claude conversation of alternating turns and admit it as a
    /// Session, bound to `task` when given.
    pub fn session(
        &self,
        task: Option<&Task>,
        title: &str,
        turns: &[&str],
        started_days_ago: f64,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let project = self
            .env
            .claude_config_dir
            .join("projects")
            .join(self.repo.to_string_lossy().replace('/', "-"));
        std::fs::create_dir_all(&project)?;
        let started = self.days_ago(started_days_ago);
        let mut lines = Vec::with_capacity(turns.len());
        let mut parent: Option<String> = None;
        for (index, text) in turns.iter().enumerate() {
            let uuid = uuid::Uuid::new_v4().to_string();
            let at = time::OffsetDateTime::from_unix_timestamp(started + 90 * index as i64)?
                .format(&time::format_description::well_known::Rfc3339)?;
            let (kind, message) = if index % 2 == 0 {
                ("user", serde_json::json!({"role": "user", "content": text}))
            } else {
                (
                    "assistant",
                    serde_json::json!({
                        "role": "assistant",
                        "model": "claude-opus-5-5",
                        "content": [{"type": "text", "text": text}],
                    }),
                )
            };
            lines.push(serde_json::to_string(&serde_json::json!({
                "type": kind,
                "uuid": uuid,
                "parentUuid": parent,
                "sessionId": id,
                "cwd": self.repo,
                "timestamp": at,
                "userType": "external",
                "message": message,
            }))?);
            parent = Some(uuid);
        }
        std::fs::write(project.join(format!("{id}.jsonl")), lines.join("\n") + "\n")?;

        let store = self.store()?;
        let session = self
            .runtime
            .block_on(crate::ops::human_session::provider_conversation::admit(
                &store, &id,
            ))?
            .ok_or_else(|| anyhow!("Claude conversation {id} was not found after writing it"))?;
        let session_id = session.id.to_string();
        if let Some(task) = task {
            self.runtime
                .block_on(store.bind_session(&session_id, session.captured, &task.id))?;
        }
        self.runtime
            .block_on(store.rename_session(&session_id, title, TitleSource::Human))?;
        Ok(session_id)
    }

    /// Write `env.json` and return the environment.
    pub fn finish(self) -> Result<WorldEnv> {
        std::fs::write(
            self.dir.join("env.json"),
            serde_json::to_string_pretty(&self.env)? + "\n",
        )?;
        Ok(self.env)
    }

    fn process(
        &self,
        trace: &TraceId,
        parent: Option<&LfProcessId>,
        cwd: &Path,
        argv: Vec<&str>,
        started_at: i64,
    ) -> LfProcess {
        LfProcess {
            id: LfProcessId::new(),
            pid: None,
            kind: ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            trace_id: trace.clone(),
            parent_lf_process_id: parent.cloned(),
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
            command: Some(serde_json::to_string(&argv).expect("argv serializes")),
            repo: Some(self.repo.to_string_lossy().to_string()),
            cwd: Some(cwd.to_string_lossy().to_string()),
            started_at,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        }
    }

    fn store(&self) -> Result<SharedStore> {
        let config = StorageConfig::sqlite(self.env.lf_home.join("loopflow.db"));
        let store = self.runtime.block_on(crate::store::open_store(&config))?;
        Ok(std::sync::Arc::new(store))
    }
}

fn create_repo(repo: &Path, origin: &Path) -> Result<()> {
    git(
        None,
        &["init", "--bare", "-b", "main", &origin.to_string_lossy()],
    )?;
    git(None, &["init", "-b", "main", &repo.to_string_lossy()])?;
    std::fs::write(repo.join("README.md"), "# Demo project\n")?;
    // Sessions are Claude's; pinning the agent keeps launch evidence the same
    // whichever providers this machine has installed.
    std::fs::create_dir_all(repo.join(".lf"))?;
    std::fs::write(repo.join(".lf/config.yaml"), "agent: claude\n")?;
    git(Some(repo), &["add", "README.md", ".lf/config.yaml"])?;
    git(Some(repo), &["commit", "-m", "Start the demo project"])?;
    git(
        Some(repo),
        &["remote", "add", "origin", &origin.to_string_lossy()],
    )?;
    git(Some(repo), &["push", "-u", "origin", "main"])?;
    Ok(())
}

fn git(cwd: Option<&Path>, args: &[&str]) -> Result<()> {
    let mut command = Command::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .args(["-c", "user.name=Demo", "-c", "user.email=demo@example.com"])
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
