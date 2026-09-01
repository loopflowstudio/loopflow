use time::OffsetDateTime;

use crate::work::project::{Project, ProjectId};

pub(crate) fn automatic_restart_bar(project: &Project) -> Option<String> {
    project.abandon_intent.as_ref().map(|intent| {
        format!(
            "Project {} is being abandoned: {}",
            project.plan.slug, intent.reason
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
    pub project_id: ProjectId,
    pub iteration: u32,
    pub observation_cursor: i64,
    pub last_state_fingerprint: Option<String>,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub updated_at: OffsetDateTime,
}
