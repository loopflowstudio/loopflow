//! Turn a Linear issue read into durable Task direction.
//!
//! Someone editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store, so calling [`reconcile_linear_observation`] twice with the same read
//! is safe.

use time::OffsetDateTime;

use super::{OpsError, OpsResult};
use crate::pm::linear::LinearClient;
use crate::store::SharedStore;

use crate::pm::{IssueComment, IssueObservation};
use crate::store::{Store, StoreResult};
use crate::work::task::{
    LinearFollowUp, LinearObservationApply, LinearObservationOutcome, Task, TaskLinearObservation,
};

/// Explicit steering is eligible even when published by an integration. Participant-authored
/// comments include the account used by Loopflow; exclude writebacks by content.
pub(crate) fn is_human_comment(comment: &IssueComment, _viewer_id: &str) -> bool {
    is_direction_comment(&comment.body, comment.author_id.as_deref())
}

pub(crate) fn is_direction_comment(body: &str, author: Option<&str>) -> bool {
    if body.contains("<!-- loopflow-steer:") {
        return true;
    }
    author.is_some()
        && !body.contains("<!-- loopflow-")
        && !body.starts_with("PR: ")
        && !body.starts_with("Shipped: ")
        && !body.starts_with("Reteamed by loopflow:")
        && !body.starts_with("[GitHub PR #")
}

/// Publish first; local events are a recoverable projection of Linear comments.
pub(crate) async fn publish_task_steer(
    store: &SharedStore,
    task: &Task,
    text: &str,
) -> OpsResult<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(OpsError::Message("Task direction cannot be empty".into()));
    }
    let client = super::pm::issue_client(&task.worktree).await?;
    let marker = format!("<!-- loopflow-steer:{} -->", uuid::Uuid::new_v4());
    let name = crate::engine::config::launch_user_name()
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let text = match name {
        Some(name) => format!(
            "{text}\n\n<!-- loopflow-requester:{} -->",
            serde_json::to_string(&name)
                .expect("name is serializable")
                .replace('<', "\\u003c")
                .replace('>', "\\u003e")
        ),
        None => text.to_string(),
    };
    let comment_id = publish_comment(&client, task.plan.id.as_str(), &text, &marker).await?;
    refresh_task_comments(store, task).await.map_err(|error| OpsError::Message(format!(
        "Posted Linear comment {comment_id}, but local delivery is pending: {error}. The worker will reconcile it from Linear."
    )))?;
    Ok(comment_id)
}

async fn publish_comment(
    client: &LinearClient,
    issue_id: &str,
    text: &str,
    marker: &str,
) -> OpsResult<String> {
    // One identity per authored instruction. Equal text can intentionally recur.
    let body = format!("{text}\n\n{marker}");
    match client.comment(issue_id, &body).await {
        Ok(id) => Ok(id),
        Err(error) => match client.find_comment_with_marker(issue_id, marker).await {
            Ok(Some(id)) => Ok(id),
            _ => Err(OpsError::Message(format!(
                "Linear did not confirm this steering comment: {error}. Check Linear for {marker} before resubmitting; no local-only steer was accepted."
            ))),
        },
    }
}

pub(crate) async fn refresh_task_comments(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let client = super::pm::issue_client(&task.worktree).await?;
    let observation = client
        .observe_issue(task.plan.id.as_str())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    reconcile_linear_observation(store, task, observation, "", OffsetDateTime::now_utc())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    Ok(())
}

pub(crate) fn comment_revision_id(id: &str, revision: Option<&str>) -> String {
    match revision {
        Some(revision) => format!("{id}@{revision}"),
        None => id.to_string(),
    }
}

pub(crate) fn render_comment(
    id: &str,
    body: &str,
    author_id: Option<&str>,
    author_name: Option<&str>,
) -> String {
    // Explicit steering can be published through an integration account. Its
    // recorded requester wins; an older anonymous steer stays anonymous.
    let requester = if body.contains("<!-- loopflow-steer:") {
        body.split_once("<!-- loopflow-requester:")
            .and_then(|(_, rest)| rest.split_once(" -->"))
            .and_then(|(name, _)| serde_json::from_str::<String>(name).ok())
    } else {
        author_name.map(str::to_string)
    };
    let attribution = requester
        .as_deref()
        .and_then(crate::engine::config::normalize_user_name)
        .map(|name| {
            format!(
                " by {}",
                serde_json::to_string(&name).expect("name is serializable")
            )
        })
        .unwrap_or_default();
    let source = author_id
        .map(|id| format!(" (provider user {id})"))
        .unwrap_or_default();
    format!("Linear comment {id}{attribution}{source}:\n\n{body}")
}

