//! Fake Loopflow worlds for demos and tests, built through production writers.
//!
//! A World is a directory holding its own `LF_HOME`, provider homes and git
//! repository. Recipes in [`recipes`] describe what goes in it; everything is
//! written by the same ops and store calls `lf` uses, so a migration never
//! leaves a recipe behind. Times are relative to the moment of building.
//!
//! A World stands in for the outside programs Loopflow runs. `env.json` puts
//! its `bin/` first on `PATH`: a `claude` ([`STUB_CLAUDE`]) resumes a world
//! Session from its transcript and echoes each input line, so Sessions open live
//! without an account or network; a `gh` answers pull-request reads from files
//! under `github/pulls/` and refuses everything else, so nothing run inside a
//! World reaches GitHub.

pub mod recipes;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use crate::flow::graph::FlowGraph;
use crate::flow::{ConcreteSkill, ConcreteStep, Skill};
use crate::id::{LfProcessId, TraceId};
use crate::ops::human_session::provider_conversation;
use crate::process::{LfProcess, ProcessKind};
use crate::provider_auth::Provider;
use crate::session::TitleSource;
use crate::store::{SharedStore, StorageConfig};
use crate::work::edit::{Disposition, WorkEdit, WorkValue};
use crate::work::project::ProjectKR;
use crate::work::task::{Task, TaskEventKind};
use crate::work::wave::metrics::MetricTarget;
pub use crate::work::wave::metrics::Threshold;

const DAY: i64 = 24 * 60 * 60;
const GITHUB_REPO: &str = "https://github.com/demo/project";

/// Directories Desktop prepends to a child's `PATH` when absent
/// (`GUIProcessEnvironment.enrichedPath` in
/// `swift/LoopflowMac/Services/ProcessEnvironment.swift`). Listing them after
/// the World's `bin` leaves Desktop nothing to prepend ahead of its `gh`.
const DESKTOP_PATH: [&str; 6] = [
    "~/.local/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
    "~/.cargo/bin",
];

const GH: &str = r#"#!/bin/sh
# lf-world's GitHub. Answers the pull-request reads Loopflow makes from files
# the World's builder wrote under ../github/pulls; refuses everything else, so
# nothing run inside a World reaches GitHub.
pulls="$(cd "$(dirname "$0")/../github/pulls" 2>/dev/null && pwd)"
refuse() {
    echo "lf-world's GitHub answers only PR reads: $*" >&2
    exit 1
}
answer() {
    if [ -e "$pulls/$1.unavailable" ]; then
        echo "HTTP 502: Server Error (https://api.github.com/graphql)" >&2
        exit 1
    fi
    if [ ! -f "$pulls/$1.$2" ]; then
        echo "GraphQL: Could not resolve to a PullRequest with the number of $1. (repository.pullRequest)" >&2
        exit 1
    fi
    cat "$pulls/$1.$2"
    exit 0
}
case "$1 $2" in
"pr view")
    number="${3##*/}"
    for arg in "$@"; do
        [ "$arg" = "--jq" ] && answer "$number" heads
    done
    answer "$number" json
    ;;
"api --hostname")
    case "$6" in
    repos/*/*/pulls/*/files*)
        number="${6#repos/*/*/pulls/}"
        answer "${number%%/*}" files.json
        ;;
    esac
    ;;
esac
refuse "$@"
"#;

/// What the World's GitHub reports for one pull request.
#[derive(Debug, Clone)]
pub enum PrSurface {
    Published {
        draft: bool,
        /// Check name and GitHub result: `SUCCESS`, `FAILURE`, `IN_PROGRESS`.
        checks: &'static [(&'static str, &'static str)],
        /// `REVIEW_REQUIRED`, `APPROVED`, `CHANGES_REQUESTED` or empty.
        review: &'static str,
        reviewers: &'static [&'static str],
    },
    /// Every read fails the way an unavailable GitHub does.
    Unavailable,
}

