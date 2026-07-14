use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::Path as FsPath;
use std::str::FromStr;
use time::OffsetDateTime;
use tokio::process::Command as TokioCommand;
use tracing::warn;

use crate::engine::wave_config::update_wave_agent_config;
use crate::lfd::http::dto::{
    session_dto, DeletedResourceResponse, ListResponse, StopWaveResponse, WaveAgentTreeDto,
    WaveAgentTreeSessionDto, WaveDto,
};
use crate::lfd::http::routes::{build_wave_dto, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::types::{Run, RunStatus, WaveStatus, LIVE_SESSION_STATUSES};

// TODO(M1): convert the mutating wave routes in this file to exec lf argv or
// remove them. lfd is the local face, not a hand: no direct git, worktree, tmux,
// or ops calls from route handlers.
#[derive(Debug, Deserialize)]
pub struct ListWavesQuery {
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExpandQuery {
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

/// Accept `expand[]=value` as either a single string or repeated params.
#[derive(Debug, Default, Clone)]
pub struct ExpandParam(Vec<String>);

impl ExpandParam {
    fn contains(&self, value: &str) -> bool {
        self.0.iter().any(|v| v == value)
    }
}

impl<'de> serde::Deserialize<'de> for ExpandParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ExpandVisitor;
        impl<'de> de::Visitor<'de> for ExpandVisitor {
            type Value = ExpandParam;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or list of strings")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<ExpandParam, E> {
                Ok(ExpandParam(vec![v.to_string()]))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<ExpandParam, A::Error> {
                let mut values = Vec::new();
                while let Some(v) = seq.next_element::<String>()? {
                    values.push(v);
                }
                Ok(ExpandParam(values))
            }
        }

        deserializer.deserialize_any(ExpandVisitor)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct GetWaveAgentTreeQuery {
    active_only: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    goal: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    workers: Option<u32>,
    status: Option<String>,
    agent: Option<String>,
    skill_agents: Option<std::collections::HashMap<String, String>>,
    serialized: Option<bool>,
}

pub async fn list_waves_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListResponse<WaveDto>> {
    let waves = state
        .store
        .list_waves(query.repo.as_deref())
        .await
        .map_err(map_store_error)?;
    let include_active_run = query.expand.contains("active_run");
    let (waves, has_more) = super::paginate(
        waves,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |w| w.id(),
    );
    let views = crate::lfd::http::routes::build_wave_dtos(&state.store, waves, include_active_run)
        .await
        .map_err(map_store_error)?;
    Ok(Json(ListResponse::new(views, has_more)))
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn update_wave_workers(
    current_workers: u32,
    requested_workers: Option<u32>,
    serialized: Option<bool>,
) -> u32 {
    requested_workers
        .or_else(|| serialized.map(|_| 1))
        .unwrap_or(current_workers)
}

pub async fn get_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ExpandQuery>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let include_active_run = query.expand.contains("active_run");
    let view = build_wave_dto(&state.store, wave, include_active_run)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

pub async fn update_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<UpdateWaveRequest>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // A Wave's authored name is its `wave/<name>/` identity. HTTP cannot move
    // that directory atomically with its Linear Initiative and durable store.
    if let Some(ref name) = payload.name {
        if !name.is_empty() && *name != *wave.name() {
            return Err(api_error(
                StatusCode::PRECONDITION_FAILED,
                "Wave names are authored by the wave/<name>/ directory and cannot be renamed through HTTP",
            ));
        }
    }

    if let Some(goal) = trimmed_non_empty(payload.goal.as_deref()) {
        wave.goal = goal;
    }
    if let Some(direction) = payload.direction {
        wave.direction = direction;
    }
    if let Some(area) = payload.area {
        wave.area = area;
    }
    wave.workers = update_wave_workers(wave.workers, payload.workers, payload.serialized);
    if let Some(status) = payload.status {
        let parsed = WaveStatus::from_str(&status)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid status"))?;
        wave.set_status(parsed);
    }

    if payload.agent.is_some() || payload.skill_agents.is_some() {
        let repo = wave.repo().to_string();
        let wave_name = wave.name().clone();
        let agent = payload.agent.clone();
        let skill_agents = payload.skill_agents.clone();
        run_blocking_result(
            move || update_wave_agent_config(FsPath::new(&repo), &wave_name, agent, skill_agents),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }

    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    let view = build_wave_dto(&state.store, wave, false)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

pub async fn delete_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<DeletedResourceResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Delete from the store (cascades to runs, sessions, etc.).
    state
        .store
        .delete_wave(&wave_id)
        .await
        .map_err(map_store_error)?;

    Ok(Json(DeletedResourceResponse {
        id: wave_id.to_string(),
        object: "wave".to_string(),
        deleted: true,
    }))
}

pub async fn get_wave_agent_tree_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<GetWaveAgentTreeQuery>,
) -> ApiResult<WaveAgentTreeDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let root_wave = build_wave_dto(&state.store, wave, false)
        .await
        .map_err(map_store_error)?;
    let children = state
        .store
        .list_child_waves(&wave_id)
        .await
        .map_err(map_store_error)?;
    let child_waves = crate::lfd::http::routes::build_wave_dtos(&state.store, children, false)
        .await
        .map_err(map_store_error)?;
    let statuses = query
        .active_only
        .unwrap_or(true)
        .then_some(LIVE_SESSION_STATUSES);
    let sessions = state
        .store
        .list_control_sessions(Some(&wave_id), statuses)
        .await
        .map_err(map_store_error)?
        .into_iter()
        .map(|session| WaveAgentTreeSessionDto {
            session: session_dto(session),
        })
        .collect();

    Ok(Json(WaveAgentTreeDto {
        object: "wave_agent_tree".to_string(),
        id: format!("tree-{wave_id}"),
        wave: root_wave,
        child_waves,
        sessions,
    }))
}

pub async fn stop_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<StopWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let run = state
        .store
        .get_active_run(&wave_id)
        .await
        .map_err(map_store_error)?;

    if let Some(mut run) = run {
        run.status = RunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_run(&run)
            .await
            .map_err(map_store_error)?;
        cancel_active_session(&state, &run).await?;
        let wave_id_for_update = run.wave_id.clone();
        if let Some(mut wave) = state
            .store
            .get_wave(&wave_id_for_update)
            .await
            .map_err(map_store_error)?
        {
            wave.set_status(WaveStatus::Failed);
            state
                .store
                .update_wave(&wave)
                .await
                .map_err(map_store_error)?;
        }
    }

    Ok(Json(StopWaveResponse { stopped: true }))
}

async fn cancel_active_session(state: &HttpState, run: &Run) -> Result<(), ApiError> {
    let Some(mut session) = state
        .store
        .get_active_control_session_for_run(&run.id)
        .await
        .map_err(map_store_error)?
    else {
        return Ok(());
    };

    if !session.cancel() {
        return Ok(());
    }

    state
        .store
        .update_control_session(&session)
        .await
        .map_err(map_store_error)?;
    if session.is_tmux_backed() {
        match TokioCommand::new("tmux")
            .args(["kill-session", "-t", &session.tmux_name])
            .status()
            .await
        {
            Ok(status) if status.success() => {}
            Ok(status) => warn!(
                session_id = %session.id,
                exit_code = ?status.code(),
                "failed to kill tmux terminal session while stopping wave"
            ),
            Err(err) => warn!(
                session_id = %session.id,
                error = %err,
                "failed to kill tmux terminal session while stopping wave"
            ),
        }
    }

    Ok(())
}

fn map_join_error(err: tokio::task::JoinError) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        ApiMessage::Untrusted(err.to_string()),
    )
}

