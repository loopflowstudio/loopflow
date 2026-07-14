use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::Path as FsPath;

use crate::lfd::http::dto::{session_dto, ListResponse, SessionDto};
use crate::lfd::http::routes::{parse_lfd_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Session, SessionUse, LIVE_SESSION_STATUSES};

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub repo: Option<String>,
    pub wave_id: Option<String>,
    pub parent_session_id: Option<String>,
    #[serde(rename = "use")]
    pub session_use: Option<String>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentSessionQuery {
    pub cwd: String,
}

pub async fn list_sessions_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListSessionsQuery>,
) -> ApiResult<ListResponse<SessionDto>> {
    let statuses = query
        .active_only
        .filter(|value| *value)
        .map(|_| LIVE_SESSION_STATUSES);
    let wave_id = query
        .wave_id
        .as_deref()
        .map(|value| parse_lfd_id(value, "invalid wave id"))
        .transpose()?;
    let mut sessions = state
        .store
        .list_control_sessions(wave_id.as_ref(), statuses)
        .await
        .map_err(map_store_error)?;

    if let Some(parent_session_id) = query.parent_session_id.as_deref() {
        let parent_session_id = parse_lfd_id(parent_session_id, "invalid parent session id")?;
        sessions.retain(|session| session.parent_session_id.as_ref() == Some(&parent_session_id));
    }

    if let Some(session_use) = query.session_use.as_deref() {
        let session_use = match session_use {
            "wave_agent" => SessionUse::WaveAgent,
            "worker" => SessionUse::Worker,
            "palette" => SessionUse::Palette,
            _ => return Err(api_error(StatusCode::BAD_REQUEST, "invalid session use")),
        };
        sessions.retain(|session| session.session_use == session_use);
    }

    if let Some(repo) = query.repo.as_deref() {
        let mut filtered = Vec::with_capacity(sessions.len());
        for session in sessions {
            let Some(wave) = state
                .store
                .get_wave(&session.wave_id)
                .await
                .map_err(map_store_error)?
            else {
                continue;
            };
            if wave.repo == repo {
                filtered.push(session);
            }
        }
        sessions = filtered;
    }

    Ok(Json(ListResponse::new(
        sessions.into_iter().map(session_dto).collect(),
        false,
    )))
}

pub async fn current_session_handler(
    State(state): State<HttpState>,
    Query(query): Query<CurrentSessionQuery>,
) -> ApiResult<SessionDto> {
    let cwd = query.cwd.trim();
    if cwd.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "cwd is required"));
    }

    let sessions = state
        .store
        .list_control_sessions(None, Some(LIVE_SESSION_STATUSES))
        .await
        .map_err(map_store_error)?;

    let matches = sessions
        .into_iter()
        .filter(|session| session_matches_cwd(session, cwd))
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(api_error(
            StatusCode::NOT_FOUND,
            "current session not found",
        )),
        1 => Ok(Json(session_dto(
            matches
                .into_iter()
                .next()
                .expect("exactly one current session match"),
        ))),
        _ => Err(api_error(
            StatusCode::CONFLICT,
            "current session is ambiguous",
        )),
    }
}

pub async fn get_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = load_session(&state, &session_id).await?;
    Ok(Json(session_dto(session)))
}

fn session_matches_cwd(session: &Session, cwd: &str) -> bool {
    let session_cwd = FsPath::new(&session.cwd);
    let cwd = FsPath::new(cwd);
    cwd.starts_with(session_cwd)
}

async fn load_session(state: &HttpState, session_id: &LfdId) -> Result<Session, ApiError> {
    state
        .store
        .get_control_session(session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "session not found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{SessionStatus, SessionUse, Wave};
    use axum::extract::{Query, State};
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        Wave::new(LfdId::new(), "terminal-test".to_string(), repo.to_string())
    }

    fn make_session(wave_id: LfdId, source: &str) -> Session {
        Session {
            id: LfdId::new(),
            wave_id,
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            skill: "design".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: source.to_string(),
            tmux_name: String::new(),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn current_session_matches_cwd_inside_session_worktree() {
        let state = test_http_state().await;
        let wave = make_wave("/tmp/repo");
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");
        let mut session = make_session(wave.id().clone(), "lf_cli");
        session.cwd = "/tmp/repo".to_string();
        session.status = SessionStatus::Running;
        state
            .store
            .create_control_session(&session)
            .await
            .expect("session should be created");

        let Json(response) = current_session_handler(
            State(state),
            Query(CurrentSessionQuery {
                cwd: "/tmp/repo/python".to_string(),
            }),
        )
        .await
        .expect("current session lookup should succeed");

        assert_eq!(response.id, session.id.to_string());
        assert_eq!(response.cwd, "/tmp/repo");
    }

    #[tokio::test]
    async fn current_session_rejects_ambiguous_cwd_matches() {
        let state = test_http_state().await;
        let wave = make_wave("/tmp/repo");
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");
        for cwd in ["/tmp/repo", "/tmp/repo/python"] {
            let mut session = make_session(wave.id().clone(), "lf_cli");
            session.cwd = cwd.to_string();
            session.status = SessionStatus::Running;
            state
                .store
                .create_control_session(&session)
                .await
                .expect("session should be created");
        }

        let error = current_session_handler(
            State(state),
            Query(CurrentSessionQuery {
                cwd: "/tmp/repo/python".to_string(),
            }),
        )
        .await
        .expect_err("ambiguous current session lookup should fail");

        assert_eq!(error.0, StatusCode::CONFLICT);
    }
}
