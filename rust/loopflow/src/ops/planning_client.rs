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
    PlanningResponse, StateHistory,
};
use crate::notion::NotionClient;
use crate::provider_auth::Provider;

use super::{OpsError, OpsResult};

/// The repository's planning selection. Unreadable configuration selects none:
/// routing is configuration, never availability.
fn transport(repo: &Path) -> Option<PlanningConfig> {
    crate::config::load_planning_transport(repo).ok()
}

/// The configured remote provider's product name; Git planning has none.
pub(crate) fn remote_name(repo: &Path) -> Option<&'static str> {
    transport(repo)?.remote_name()
}

pub(crate) fn connected(repo: &str) -> bool {
    remote_name(Path::new(repo)).is_some()
}

pub(crate) fn is_linear(repo: &str) -> bool {
    matches!(
        transport(Path::new(repo)),
        Some(PlanningConfig::Linear { .. })
    )
}

/// The team every repository Task belongs to: a Linear Team, an Asana team or
/// the Notion page that holds the Tasks database.
pub(crate) fn repository_scope(repo: &Path) -> OpsResult<String> {
    match transport(repo) {
        Some(PlanningConfig::Asana { team }) => Ok(crate::asana::team_gid(&team)),
        Some(PlanningConfig::Notion { team }) => Ok(crate::notion::notion_id(&team)),
        _ => super::linear::repository_team_id(repo),
    }
}

/// Where a Wave's Projects belong in the provider: its Linear Initiative, or
/// for Asana and Notion the repository's team itself.
pub(crate) fn wave_scope(repo: &Path, wave: &str) -> OpsResult<Option<String>> {
    match transport(repo) {
        Some(PlanningConfig::Asana { .. } | PlanningConfig::Notion { .. }) => {
            repository_scope(repo).map(Some)
        }
        _ => Ok(super::linear::read_initiative(repo, wave)),
    }
}

/// The repository's name, which names its Asana team project: the origin's
/// repository name, else the main checkout's directory.
fn repository_name(repo: &Path) -> String {
    if let Ok(id) = crate::repository::RepoId::discover(repo) {
        return id.name().to_string();
    }
    let main = crate::git::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    main.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "loopflow".into())
}

#[derive(Debug, Clone)]
pub(crate) enum PlanningClient {
    Linear(LinearClient),
    Asana(AsanaClient),
    Notion(NotionClient),
}

impl PlanningClient {
    /// Repository-scoped access; Linear validates the repository's Team claim.
    pub(crate) async fn for_repository(repo: &Path) -> OpsResult<Self> {
        match transport(repo) {
            Some(PlanningConfig::Asana { team }) => asana(repo, &team).await,
            Some(PlanningConfig::Notion { team }) => notion(&team).await,
            _ => Ok(Self::Linear(super::linear::linear_client(repo).await?)),
        }
    }

    /// Exact-item access for a Task that already names its planning identity.
    pub(crate) async fn for_items(repo: &Path) -> OpsResult<Self> {
        match transport(repo) {
            Some(PlanningConfig::Asana { team }) => asana(repo, &team).await,
            Some(PlanningConfig::Notion { team }) => notion(&team).await,
            _ => Ok(Self::Linear(
                super::linear::issue_client().await?.for_repository(repo),
            )),
        }
    }

    /// Linear-only planning: Project edits and order, Initiatives, attachments
    /// and relations.
    pub(crate) fn linear(&self) -> OpsResult<&LinearClient> {
        let provider = match self {
            Self::Linear(client) => return Ok(client),
            Self::Asana(_) => "Asana",
            Self::Notion(_) => "Notion",
        };
        Err(OpsError::Message(format!(
            "{provider} mirrors a Project when it is created; later Project edits stay local"
        )))
    }

    /// Create an Asana or Notion Project from its export receipt input
    /// (`id`, `name`, `description`), answering the provider revision.
    pub(crate) async fn create_project(
        &self,
        input: &serde_json::Value,
    ) -> LinearResult<Option<String>> {
        match self {
            Self::Linear(_) => Err(crate::linear::LinearError::Message(
                "Linear Projects are created through their Initiative".into(),
            )),
            Self::Asana(client) => client.create_project(input).await,
            Self::Notion(client) => client.create_project(input).await,
        }
    }

