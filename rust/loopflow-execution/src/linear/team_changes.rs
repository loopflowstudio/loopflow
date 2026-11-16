//! One paginated team delta, including comment-only changes.
use std::collections::{btree_map::Entry, BTreeMap};

use serde::Deserialize;
use serde_json::json;

use super::{CommentNode, Connection, IssueNode, LinearClient, PageInfo, CHANGED_ISSUE_FIELDS};
use super::{LinearIssue, LinearProject};
use crate::linear::{LinearComment, LinearError, LinearIssueObservation, LinearResult};

const QUERY: &str = r#"query TeamChanges($issues: IssueFilter!, $comments: CommentFilter!, $issuesAfter: String, $commentsAfter: String, $readIssues: Boolean!, $readComments: Boolean!) {
  issues(filter: $issues, first: 25, after: $issuesAfter, orderBy: updatedAt) @include(if: $readIssues) {
    nodes { ...ChangedIssue }
    pageInfo { hasNextPage endCursor }
  }
  comments(filter: $comments, first: 25, after: $commentsAfter, orderBy: updatedAt) @include(if: $readComments) {
    nodes { id body createdAt updatedAt user { id displayName name } issue { ...ChangedIssue } }
    pageInfo { hasNextPage endCursor }
  }
}"#;

#[derive(Deserialize)]
struct Changes {
    issues: Option<Connection<IssueNode>>,
    comments: Option<Connection<ChangedComment>>,
}

#[derive(Deserialize)]
struct ChangedComment {
    #[serde(flatten)]
    comment: CommentNode,
    issue: Option<IssueNode>,
}

impl LinearClient {
    pub(crate) async fn team_changes(
        &self,
        team: &str,
        since: Option<&str>,
        through: &str,
        live: &[String],
        finished: &[String],
    ) -> LinearResult<Vec<(LinearIssue, Option<LinearProject>, LinearIssueObservation)>> {
        let mut eligible = json!({"team":{"id":{"eq":team}}, "id":{"nin":finished},
            "or":[{"id":{"in":live}}, {"state":{"type":{"nin":["completed","canceled"]}}}]});
        let mut revision = json!({"lte":through});
        if let Some(since) = since {
            revision["gte"] = json!(since);
        }
        let comment_filter = json!({"issue":eligible,"updatedAt":revision});
        eligible["updatedAt"] = revision;
        let mut issues_after = None;
        let mut comments_after = None;
        let mut read_issues = true;
        let mut read_comments = true;
        // An issue can occur on several comment pages or complete in this batch.
        // Keep its latest fields and all comments together until ingestion.
        let mut changes = BTreeMap::new();
        let mut record = |node: IssueNode, mut comments: Vec<LinearComment>| {
            let issue = node.into_issue(0)?;
            match changes.entry(issue.id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert((issue, comments));
                }
                Entry::Occupied(mut entry) => {
                    let (saved, saved_comments) = entry.get_mut();
                    if issue.revision >= saved.revision {
                        *saved = issue;
                    }
                    saved_comments.append(&mut comments);
                }
            }
            Ok::<(), LinearError>(())
        };
        let query = format!("{QUERY}\nfragment ChangedIssue on Issue {{ {CHANGED_ISSUE_FIELDS} }}");
        while read_issues || read_comments {
            let page: Changes = self
                .graphql(
                    &query,
                    json!({
                        "issues":eligible,"comments":comment_filter,
                        "issuesAfter":issues_after,"commentsAfter":comments_after,
                        "readIssues":read_issues,"readComments":read_comments,
                    }),
                )
                .await?;
            if read_issues {
                let page = page
                    .issues
                    .ok_or_else(|| LinearError::Message("Linear issue delta is missing".into()))?;
                for issue in page.nodes {
                    record(issue, Vec::new())?;
                }
                read_issues = page.page_info.has_next_page;
                issues_after = continuation(page.page_info)?;
            }
            if read_comments {
                let page = page.comments.ok_or_else(|| {
                    LinearError::Message("Linear comment delta is missing".into())
                })?;
                for comment in page.nodes {
                    if let Some(issue) = comment.issue {
                        record(issue, vec![comment.comment.into()])?;
                    }
                }
                read_comments = page.page_info.has_next_page;
                comments_after = continuation(page.page_info)?;
            }
        }
        let mut projects = BTreeMap::new();
        for id in changes
            .values()
            .filter_map(|(issue, _)| issue.project_id.clone())
        {
            if let Entry::Vacant(entry) = projects.entry(id) {
                let project = self.project_node(entry.key()).await?.into_observation()?;
                entry.insert(project);
            }
        }
        Ok(changes
            .into_values()
            .map(|(issue, mut comments)| {
                let project = issue
                    .project_id
                    .as_ref()
                    .and_then(|id| projects.get(id).cloned());
                comments.sort_by(|a, b| a.revision.cmp(&b.revision).then(a.id.cmp(&b.id)));
                let observation = LinearIssueObservation {
                    revision: issue.revision.clone().expect("Linear issue has a revision"),
                    title: issue.name.clone(),
                    description: issue.description.clone(),
                    comments,
                };
                (issue, project, observation)
            })
            .collect())
    }
}

