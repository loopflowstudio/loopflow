use axum::extract::{Path, Query, State};
use axum::http::header::HOST;
use axum::http::uri::Authority;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use tokio::process::Command;
use tracing::warn;

use crate::lfd::http::dto::{
    terminal_connection_info_dto, terminal_session_dto, ListResponse, TerminalConnectionInfoDto,
    TerminalSessionDto,
};
use crate::lfd::http::routes::{parse_lfd_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Event, TerminalSession, TerminalSessionStatus, TMUX_TERMINAL_SOURCE};

const COMPLETION_TOKEN_HEADER: &str = "x-terminal-completion-token";

#[derive(Debug, Deserialize)]
pub struct ListTerminalSessionsQuery {
    pub repo: Option<String>,
    pub wave_id: Option<String>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteTerminalSessionRequest {
    pub exit_code: i32,
}

pub async fn list_terminal_sessions_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListTerminalSessionsQuery>,
) -> ApiResult<ListResponse<TerminalSessionDto>> {
    let statuses = query.active_only.filter(|value| *value).map(|_| {
        vec![
            TerminalSessionStatus::Pending,
            TerminalSessionStatus::Attached,
            TerminalSessionStatus::Running,
        ]
    });
    let wave_id = query
        .wave_id
        .as_deref()
        .map(|value| parse_lfd_id(value, "invalid wave id"))
        .transpose()?;
    let mut sessions = state
        .store
        .list_terminal_sessions(wave_id.as_ref(), statuses.as_deref())
        .await
        .map_err(map_store_error)?;

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
            if wave.repo() == repo {
                filtered.push(session);
            }
        }
        sessions = filtered;
    }

    Ok(Json(ListResponse::new(
        sessions.into_iter().map(terminal_session_dto).collect(),
        false,
    )))
}

pub async fn get_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session = load_terminal_session(&state, &session_id).await?;
    Ok(Json(terminal_session_dto(session)))
}

pub async fn attach_terminal_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalConnectionInfoDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session = update_terminal_session(&state, &session_id, |session| {
        if session.source != TMUX_TERMINAL_SOURCE {
            return Err(api_error(
                StatusCode::PRECONDITION_FAILED,
                "terminal session is not tmux-backed",
            ));
        }
        Ok(session.attach())
    })
    .await?;

    Ok(Json(terminal_connection_info_dto(
        &session,
        connection_host(&headers),
    )))
}

pub async fn start_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session =
        update_terminal_session(&state, &session_id, |session| Ok(session.start())).await?;
    Ok(Json(terminal_session_dto(session)))
}

pub async fn complete_terminal_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(payload): Json<CompleteTerminalSessionRequest>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session = update_terminal_session(&state, &session_id, |session| {
        verify_completion_token(&headers, session)?;
        Ok(session.complete(payload.exit_code))
    })
    .await?;
    Ok(Json(terminal_session_dto(session)))
}

pub async fn cancel_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session =
        update_terminal_session(&state, &session_id, |session| Ok(session.cancel())).await?;
    if session.source == TMUX_TERMINAL_SOURCE {
        stop_tmux_terminal_session(&session).await;
    }
    Ok(Json(terminal_session_dto(session)))
}

async fn load_terminal_session(
    state: &HttpState,
    session_id: &LfdId,
) -> Result<TerminalSession, ApiError> {
    state
        .store
        .get_terminal_session(session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))
}

async fn store_terminal_session_update(
    state: &HttpState,
    session: &TerminalSession,
) -> Result<(), ApiError> {
    state
        .store
        .update_terminal_session(session)
        .await
        .map_err(map_store_error)?;
    state
        .event_hub
        .send(Event::terminal_session_updated(session.clone()));
    Ok(())
}

async fn update_terminal_session<F>(
    state: &HttpState,
    session_id: &LfdId,
    update: F,
) -> Result<TerminalSession, ApiError>
where
    F: FnOnce(&mut TerminalSession) -> Result<bool, ApiError>,
{
    let mut session = load_terminal_session(state, session_id).await?;
    let changed = update(&mut session)?;
    if changed {
        store_terminal_session_update(state, &session).await?;
    }
    Ok(session)
}

