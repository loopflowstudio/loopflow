//! One Wave's replaceable plan and the receipt for its chapter boundary.

use serde::{Deserialize, Serialize};

use crate::id::WaveId;
use crate::pm::{PmItem, PmProject, ProjectContent};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChapterId(String);

impl ChapterId {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty()
            || value.len() > 160
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        {
            return Err("chapter id must contain 1–160 letters, digits, '-', '_', or '.'".into());
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDisposition {
    Move,
    Abandon,
    Historical,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterTask {
    pub task: PmItem,
    pub disposition: TaskDisposition,
    pub reason: String,
    pub applied: bool,
    pub observed_at: i64,
    pub at_boundary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterPhase {
    Preview,
    Preparing,
    Transferring,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub predecessor_metrics: Vec<ChapterMetricEvidence>,
    pub id: ChapterId,
    pub wave_id: WaveId,
    pub wave: String,
    pub project_id: String,
    pub content: ProjectContent,
    pub predecessors: Vec<PmProject>,
    pub tasks: Vec<ChapterTask>,
    pub phase: ChapterPhase,
    pub created_at: i64,
    pub activated_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterMetricEvidence {
    pub project_id: String,
    pub evaluated_at: i64,
    pub portfolio: crate::controller::wave::metrics::MetricPortfolioDto,
}

/// A dated chapter read. Closed chapters read their successor's frozen boundary
/// evidence, so a Task shipping later never changes the earlier chapter's proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterSnapshot {
    pub metrics: crate::controller::wave::metrics::MetricPortfolioDto,
    pub metrics_evaluated_at: i64,
    pub id: ChapterId,
    pub wave: String,
    pub source_project_id: String,
    pub source_project_slug: String,
    pub content: ProjectContent,
    pub tasks: Vec<ChapterTask>,
    pub observed_at: i64,
    pub closed_at: Option<i64>,
    pub transition: Chapter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterHistoryEntry {
    pub id: ChapterId,
    pub source_project_id: String,
    pub source_project_slug: String,
    pub source_work_id: Option<String>,
    pub closed_at: Option<i64>,
    pub phase: ChapterPhase,
}

/// Missing local evidence is distinct from evidence that no work began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartEvidence {
    pub begun: bool,
    pub worker_claimed: bool,
    pub authored: Option<bool>,
    pub published: bool,
    pub abandoned: bool,
    pub completed: bool,
}

pub fn classify_task(task: &PmItem, evidence: &TaskStartEvidence) -> (TaskDisposition, String) {
    let state = task.state.as_deref();
    let terminal = matches!(state, Some("completed" | "canceled" | "duplicate")) || task.completed;
    if evidence.abandoned
        && !terminal
        && (state == Some("started")
            || evidence.begun
            || evidence.published
            || evidence.authored == Some(true))
    {
        return (
            TaskDisposition::Unresolved,
            "local retirement conflicts with refreshed start evidence".into(),
        );
    }
    if terminal || evidence.abandoned || evidence.completed {
        if evidence.worker_claimed {
            return (
                TaskDisposition::Unresolved,
                "terminal planning state conflicts with an active worker claim".into(),
            );
        }
        return (
            TaskDisposition::Historical,
            "work is already terminal".into(),
        );
    }
    if evidence.worker_claimed
        || evidence.begun
        || evidence.published
        || evidence.authored == Some(true)
        || state == Some("started")
    {
        return (
            TaskDisposition::Move,
            "work has started; retain its identity and execution".into(),
        );
    }
    if evidence.authored == Some(false) && matches!(state, Some("backlog" | "unstarted" | "triage"))
    {
        return (
            TaskDisposition::Abandon,
            "unstarted backlog expires at the chapter boundary".into(),
        );
    }
    (
        TaskDisposition::Unresolved,
        "start evidence is unavailable; refresh before retiring this Task".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::{classify_task, TaskDisposition, TaskStartEvidence};
    use crate::pm::PmItem;

    fn task(state: &str) -> PmItem {
        PmItem {
            id: "issue".into(),
            identifier: "LOO-1".into(),
            url: None,
            name: "Work".into(),
            description: String::new(),
            rank: 0,
            completed: state == "completed",
            state: Some(state.into()),
            project_id: "old".into(),
            project: "old".into(),
            team_id: "team".into(),
            assignee: None,
        }
    }

    #[test]
    fn chapter_moves_started_work_and_expires_only_untouched_backlog() {
        let mut evidence = TaskStartEvidence {
            begun: false,
            worker_claimed: false,
            authored: Some(false),
            published: false,
            abandoned: false,
            completed: false,
        };
        assert_eq!(
            classify_task(&task("unstarted"), &evidence).0,
            TaskDisposition::Abandon
        );
        assert_eq!(
            classify_task(&task("started"), &evidence).0,
            TaskDisposition::Move
        );
        evidence.begun = true;
        assert_eq!(
            classify_task(&task("backlog"), &evidence).0,
            TaskDisposition::Move
        );
        assert_eq!(
            classify_task(&task("completed"), &evidence).0,
            TaskDisposition::Historical
        );
        evidence.worker_claimed = true;
        assert_eq!(
            classify_task(&task("canceled"), &evidence).0,
            TaskDisposition::Unresolved
        );
    }

    #[test]
    fn missing_worktree_evidence_never_abandons_a_task() {
        let evidence = TaskStartEvidence {
            begun: false,
            worker_claimed: false,
            authored: None,
            published: false,
            abandoned: false,
            completed: false,
        };
        assert_eq!(
            classify_task(&task("unstarted"), &evidence).0,
            TaskDisposition::Unresolved
        );
    }

    #[test]
    fn retired_task_with_new_start_evidence_requires_reconciliation() {
        let mut evidence = TaskStartEvidence {
            begun: false,
            worker_claimed: false,
            authored: Some(false),
            published: false,
            abandoned: true,
            completed: false,
        };
        assert_eq!(
            classify_task(&task("started"), &evidence).0,
            TaskDisposition::Unresolved
        );
        evidence.authored = Some(true);
        assert_eq!(
            classify_task(&task("unstarted"), &evidence).0,
            TaskDisposition::Unresolved
        );
        for state in ["completed", "duplicate", "canceled"] {
            assert_eq!(
                classify_task(&task(state), &evidence).0,
                TaskDisposition::Historical
            );
        }
        evidence.worker_claimed = true;
        assert_eq!(
            classify_task(&task("canceled"), &evidence).0,
            TaskDisposition::Unresolved
        );
    }
}
