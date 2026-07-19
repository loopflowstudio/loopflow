//! Turn a Linear issue read into durable Task direction.
//!
//! A human editing a Linear issue's title or description or adding a comment
//! appends an ordered Steer. This module maps one observation onto that input
//! spine, and [`Store::apply_linear_observation`] persists it atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store, so calling [`reconcile_linear_observation`] twice with the same read
//! is safe.

use time::OffsetDateTime;

use crate::pm::{IssueComment, IssueObservation};
use crate::store::{Store, StoreResult};
use crate::task::{
    LinearFollowUp, LinearObservationApply, LinearObservationOutcome, Task, TaskLinearObservation,
};

/// A comment carries human direction when it has an author that is not
/// Loopflow's own Linear user. A null author (an integration/bot actor) or
/// Loopflow's own writeback never becomes Task direction — this is what keeps
/// ingestion from feeding itself.
pub(crate) fn is_human_comment(comment: &IssueComment, viewer_id: &str) -> bool {
    comment
        .author_id
        .as_deref()
        .is_some_and(|id| id != viewer_id)
}

fn content_steer_text(title: &str, description: &str) -> String {
    format!(
        "The linked Linear task was edited; use this current definition.\n\n\
         Title: {title}\n\n{description}"
    )
}

fn follow_up_text(body: &str) -> String {
    format!("New Linear comment:\n\n{body}")
}

pub(crate) fn linear_follow_up_text(body: &str) -> String {
    follow_up_text(body)
}

/// Read one Linear observation into durable, exactly-once Task direction.
pub async fn reconcile_linear_observation(
    store: &Store,
    session: &Task,
    observation: IssueObservation,
    viewer_id: &str,
    observed_at: OffsetDateTime,
) -> StoreResult<LinearObservationOutcome> {
    let cursor = store.task_linear_observation(&session.id).await?;
    let apply = plan_apply(
        session,
        observation,
        viewer_id,
        observed_at,
        cursor.as_ref(),
    );
    store.apply_linear_observation(apply).await
}

/// Build the durable apply from a read and the current cursor. A
/// title/description edit becomes a Steer only when a baseline exists and the
/// content changed; every user comment rides as a candidate Steer, and the
/// store drops the ones already seen.
pub(crate) fn plan_apply(
    session: &Task,
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
            comment_id: comment.id.clone(),
            text: linear_follow_up_text(&comment.body),
        })
        .collect();
    LinearObservationApply {
        task_id: session.id.clone(),
        revision: observation.revision,
        title: observation.title,
        description: observation.description,
        observed_at,
        content_steer,
        follow_ups,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_human_comment, plan_apply};
    use crate::launch_context::{LinearIssueId, LinearIssueSnapshot, LinearProjectSnapshot};
    use crate::pm::{IssueComment, IssueObservation};
    use crate::task::{Task, TaskId, TaskLinearObservation};

    const VIEWER: &str = "user-loopflow";

    fn comment(id: &str, body: &str, author: Option<&str>) -> IssueComment {
        IssueComment {
            id: id.to_string(),
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

    fn session() -> Task {
        let now = time::OffsetDateTime::now_utc();
        Task {
            id: TaskId::from_raw("ts_plan"),
            launch: crate::launch_context::TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-1").unwrap(),
                    identifier: "INF-123".to_string(),
                    title: "Old title".to_string(),
                    description: "Old body".to_string(),
                },
                project: LinearProjectSnapshot {
                    id: crate::launch_context::LinearProjectId::new("project-1").unwrap(),
                    slug: "runtime".to_string(),
                    name: "Runtime".to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: crate::task::PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::defaults(),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Loop,
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
            observation: crate::task::Observation::NotRequired,
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
    fn only_non_viewer_authored_comments_are_user_input() {
        assert!(is_human_comment(
            &comment("c", "hi", Some("user-human")),
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
        // comments still ride so the store can baseline them.
        let obs = observation(
            "New title",
            "New body",
            vec![
                comment("c-1", "please prioritize", Some("user-human")),
                comment("c-2", "PR: x", Some(VIEWER)),
            ],
        );
        let apply = plan_apply(
            &session(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            None,
        );
        assert!(apply.content_steer.is_none());
        assert_eq!(apply.follow_ups.len(), 1);
        assert_eq!(apply.follow_ups[0].comment_id, "c-1");
    }

    #[test]
    fn a_content_edit_becomes_one_steer() {
        let obs = observation("New title", "New body", vec![]);
        let apply = plan_apply(
            &session(),
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
            &session(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        assert!(apply.content_steer.is_none());
    }
}