    /// The revision of the Asana or Notion Project carrying Loopflow ID `id`.
    pub(crate) async fn project_revision(&self, id: &str) -> LinearResult<Option<Option<String>>> {
        match self {
            Self::Linear(_) => Ok(None),
            Self::Asana(client) => client.project_revision(id).await,
            Self::Notion(client) => client.project_revision(id).await,
        }
    }

    /// Asana and Notion comments carry no caller identity, so a Loopflow comment
    /// is its visible text as the provider reads it back. Delivery and read-back
    /// claim a comment by this text. Linear comments keep their Loopflow IDs.
    pub(crate) fn claimed_comment(&self, body: &str) -> Option<String> {
        match self {
            Self::Linear(_) => None,
            Self::Asana(_) => Some(crate::asana::comment_markdown(&visible_text(body))),
            Self::Notion(_) => Some(crate::notion::comment_markdown(&visible_text(body))),
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
            (Self::Asana(_) | Self::Notion(_), _) => Ok(target.to_string()),
        }
    }

    pub(crate) async fn find_export_issue(
        &self,
        id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        match self {
            Self::Linear(client) => client.find_export_issue(id).await,
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
            Self::Notion(client) => Ok(client.task(id).await?.map(|task| (task, None))),
        }
    }

    pub(crate) async fn issue_ownership(
        &self,
        id: &str,
    ) -> LinearResult<Option<(LinearIssue, Option<LinearProject>)>> {
        match self {
            Self::Linear(client) => client.issue_ownership(id).await,
            Self::Asana(client) => Ok(client.task(id).await?.map(|task| (task, None))),
            Self::Notion(client) => Ok(client.task(id).await?.map(|task| (task, None))),
        }
    }

    /// Asana and Notion keep no readable state history.
    pub(crate) async fn state_history_since(
        &self,
        id: &str,
        base_revision: &str,
        base_state: &str,
    ) -> LinearResult<StateHistory> {
        match self {
            Self::Linear(client) => {
                client
                    .state_history_since(id, base_revision, base_state)
                    .await
            }
            Self::Asana(_) | Self::Notion(_) => Ok(StateHistory::Unknown),
        }
    }

    pub(crate) async fn create_task(
        &self,
        input: serde_json::Value,
    ) -> LinearResult<PlanningResponse> {
        match self {
            Self::Linear(client) => client.deliver_planning_creation(false, input).await,
            Self::Asana(client) => client.create_task(&input).await,
            Self::Notion(client) => client.create_task(&input).await,
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
            Self::Notion(client) => client.update_task(id, &input).await,
        }
    }

    pub(crate) async fn set_item_state(&self, id: &str, state: &str) -> LinearResult<LinearIssue> {
        match self {
            Self::Linear(client) => client.set_item_state(id, state).await,
            Self::Asana(client) => client.set_state(id, state).await,
            Self::Notion(client) => client.set_state(id, state).await,
        }
    }

    /// Whether the provider holds the item in its trash, and the revision read.
    /// Asana and Notion remove deleted tasks from reads, so they report only live tasks.
    pub(crate) async fn issue_trash(&self, id: &str) -> LinearResult<Option<(bool, String)>> {
        let task = match self {
            Self::Linear(client) => return client.issue_trash(id).await,
            Self::Asana(client) => client.task(id).await?,
            Self::Notion(client) => client.task(id).await?,
        };
        Ok(task.map(|task| (false, task.revision.unwrap_or_default())))
    }

    pub(crate) async fn delete_issue(&self, id: &str) -> LinearResult<()> {
        match self {
            Self::Linear(client) => client.delete_issue(id).await,
            Self::Asana(client) => client.delete_task(id).await,
            Self::Notion(client) => client.delete_task(id).await,
        }
    }

