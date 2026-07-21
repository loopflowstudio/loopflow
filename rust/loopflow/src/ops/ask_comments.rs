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
    use crate::durable::{
        AdvanceReceipt, Containment, ControlCtx, InvocationRoute, RunAdvance, RunTrigger, WorkRef,
    };
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::pm::linear::LinearClient;
    use crate::pm::test_server::{self, json_response};
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, StorageConfig, Store};
    use crate::task::{
        Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
        TaskPrId,
    };
    use crate::wave::Wave;

    async fn start_invocation(
        store: &Store,
        work: &WorkRef,
        process_group: i64,
    ) -> (crate::durable::RunLease, crate::durable::AgentInvocation) {
        let (_, lease) = store.reserve_run(work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::ProcessGroup { id: process_group },
                    cwd: "/repo".into(),
                },
            )
            .await
            .unwrap();
        let AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        (lease, invocation)
    }

    #[tokio::test]
    async fn ask_and_answer_comments_commit_first_and_retry_without_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
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
                prompt_context: "Answer child questions.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
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
            lifecycle: TaskLifecyclePlan::standard("task-design", "task", "ship"),
            lifecycle_phase: TaskLifecyclePhase::Loop,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
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

        let parent_work = WorkRef::Project(project.id.clone());
        let (parent_lease, _) = start_invocation(&store, &parent_work, 1).await;
        let child_work = WorkRef::Task(task.id.clone());
        let (child_lease, child_invocation) = start_invocation(&store, &child_work, 2).await;
        store
            .advance_run(
                &child_lease,
                RunAdvance::TurnStarting {
                    invocation_id: child_invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let ask = store
            .open_ask(
                &child_lease,
                &child_invocation.id,
                "Which durable proof matters?",
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

        let answer = store
            .answer_ask(
                &ControlCtx::Run(&parent_lease),
                &ask.id,
                "The committed exchange.",
            )
            .await
            .unwrap();
        let answer_write = store.pending_ask_comment_writes().await.unwrap().remove(0);
        assert!(answer_write.body.contains("The committed exchange."));
        assert!(answer_write
            .body
            .contains(&format!("Run `{}`", parent_lease.run_id)));

        let (base_url, _) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "errors": [{ "message": "Linear is unavailable" }] }),
        )])
        .await;
        let client = LinearClient::with_base_url("token".to_string(), None, base_url);
        let claimed = store
            .claim_ask_comment_write(&ask.id, answer_write.transition, 600, 600)
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
                .current_ask(&child_lease, &child_invocation.id, Some(&ask.id))
                .await
                .unwrap()
                .answer,
            Some(answer),
            "provider failure cannot roll back the committed Answer"
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
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-answer" } } } }),
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