fn content_steer_text(title: &str, description: &str) -> String {
    format!(
        "The linked Linear task was edited; use this current definition.\n\n\
         Title: {title}\n\n{description}"
    )
}

/// Read one Linear observation into durable, exactly-once Task direction.
pub async fn reconcile_linear_observation(
    store: &Store,
    task: &Task,
    observation: IssueObservation,
    viewer_id: &str,
    observed_at: OffsetDateTime,
) -> StoreResult<LinearObservationOutcome> {
    let cursor = store.task_linear_observation(&task.id).await?;
    let apply = plan_apply(task, observation, viewer_id, observed_at, cursor.as_ref());
    store.apply_linear_observation(apply).await
}

/// Build the durable apply from a read and the current cursor. A
/// title/description edit becomes a Steer only when a baseline exists and the
/// content changed; every user comment rides as a candidate Steer, and the
/// store drops the ones already seen.
pub(crate) fn plan_apply(
    task: &Task,
    observation: IssueObservation,
    viewer_id: &str,
    observed_at: OffsetDateTime,
    cursor: Option<&TaskLinearObservation>,
) -> LinearObservationApply {
    let content_steer = match cursor {
        Some(cursor)
            if cursor.last_title != observation.title
                || cursor.last_description != observation.description =>
        {
            Some(content_steer_text(
                &observation.title,
                &observation.description,
            ))
        }
        _ => None,
    };
    let follow_ups = observation
        .comments
        .iter()
        .filter(|comment| is_human_comment(comment, viewer_id))
        .map(|comment| LinearFollowUp {
            comment_id: comment_revision_id(&comment.id, comment.revision.as_deref()),
            text: render_comment(
                &comment.id,
                &comment.body,
                comment.author_id.as_deref(),
                comment.author_name.as_deref(),
            ),
        })
        .collect();
    LinearObservationApply {
        task_id: task.id.clone(),
        revision: observation.revision,
        title: observation.title,
        description: observation.description,
        observed_at,
        content_steer,
        follow_ups,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{is_human_comment, plan_apply};
    use crate::planning::{LinearIssueId, TaskPlan};
    use crate::pm::{IssueComment, IssueObservation};
    use crate::work::task::{Task, TaskId, TaskLinearObservation};

    use crate::pm::test_server::{self, json_response};
    use crate::store::{CredentialType, ProviderToken, SharedStore};
    use axum::http::StatusCode;
    use serde_json::json;
    use std::future::Future;

    pub(crate) async fn with_posted_comment<T>(
        store: &SharedStore,
        task: &Task,
        body: &str,
        future: impl Future<Output = T>,
    ) -> T {
        let (url, _) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({"data": {"commentCreate": {"comment": {"id": "comment-1"}}}})),
            json_response(StatusCode::OK, json!({"data": {"issue": {
                "updatedAt": "2026-09-23T00:00:00Z", "title": task.plan.title, "description": task.plan.description,
                "comments": {"nodes": [{"id": "comment-1", "body": body, "updatedAt": "2026-09-23T00:00:00Z", "user": {"id": "user-loopflow"}}], "pageInfo": {"hasNextPage": false, "endCursor": null}}
            }}})),
        ]).await;
        let file = tempfile::NamedTempFile::new().unwrap();
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".into(),
                access_token: "fixture-token".into(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: Some("fixture".into()),
                updated_at: 1,
                credential_type: CredentialType::OAuth,
            })
            .await
            .unwrap();
        crate::ops::pm::PM_TEST_CONTEXT
            .scope(
                crate::ops::pm::PmTestContext {
                    path: file.path().to_path_buf(),
                    store: store.clone(),
                    graphql_url: url,
                },
                future,
            )
            .await
    }

    #[tokio::test]
    async fn uncertain_publication_reconciles_the_existing_linear_comment() {
        let marker = "<!-- loopflow-steer:request-1 -->";
        let page = |nodes| json!({"data": {"issue": {"comments": {"nodes": nodes, "pageInfo": {"hasNextPage": false, "endCursor": null}}}}});
        let (url, _) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({"errors": [{"message": "response interrupted"}]}),
            ),
            json_response(
                StatusCode::OK,
                page(
                    json!([{"id": "posted-comment", "body": format!("keep the API\n\n{marker}")}]),
                ),
            ),
        ])
        .await;
        let client =
            crate::pm::linear::LinearClient::with_base_url("fixture-token".into(), None, url);
        assert_eq!(
            super::publish_comment(&client, "issue-1", "keep the API", marker)
                .await
                .unwrap(),
            "posted-comment"
        );
    }

    const VIEWER: &str = "user-loopflow";

    #[test]
    fn request_authors_survive_linear_direction_rendering() {
        let comments = [
            ("one", Some("Jack")),
            ("two", Some("Maya")),
            ("three", None),
        ]
        .into_iter()
        .map(|(id, name)| IssueComment {
            id: id.into(),
            revision: None,
            body: "prototype".into(),
            author_id: Some(format!("person-{id}")),
            author_name: name.map(str::to_string),
        })
        .collect();
        let apply = plan_apply(
            &task(),
            observation("title", "body", comments),
            VIEWER,
            time::OffsetDateTime::now_utc(),
            None,
        );
        assert!(apply.follow_ups[0]
            .text
            .contains("by \"Jack\" (provider user person-one)"));
        assert!(apply.follow_ups[1]
            .text
            .contains("by \"Maya\" (provider user person-two)"));
        assert!(!apply.follow_ups[2].text.contains(" by "));
        let explicit =
            "prototype\n<!-- loopflow-requester:\"Jack\" -->\n<!-- loopflow-steer:one -->";
        let rendered =
            super::render_comment("one", explicit, Some("publisher"), Some("Account Owner"));
        assert!(rendered.contains("by \"Jack\""));
        assert!(!rendered.contains("Account Owner"));
        let anonymous = super::render_comment(
            "old",
            "prototype\n<!-- loopflow-steer:old -->",
            Some("publisher"),
            Some("Account Owner"),
        );
        assert!(!anonymous.contains(" by "));
    }

    fn comment(id: &str, body: &str, author: Option<&str>) -> IssueComment {
        IssueComment {
            author_name: None,
            id: id.to_string(),
            revision: None,
            body: body.to_string(),
            author_id: author.map(str::to_string),
        }
    }

    fn observation(
        title: &str,
        description: &str,
        comments: Vec<IssueComment>,
    ) -> IssueObservation {
        IssueObservation {
            revision: "2026-07-15T18:00:00.000Z".to_string(),
            title: title.to_string(),
            description: description.to_string(),
            comments,
        }
    }

    fn task() -> Task {
        let now = time::OffsetDateTime::now_utc();
        Task {
            id: TaskId::from_raw("ts_plan"),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-1").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Old title".to_string(),
                description: "Old body".to_string(),
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: crate::work::task::PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::work::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::work::task::Observation::NotRequired,
        }
    }

    fn cursor(title: &str, description: &str) -> TaskLinearObservation {
        let now = time::OffsetDateTime::now_utc();
        TaskLinearObservation {
            task_id: TaskId::from_raw("task_plan"),
            last_revision: "2026-07-15T00:00:00.000Z".to_string(),
            last_title: title.to_string(),
            last_description: description.to_string(),
            last_success_at: now,
            degraded_reason: None,
            updated_at: now,
        }
    }

    #[test]
    fn own_account_comments_are_direction_but_writebacks_are_not() {
        assert!(is_human_comment(
            &comment("c", "hi", Some("user-human")),
            VIEWER
        ));
        assert!(is_human_comment(
            &comment("c", "please fix this", Some(VIEWER)),
            VIEWER
        ));
        assert!(!is_human_comment(
            &comment("c", "PR: x", Some(VIEWER)),
            VIEWER
        ));
        assert!(!is_human_comment(&comment("c", "bot", None), VIEWER));
    }

    #[test]
    fn baseline_emits_no_content_steer_and_keeps_user_comments_as_candidates() {
        // No cursor yet: a title change must not become a Steer, but user
        // comments still ride so the store can deliver them.
        let obs = observation(
            "New title",
            "New body",
            vec![
                comment("c-1", "please prioritize", Some("user-human")),
                comment("c-2", "PR: x", Some(VIEWER)),
            ],
        );
        let apply = plan_apply(&task(), obs, VIEWER, time::OffsetDateTime::now_utc(), None);
        assert!(apply.content_steer.is_none());
        assert_eq!(apply.follow_ups.len(), 1);
        assert_eq!(apply.follow_ups[0].comment_id, "c-1");
    }

    #[test]
    fn a_content_edit_becomes_one_steer() {
        let obs = observation("New title", "New body", vec![]);
        let apply = plan_apply(
            &task(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        let steer = apply.content_steer.expect("Steer for a content edit");
        assert!(steer.contains("New title"));
        assert!(steer.contains("New body"));
    }

    #[test]
    fn an_unchanged_issue_emits_no_steer() {
        let obs = observation("Old title", "Old body", vec![]);
        let apply = plan_apply(
            &task(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        assert!(apply.content_steer.is_none());
    }
}