async fn run_blocking_result<T, E, F>(func: F, failure_status: StatusCode) -> Result<T, ApiError>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    tokio::task::spawn_blocking(func)
        .await
        .map_err(map_join_error)?
        .map_err(|err| api_error(failure_status, ApiMessage::Untrusted(err.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::{init_git_repo, test_http_state};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Session, SessionStatus, SessionUse, Wave};
    use std::path::Path;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    fn make_wave(repo: &str, name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_session(wave: &Wave, session_use: SessionUse, cwd: &Path) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use,
            skill: "implement".to_string(),
            agent: "lf".to_string(),
            cwd: cwd.to_string_lossy().to_string(),
            argv: Vec::new(),
            env: Default::default(),
            source: "wave_skill_tmux".to_string(),
            tmux_name: "lf-worker-caller".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn get_wave_agent_tree_returns_attributed_sessions() {
        let state = test_http_state().await;
        let repo = tempdir().expect("tempdir");
        init_git_repo(repo.path());
        let wave = make_wave(&repo.path().to_string_lossy(), "tree-wave");
        state.store.create_wave(&wave).await.expect("seed wave");
        let parent = make_session(&wave, SessionUse::WaveAgent, repo.path());
        let mut child = make_session(&wave, SessionUse::Worker, repo.path());
        child.parent_session_id = Some(parent.id.clone());
        child.tmux_name = "lf-worker".to_string();
        state
            .store
            .create_control_session(&parent)
            .await
            .expect("seed parent session");
        state
            .store
            .create_control_session(&child)
            .await
            .expect("seed child session");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            Path(wave.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.object, "wave_agent_tree");
        assert_eq!(response.wave.id, wave.id().to_string());
        // This wave is a leaf (no child waves), so the chord tree is empty here.
        assert_eq!(response.child_waves.len(), 0);
        let child_node = response
            .sessions
            .iter()
            .find(|node| node.session.id == child.id.to_string())
            .expect("child session in tree");
        assert_eq!(
            child_node.session.parent_session_id.as_deref(),
            Some(parent.id.as_str())
        );
    }

    // A chord's WaveAgentTree exposes its children: one leaf child wave per
    // repo, each with its own repo. This is the two-repo chord shape — the
    // ancestry query drives which children a chord would loop.
    #[tokio::test]
    async fn get_wave_agent_tree_returns_child_waves_for_chord() {
        let state = test_http_state().await;
        let repo_a = tempdir().expect("tempdir a");
        let repo_b = tempdir().expect("tempdir b");
        init_git_repo(repo_a.path());
        init_git_repo(repo_b.path());

        let chord = make_wave(&repo_a.path().to_string_lossy(), "chord-root");
        state.store.create_wave(&chord).await.expect("seed chord");

        let child_a = make_wave(&repo_a.path().to_string_lossy(), "chord-child-a")
            .with_parent(chord.id().clone());
        let child_b = make_wave(&repo_b.path().to_string_lossy(), "chord-child-b")
            .with_parent(chord.id().clone());
        state
            .store
            .create_wave(&child_a)
            .await
            .expect("seed child a");
        state
            .store
            .create_wave(&child_b)
            .await
            .expect("seed child b");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            Path(chord.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.child_waves.len(), 2);
        let repos: Vec<&str> = response
            .child_waves
            .iter()
            .map(|w| w.repo.as_str())
            .collect();
        assert!(repos.contains(&repo_a.path().to_string_lossy().as_ref()));
        assert!(repos.contains(&repo_b.path().to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn stop_wave_cancels_active_session_for_waiting_run() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let mut wave = make_wave(&repo, "designer");
        wave.set_status(WaveStatus::Running);
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.repo = repo.clone();
        run.flow = "build".to_string();
        run.status = RunStatus::Waiting;
        run.worktree = repo.clone();
        run.branch = "main".to_string();
        state
            .store
            .create_run(&run)
            .await
            .expect("run should be created");

        let session = Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: Some(run.id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            skill: "design".to_string(),
            agent: "lf".to_string(),
            cwd: repo.clone(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: "wave_skill".to_string(),
            tmux_name: "lf-test-branch".to_string(),
            status: SessionStatus::Running,
            attached_at: Some(OffsetDateTime::now_utc()),
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: Some("token".to_string()),
        };
        state
            .store
            .create_control_session(&session)
            .await
            .expect("terminal session should be created");

        let Json(response) = stop_wave_handler(State(state.clone()), Path(wave.id().to_string()))
            .await
            .expect("stop wave");
        assert!(response.stopped);

        let updated_run = state
            .store
            .get_run(&run.id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, RunStatus::Failed);
        assert_eq!(updated_run.error.as_deref(), Some("stopped"));

        let updated_wave = state
            .store
            .get_wave(wave.id())
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        assert_eq!(updated_wave.status(), WaveStatus::Failed);

        let updated_session = state
            .store
            .get_control_session(&session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should exist");
        assert_eq!(updated_session.status, SessionStatus::Canceled);
        assert!(updated_session.completed_at.is_some());
    }

    #[tokio::test]
    async fn list_with_repo_filter() {
        let state = test_http_state().await;
        let repo_a_tmp = tempdir().expect("tempdir");
        let repo_b_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_a_tmp.path());
        init_git_repo(repo_b_tmp.path());
        let repo_a = repo_a_tmp.path().to_string_lossy().to_string();
        let repo_b = repo_b_tmp.path().to_string_lossy().to_string();

        state
            .store
            .create_wave(&make_wave(&repo_a, "wave-a"))
            .await
            .expect("create wave in repo a");
        state
            .store
            .create_wave(&make_wave(&repo_b, "wave-b"))
            .await
            .expect("create wave in repo b");

        let Json(listed) = list_waves_handler(
            State(state),
            Query(ListWavesQuery {
                repo: Some(repo_a.clone()),
                limit: None,
                starting_after: None,
                ending_before: None,
                expand: ExpandParam::default(),
            }),
        )
        .await
        .expect("list waves");

        assert_eq!(listed.data.len(), 1);
        assert_eq!(listed.data[0].repo, repo_a);
        assert_eq!(listed.data[0].name, "wave-a");
    }

    #[tokio::test]
    async fn update_fields_handler() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let mut wave = make_wave(&repo, "before");
        wave.direction = vec!["infra".to_string()];
        wave.area = vec!["src/".to_string()];
        state.store.create_wave(&wave).await.expect("create wave");
        let created_id = wave.id().to_string();

        let Json(updated) = update_wave_handler(
            State(state.clone()),
            Path(created_id.clone()),
            Json(UpdateWaveRequest {
                direction: Some(vec!["clarity".to_string()]),
                area: Some(vec!["docs/".to_string()]),
                ..Default::default()
            }),
        )
        .await
        .expect("update wave");

        assert_eq!(updated.name, "before");
        assert_eq!(updated.direction, vec!["clarity".to_string()]);
        assert_eq!(updated.area, vec!["docs/".to_string()]);

        let Json(found) = get_wave_handler(
            State(state),
            Path(created_id),
            Query(ExpandQuery::default()),
        )
        .await
        .expect("get updated wave");
        assert_eq!(found.name, "before");
        assert_eq!(found.direction, vec!["clarity".to_string()]);
        assert_eq!(found.area, vec!["docs/".to_string()]);
    }

    #[tokio::test]
    async fn update_rejects_wave_rename_without_mutating_identity() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();
        let wave = make_wave(&repo, "before");
        state.store.create_wave(&wave).await.expect("create wave");

        let result = update_wave_handler(
            State(state.clone()),
            Path(wave.id().to_string()),
            Json(UpdateWaveRequest {
                name: Some("after".to_string()),
                ..Default::default()
            }),
        )
        .await;

        assert!(matches!(result, Err((StatusCode::PRECONDITION_FAILED, _))));
        let stored = state
            .store
            .get_wave(wave.id())
            .await
            .expect("lookup")
            .expect("wave");
        assert_eq!(stored.name(), "before");
    }

    #[tokio::test]
    async fn delete_then_get_returns_not_found() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let wave = make_wave(&repo, "delete-me");
        state.store.create_wave(&wave).await.expect("create wave");
        let created_id = wave.id().to_string();

        let Json(deleted) = delete_wave_handler(State(state.clone()), Path(created_id.clone()))
            .await
            .expect("delete wave");
        assert!(deleted.deleted);

        let missing = get_wave_handler(
            State(state),
            Path(created_id),
            Query(ExpandQuery::default()),
        )
        .await;
        assert!(matches!(missing, Err((StatusCode::NOT_FOUND, _))));
    }
}
