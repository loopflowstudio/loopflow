use std::path::PathBuf;
use std::str::FromStr;

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::child_session::{
    ChildCommand, ChildCommandKind, ChildCommandSource, ChildRef, ChildWriteLease,
};
use crate::engine::InteractionPolicy;
use crate::id::WaveId;
use crate::interaction_review::{
    InteractionReview, InteractionReviewDataError, InteractionReviewDisposition,
    InteractionReviewEvidence, InteractionReviewId, InteractionReviewMessageAuthor,
    InteractionReviewPr, InteractionReviewStatus, InteractionReviewer,
};
use crate::project_session::ProjectSessionId;
use crate::store::rows::unix_to_datetime;
use crate::store::{StoreError, StoreResult};
use crate::task::{TaskEventKind, TaskLifecyclePhase, TaskSession, TaskSessionId};

use super::child_sessions::{
    insert_child_command, insert_task_event_in, map_task_session_row, require_child_write_lease,
    TASK_SESSION_SELECT,
};
use super::SqliteStore;

const INTERACTION_REVIEW_COLUMNS: &str = "SELECT
    id, wave_id, project_session_id, task_session_id,
    phase, phase_epoch, flow, step, step_index, phase_iteration, policy,
    reviewer_kind, reviewer_id, status, reason, prompt,
    worktree, branch, base_commit, head_commit, worktree_fingerprint,
    pr_number, pr_url, requested_by_generation, reviewer_generation,
    disposition, outcome, requested_at, completed_at
    FROM interaction_reviews";

impl SqliteStore {
    pub(crate) fn open_interaction_review(
        &self,
        session: &TaskSession,
        review: &InteractionReview,
        lease: &ChildWriteLease,
    ) -> StoreResult<(InteractionReview, bool)> {
        review.validate().map_err(invalid_review)?;
        session
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if session.id != review.task_session_id
            || session.wave_id != review.wave_id
            || session.project_session_id != review.project_session_id
            || session.lifecycle_phase != review.phase
            || session.phase_epoch != review.phase_epoch
            || session.phase_plan().flow != review.flow
            || session.phase_plan().interaction_policy != review.policy
            || session.phase_cursor != review.step_index
            || session.phase_iteration != review.phase_iteration
        {
            return Err(StoreError::InvalidData(
                "interaction review does not match its Task lifecycle waitpoint".to_string(),
            ));
        }
        if review.status != InteractionReviewStatus::Requested {
            return Err(StoreError::InvalidData(
                "new interaction reviews must be requested".to_string(),
            ));
        }
        if review.requested_by_generation != lease.generation {
            return Err(StoreError::InvalidData(format!(
                "interaction review generation {} does not match Task lease generation {}",
                review.requested_by_generation, lease.generation
            )));
        }
        if !matches!(
            &review.reviewer,
            InteractionReviewer::Project(id) if id == &session.project_session_id
        ) && review.reviewer != InteractionReviewer::Human
        {
            return Err(StoreError::InvalidData(
                "Task reviews must target their owning Project or the human".to_string(),
            ));
        }

        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, &ChildRef::Task(session.id.clone()), lease)?;
        if let Some(existing) = review_at_waitpoint(
            &transaction,
            &review.task_session_id,
            review.phase_epoch,
            review.phase_iteration,
            review.step_index,
        )? {
            if !same_waitpoint(&existing, review) {
                return Err(StoreError::InvalidData(
                    "interaction review waitpoint already belongs to a different exercise"
                        .to_string(),
                ));
            }
            transaction.commit()?;
            return Ok((existing, false));
        }