fn verify_completion_token(headers: &HeaderMap, session: &TerminalSession) -> Result<(), ApiError> {
    let expected = session.completion_token.as_deref().ok_or_else(|| {
        api_error(
            StatusCode::PRECONDITION_FAILED,
            "terminal session is not attachable",
        )
    })?;
    let provided = headers
        .get(COMPLETION_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "missing completion token"))?;
    if provided != expected {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid completion token",
        ));
    }
    Ok(())
}

fn connection_host(headers: &HeaderMap) -> String {
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
        .unwrap_or_else(|| "localhost".to_string());
    let parsed = host
        .parse::<Authority>()
        .ok()
        .map(|authority| authority.host().to_string())
        .unwrap_or(host);
    let normalized = parsed.trim_matches(['[', ']']);
    if matches!(normalized, "127.0.0.1" | "::1" | "localhost") {
        "localhost".to_string()
    } else {
        parsed
    }
}

async fn stop_tmux_terminal_session(session: &TerminalSession) {
    match Command::new("tmux")
        .args(["kill-session", "-t", &session.tmux_name])
        .status()
        .await
    {
        Ok(status) if status.success() => {}
        Ok(status) => warn!(
            session_id = %session.id,
            status = ?status.code(),
            "failed to kill tmux terminal session"
        ),
        Err(err) => warn!(
            session_id = %session.id,
            error = %err,
            "failed to kill tmux terminal session"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        tmux_session_name, TerminalSessionStatus, Wave, WaveMode, WaveStatus,
    };
    use axum::extract::{Path, State};
    use axum::http::{header::HOST, HeaderValue};
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "terminal-test".to_string(),
            repo: repo.to_string(),
            mode: WaveMode::Manual,
            primary_flow: "build".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        }
    }

    fn make_terminal_session(wave_id: LfdId, source: &str) -> TerminalSession {
        let id = LfdId::new();
        let tmux_name = tmux_session_name("test-branch");
        TerminalSession {
            id,
            wave_id,
            wave_run_id: None,
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: source.to_string(),
            tmux_name,
            status: TerminalSessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn attach_returns_tmux_connection_info() {
        let state = test_http_state().await;
        let wave = make_wave("/tmp/repo");
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");
        let session = make_terminal_session(wave.id().clone(), TMUX_TERMINAL_SOURCE);
        state
            .store
            .create_terminal_session(&session)
            .await
            .expect("terminal session should be created");

        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("127.0.0.1:2486"));

        let Json(response) = attach_terminal_session_handler(
            State(state.clone()),
            headers,
            Path(session.id.to_string()),
        )
        .await
        .expect("attach should succeed");

        assert_eq!(response.session_name, "lf-test-branch");
        assert_eq!(response.host, "localhost");
        assert_eq!(response.cwd, "/tmp/repo");
        assert_eq!(response.status, "attached");

        let stored = state
            .store
            .get_terminal_session(&session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should still exist");
        assert_eq!(stored.status, TerminalSessionStatus::Attached);
        assert!(stored.attached_at.is_some());
    }

    #[tokio::test]
    async fn attach_rejects_non_tmux_sessions() {
        let state = test_http_state().await;
        let wave = make_wave("/tmp/repo");
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");
        let session = make_terminal_session(wave.id().clone(), "wave_run");
        state
            .store
            .create_terminal_session(&session)
            .await
            .expect("terminal session should be created");

        let error = attach_terminal_session_handler(
            State(state),
            HeaderMap::new(),
            Path(session.id.to_string()),
        )
        .await
        .expect_err("attach should fail");

        assert_eq!(error.0, StatusCode::PRECONDITION_FAILED);
        assert_eq!(
            error.1 .0.error.message,
            "terminal session is not tmux-backed"
        );
    }
}
