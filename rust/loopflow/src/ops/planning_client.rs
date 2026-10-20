//! The repository's remote planning provider, selected in `.lf/config.yaml` the
//! way an agent harness is. Export, field, state and comment delivery speak to
//! one [`PlanningClient`]; Linear-only planning (Teams, Initiatives, Projects,
//! attachments) stays in `ops::linear` and reaches its client through
//! [`PlanningClient::linear`].

use std::path::Path;

use crate::asana::AsanaClient;
use crate::config::PlanningConfig;
use crate::linear::{
    LinearClient, LinearIssue, LinearIssueObservation, LinearProject, LinearResult,
    PlanningResponse,
};
use crate::provider_auth::Provider;

use super::{OpsError, OpsResult};

/// The repository's planning selection. Unreadable configuration selects none:
/// routing is configuration, never availability.
fn transport(repo: &Path) -> Option<PlanningConfig> {
    Some(
        crate::config::load_config(Some(repo))
            .ok()?
            .unwrap_or_default()
            .planning_transport(),
    )
}

/// The configured remote provider's product name; Git planning has none.
pub(crate) fn remote_name(repo: &Path) -> Option<&'static str> {
    transport(repo)?.remote_name()
}

pub(crate) fn connected(repo: &str) -> bool {
    remote_name(Path::new(repo)).is_some()
}

pub(crate) fn is_linear(repo: &str) -> bool {
    matches!(transport(Path::new(repo)), Some(PlanningConfig::Linear {}))
}

fn asana_project(repo: &Path) -> Option<String> {
    match transport(repo)? {
        PlanningConfig::Asana { project } => Some(project),
        _ => None,
    }
}

/// The provider scope every repository Task belongs to: a Linear Team or the
/// repository's Asana project.
pub(crate) fn repository_scope(repo: &Path) -> OpsResult<String> {
    match asana_project(repo) {
        Some(project) => Ok(project),
        None => super::linear::repository_team_id(repo),
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PlanningClient {
    Linear(LinearClient),
    Asana(AsanaClient),
}

impl PlanningClient {
    /// Repository-scoped access; Linear validates the repository's Team claim.
    pub(crate) async fn for_repository(repo: &Path) -> OpsResult<Self> {
        match asana_project(repo) {
            Some(project) => asana(project).await,
            None => Ok(Self::Linear(super::linear::linear_client(repo).await?)),
        }
    }

    /// Exact-item access for a Task that already names its planning identity.
    pub(crate) async fn for_items(repo: &Path) -> OpsResult<Self> {
        match asana_project(repo) {
            Some(project) => asana(project).await,
            None => Ok(Self::Linear(
                super::linear::issue_client().await?.for_repository(repo),
            )),
        }
    }

    /// Linear-only planning: Projects, Initiatives, attachments and relations.
    pub(crate) fn linear(&self) -> OpsResult<&LinearClient> {
        match self {
            Self::Linear(client) => Ok(client),
            Self::Asana(_) => Err(OpsError::Message(
                "Asana does not mirror Projects yet; the Project stays local".into(),
            )),
        }
    }

    /// The provider value that moves an item to `target` (`completed`,
    /// `unstarted` or `canceled`). `scope` names the item, or the Team for a creation.
    pub(crate) async fn state_id(
        &self,
        scope: StateScope<'_>,
        target: &str,
    ) -> LinearResult<String> {
        match (self, scope) {
            (Self::Linear(client), StateScope::Item(item)) => {
                client.item_state_id(item, target).await
            }
            (Self::Linear(client), StateScope::Team(team)) => {
                client.team_state_id(team, target).await
            }
            (Self::Asana(_), _) => Ok(target.to_string()),
        }
    }

    pub(crate) async fn find_export_issue(
        &self,
        id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        match self {
            Self::Linear(client) => client.find_export_issue(id).await,
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
        }
    }

    pub(crate) async fn issue_ownership(
        &self,
        id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        match self {
            Self::Linear(client) => client.issue_ownership(id).await,
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
        }
    }

    pub(crate) async fn create_task(
        &self,
        input: serde_json::Value,
    ) -> LinearResult<PlanningResponse> {
        match self {
            Self::Linear(client) => client.deliver_planning_creation(false, input).await,
            Self::Asana(client) => client.create_task(&input).await,
        }
    }

    pub(crate) async fn update_task(
        &self,
        id: &str,
        input: serde_json::Value,
    ) -> LinearResult<PlanningResponse> {
        match self {
            Self::Linear(client) => client.deliver_planning_field(id, false, input).await,
            Self::Asana(client) => client.update_task(id, &input).await,
        }
    }

    pub(crate) async fn set_item_state(&self, id: &str, state: &str) -> LinearResult<LinearIssue> {
        match self {
            Self::Linear(client) => client.set_item_state(id, state).await,
            Self::Asana(client) => client.set_state(id, state).await,
        }
    }

    /// Whether the provider holds the item in its trash, and the revision read.
    /// Asana removes deleted tasks from reads, so it reports only live tasks.
    pub(crate) async fn issue_trash(&self, id: &str) -> LinearResult<Option<(bool, String)>> {
        match self {
            Self::Linear(client) => client.issue_trash(id).await,
            Self::Asana(client) => Ok(client
                .task(id)
                .await?
                .map(|task| (false, task.revision.unwrap_or_default()))),
        }
    }

    pub(crate) async fn delete_issue(&self, id: &str) -> LinearResult<()> {
        match self {
            Self::Linear(client) => client.delete_issue(id).await,
            Self::Asana(client) => client.delete_task(id).await,
        }
    }

    pub(crate) async fn sync_comment(&self, id: &str, issue: &str, body: &str) -> LinearResult<()> {
        match self {
            Self::Linear(client) => client.sync_comment(id, issue, body).await,
            Self::Asana(client) => client.sync_comment(issue, body).await,
        }
    }

    pub(crate) async fn observe_issue(&self, id: &str) -> LinearResult<LinearIssueObservation> {
        match self {
            Self::Linear(client) => client.observe_issue(id).await,
            Self::Asana(client) => client.observe(id).await,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StateScope<'a> {
    Item(&'a str),
    Team(&'a str),
}

async fn asana(project: String) -> OpsResult<PlanningClient> {
    let token = super::linear::resolve_planning_token(Provider::Asana).await?;
    #[cfg(test)]
    if let Ok(url) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|context| context.graphql_url.clone())
    {
        return Ok(PlanningClient::Asana(AsanaClient::with_base_url(
            token, project, url,
        )));
    }
    Ok(PlanningClient::Asana(AsanaClient::new(token, project)))
}