        insert_review(&transaction, review)?;
        insert_task_event_in(
            &transaction,
            session,
            &TaskEventKind::InteractionReviewRequested {
                review: Box::new(review.clone()),
            },
        )?;
        transaction.commit()?;
        Ok((review.clone(), true))
    }

    pub(crate) fn get_interaction_review(
        &self,
        review_id: &InteractionReviewId,
    ) -> StoreResult<Option<InteractionReview>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
            [review_id.as_str()],
            map_review_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub(crate) fn interaction_review_at(
        &self,
        task_session_id: &TaskSessionId,
        phase_epoch: u32,
        phase_iteration: u32,
        step_index: u32,
    ) -> StoreResult<Option<InteractionReview>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        review_at_waitpoint(
            &conn,
            task_session_id,
            phase_epoch,
            phase_iteration,
            step_index,
        )
    }

    pub(crate) fn list_interaction_reviews(
        &self,
        wave_id: Option<&WaveId>,
    ) -> StoreResult<Vec<InteractionReview>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = match wave_id {
            Some(_) => {
                format!("{INTERACTION_REVIEW_COLUMNS} WHERE wave_id=?1 ORDER BY requested_at, id")
            }
            None => format!("{INTERACTION_REVIEW_COLUMNS} ORDER BY requested_at, id"),
        };
        let mut statement = conn.prepare(&query)?;
        let mut reviews = Vec::new();
        if let Some(wave_id) = wave_id {
            let rows = statement.query_map([wave_id.as_str()], map_review_row)?;
            for row in rows {
                reviews.push(row?);
            }
        } else {
            let rows = statement.query_map([], map_review_row)?;
            for row in rows {
                reviews.push(row?);
            }
        }
        Ok(reviews)
    }

    pub(crate) fn send_project_interaction_review_message(
        &self,
        review_id: &InteractionReviewId,
        project_session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        text: &str,
    ) -> StoreResult<ChildCommand> {
        let text = require_text("review message", text)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
            &transaction,
            &ChildRef::Project(project_session_id.clone()),
            lease,
        )?;
        let review = require_open_project_review(&transaction, review_id, project_session_id)?;
        let command = _send_interaction_review_message(
            &transaction,
            &review,
            ChildCommandSource::Project(project_session_id.clone()),
            Some(lease.generation),
            &text,
        )?;
        transaction.commit()?;
        Ok(command)
    }

    pub(crate) fn activate_human_interaction_review(
        &self,
        session: &TaskSession,
        review_id: &InteractionReviewId,
        lease: &ChildWriteLease,
    ) -> StoreResult<InteractionReview> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(&transaction, &ChildRef::Task(session.id.clone()), lease)?;
        let review = require_open_human_review(&transaction, review_id)?;
        if review.task_session_id != session.id
            || review.phase_epoch != session.phase_epoch
            || review.phase_iteration != session.phase_iteration
            || review.step_index != session.phase_cursor
        {
            return Err(StoreError::InvalidData(
                "human interaction review is stale for this Task lifecycle waitpoint".to_string(),
            ));
        }
        transaction.execute(
            "UPDATE interaction_reviews SET status='active'
             WHERE id=?1 AND status IN ('requested', 'active')",
            [review_id.as_str()],
        )?;
        let active = transaction.query_row(
            &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
            [review_id.as_str()],
            map_review_row,
        )?;
        transaction.commit()?;
        Ok(active)
    }

    pub(crate) fn send_human_interaction_review_message(
        &self,
        review_id: &InteractionReviewId,
        source: ChildCommandSource,
        text: &str,
    ) -> StoreResult<ChildCommand> {
        if !matches!(
            source,
            ChildCommandSource::Human | ChildCommandSource::Attachment
        ) {
            return Err(StoreError::InvalidData(
                "human review messages require human or attachment authority".to_string(),
            ));
        }
        let text = require_text("review message", text)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let review = require_open_human_review(&transaction, review_id)?;
        let command = _send_interaction_review_message(&transaction, &review, source, None, &text)?;
        transaction.commit()?;
        Ok(command)
    }

    pub(crate) fn reply_to_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        task_session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        text: &str,
    ) -> StoreResult<()> {
        let text = require_text("review reply", text)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
            &transaction,
            &ChildRef::Task(task_session_id.clone()),
            lease,
        )?;
        let review = transaction
            .query_row(
                &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
                [review_id.as_str()],
                map_review_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        if &review.task_session_id != task_session_id || review.status.is_terminal() {
            return Err(StoreError::InvalidData(
                "interaction review is not open for this Task Session".to_string(),
            ));
        }
        let session = transaction.query_row(
            TASK_SESSION_SELECT,
            [task_session_id.as_str()],
            map_task_session_row,
        )?;
        transaction.execute(
            "UPDATE interaction_reviews SET status='active'
             WHERE id=?1 AND status IN ('requested', 'active')",
            [review_id.as_str()],
        )?;
        insert_task_event_in(
            &transaction,
            &session,
            &TaskEventKind::InteractionReviewMessage {
                review_id: review_id.clone(),
                author: InteractionReviewMessageAuthor::Task,
                text,
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn complete_project_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        project_session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        disposition: InteractionReviewDisposition,
        outcome: &str,
    ) -> StoreResult<(InteractionReview, bool)> {
        let outcome = require_text("review outcome", outcome)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_child_write_lease(
            &transaction,
            &ChildRef::Project(project_session_id.clone()),
            lease,
        )?;
        let review = transaction
            .query_row(
                &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
                [review_id.as_str()],
                map_review_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        if review.reviewer != InteractionReviewer::Project(project_session_id.clone()) {
            return Err(StoreError::InvalidData(
                "only the assigned Project reviewer may complete this review".to_string(),
            ));
        }
        let result = _complete_interaction_review(
            &transaction,
            review,
            ChildCommandSource::Project(project_session_id.clone()),
            Some(lease.generation),
            disposition,
            &outcome,
        )?;
        transaction.commit()?;
        Ok(result)
    }

    pub(crate) fn complete_human_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        disposition: InteractionReviewDisposition,
        outcome: &str,
    ) -> StoreResult<(InteractionReview, bool)> {
        let outcome = require_text("review outcome", outcome)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let review = transaction
            .query_row(
                &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
                [review_id.as_str()],
                map_review_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        if review.reviewer != InteractionReviewer::Human {
            return Err(StoreError::InvalidData(
                "only a human may complete this interaction review".to_string(),
            ));
        }
        let result = _complete_interaction_review(
            &transaction,
            review,
            ChildCommandSource::Human,
            None,
            disposition,
            &outcome,
        )?;
        transaction.commit()?;
        Ok(result)
    }
}

fn _send_interaction_review_message(
    transaction: &rusqlite::Transaction<'_>,
    review: &InteractionReview,
    source: ChildCommandSource,
    reviewer_generation: Option<u32>,
    text: &str,
) -> StoreResult<ChildCommand> {
    let session = transaction.query_row(
        TASK_SESSION_SELECT,
        [review.task_session_id.as_str()],
        map_task_session_row,
    )?;
    match reviewer_generation {
        Some(generation) => {
            transaction.execute(
                "UPDATE interaction_reviews
                 SET status='active', reviewer_generation=COALESCE(reviewer_generation, ?2)
                 WHERE id=?1 AND status IN ('requested', 'active')",
                params![review.id.as_str(), i64::from(generation)],
            )?;
        }
        None => {
            transaction.execute(
                "UPDATE interaction_reviews SET status='active'
                 WHERE id=?1 AND status IN ('requested', 'active')",
                [review.id.as_str()],
            )?;
        }
    }
    let command = ChildCommand::new(
        ChildRef::Task(review.task_session_id.clone()),
        source,
        ChildCommandKind::FollowUp {
            text: format!(
                "<interaction_review_message review_id=\"{}\" from=\"reviewer\">\n{text}\n\nReply with `lf task review reply {} \"<answer and evidence>\"`.\n</interaction_review_message>",
                review.id, review.id
            ),
        },
    );
    insert_child_command(transaction, &command)?;
    insert_task_event_in(
        transaction,
        &session,
        &TaskEventKind::CommandChanged {
            command_id: command.id.clone(),
            state: crate::child_session::ChildCommandState::Persisted,
            effect: command.effect,
            error: None,
        },
    )?;
    insert_task_event_in(
        transaction,
        &session,
        &TaskEventKind::InteractionReviewMessage {
            review_id: review.id.clone(),
            author: InteractionReviewMessageAuthor::Reviewer,
            text: text.to_string(),
        },
    )?;
    Ok(command)
}

fn _complete_interaction_review(
    transaction: &rusqlite::Transaction<'_>,
    review: InteractionReview,
    source: ChildCommandSource,
    reviewer_generation: Option<u32>,
    disposition: InteractionReviewDisposition,
    outcome: &str,
) -> StoreResult<(InteractionReview, bool)> {
    if review.status == InteractionReviewStatus::Completed {
        if review.disposition == Some(disposition) && review.outcome.as_deref() == Some(outcome) {
            return Ok((review, false));
        }
        return Err(StoreError::InvalidData(
            "interaction review already has a different completion".to_string(),
        ));
    }
    if review.status.is_terminal() {
        return Err(StoreError::InvalidData(
            "interaction review is already terminal".to_string(),
        ));
    }
    let completed_at = time::OffsetDateTime::now_utc();
    transaction.execute(
        "UPDATE interaction_reviews
         SET status='completed', reviewer_generation=?2, disposition=?3,
             outcome=?4, completed_at=?5
         WHERE id=?1",
        params![
            review.id.as_str(),
            reviewer_generation.map(i64::from),
            disposition.as_str(),
            outcome,
            completed_at.unix_timestamp(),
        ],
    )?;
    let command = ChildCommand::new(
        ChildRef::Task(review.task_session_id.clone()),
        source,
        ChildCommandKind::FollowUp {
            text: format!(
                "<interaction_review_completed review_id=\"{}\" disposition=\"{}\">\n{outcome}\n</interaction_review_completed>",
                review.id,
                disposition.as_str()
            ),
        },
    );
    insert_child_command(transaction, &command)?;
    let session = transaction.query_row(
        TASK_SESSION_SELECT,
        [review.task_session_id.as_str()],
        map_task_session_row,
    )?;
    insert_task_event_in(
        transaction,
        &session,
        &TaskEventKind::CommandChanged {
            command_id: command.id.clone(),
            state: crate::child_session::ChildCommandState::Persisted,
            effect: command.effect,
            error: None,
        },
    )?;
    insert_task_event_in(
        transaction,
        &session,
        &TaskEventKind::InteractionReviewCompleted {
            review_id: review.id.clone(),
            disposition,
            outcome: outcome.to_string(),
        },
    )?;
    let completed = transaction.query_row(
        &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
        [review.id.as_str()],
        map_review_row,
    )?;
    Ok((completed, true))
}

fn insert_review(conn: &rusqlite::Connection, review: &InteractionReview) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO interaction_reviews (
            id, wave_id, project_session_id, task_session_id,
            phase, phase_epoch, flow, step, step_index, phase_iteration, policy,
            reviewer_kind, reviewer_id, status, reason, prompt,
            worktree, branch, base_commit, head_commit, worktree_fingerprint,
            pr_number, pr_url, requested_by_generation, reviewer_generation,
            disposition, outcome, requested_at, completed_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
            ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29
         )",
        params![
            review.id.as_str(),
            review.wave_id.as_str(),
            review.project_session_id.as_str(),
            review.task_session_id.as_str(),
            review.phase.as_str(),
            i64::from(review.phase_epoch),
            review.flow,
            review.step,
            i64::from(review.step_index),
            i64::from(review.phase_iteration),
            review.policy.as_str(),
            review.reviewer.kind(),
            review.reviewer.id(),
            review.status.as_str(),
            review.reason,
            review.prompt,
            review.evidence.worktree.display().to_string(),
            review.evidence.branch,
            review.evidence.base_commit,
            review.evidence.head_commit,
            review.evidence.worktree_fingerprint,
            review.evidence.pr.as_ref().map(|pr| i64::from(pr.number)),
            review.evidence.pr.as_ref().map(|pr| pr.url.as_str()),
            i64::from(review.requested_by_generation),
            review.reviewer_generation.map(i64::from),
            review.disposition.map(InteractionReviewDisposition::as_str),
            review.outcome,
            review.requested_at.unix_timestamp(),
            review.completed_at.map(|at| at.unix_timestamp()),
        ],
    )?;
    Ok(())
}

