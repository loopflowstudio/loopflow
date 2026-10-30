//! Regressions as Linear records them: a `Regressed in` label group with one
//! child label per release version. Linear applies at most one label of a
//! group to an issue, so each issue names one version.
use serde::Deserialize;
use serde_json::json;

use super::{Connection, LinearClient};
use crate::linear::{LinearError, LinearResult};

pub(crate) const REGRESSED_IN_GROUP: &str = "Regressed in";

const REGRESSED_ISSUES_QUERY: &str = r#"query RegressedIssues($filter: IssueFilter!, $after: String) {
  issues(filter: $filter, first: 100, after: $after, includeArchived: true) {
    nodes {
      identifier
      completedAt
      state { type }
      labels { nodes { name parent { name } } }
    }
    pageInfo { hasNextPage endCursor }
  }
}"#;

const REGRESSION_LABELS_QUERY: &str = r#"query RegressionLabels($labels: IssueLabelFilter!, $issueId: String!) {
  issueLabels(filter: $labels, first: 250) {
    nodes { id name isGroup team { id } parent { id } }
  }
  issue(id: $issueId) {
    labels { nodes { id parent { id } } }
  }
}"#;

const CREATE_LABEL_MUTATION: &str = r#"mutation CreateRegressionLabel($input: IssueLabelCreateInput!) {
  issueLabelCreate(input: $input) { success issueLabel { id } }
}"#;

const MARK_REGRESSED_IN_MUTATION: &str = r#"mutation MarkRegressedIn($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) { success }
}"#;

/// One issue labelled with the release it regressed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinearRegression {
    pub identifier: String,
    /// The child label's name, as written: `v0.15.3`.
    pub version: String,
    /// Provider workflow category: `completed`, `canceled`, `started`, ...
    pub state: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Deserialize)]
struct RegressedIssues {
    issues: Connection<RegressedIssue>,
}

#[derive(Deserialize)]
struct RegressedIssue {
    identifier: String,
    #[serde(rename = "completedAt")]
    completed_at: Option<String>,
    state: Option<StateType>,
    labels: LabelNodes<IssueLabelName>,
}

#[derive(Deserialize)]
struct StateType {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Deserialize)]
struct LabelNodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct IssueLabelName {
    name: String,
    parent: Option<LabelName>,
}

#[derive(Deserialize)]
struct LabelName {
    name: String,
}

#[derive(Deserialize)]
struct RegressionLabels {
    #[serde(rename = "issueLabels")]
    issue_labels: LabelNodes<RegressionLabel>,
    issue: Option<IssueLabelIds>,
}

#[derive(Deserialize)]
struct RegressionLabel {
    id: String,
    name: String,
    #[serde(rename = "isGroup")]
    is_group: bool,
    team: Option<LabelId>,
    parent: Option<LabelId>,
}

#[derive(Deserialize)]
struct IssueLabelIds {
    labels: LabelNodes<IssueLabelId>,
}

#[derive(Deserialize)]
struct IssueLabelId {
    id: String,
    parent: Option<LabelId>,
}

#[derive(Deserialize)]
struct LabelId {
    id: String,
}

#[derive(Deserialize)]
struct LabelCreated {
    #[serde(rename = "issueLabelCreate")]
    issue_label_create: LabelCreatePayload,
}

#[derive(Deserialize)]
struct LabelCreatePayload {
    success: bool,
    #[serde(rename = "issueLabel")]
    issue_label: Option<LabelId>,
}

#[derive(Deserialize)]
struct IssueUpdated {
    #[serde(rename = "issueUpdate")]
    issue_update: Success,
}

#[derive(Deserialize)]
struct Success {
    success: bool,
}

