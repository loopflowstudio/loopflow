//! Read identity without preparing Work, claiming a client, or inferring remote
//! absence. The read timestamp is not planning freshness or execution admission.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::store::SharedStore;

use super::run::{select_execution_work, select_work, WorkSelection};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ContextFact {
    Bound { value: String, source: String },
    Unbound,
    Unavailable { reason: String },
}

impl ContextFact {
    fn bound(value: impl ToString, source: &str) -> Self {
        Self::Bound {
            value: value.to_string(),
            source: source.into(),
        }
    }

    fn unavailable(reason: impl ToString) -> Self {
        Self::Unavailable {
            reason: reason.to_string(),
        }
    }

    fn render(&self) -> String {
        match self {
            Self::Bound { value, source } => format!("{value} ({source})"),
            Self::Unbound => "unbound".into(),
            Self::Unavailable { reason } => format!("unavailable: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextExplanation {
    pub observed_at: i64,
    pub machine: ContextFact,
    pub repository: ContextFact,
    pub repository_path: ContextFact,
    pub checkout: ContextFact,
    pub execution_machine: ContextFact,
    pub wave: ContextFact,
    pub task: ContextFact,
    pub session: ContextFact,
    pub process: ContextFact,
    pub planning_observed_at: Option<i64>,
}

impl ContextExplanation {
    pub(crate) fn empty() -> Self {
        Self {
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            machine: ContextFact::Unbound,
            repository: ContextFact::Unbound,
            repository_path: ContextFact::Unbound,
            checkout: ContextFact::Unbound,
            execution_machine: ContextFact::Unbound,
            wave: ContextFact::Unbound,
            task: ContextFact::Unbound,
            session: ContextFact::Unbound,
            process: ContextFact::Unbound,
            planning_observed_at: None,
        }
    }

    pub(crate) fn unavailable(reason: impl ToString) -> Self {
        let fact = ContextFact::unavailable(reason);
        Self {
            machine: fact.clone(),
            repository: fact.clone(),
            repository_path: fact.clone(),
            checkout: fact.clone(),
            execution_machine: fact.clone(),
            wave: fact.clone(),
            task: fact.clone(),
            session: fact.clone(),
            process: fact,
            ..Self::empty()
        }
    }

    pub fn render(&self) -> String {
        let mut lines = vec![format!(
            "Identity reading: {} (not start permission)",
            self.observed_at
        )];
        for (label, fact) in [
            ("Machine", &self.machine),
            ("Repository", &self.repository),
            ("Repository path", &self.repository_path),
            ("Checkout", &self.checkout),
            ("Execution Machine", &self.execution_machine),
            ("Wave", &self.wave),
            ("Task", &self.task),
            ("Session", &self.session),
            ("Process", &self.process),
        ] {
            lines.push(format!("{label}: {}", fact.render()));
        }
        lines.push(format!(
            "Planning observed: {}",
            self.planning_observed_at
                .map(|at| at.to_string())
                .unwrap_or_else(|| "unknown".into())
        ));
        lines.join("\n")
    }
}

/// Explicit subjects and checkout inference share the launch identity reader.
/// Session/Process arguments name recorded history; neither requests takeover.
pub async fn explain_context(
    store: &SharedStore,
    cwd: &Path,
    work: WorkSelection<'_>,
    session: Option<&str>,
    process: Option<&str>,
) -> ContextExplanation {
    let mut report = ContextExplanation::empty();
    if let Err(error) = read_context(store, cwd, work, session, process, &mut report).await {
        // Preserve independently read provenance; failure never becomes unbound.
        let failure = ContextFact::unavailable(error);
        for fact in [
            &mut report.repository,
            &mut report.repository_path,
            &mut report.checkout,
            &mut report.execution_machine,
            &mut report.wave,
            &mut report.task,
        ] {
            if matches!(fact, ContextFact::Unbound) {
                *fact = failure.clone();
            }
        }
    }
    report
}

async fn read_context(
    store: &SharedStore,
    cwd: &Path,
    work: WorkSelection<'_>,
    session: Option<&str>,
    process: Option<&str>,
    report: &mut ContextExplanation,
) -> anyhow::Result<()> {
    let local = match store.local_machine().await {
        Ok(machine) => machine.id,
        Err(error) => {
            report.machine = ContextFact::unavailable(&error);
            return Err(error.into());
        }
    };
    report.machine = ContextFact::bound(&local, "observing_machine");
    let mut directory = cwd.to_path_buf();
    let mut source = "checkout";
    let mut session_record = None;
    if let Some(selector) = process {
        let record = match store.resolve_process(selector).await {
            Ok(Some(record)) => record,
            result => {
                let error = result.err().map(|e| e.to_string()).unwrap_or_else(|| {
                    format!("Process {selector} is not recorded on this Machine")
                });
                report.process = ContextFact::unavailable(&error);
                anyhow::bail!(error);
            }
        };
        report.process = ContextFact::bound(record.lfid, "explicit");
        // The caller is provenance, not the work this command performed.
        if let Some(id) = record.caller_session_id {
            report.session = ContextFact::bound(id, "process_caller");
        }
        let Some(path) = record.cwd else {
            anyhow::bail!("Process has no recorded checkout")
        };
        directory = path.into();
        source = "process_checkout";
    } else if let Some(selector) = session {
        let record = match super::human_session::session_by_id(store, selector).await {
            Ok(Some(record)) => record,
            result => {
                let error = result.err().map(|e| e.to_string()).unwrap_or_else(|| {
                    format!("Session {selector} is not recorded on this Machine")
                });
                report.session = ContextFact::unavailable(&error);
                anyhow::bail!(error);
            }
        };
        report.session = ContextFact::bound(&record.id, "explicit");
        directory = record.cwd.clone();
        source = "session_checkout";
        session_record = Some(record);
    } else if let Some(id) = crate::journal::current_process_lfid() {
        report.process = ContextFact::bound(id, "inspecting_process");
    }
    let session_task = if let Some(record) = &session_record {
        let tasks = store.sqlite.session_task_ids(&record.id)?;
        anyhow::ensure!(
            tasks.len() <= 1,
            "Session belongs to multiple Tasks; select an explicit Task to inspect its context"
        );
        tasks.first().map(ToString::to_string)
    } else {
        None
    };
    let session_wave = session_record
        .as_ref()
        .and_then(|s| s.wave_id.as_ref())
        .map(ToString::to_string);
    let selection = WorkSelection {
        task: work.task.or(session_task.as_deref()),
        wave: work.wave.or(session_wave.as_deref()),
    };
    // Repo/Wave conversations must not acquire checkout membership merely
    // because their retained cwd happens to be a Task checkout.
    let selected = if session.is_some() && selection.task.is_none() && selection.wave.is_none() {
        None
    } else if session.is_none()
        && process.is_none()
        && selection.task.is_none()
        && selection.wave.is_none()
    {
        select_execution_work(store, &directory).await?
    } else {
        select_work(store, &directory, selection).await?
    };
    let repo_path = if let Some(selected) = selected {
        let provenance = if work.task.is_some() || work.wave.is_some() {
            "explicit"
        } else if session_task.is_some() || session_wave.is_some() {
            "session_membership"
        } else if selected.source == crate::session::WorkSource::Declared {
            "inherited_declaration"
        } else {
            source
        };
        report.wave = ContextFact::bound(selected.wave.id(), provenance);
        if let Some(task) = selected.task {
            report.task = ContextFact::bound(&task.id, provenance);
            report.planning_observed_at = task.plan.pm_snapshot_synced_at;
            if let Some(checkout) = store.task_checkout(&task.id).await? {
                report.checkout =
                    ContextFact::bound(checkout.worktree.display(), "recorded_checkout");
                report.execution_machine = match checkout.machine_id {
                    Some(machine) => ContextFact::bound(machine, "recorded_checkout"),
                    None => ContextFact::unavailable("Recorded checkout Machine is unknown"),
                };
            } else {
                // Local absence is not a distributed negative observation.
                report.execution_machine = ContextFact::unavailable(
                    "No local execution location; peer execution has not been observed",
                );
            }
        } else if work.wave.is_some() {
            let repo = Path::new(selected.wave.repo());
            if !crate::repository::CanonicalRepo::discover(repo)
                .is_ok_and(|repo| repo.contains(&directory))
            {
                directory = repo.to_path_buf();
                source = "wave_repository";
            }
        }
        Some(selected.wave.repo().to_string())
    } else {
        crate::repository::CanonicalRepo::discover(&directory)
            .ok()
            .map(|repo| repo.to_string())
    };
    if matches!(report.task, ContextFact::Unbound) {
        match crate::engine::git::worktree_root(&directory) {
            Ok(root) => {
                report.checkout = ContextFact::bound(root.display(), source);
                report.execution_machine = ContextFact::bound(local, "local_checkout");
            }
            Err(error) if work.wave.is_some() || session.is_some() || process.is_some() => {
                report.checkout = ContextFact::unavailable(error);
                report.execution_machine =
                    ContextFact::unavailable("Selected location is not available locally");
            }
            Err(_) => {}
        }
    }
    if let Some(path) = repo_path {
        report.repository_path = ContextFact::bound(&path, "local_locator");
        report.repository = match store.repository_id(&path).await? {
            Some(id) => ContextFact::bound(id, "selected_plan"),
            None => ContextFact::Unbound,
        };
    }
    Ok(())
}
