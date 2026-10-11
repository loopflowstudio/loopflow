//! Shared Work values. Execution, Workflow positions, paths and controls are absent.
use crate::planning_exchange::{PlanningKind, PlanningObject};
use crate::store::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An edit carries its target; its value determines the scope and conflict field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEdit {
    pub target: String,
    pub change: WorkValue,
}

impl WorkEdit {
    pub(crate) fn for_target(target: &str, values: &[WorkValue]) -> Vec<Self> {
        values
            .iter()
            .cloned()
            .map(|change| Self {
                target: target.into(),
                change,
            })
            .collect()
    }

    pub(crate) fn object(&self) -> PlanningObject {
        PlanningObject {
            kind: self.change.kind(),
            id: self.target.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disposition {
    pub planning_state: Option<String>,
    pub planning_completed: i64,
    pub planning_completed_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentContent {
    pub body: String,
    pub author: crate::work::task::comment::TaskCommentAuthor,
    pub created_at: Option<String>,
}

macro_rules! planning_fields {
    ($(($kind:ident, $variant:ident, $column:literal, $value:ty)),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "field", content = "value", rename_all = "snake_case", deny_unknown_fields)]
        pub enum WorkValue {
            $($variant($value),)*
            Workflow(String),
            Krs(Vec<crate::work::project::ProjectKR>),
            MetricTargets(Vec<crate::work::wave::metrics::MetricTarget>),
            Disposition(Disposition),
            CommentContent(CommentContent),
        }
        impl WorkValue {
            pub(crate) fn from_value(kind: PlanningKind, field: &str, value: Value) -> StoreResult<Self> {
                let edit=match (kind, field) {
                    $((PlanningKind::$kind, $column) => Self::$variant(serde_json::from_value(value)?),)*
                    (PlanningKind::Project, "workflow") => Self::Workflow(serde_json::from_value(value)?),
                    (PlanningKind::Project, "krs") => Self::Krs(serde_json::from_value(value)?),
                    (PlanningKind::Project, "metric_targets") => Self::MetricTargets(serde_json::from_value(value)?),
                    (PlanningKind::Task, "disposition") => Self::Disposition(serde_json::from_value(value)?),
                    (PlanningKind::Comment, "content") => Self::CommentContent(serde_json::from_value(value)?),
                    _ => return Err(StoreError::InvalidData("unknown planning field".into())),
                };
                edit.validate()?;
                Ok(edit)
            }
            pub(crate) fn kind(&self) -> PlanningKind {
                match self {
                    $(Self::$variant(_) => PlanningKind::$kind,)*
                    Self::Workflow(_) | Self::Krs(_) | Self::MetricTargets(_) => PlanningKind::Project,
                    Self::Disposition(_) => PlanningKind::Task,
                    Self::CommentContent(_) => PlanningKind::Comment,
                }
            }
            pub fn field(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => $column,)*
                    Self::Workflow(_) => "workflow", Self::Krs(_) => "krs",
                    Self::MetricTargets(_) => "metric_targets", Self::Disposition(_) => "disposition",
                    Self::CommentContent(_) => "content",
                }
            }
            pub fn value(&self) -> StoreResult<Value> {
                Ok(match self {
                    $(Self::$variant(v) => serde_json::to_value(v)?,)*
                    Self::Workflow(v) => serde_json::to_value(v)?,
                    Self::Krs(v) => serde_json::to_value(v)?,
                    Self::MetricTargets(v) => serde_json::to_value(v)?,
                    Self::Disposition(v) => serde_json::to_value(v)?,
                    Self::CommentContent(v) => serde_json::to_value(v)?,
                })
            }
        }
        pub(crate) fn fields(kind: PlanningKind) -> Vec<&'static str> {
            let mut fields = Vec::new();
            $(if kind == PlanningKind::$kind { fields.push($column); })*
            fields.extend(match kind {
                PlanningKind::Wave => &[][..],
                PlanningKind::Project => &["workflow", "krs", "metric_targets"],
                PlanningKind::Task => &["disposition"],
                PlanningKind::Comment => &["content"],
            });
            fields
        }
    }
}
planning_fields! {
    (Wave, WaveName, "name", String),
    (Wave, WaveParent, "parent_wave_id", Option<String>),
    (Wave, WaveProject, "current_project_id", Option<String>),
    (Project, ProjectWave, "wave_id", String),
    (Project, ProjectLinearId, "external_project_id", Option<String>),
    (Project, ProjectSlug, "project_slug", Option<String>),
    (Project, ProjectName, "project_name", Option<String>),
    (Project, ProjectSummary, "project_summary", String),
    (Project, ProjectDescription, "description", Option<String>),
    (Project, ProjectStatus, "status", crate::work::project::ProjectStatus),
    (Project, ProjectRank, "planning_rank", u32),
    (Project, ProjectInitiatives, "planning_initiatives", Vec<String>),
    (Project, ProjectTeams, "planning_teams", Vec<String>),
    (Task, TaskProject, "project_id", String),
    (Task, TaskLinearId, "external_issue_id", Option<String>),
    (Task, TaskIdentifier, "issue_identifier", String),
    (Task, TaskTitle, "issue_title", Option<String>),
    (Task, TaskDescription, "issue_description", Option<String>),
    (Task, TaskRank, "planning_rank", u32),
    (Task, TaskAssignee, "planning_assignee", Option<String>),
    (Task, TaskDueDate, "planning_due_date", Option<String>),
    (Task, TaskDeletedAt, "planning_deleted_at", Option<i64>),
    (Task, TaskUrl, "planning_url", Option<String>),
    (Task, TaskBranch, "planning_branch_name", Option<String>),
    (Task, TaskTeam, "planning_team_id", Option<String>),
    (Comment, CommentTask, "task_id", String),
}

impl WorkValue {
    pub(crate) fn validate(&self) -> StoreResult<()> {
        match self {
            Self::Disposition(v) if !matches!(v.planning_completed, 0 | 1) => {
                return Err(StoreError::InvalidData("invalid completion value".into()))
            }
            Self::MetricTargets(v) => {
                crate::work::project::validate_metric_targets(v).map_err(StoreError::InvalidData)?
            }
            _ => {}
        }
        Ok(())
    }
}