impl LinearClient {
    /// Every issue in `team`, archived ones included, carrying a `Regressed in`
    /// label.
    pub(crate) async fn regressions(&self, team: &str) -> LinearResult<Vec<LinearRegression>> {
        let filter = json!({
            "team": {"id": {"eq": team}},
            "labels": {"some": {"parent": {"name": {"eq": REGRESSED_IN_GROUP}}}},
        });
        let mut regressions = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page: RegressedIssues = self
                .graphql(
                    REGRESSED_ISSUES_QUERY,
                    json!({"filter": filter, "after": after}),
                )
                .await?;
            regressions.extend(page.issues.nodes.into_iter().filter_map(|issue| {
                let version = issue.labels.nodes.into_iter().find_map(|label| {
                    (label.parent?.name == REGRESSED_IN_GROUP).then_some(label.name)
                })?;
                Some(LinearRegression {
                    identifier: issue.identifier,
                    version,
                    state: issue.state.map(|state| state.type_),
                    completed_at: issue.completed_at,
                })
            }));
            if !page.issues.page_info.has_next_page {
                return Ok(regressions);
            }
            after = page.issues.page_info.end_cursor;
        }
    }

    /// Label `issue` as regressed in `version`, replacing any earlier version.
    /// Creates the `Regressed in` group in `team` and the version label on
    /// first use.
    pub(crate) async fn mark_regressed_in(
        &self,
        issue: &str,
        team: &str,
        version: &str,
    ) -> LinearResult<()> {
        let labels: RegressionLabels = self
            .graphql(
                REGRESSION_LABELS_QUERY,
                json!({
                    "labels": {"or": [
                        {"name": {"eq": REGRESSED_IN_GROUP}, "isGroup": {"eq": true}},
                        {"parent": {"name": {"eq": REGRESSED_IN_GROUP}}},
                    ]},
                    "issueId": issue,
                }),
            )
            .await?;
        let applied = labels
            .issue
            .ok_or_else(|| LinearError::Message(format!("no Linear issue with id {issue}")))?
            .labels
            .nodes;
        // A workspace group serves every team; a team group serves its own.
        let group = labels.issue_labels.nodes.iter().find(|label| {
            label.is_group
                && label.name == REGRESSED_IN_GROUP
                && label.team.as_ref().is_none_or(|owner| owner.id == team)
        });
        let (group_id, group_team) = match group {
            Some(group) => (
                group.id.clone(),
                group.team.as_ref().map(|owner| owner.id.clone()),
            ),
            None => (
                self.create_label(REGRESSED_IN_GROUP, None, Some(team), true)
                    .await?,
                Some(team.to_owned()),
            ),
        };
        let existing = labels.issue_labels.nodes.iter().find(|label| {
            label.name == version
                && label
                    .parent
                    .as_ref()
                    .is_some_and(|parent| parent.id == group_id)
        });
        let version_id = match existing {
            Some(label) => label.id.clone(),
            None => {
                self.create_label(version, Some(&group_id), group_team.as_deref(), false)
                    .await?
            }
        };
        let removed: Vec<&str> = applied
            .iter()
            .filter(|label| {
                label.id != version_id
                    && label
                        .parent
                        .as_ref()
                        .is_some_and(|parent| parent.id == group_id)
            })
            .map(|label| label.id.as_str())
            .collect();
        let updated: IssueUpdated = self
            .graphql(
                MARK_REGRESSED_IN_MUTATION,
                json!({"id": issue, "input": {"addedLabelIds": [version_id], "removedLabelIds": removed}}),
            )
            .await?;
        if !updated.issue_update.success {
            return Err(LinearError::Message(format!(
                "Linear did not confirm the {REGRESSED_IN_GROUP} {version} label"
            )));
        }
        Ok(())
    }

    async fn create_label(
        &self,
        name: &str,
        parent: Option<&str>,
        team: Option<&str>,
        group: bool,
    ) -> LinearResult<String> {
        let created: LabelCreated = self
            .graphql(
                CREATE_LABEL_MUTATION,
                json!({"input": {"name": name, "parentId": parent, "teamId": team, "isGroup": group}}),
            )
            .await?;
        match created.issue_label_create {
            LabelCreatePayload {
                success: true,
                issue_label: Some(label),
            } => Ok(label.id),
            _ => Err(LinearError::Message(format!(
                "Linear did not confirm creating label {name:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    use crate::linear::test_server::{self, json_response};
    use crate::linear::LinearClient;

    #[tokio::test]
    async fn marking_a_regression_replaces_the_earlier_version_label() {
        let labels = json!({"data": {
            "issueLabels": {"nodes": [
                {"id": "group", "name": "Regressed in", "isGroup": true, "team": {"id": "team-1"}, "parent": null},
                {"id": "v0153", "name": "v0.15.3", "isGroup": false, "team": {"id": "team-1"}, "parent": {"id": "group"}},
            ]},
            "issue": {"labels": {"nodes": [
                {"id": "bug", "parent": null},
                {"id": "v0153", "parent": {"id": "group"}},
            ]}},
        }});
        let (url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, labels),
            json_response(
                StatusCode::OK,
                json!({"data": {"issueLabelCreate": {"success": true, "issueLabel": {"id": "v0154"}}}}),
            ),
            json_response(
                StatusCode::OK,
                json!({"data": {"issueUpdate": {"success": true}}}),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("token".into(), None, url);

        client
            .mark_regressed_in("issue-1", "team-1", "v0.15.4")
            .await
            .unwrap();

        let requests = requests.lock().await;
        let variables: Vec<Value> = requests
            .iter()
            .map(|request| {
                serde_json::from_str::<Value>(&request.body).unwrap()["variables"].clone()
            })
            .collect();
        assert_eq!(
            variables[1]["input"],
            json!({"name": "v0.15.4", "parentId": "group", "teamId": "team-1", "isGroup": false})
        );
        assert_eq!(
            variables[2],
            json!({"id": "issue-1", "input": {"addedLabelIds": ["v0154"], "removedLabelIds": ["v0153"]}})
        );
    }
}