    pub(crate) async fn sync_comment(&self, id: &str, issue: &str, body: &str) -> LinearResult<()> {
        let text = self.claimed_comment(body).unwrap_or_default();
        match self {
            Self::Linear(client) => client.sync_comment(id, issue, body).await,
            Self::Asana(client) => client.sync_comment(issue, &text).await,
            Self::Notion(client) => client.sync_comment(issue, &text).await,
        }
    }

    /// Whether the provider positively reports the item deleted: Linear's trash,
    /// Asana's missing task, or Notion's archived page (found through the
    /// Task's page URL). A lookup that merely finds nothing is not evidence.
    pub(crate) async fn removed(&self, id: &str, url: Option<&str>) -> LinearResult<bool> {
        match self {
            Self::Linear(client) => Ok(client
                .issue_trash(id)
                .await?
                .is_some_and(|(trashed, _)| trashed)),
            Self::Asana(client) => Ok(client.task(id).await?.is_none()),
            Self::Notion(client) => client.removed(url).await,
        }
    }

    /// Items in the repository's planning scope changed since `since`: new items
    /// people created there and edits to known ones, with their observations.
    pub(crate) async fn changes(
        &self,
        scope: &str,
        since: Option<&str>,
        through: &str,
        live: &[String],
        finished: &[String],
    ) -> LinearResult<Vec<(LinearIssue, Option<LinearProject>, LinearIssueObservation)>> {
        let changes = match self {
            Self::Linear(client) => {
                return client
                    .team_changes(scope, since, through, live, finished)
                    .await
            }
            Self::Asana(client) => client.changes(since, live, finished).await?,
            Self::Notion(client) => client.changes(since, live, finished).await?,
        };
        Ok(changes
            .into_iter()
            .map(|(issue, observation)| (issue, None, observation))
            .collect())
    }

    pub(crate) async fn observe_issue(&self, id: &str) -> LinearResult<LinearIssueObservation> {
        match self {
            Self::Linear(client) => client.observe_issue(id).await,
            Self::Asana(client) => client.observe(id).await,
            Self::Notion(client) => client.observe(id).await,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StateScope<'a> {
    Item(&'a str),
    Team(&'a str),
}

async fn asana(repo: &Path, team: &str) -> OpsResult<PlanningClient> {
    let token = super::linear::resolve_planning_token(Provider::Asana).await?;
    let repository = repository_name(repo);
    #[cfg(test)]
    if let Ok(url) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|context| context.graphql_url.clone())
    {
        return Ok(PlanningClient::Asana(AsanaClient::with_base_url(
            token, team, repository, url,
        )));
    }
    Ok(PlanningClient::Asana(AsanaClient::new(
        token, team, repository,
    )))
}

async fn notion(team: &str) -> OpsResult<PlanningClient> {
    let token = super::linear::resolve_planning_token(Provider::Notion).await?;
    #[cfg(test)]
    if let Ok(url) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|context| context.graphql_url.clone())
    {
        return Ok(PlanningClient::Notion(NotionClient::with_base_url(
            token, team, url,
        )));
    }
    Ok(PlanningClient::Notion(NotionClient::new(token, team)))
}

/// A comment without Loopflow's hidden `<!-- loopflow-… -->` markers, which
/// providers without comment IDs would otherwise show.
fn visible_text(body: &str) -> String {
    let mut text = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("<!-- loopflow-") {
        text.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => &rest[start + end + 3..],
            None => "",
        };
    }
    text.push_str(rest);
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_text_hides_loopflow_markers() {
        assert_eq!(
            visible_text(
                "Ship it\n\n<!-- loopflow-requester:\"Jack\" -->\n\n<!-- loopflow-steer:1 -->"
            ),
            "Ship it"
        );
        assert_eq!(visible_text("<!-- other --> kept"), "<!-- other --> kept");
    }

    #[test]
    fn local_identifier_matches_the_task_identifier() {
        let task = crate::durable::TaskId::new();
        let uuid = uuid::Uuid::parse_str(task.as_str().trim_start_matches("task_")).unwrap();
        assert_eq!(
            crate::linear::local_identifier(&uuid.to_string()),
            format!("lf-{}", task.as_str().trim_start_matches("task_"))
        );
    }
}
