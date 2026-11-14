//! Complete, typed payloads for a board, Task detail and Session card.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::waves::{
    Evidence, NextMove, PrSnapshot, RoadmapSnapshot, RoadmapTask, TaskConditionState, TaskSummary,
};
use crate::durable::{GraphProcessDetail, TaskSection, TaskState};
use crate::ops::human_session::{SessionRecord, SessionState};
use crate::work::task::comment::TaskComments;
use crate::work::task::{CiCheck, CiState};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlanBoard {
    pub tasks: Evidence<PlanBoardTask>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlanBoardTask {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub section: TaskSection,
    pub state: TaskState,
    #[serde(deserialize_with = "Option::deserialize")]
    pub pr: Option<BoardPr>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub waiting_on: Option<NextMove>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BoardPr {
    pub url: String,
}

impl From<RoadmapTask> for PlanBoardTask {
    fn from(row: RoadmapTask) -> Self {
        Self {
            state: super::waves::task_state(&row),
            id: row.task.id,
            identifier: row.task.identifier,
            title: row.task.name,
            section: row.section,
            pr: row.pr.and_then(|pr| {
                Some(BoardPr {
                    url: pr.publication?.github?.url,
                })
            }),
            waiting_on: matches!(
                row.condition.state,
                TaskConditionState::Waiting | TaskConditionState::Blocked
            )
            .then_some(row.next_move),
        }
    }
}

impl From<RoadmapSnapshot> for PlanBoard {
    fn from(plan: RoadmapSnapshot) -> Self {
        Self {
            generated_at: plan.generated_at,
            tasks: match plan.tasks {
                Evidence::Ok { items, truncated } => Evidence::Ok {
                    items: items.into_iter().map(PlanBoardTask::from).collect(),
                    truncated,
                },
                Evidence::Unavailable { reason } => Evidence::Unavailable { reason },
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskDetail {
    pub task: TaskSummary,
    pub section: TaskSection,
    pub state: TaskState,
    #[serde(deserialize_with = "Option::deserialize")]
    pub pr: Option<PrSnapshot>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub checks: Option<TaskChecks>,
    pub graph_processes: Vec<GraphProcessDetail>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub comments: Option<TaskComments>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub current_session: Option<SessionRecord>,
}

/// Recorded required checks, pinned to the observed PR head. Null means unobserved.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskChecks {
    pub head_sha: String,
    pub state: CiState,
    pub failing_checks: Vec<CiCheck>,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionCard {
    pub session: SessionRecord,
    pub liveness: SessionLiveness,
    /// The recorded occupied terminal, never an input-control credential.
    #[serde(deserialize_with = "Option::deserialize")]
    pub control_holder: Option<String>,
    /// Last nonempty line of the current provider preview, not terminal scrollback.
    #[serde(deserialize_with = "Option::deserialize")]
    pub last_line: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionLiveness {
    Live,
    Dead,
    Unknown,
}

impl From<SessionRecord> for SessionCard {
    fn from(session: SessionRecord) -> Self {
        let liveness = match session.state {
            SessionState::Active | SessionState::Detached => SessionLiveness::Live,
            SessionState::Closed => SessionLiveness::Dead,
            SessionState::Unknown => SessionLiveness::Unknown,
        };
        Self {
            control_holder: session.terminal_ids.first().cloned(),
            last_line: session.preview.as_ref().and_then(|preview| {
                preview
                    .text
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .map(str::to_owned)
            }),
            session,
            liveness,
        }
    }
}

pub fn board(
    directory: &Path,
    wave: Option<&str>,
    task: Option<&str>,
    all: bool,
) -> Result<PlanBoard> {
    let mut plan = super::waves::show_snapshot(directory, wave, task, all, false)?;
    if wave.is_some() {
        if let Evidence::Ok { items, .. } = &mut plan.tasks {
            items.retain(|row| {
                plan.waves
                    .iter()
                    .any(|wave| row.task.wave_id.as_deref() == Some(wave.wave.id.as_str()))
            });
        }
    }
    Ok(plan.into())
}

pub fn task_detail(directory: &Path, issue: &str) -> Result<TaskDetail> {
    let repo = crate::repo::working_directory_in(directory)?;
    let store = crate::store::read_existing_registry()?
        .map(Arc::new)
        .context("no Loopflow store exists on this Machine")?;
    let id = match store
        .sqlite
        .resolve_task_id(issue, Some(&repo.to_string_lossy()))?
    {
        Some(id) => Some(id),
        None => store.sqlite.resolve_task_id(issue, None)?,
    }
    .ok_or_else(|| {
        crate::engine::Error::new(
            crate::engine::ErrorCode::NotFound,
            format!("Task {issue} is not registered"),
        )
        .with(serde_json::json!({ "issue": issue }))
    })?;
    let task = store
        .sqlite
        .task(&id)?
        .context("Task is no longer recorded")?;
    let pane = tokio::runtime::Runtime::new()?.block_on(async {
        let status = crate::ops::task::registered_task_status(&store, task).await?;
        super::waves::task_pane(&store, status).await
    })?;
    let row = pane.row.context("registered Task has no planning row")?;
    let current_session = store
        .sqlite
        .task_primary(&id)?
        .map(|session| {
            let capacity = crate::provider_account::LaunchCapacity::read(&store.sqlite)?;
            crate::ops::human_session::listed(&store, session.id.as_str(), &capacity)
        })
        .transpose()?
        .flatten();
    let checks = pane
        .status
        .execution
        .and_then(|execution| execution.pr)
        .and_then(|pr| pr.ci_observation)
        .map(|checks| -> Result<TaskChecks> {
            Ok(TaskChecks {
                head_sha: checks.head_sha,
                state: checks.state,
                failing_checks: checks.failing_checks,
                observed_at: checks
                    .observed_at
                    .format(&time::format_description::well_known::Rfc3339)?,
            })
        })
        .transpose()?;
    Ok(TaskDetail {
        state: super::waves::task_state(&row),
        section: row.section,
        task: row.task,
        pr: row.pr,
        checks,
        graph_processes: pane.status.graph_processes,
        comments: pane.status.comments,
        current_session,
    })
}

pub fn session_card(id: &str) -> Result<SessionCard> {
    let store = crate::store::read_existing_registry()?
        .map(Arc::new)
        .context("no Loopflow store exists on this Machine")?;
    let capacity = crate::provider_account::LaunchCapacity::read(&store.sqlite)?;
    let session = crate::ops::human_session::listed(&store, id, &capacity)?.ok_or_else(|| {
        crate::engine::Error::new(
            crate::engine::ErrorCode::NotFound,
            format!("Session {id} is not recorded"),
        )
        .with(serde_json::json!({ "id": id }))
    })?;
    Ok(session.into())
}

pub fn print(read: super::read::Read) -> Result<()> {
    println!("{}", super::subscribe::selected_answer(&read.read()?)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PlanBoard, SessionCard, SessionLiveness, TaskDetail};
    use crate::lf::commands::waves::{Evidence, RoadmapSnapshot};
    use crate::ops::human_session::{SessionRecord, SessionState};

    #[test]
    fn board_keeps_sections_and_waiting_without_runtime_payload() {
        let source = include_str!("../../../../../tests/fixtures/dto/roadmap_snapshot.json");
        let plan: RoadmapSnapshot = serde_json::from_str(source).unwrap();
        let board = serde_json::to_value(PlanBoard::from(plan)).unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/plan_board.json"
        ))
        .unwrap();
        assert_eq!(board, expected);
        assert!(serde_json::to_vec(&board).unwrap().len() < source.len() / 4);
        let decoded: PlanBoard = serde_json::from_value(board.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), board);
    }

    #[test]
    fn board_keeps_unavailable_planning_distinct_from_empty() {
        let board = PlanBoard::from(RoadmapSnapshot {
            tasks: Evidence::Unavailable {
                reason: "offline".into(),
            },
            generated_at: "2026-11-09T20:00:00Z".into(),
            waves: vec![],
        });
        assert!(matches!(board.tasks, Evidence::Unavailable { reason } if reason == "offline"));
    }

    #[test]
    fn screen_dto_fixtures_round_trip_and_require_fields() {
        let detail: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_screen_detail.json"
        ))
        .unwrap();
        let decoded: TaskDetail = serde_json::from_value(detail.clone()).unwrap();
        assert_eq!(
            decoded.checks.as_ref().unwrap().failing_checks[0].name,
            "rust-test"
        );
        assert_eq!(serde_json::to_value(decoded).unwrap(), detail);
        for field in detail.as_object().unwrap().keys() {
            let mut missing = detail.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<TaskDetail>(missing).is_err(),
                "accepted missing {field}"
            );
        }

        let card: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/session_card.json"
        ))
        .unwrap();
        let decoded: SessionCard = serde_json::from_value(card.clone()).unwrap();
        assert_eq!(decoded.liveness, SessionLiveness::Unknown);
        assert_eq!(serde_json::to_value(decoded).unwrap(), card);
        for field in card.as_object().unwrap().keys() {
            let mut missing = card.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<SessionCard>(missing).is_err(),
                "accepted missing {field}"
            );
        }
    }

    #[test]
    fn card_reports_live_detached_dead_and_unknown_without_inventing_text() {
        let session: SessionRecord = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/session_metadata.json"
        ))
        .unwrap();
        for (state, expected) in [
            (SessionState::Active, SessionLiveness::Live),
            (SessionState::Detached, SessionLiveness::Live),
            (SessionState::Closed, SessionLiveness::Dead),
            (SessionState::Unknown, SessionLiveness::Unknown),
        ] {
            let card = SessionCard::from(SessionRecord {
                state,
                ..session.clone()
            });
            assert_eq!(card.liveness, expected);
            assert_eq!(card.last_line, None);
            assert_eq!(card.control_holder, None);
        }
        let mut live = session;
        live.state = SessionState::Active;
        live.terminal_ids = vec!["rex:pane".into()];
        live.preview = Some(crate::session::SessionPreview {
            provider_thread: "thread".into(),
            provider_turn: "turn".into(),
            observed_at: 42,
            source: "native".into(),
            text: "first\nlast\n\n".into(),
        });
        let card = SessionCard::from(live);
        assert_eq!(card.control_holder.as_deref(), Some("rex:pane"));
        assert_eq!(card.last_line.as_deref(), Some("last"));
    }
}
