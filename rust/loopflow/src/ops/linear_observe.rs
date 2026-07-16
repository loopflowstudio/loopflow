//! Turn a Linear issue read into durable Task direction.
//!
//! A human editing a Linear issue's title or description is a replacement
//! directive; a new human comment is an ordered follow-up. This module is the
//! single writer that maps one observation onto the existing Task control
//! substrate ([`crate::child_session`]) — it builds the commands and directive,
//! and the store's [`Store::apply_linear_observation`] persists them atomically.
//! Exactly-once, the baseline, and the monotonic-revision guard all live in the
//! store, so calling [`reconcile_linear_observation`] twice with the same read
//! is safe.

use time::OffsetDateTime;

use crate::child_session::{
    ChildCommand, ChildCommandKind, ChildCommandSource, ChildDirective, ChildRef,
};
use crate::pm::{IssueComment, IssueObservation};
use crate::store::{Store, StoreResult};
use crate::task::{
    LinearFollowUp, LinearObservationApply, LinearObservationOutcome, TaskLinearObservation,
    TaskSession,
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

fn directive_text(title: &str, description: &str) -> String {
    format!(
        "The linked Linear task was edited; treat this as the current definition \
         and re-acknowledge.\n\nTitle: {title}\n\n{description}"
    )
}

fn follow_up_text(body: &str) -> String {
    format!("New Linear comment:\n\n{body}")
}

/// Read one Linear observation into durable, exactly-once Task direction.
pub async fn reconcile_linear_observation(
    store: &Store,
    session: &TaskSession,
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
/// title/description edit becomes a replacement directive only when a baseline
/// exists and the content changed; every human comment rides as a candidate
/// follow-up, and the store drops the ones already seen.
pub(crate) fn plan_apply(
    session: &TaskSession,
    observation: IssueObservation,
    viewer_id: &str,
    observed_at: OffsetDateTime,
    cursor: Option<&TaskLinearObservation>,
) -> LinearObservationApply {
    let target = ChildRef::Task(session.id.clone());
    let directive = match cursor {
        Some(cursor)
            if cursor.last_title != observation.title
                || cursor.last_description != observation.description =>
        {
            let text = directive_text(&observation.title, &observation.description);
            let command = ChildCommand::new(
                target.clone(),
                ChildCommandSource::Linear,
                ChildCommandKind::Steer { text: text.clone() },
            );
            let directive = ChildDirective::replacement(
                target.clone(),
                session.current_directive_version + 1,
                text,
                ChildCommandSource::Linear,
                command.id.clone(),
            );
            Some((command, directive))
        }
        _ => None,
    };
    let follow_ups = observation
        .comments
        .iter()
        .filter(|comment| is_human_comment(comment, viewer_id))
        .map(|comment| LinearFollowUp {
            comment_id: comment.id.clone(),
            command: ChildCommand::new(
                target.clone(),
                ChildCommandSource::Linear,
                ChildCommandKind::FollowUp {
                    text: follow_up_text(&comment.body),
                },
            ),
        })
        .collect();
    LinearObservationApply {
        session_id: session.id.clone(),
        revision: observation.revision,
        title: observation.title,
        description: observation.description,
        observed_at,
        directive,
        follow_ups,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_human_comment, plan_apply};
    use crate::child_session::{ChildCommandKind, ChildCommandSource};
    use crate::pm::{IssueComment, IssueObservation};
    use crate::session_context::{LinearIssueId, LinearIssueSnapshot, LinearProjectSnapshot};
    use crate::task::{TaskLinearObservation, TaskSession, TaskSessionId, TaskSessionStatus};

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

    fn session() -> TaskSession {
        let now = time::OffsetDateTime::now_utc();
        TaskSession {
            id: TaskSessionId::from_raw("ts_plan"),
            launch: crate::session_context::TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-1").unwrap(),
                    identifier: "INF-123".to_string(),
                    title: "Old title".to_string(),
                    description: "Old body".to_string(),
                },
                project: LinearProjectSnapshot {
                    id: crate::session_context::LinearProjectId::new("project-1").unwrap(),
                    slug: "runtime".to_string(),
                    name: "Runtime".to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: crate::task::PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_session_id: crate::project_session::ProjectSessionId::new(),
            current_directive_version: 3,
            incorporated_directive_version: 3,
            status: TaskSessionStatus::Running,
            status_reason: "running".to_string(),
            status_at: now,
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            resolved_flow: "task".to_string(),
            interaction_policy: crate::engine::InteractionPolicy::Require,
            flow_cursor: 0,
            flow_iteration: 0,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn cursor(title: &str, description: &str) -> TaskLinearObservation {
        let now = time::OffsetDateTime::now_utc();
        TaskLinearObservation {
            session_id: TaskSessionId::from_raw("ts_plan"),
            last_revision: "2026-07-15T00:00:00.000Z".to_string(),
            last_title: title.to_string(),
            last_description: description.to_string(),
            last_success_at: now,
            degraded_reason: None,
            updated_at: now,
        }
    }

    #[test]
    fn only_non_viewer_authored_comments_are_human() {
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
    fn baseline_emits_no_directive_and_keeps_human_comments_as_candidates() {
        // No cursor yet: a title change must not become a directive, but human
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
        assert!(apply.directive.is_none());
        assert_eq!(apply.follow_ups.len(), 1);
        assert_eq!(apply.follow_ups[0].comment_id, "c-1");
    }

    #[test]
    fn a_content_edit_becomes_a_versioned_replacement_directive() {
        let obs = observation("New title", "New body", vec![]);
        let apply = plan_apply(
            &session(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        let (command, directive) = apply.directive.expect("directive for a content edit");
        assert!(matches!(command.kind, ChildCommandKind::Steer { .. }));
        assert_eq!(command.source, ChildCommandSource::Linear);
        assert_eq!(directive.version, 4);
        assert!(directive.text.contains("New title"));
        assert!(directive.text.contains("New body"));
    }

    #[test]
    fn an_unchanged_issue_emits_no_directive() {
        let obs = observation("Old title", "Old body", vec![]);
        let apply = plan_apply(
            &session(),
            obs,
            VIEWER,
            time::OffsetDateTime::now_utc(),
            Some(&cursor("Old title", "Old body")),
        );
        assert!(apply.directive.is_none());
    }
}