fn review_at_waitpoint(
    conn: &rusqlite::Connection,
    task_session_id: &TaskSessionId,
    phase_epoch: u32,
    phase_iteration: u32,
    step_index: u32,
) -> StoreResult<Option<InteractionReview>> {
    conn.query_row(
        &format!(
            "{INTERACTION_REVIEW_COLUMNS}
             WHERE task_session_id=?1 AND phase_epoch=?2
               AND phase_iteration=?3 AND step_index=?4"
        ),
        params![
            task_session_id.as_str(),
            i64::from(phase_epoch),
            i64::from(phase_iteration),
            i64::from(step_index)
        ],
        map_review_row,
    )
    .optional()
    .map_err(StoreError::from)
}

fn same_waitpoint(left: &InteractionReview, right: &InteractionReview) -> bool {
    left.wave_id == right.wave_id
        && left.project_session_id == right.project_session_id
        && left.task_session_id == right.task_session_id
        && left.phase == right.phase
        && left.phase_epoch == right.phase_epoch
        && left.flow == right.flow
        && left.step == right.step
        && left.step_index == right.step_index
        && left.phase_iteration == right.phase_iteration
        && left.policy == right.policy
        && left.reviewer == right.reviewer
}

fn require_open_project_review(
    conn: &rusqlite::Connection,
    review_id: &InteractionReviewId,
    project_session_id: &ProjectSessionId,
) -> StoreResult<InteractionReview> {
    let review = conn
        .query_row(
            &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
            [review_id.as_str()],
            map_review_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    if review.reviewer != InteractionReviewer::Project(project_session_id.clone())
        || review.status.is_terminal()
    {
        return Err(StoreError::InvalidData(
            "interaction review is not open for this Project reviewer".to_string(),
        ));
    }
    Ok(review)
}

fn require_open_human_review(
    conn: &rusqlite::Connection,
    review_id: &InteractionReviewId,
) -> StoreResult<InteractionReview> {
    let review = conn
        .query_row(
            &format!("{INTERACTION_REVIEW_COLUMNS} WHERE id=?1"),
            [review_id.as_str()],
            map_review_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    if review.reviewer != InteractionReviewer::Human || review.status.is_terminal() {
        return Err(StoreError::InvalidData(
            "interaction review is not open for the human reviewer".to_string(),
        ));
    }
    Ok(review)
}

fn map_review_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InteractionReview> {
    let reviewer_kind = row.get::<_, String>(11)?;
    let reviewer_id = row.get::<_, Option<String>>(12)?;
    let reviewer = match (reviewer_kind.as_str(), reviewer_id) {
        ("human", None) => InteractionReviewer::Human,
        ("project", Some(id)) => InteractionReviewer::Project(ProjectSessionId::from_raw(id)),
        ("wave", Some(id)) => InteractionReviewer::Wave(
            WaveId::parse(&id).map_err(|error| invalid_column(12, error))?,
        ),
        _ => {
            return Err(invalid_column(
                11,
                InteractionReviewDataError::InvalidInvariant(
                    "stored reviewer is incomplete".to_string(),
                ),
            ))
        }
    };
    let pr_number = row.get::<_, Option<i64>>(21)?.map(|number| number as u32);
    let pr_url = row.get::<_, Option<String>>(22)?;
    let pr = match (pr_number, pr_url) {
        (Some(number), Some(url)) => Some(InteractionReviewPr { number, url }),
        (None, None) => None,
        _ => {
            return Err(invalid_column(
                21,
                InteractionReviewDataError::InvalidInvariant(
                    "stored PR evidence is incomplete".to_string(),
                ),
            ))
        }
    };
    Ok(InteractionReview {
        id: InteractionReviewId::from_raw(row.get::<_, String>(0)?),
        wave_id: row.get(1)?,
        project_session_id: ProjectSessionId::from_raw(row.get::<_, String>(2)?),
        task_session_id: TaskSessionId::from_raw(row.get::<_, String>(3)?),
        phase: TaskLifecyclePhase::from_str(&row.get::<_, String>(4)?)
            .map_err(|error| invalid_column(4, error))?,
        phase_epoch: row.get::<_, i64>(5)? as u32,
        flow: row.get(6)?,
        step: row.get(7)?,
        step_index: row.get::<_, i64>(8)? as u32,
        phase_iteration: row.get::<_, i64>(9)? as u32,
        policy: InteractionPolicy::from_str(&row.get::<_, String>(10)?)
            .map_err(|error| invalid_column(10, error))?,
        reviewer,
        status: InteractionReviewStatus::from_str(&row.get::<_, String>(13)?)
            .map_err(|error| invalid_column(13, error))?,
        reason: row.get(14)?,
        prompt: row.get(15)?,
        evidence: InteractionReviewEvidence {
            worktree: PathBuf::from(row.get::<_, String>(16)?),
            branch: row.get(17)?,
            base_commit: row.get(18)?,
            head_commit: row.get(19)?,
            worktree_fingerprint: row.get(20)?,
            pr,
        },
        requested_by_generation: row.get::<_, i64>(23)? as u32,
        reviewer_generation: row.get::<_, Option<i64>>(24)?.map(|value| value as u32),
        disposition: row
            .get::<_, Option<String>>(25)?
            .map(|value| InteractionReviewDisposition::from_str(&value))
            .transpose()
            .map_err(|error| invalid_column(25, error))?,
        outcome: row.get(26)?,
        requested_at: unix_to_datetime(row.get(27)?),
        completed_at: row.get::<_, Option<i64>>(28)?.map(unix_to_datetime),
    })
}

fn require_text(name: &str, value: &str) -> StoreResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StoreError::InvalidData(format!("{name} cannot be empty")));
    }
    Ok(value.to_string())
}

fn invalid_review(error: InteractionReviewDataError) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}