/// The world's `claude`. `--resume <id>` prints `resumed <id>: <n> lines`
/// after reading the world transcript (exit 1 when there is none), then answers
/// each input line with `reply <id>: <line>` until `quit` or EOF. A new
/// conversation (`--session-id <id>`) echoes the same way. Transcripts are
/// never written. It needs only `sh` and `grep`, which the Desktop benchmark's
/// exec policy allows.
pub const STUB_CLAUDE: &str = r#"#!/bin/sh
# Stand-in claude for an lf-world world.
if [ "$1" = --version ]; then echo "2.1.0 (Claude Code)"; exit 0; fi
id=
new=
while [ $# -gt 0 ]; do
  case "$1" in
    --resume|-r) id=$2; shift ;;
    --session-id) id=$2; new=1; shift ;;
  esac
  shift
done
if [ -z "$id" ]; then echo "world claude only resumes or starts identified conversations" >&2; exit 2; fi
if [ -z "$new" ]; then
  transcript=
  for candidate in "$CLAUDE_CONFIG_DIR"/projects/*/"$id".jsonl; do
    [ -f "$candidate" ] && transcript=$candidate
  done
  if [ -z "$transcript" ]; then echo "No conversation found with session ID: $id" >&2; exit 1; fi
  lines=$(grep -c . "$transcript") || exit 1
  echo "resumed $id: $lines lines"
else
  echo "started $id"
fi
while IFS= read -r input; do
  [ "$input" = quit ] && exit 0
  echo "reply $id: $input"
done
"#;

/// The environment a World's commands run under, written as `env.json`.
#[derive(Debug, Clone, Serialize)]
pub struct WorldEnv {
    #[serde(rename = "LF_HOME")]
    pub lf_home: PathBuf,
    #[serde(rename = "CLAUDE_CONFIG_DIR")]
    pub claude_config_dir: PathBuf,
    #[serde(rename = "CODEX_HOME")]
    pub codex_home: PathBuf,
    /// The World's `bin` first, so its `claude` and `gh` answer instead of real ones.
    #[serde(rename = "PATH")]
    pub path: String,
}

impl WorldEnv {
    fn new(dir: &Path) -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let desktop = DESKTOP_PATH.map(|entry| match entry.strip_prefix("~/") {
            Some(rest) => home.join(rest).to_string_lossy().into_owned(),
            None => entry.to_string(),
        });
        let path = std::iter::once(dir.join("bin").to_string_lossy().into_owned())
            .chain(desktop)
            .chain(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(String::from))
            .collect::<Vec<_>>()
            .join(":");
        Self {
            lf_home: dir.join("home"),
            claude_config_dir: dir.join("claude"),
            codex_home: dir.join("codex"),
            path,
        }
    }

    fn vars(&self) -> [(&'static str, &std::ffi::OsStr); 4] {
        [
            ("LF_HOME", self.lf_home.as_os_str()),
            ("CLAUDE_CONFIG_DIR", self.claude_config_dir.as_os_str()),
            ("CODEX_HOME", self.codex_home.as_os_str()),
            ("PATH", std::ffi::OsStr::new(&self.path)),
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
        std::fs::create_dir_all(dir)?;
        let dir = dir.canonicalize()?;
        let env = WorldEnv::new(&dir);
        for path in [&env.lf_home, &env.claude_config_dir, &env.codex_home] {
            std::fs::create_dir_all(path)?;
        }
        // A builder launched from an lf Session inherits that Session's
        // identity; the world's writes must not attribute to it.
        for (name, _) in std::env::vars_os() {
            let name = name.to_string_lossy();
            if name.starts_with("LF_") && name != "LF_BIN" {
                std::env::remove_var(&*name);
            }
        }
        for (name, value) in env.vars() {
            std::env::set_var(name, value);
        }
        write_executable(&dir, "claude", STUB_CLAUDE)?;
        write_executable(&dir, "gh", GH)?;
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

    /// Claim a Task for this Machine's person the way `lf task claim` does.
    pub fn claim(&self, task: &Task) -> Result<()> {
        crate::ops::task_ownership::change(
            &self.repo,
            &task.identifier,
            crate::ops::task_ownership::OwnershipChange::Claim,
        )?;
        Ok(())
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
        let cwd = self.cwd(task)?;
        let flow = LfProcessId::new();
        let mut flow_process = self.process(&trace, None, &cwd, vec!["lf", "flow", name], started);
        flow_process.id = flow.clone();
        flow_process.completed_at = Some(ended);
        flow_process.outcome = Some(outcome.to_string());
        flow_process.exit_code = Some(i32::from(outcome != "succeeded"));
        store.sqlite.record_process(&flow_process)?;
        store.sqlite.record_flow_process(&flow, &graph)?;
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
    /// Session, bound to `task` when given. It resumes in the Task's checkout
    /// when it has one.
    pub fn session(
        &self,
        task: Option<&Task>,
        title: &str,
        turns: &[&str],
        started_days_ago: f64,
    ) -> Result<String> {
        let cwd = task
            .and_then(|task| task.worktree.clone())
            .unwrap_or_else(|| self.repo.clone());
        let id = self.transcript(&cwd, turns, started_days_ago)?;
        let store = self.store()?;
        let session = self
            .runtime
            .block_on(provider_conversation::admit(&store, &id))?
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

    /// Open `wave`'s ongoing conversation the way `lf wave session` does and
    /// give it `turns` as its Claude history.
    pub fn wave_conversation(
        &self,
        wave: &str,
        turns: &[&str],
        started_days_ago: f64,
    ) -> Result<String> {
        let store = self.store()?;
        let record = self
            .runtime
            .block_on(crate::ops::human_session::primary::ensure(
                &store,
                &self.repo,
                Some(wave),
                false,
            ))?;
        let session = self
            .runtime
            .block_on(store.session(record.id.as_str()))?
            .ok_or_else(|| anyhow!("Wave {wave}'s conversation disappeared"))?;
        let id = self.transcript(&session.cwd, turns, started_days_ago)?;
        self.runtime.block_on(provider_conversation::select(
            &store,
            &session,
            Provider::Claude,
            &id,
            None,
        ))?;
        Ok(session.id.to_string())
    }

    /// Write `files` into `task`'s checkout (the repository when absent) and
    /// commit them. Repository commits are pushed, so later checkouts start
    /// from them.
    pub fn files(
        &self,
        task: Option<&Task>,
        files: &[(impl AsRef<str>, impl AsRef<str>)],
        message: &str,
    ) -> Result<()> {
        let cwd = self.cwd(task)?;
        let paths = write_files(&cwd, files)?;
        git(Some(&cwd), &[&["add", "--"][..], &paths[..]].concat())?;
        git(Some(&cwd), &["commit", "-m", message])?;
        if task.is_none() {
            git(Some(&cwd), &["push", "origin", "main"])?;
        }
        Ok(())
    }

    /// Write `files` into `task`'s checkout without committing them.
    pub fn uncommitted(
        &self,
        task: &Task,
        files: &[(impl AsRef<str>, impl AsRef<str>)],
    ) -> Result<()> {
        write_files(&self.cwd(Some(task))?, files)?;
        Ok(())
    }

    /// Publish `task`'s checkout as pull request `number` the way `lf pr
    /// publish` records it, and have the World's GitHub report `surface` for
    /// it. Its files and counts come from the checkout's commits.
    pub fn pr(
        &self,
        task: &Task,
        number: u64,
        title: &str,
        body: &str,
        surface: &PrSurface,
    ) -> Result<()> {
        let worktree = self.cwd(Some(task))?;
        let body = format!(
            "{body}\n\n> **Task:** {} · {}\n",
            task.title, task.identifier
        );
        crate::ops::task::request_task_pr_publication(&worktree, title, &body)?;
        let head = git(Some(&worktree), &["rev-parse", "HEAD"])?;
        let url = format!("{GITHUB_REPO}/pull/{number}");
        crate::ops::task::attach_task_github_pr(
            &worktree,
            Some(&crate::ops::PrInfo {
                url: url.clone(),
                number,
                state: "open".to_string(),
                branch: task.branch.clone(),
                merge_commit: None,
                merged_at: None,
                head_sha: Some(head.clone()),
                merge_state: None,
            }),
            &|_| {},
        )?;

        let pulls = self.pulls()?;
        let PrSurface::Published {
            draft,
            checks,
            review,
            reviewers,
        } = surface
        else {
            std::fs::write(pulls.join(format!("{number}.unavailable")), "")?;
            return Ok(());
        };
        let base = &task.base_commit;
        let files = pr_files(&worktree, base, &head)?;
        let total = |key: &str| {
            files
                .iter()
                .map(|file| file[key].as_u64().unwrap_or(0))
                .sum::<u64>()
        };
        let checks: Vec<_> = checks
            .iter()
            .map(|(name, result)| {
                let pending = ["QUEUED", "IN_PROGRESS", "PENDING"].contains(result);
                serde_json::json!({
                    "__typename": "CheckRun",
                    "name": name,
                    "status": if pending { *result } else { "COMPLETED" },
                    "conclusion": if pending { "" } else { *result },
                    "detailsUrl": format!("{GITHUB_REPO}/actions/runs/{number}"),
                })
            })
            .collect();
        let reviewers: Vec<_> = reviewers
            .iter()
            .map(|login| serde_json::json!({"__typename": "User", "login": login}))
            .collect();
        let pr = serde_json::json!({
            "number": number,
            "url": url,
            "title": title,
            "body": body,
            "state": "OPEN",
            "isDraft": draft,
            "additions": total("additions"),
            "deletions": total("deletions"),
            "changedFiles": files.len(),
            "headRefOid": head,
            "baseRefOid": base,
            "headRefName": task.branch,
            "baseRefName": "main",
            "reviewDecision": review,
            "reviewRequests": reviewers,
            "statusCheckRollup": checks,
        });
        std::fs::write(
            pulls.join(format!("{number}.json")),
            serde_json::to_string_pretty(&pr)? + "\n",
        )?;
        std::fs::write(
            pulls.join(format!("{number}.heads")),
            format!("{head} {base}\n"),
        )?;
        std::fs::write(
            pulls.join(format!("{number}.files.json")),
            serde_json::to_string(&[files])? + "\n",
        )?;
        Ok(())
    }

    /// Merge `task`'s published branch into `main` and push it, then settle
    /// its pull request as merged the way GitHub observation does. The Task
    /// stays open until completed.
    pub fn merge(&self, task: &Task) -> Result<()> {
        let store = self.store()?;
        let mut pr = self
            .runtime
            .block_on(store.active_task_pr(&task.id))?
            .ok_or_else(|| anyhow!("Task {} has no pull request", task.identifier))?;
        let github = pr
            .github()
            .cloned()
            .ok_or_else(|| anyhow!("Task {}'s pull request is unpublished", task.identifier))?;
        git(
            Some(&self.repo),
            &[
                "merge",
                "--no-ff",
                "-m",
                &format!("Merge pull request #{} from {}", github.number, pr.branch),
                &pr.branch,
            ],
        )?;
        git(Some(&self.repo), &["push", "origin", "main"])?;
        let merge_commit = git(Some(&self.repo), &["rev-parse", "HEAD"])?;
        let now = time::OffsetDateTime::now_utc();
        pr.merge_commit = Some(merge_commit.clone());
        pr.updated_at = now;
        self.runtime
            .block_on(store.settle_task_pr_merged(&pr, Some(now)))?;
        self.runtime.block_on(store.append_task_event(
            &task.id,
            &TaskEventKind::PrMerged {
                pr_id: pr.id.clone(),
                sequence: pr.sequence,
                number: github.number,
                url: github.url,
                merge_commit,
            },
        ))?;
        let stub = self.pulls()?.join(format!("{}.json", github.number));
        if stub.exists() {
            let mut published: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&stub)?)?;
            published["state"] = "MERGED".into();
            std::fs::write(&stub, serde_json::to_string_pretty(&published)? + "\n")?;
        }
        Ok(())
    }

    /// Write `env.json` and return the environment.
    pub fn finish(self) -> Result<WorldEnv> {
        std::fs::write(
            self.dir.join("env.json"),
            serde_json::to_string_pretty(&self.env)? + "\n",
        )?;
        Ok(self.env)
    }

    /// `task`'s checkout, or the repository when absent.
    fn cwd(&self, task: Option<&Task>) -> Result<PathBuf> {
        match task {
            Some(task) => task.worktree.clone().ok_or_else(|| {
                anyhow!(
                    "Task {} has no checkout; call checkout first",
                    task.identifier
                )
            }),
            None => Ok(self.repo.clone()),
        }
    }

    fn pulls(&self) -> Result<PathBuf> {
        let pulls = self.dir.join("github/pulls");
        std::fs::create_dir_all(&pulls)?;
        Ok(pulls)
    }

    /// Write a Claude transcript of alternating turns recorded in `cwd` and
    /// return its conversation id.
    fn transcript(&self, cwd: &Path, turns: &[&str], started_days_ago: f64) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        // Claude names a project directory after its path, every other
        // character a dash.
        let directory: String = cwd
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let project = self.env.claude_config_dir.join("projects").join(directory);
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
                "cwd": cwd,
                "timestamp": at,
                "userType": "external",
                "message": message,
            }))?);
            parent = Some(uuid);
        }
        std::fs::write(project.join(format!("{id}.jsonl")), lines.join("\n") + "\n")?;
        Ok(id)
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
    // Worlds write Claude history, so conversations they open are Claude's.
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

fn write_executable(dir: &Path, name: &str, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin)?;
    let path = bin.join(name);
    std::fs::write(&path, body)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn write_files<'a>(
    cwd: &Path,
    files: &'a [(impl AsRef<str>, impl AsRef<str>)],
) -> Result<Vec<&'a str>> {
    let mut paths = Vec::with_capacity(files.len());
    for (path, content) in files {
        let target = cwd.join(path.as_ref());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, content.as_ref())?;
        paths.push(path.as_ref());
    }
    Ok(paths)
}

/// GitHub's file list for `base..head`, from git: status, counts and the
/// patch hunks without their file header.
fn pr_files(cwd: &Path, base: &str, head: &str) -> Result<Vec<serde_json::Value>> {
    let range = format!("{base}..{head}");
    let statuses = git(
        Some(cwd),
        &["diff", "--no-renames", "--name-status", &range],
    )?;
    let counts = git(Some(cwd), &["diff", "--no-renames", "--numstat", &range])?;
    statuses
        .lines()
        .zip(counts.lines())
        .map(|(status, count)| {
            let (status, path) = status
                .split_once('\t')
                .ok_or_else(|| anyhow!("unexpected git status line {status:?}"))?;
            let mut count = count.splitn(3, '\t');
            let mut number = || {
                count
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
                    .unwrap_or(0)
            };
            let (additions, deletions) = (number(), number());
            let patch = git(Some(cwd), &["diff", "--no-renames", &range, "--", path])?;
            let patch = patch
                .find("\n@@")
                .map(|start| patch[start + 1..].to_string());
            Ok(serde_json::json!({
                "filename": path,
                "status": match status {
                    "A" => "added",
                    "D" => "removed",
                    _ => "modified",
                },
                "additions": additions,
                "deletions": deletions,
                "patch": patch,
                "blob_url": format!("{GITHUB_REPO}/blob/{head}/{path}"),
            }))
        })
        .collect()
}

/// Run git as the World's author and return its trimmed stdout.
fn git(cwd: Option<&Path>, args: &[&str]) -> Result<String> {
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
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
