use std::path::Path;

use time::OffsetDateTime;

use crate::pm::linear::LinearClient;
use crate::store::{AskCommentWrite, Store};

use super::{OpsError, OpsResult};

const ABANDONED_ATTEMPT_AFTER_SECS: i64 = 300;

/// Publish each currently-pending Ask transition once.
///
/// Provider failures are durable outbox state, not command failures. Store
/// failures still return because they can prevent that failure from being made
/// observable and retryable.
pub(crate) async fn publish_pending_ask_comments(store: &Store) -> OpsResult<()> {
    let writes = store
        .pending_ask_comment_writes()
        .await
        .map_err(store_error)?;
    for candidate in writes {
        let attempted_at = OffsetDateTime::now_utc().unix_timestamp();
        let Some(write) = store
            .claim_ask_comment_write(
                &candidate.ask_id,
                candidate.transition,
                attempted_at,
                attempted_at - ABANDONED_ATTEMPT_AFTER_SECS,
            )
            .await
            .map_err(store_error)?
        else {
            continue;
        };
        let client =
            match super::pm::linear_issue_client(Path::new(&write.repo), &write.issue_id).await {
                Ok(client) => client,
                Err(error) => {
                    record_failure(store, &write, &error.to_string()).await?;
                    continue;
                }
            };
        publish_claimed_ask_comment(store, &client, write).await?;
    }
    Ok(())
}

