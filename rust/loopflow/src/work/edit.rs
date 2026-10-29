//! Shared Work values. Execution, Workflow positions, paths and controls are absent.
use crate::store::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An edit carries its target; its value determines the scope and conflict field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    pub(crate) fn canonicalized(&self) -> StoreResult<Self> {
        let mut edit = self.clone();
        let table = crate::store::sqlite::planning_write::table(&edit.change);
        edit.target = match table {
            "waves" | "task_comments" => crate::id::canonical_uuid(&edit.target),
            "projects" => crate::id::canonical_prefixed(&edit.target, "proj_"),
            "tasks" => crate::id::canonical_prefixed(&edit.target, "task_"),
            _ => unreachable!("Work tables are exhaustive"),
        };
        edit.change.canonicalize()?;
        Ok(edit)
    }

    pub(crate) fn key(&self) -> (&str, &str) {
        (&self.target, self.change.field())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disposition {
    pub planning_state: Option<String>,
    pub planning_completed: i64,
    pub planning_completed_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentContent {
    pub body: String,
    pub author: crate::work::task::comment::TaskCommentAuthor,
    pub created_at: Option<String>,
}

macro_rules! work_values {
    ($(($variant:ident, $field:literal, $value:ty)),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "field", content = "value")]
        pub enum WorkValue {
            $(#[serde(rename = $field)] $variant($value),)*
        }
        impl WorkValue {
            /// The typed variant is also its portable conflict key.
            pub fn field(&self) -> &'static str {
                match self { $(Self::$variant(_) => $field,)* }
            }
            pub fn value(&self) -> StoreResult<Value> {
                Ok(match self { $(Self::$variant(v) => serde_json::to_value(v)?,)* })
            }

        }
    }
}
work_values! {
    (WaveName, "wave_name", String),
    (WaveParent, "wave_parent", Option<String>),
    (WaveProject, "wave_project", Option<String>),
    (ProjectWave, "project_wave", String),
    (ProjectLinearId, "project_linear_id", Option<String>),
    (ProjectSlug, "project_slug", Option<String>),
    (ProjectName, "project_name", Option<String>),
    (ProjectSummary, "project_summary", String),
    (ProjectDescription, "project_description", Option<String>),
    (ProjectStatus, "project_status", crate::work::project::ProjectStatus),
    (ProjectRank, "project_rank", u32),
    (ProjectInitiatives, "project_initiatives", Vec<String>),
    (ProjectTeams, "project_teams", Vec<String>),
    (TaskProject, "task_project", Option<String>),
    (TaskLinearId, "task_linear_id", Option<String>),
    (TaskIdentifier, "task_identifier", String),
    (TaskTitle, "task_title", Option<String>),
    (TaskDescription, "task_description", Option<String>),
    (TaskRank, "task_rank", u32),
    (TaskAssignee, "task_assignee", Option<String>),
    // Released peers and stored edits name it `task_paused`.
    (TaskDeferred, "task_paused", bool),
    (TaskShippedAt, "task_shipped_at", Option<i64>),
    (TaskDueDate, "task_due_date", Option<String>),
    (TaskDeletedAt, "task_deleted_at", Option<i64>),
    (TaskUrl, "task_url", Option<String>),
    (TaskBranch, "task_branch", Option<String>),
    (TaskTeam, "task_team", Option<String>),
    (TaskRegressedIn, "task_regressed_in", Option<String>),
    (CommentTask, "comment_task", String),
    (HistoricalProjectWorkflow, "workflow", String),
    // Released peers recorded the Wave default here; it lives in the Wave config.yaml.
    (HistoricalWaveWorkflow, "wave_workflow", Option<String>),
    (Krs, "krs", Vec<crate::work::project::ProjectKR>),
    (MetricTargets, "metric_targets", Vec<crate::work::wave::metrics::MetricTarget>),
    (Disposition, "disposition", Disposition),
    (CommentContent, "comment_content", CommentContent),
}

impl WorkValue {
    fn canonicalize(&mut self) -> StoreResult<()> {
        use crate::id::{canonical_prefixed, canonical_task_identifier, canonical_uuid};
        match self {
            Self::WaveParent(v)
            | Self::ProjectLinearId(v)
            | Self::TaskLinearId(v)
            | Self::TaskAssignee(v)
            | Self::TaskTeam(v) => {
                if let Some(v) = v {
                    *v = canonical_uuid(v);
                }
            }
            Self::WaveProject(Some(v)) | Self::TaskProject(Some(v)) => {
                *v = canonical_prefixed(v, "proj_")
            }
            Self::ProjectWave(v) => *v = canonical_uuid(v),
            Self::CommentTask(v) => *v = canonical_prefixed(v, "task_"),
            Self::TaskIdentifier(v) => *v = canonical_task_identifier(v),
            Self::ProjectInitiatives(values) | Self::ProjectTeams(values) => {
                let mut seen = std::collections::BTreeMap::new();
                for value in values {
                    let canonical = canonical_uuid(value);
                    if let Some(original) = seen.insert(canonical.clone(), value.clone()) {
                        if original != *value {
                            return Err(StoreError::InvalidData(format!(
                                "canonical identity collision: membership: {original} / {value}"
                            )));
                        }
                    }
                    *value = canonical;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> StoreResult<()> {
        match self {
            Self::Disposition(v) if !matches!(v.planning_completed, 0 | 1) => {
                return Err(StoreError::InvalidData("invalid completion value".into()))
            }
            Self::TaskRegressedIn(Some(v))
                if crate::work::task::release_version(v).as_ref() != Some(v) =>
            {
                return Err(StoreError::InvalidData(format!(
                    "regressed-in {v:?} is not a release version like v0.15.3"
                )))
            }
            Self::MetricTargets(v) => {
                crate::work::project::validate_metric_targets(v).map_err(StoreError::InvalidData)?
            }
            _ => {}
        }
        Ok(())
    }
}