fn continuation(page: PageInfo) -> LinearResult<Option<String>> {
    if page.has_next_page && page.end_cursor.is_none() {
        return Err(LinearError::Message(
            "Linear delta page is missing its continuation cursor".into(),
        ));
    }
    Ok(page.end_cursor)
}

#[cfg(test)]
mod tests {
    use super::LinearClient;
    use crate::linear::test_server::{self, json_response};
    use reqwest::StatusCode;
    use serde_json::json;

    #[tokio::test]
    async fn team_delta_keeps_comments_across_pages_and_completion() {
        let issue = json!({"id":"issue-1","identifier":"FIX-1","title":"Completed with direction",
            "description":"Body","branchName":null,"completedAt":null,"dueDate":null,
            "updatedAt":"2026-10-11T12:00:00Z","url":null,"prioritySortOrder":0.0,"sortOrder":1.0,
            "assignee":null,"state":{"type":"completed"},"team":{"id":"team-1"},"project":null});
        let comment = |id, revision| {
            let mut issue = issue.clone();
            issue["title"] = json!("Older fields from a comment page");
            issue["updatedAt"] = json!(revision);
            issue["state"] = json!({"type":"started"});
            json!({"id":id,"body":id,"createdAt":revision,"updatedAt":revision,
            "user":{"id":"author-1","displayName":"Maya","name":"Maya"},"issue":issue})
        };
        let (url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({"data":{
                "issues":{"nodes":[issue],"pageInfo":{"hasNextPage":false,"endCursor":null}},
                "comments":{"nodes":[comment("newer", "2026-10-11T11:00:00Z")],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}
            }})),
            json_response(StatusCode::OK, json!({"data":{
                "comments":{"nodes":[comment("older", "2026-10-11T10:00:00Z")],"pageInfo":{"hasNextPage":false,"endCursor":null}}
            }})),
        ]).await;
        let client = LinearClient::with_base_url("fixture".into(), None, url);
        let changes = client
            .team_changes(
                "team-1",
                Some("2026-10-11T09:00:00Z"),
                "2026-10-11T13:00:00Z",
                &["issue-1".into()],
                &["finished".into()],
            )
            .await
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].0.completed);
        assert_eq!(changes[0].0.name, "Completed with direction");
        assert_eq!(changes[0].2.title, changes[0].0.name);
        assert_eq!(changes[0].2.revision, "2026-10-11T12:00:00Z");
        let comments = &changes[0].2.comments;
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.id.as_str())
                .collect::<Vec<_>>(),
            ["older", "newer"]
        );
        assert_eq!(comments[0].author_name.as_deref(), Some("Maya"));
        assert_eq!(requests.lock().await.len(), 2);
    }
}