async fn publish_claimed_ask_comment(
    store: &Store,
    client: &LinearClient,
    write: AskCommentWrite,
) -> OpsResult<()> {
    let marker = write.transition.marker(&write.ask_id);
    let result = async {
        if write.attempt_count > 1 {
            if let Some(comment_id) = client
                .find_comment_with_marker(&write.issue_id, &marker)
                .await?
            {
                return Ok(comment_id);
            }
        }
        client.comment(&write.issue_id, &write.body).await
    }
    .await;

    match result {
        Ok(comment_id) => store
            .complete_ask_comment_write(
                &write.ask_id,
                write.transition,
                &comment_id,
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .map_err(store_error),
        Err(error) => record_failure(store, &write, &error.to_string()).await,
    }
}

async fn record_failure(store: &Store, write: &AskCommentWrite, error: &str) -> OpsResult<()> {
    store
        .fail_ask_comment_write(&write.ask_id, write.transition, error)
        .await
        .map_err(store_error)?;
    tracing::warn!(
        ask_id = %write.ask_id,
        transition = write.transition.as_str(),
        %error,
        "Linear Ask comment publication failed; the durable outbox will retry"
    );
    Ok(())
}

fn store_error(error: crate::store::StoreError) -> OpsError {
    OpsError::Message(format!("failed to update Ask comment outbox: {error}"))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::publish_claimed_ask_comment;
    use crate::durable::{AskResult, WorkRef};
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::pm::linear::LinearClient;
    use crate::pm::test_server::{self, json_response};
    use crate::store::{open_store, StorageConfig};
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{Observation, PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
    use crate::work::wave::Wave;

    #[tokio::test]
    async fn ask_request_and_result_comments_commit_first_and_retry_without_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = open_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("linear-project").unwrap(),
                slug: "runtime-project".to_string(),
                name: "Runtime Project".to_string(),
                prompt_context: "Resolve child requests.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("linear-issue").unwrap(),
                identifier: "RUN-1".to_string(),
                title: "Prove Ask comments".to_string(),
                description: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: directory.path().join("task"),
            workspace_slug: "ask-comments".to_string(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: "task/ask-comments".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task(&task, &pr).await.unwrap();

        let child_work = WorkRef::Task(task.id.clone());
        let source_run_id = crate::durable::RunId::new();
        let home_id = store.local_home().await.unwrap().id;
        let ask = store
            .request_intervention(
                crate::durable::AskOrigin {
                    work: child_work.clone(),
                    source_run_id: Some(source_run_id),
                    home_id,
                    cwd: task.worktree.clone(),
                },
                "Which durable proof matters?",
                false,
            )
            .await
            .unwrap();

        let pending = store.pending_ask_comment_writes().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending[0].body.contains("Which durable proof matters?"));
        assert!(pending[0]
            .body
            .contains(&format!("project `{}`", project.id)));

        // Linear accepted the first create, then the process died before it
        // could record the returned id. The durable started attempt survives.
        let claimed = store
            .claim_ask_comment_write(&ask.id, pending[0].transition, 100, 0)
            .await
            .unwrap()
            .unwrap();
        let (base_url, first_requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "commentCreate": { "comment": { "id": "comment-ask" } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("token".to_string(), None, base_url);
        client
            .comment(&claimed.issue_id, &claimed.body)
            .await
            .unwrap();

        // Recovery adopts the marked remote comment. It does not create it a
        // second time even though the local outbox still looked unfinished.
        let (base_url, recovery_requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "issue": { "comments": {
                "nodes": [{ "id": "comment-ask", "body": claimed.body }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )])
        .await;
        let client = LinearClient::with_base_url("token".to_string(), None, base_url);
        let recovered = store
            .claim_ask_comment_write(&ask.id, claimed.transition, 500, 500)
            .await
            .unwrap()
            .unwrap();
        publish_claimed_ask_comment(&store, &client, recovered)
            .await
            .unwrap();
        assert_eq!(first_requests.lock().await.len(), 1);
        let recovery_requests = recovery_requests.lock().await;
        assert_eq!(recovery_requests.len(), 1);
        assert!(recovery_requests[0].body.contains("IssueComments"));
        drop(recovery_requests);

        let claim = store.claim_ask(&ask.id).await.unwrap();
        store
            .mark_ask_presented(&ask.id, &claim.run_id)
            .await
            .unwrap();
        let settled = store
            .settle_ask(
                &ask.id,
                &claim.run_id,
                AskResult::Resolved {
                    summary: "The committed exchange.".to_string(),
                },
            )
            .await
            .unwrap();
        let result_write = store.pending_ask_comment_writes().await.unwrap().remove(0);
        assert!(result_write.body.contains("The committed exchange."));
        assert!(result_write
            .body
            .contains(&format!("Run `{}`", claim.run_id)));

        let (base_url, _) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "errors": [{ "message": "Linear is unavailable" }] }),
        )])
        .await;
        let client = LinearClient::with_base_url("token".to_string(), None, base_url);
        let claimed = store
            .claim_ask_comment_write(&ask.id, result_write.transition, 600, 600)
            .await
            .unwrap()
            .unwrap();
        publish_claimed_ask_comment(&store, &client, claimed)
            .await
            .unwrap();
        let failed = store.pending_ask_comment_writes().await.unwrap();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].last_error.is_some());
        assert_eq!(
            store
                .asks_for_work(&child_work)
                .await
                .unwrap()
                .into_iter()
                .find(|candidate| candidate.id == ask.id)
                .unwrap(),
            settled,
            "provider failure cannot roll back the committed Ask result"
        );

        let (base_url, retry_requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "issue": { "comments": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-result" } } } }),
            ),
        ])
        .await;
        let client = LinearClient::with_base_url("token".to_string(), None, base_url);
        let retry = store
            .claim_ask_comment_write(&ask.id, failed[0].transition, 700, 700)
            .await
            .unwrap()
            .unwrap();
        publish_claimed_ask_comment(&store, &client, retry)
            .await
            .unwrap();

        assert!(store.pending_ask_comment_writes().await.unwrap().is_empty());
        let retry_requests = retry_requests.lock().await;
        assert_eq!(retry_requests.len(), 2);
        assert_eq!(
            retry_requests
                .iter()
                .filter(|request| request.body.contains("commentCreate"))
                .count(),
            1
        );
    }
}
