use axum::extract::{Path, Query, State};
use axum::http::header::HOST;
use axum::http::uri::Authority;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use std::path::Path as FsPath;
use tokio::process::Command;
use tracing::warn;

use crate::lfd::http::dto::{
    session_connection_info_dto, session_dto, CreateSessionRequestDto, CreateSessionResponseDto,
    ListResponse, SessionConnectionInfoDto, SessionDto,
};
use crate::lfd::http::routes::{parse_lfd_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Event, Session, SessionUse, LIVE_SESSION_STATUSES};

const COMPLETION_TOKEN_HEADER: &str = "x-terminal-completion-token";

// TODO(M1): move tmux-backed session control behind the session registry owner.
// Preserve host normalization, attach metadata, completion-token close, and
// best-effort tmux kill behavior.
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

#[derive(Debug, Deserialize)]
pub struct CompleteSessionRequest {
    pub exit_code: i32,
}

pub async fn create_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSessionRequestDto>,
) -> ApiResult<CreateSessionResponseDto> {
    let wave_id = parse_lfd_id(&payload.wave_id, "invalid wave id")?;
    let session = state
        .executor
        .launch_palette_session(&wave_id, &payload.flow, &payload.worktree, &payload.agent)
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;
    let connection = session_connection_info_dto(&session, connection_host(&headers));
    Ok(Json(CreateSessionResponseDto {
        session: session_dto(session),
        connection,
    }))
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
            if wave.repos.iter().any(|rw| rw.repo == repo) {
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

pub async fn attach_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> ApiResult<SessionConnectionInfoDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = update_session(&state, &session_id, |session| {
        if !session.is_tmux_backed() {
            return Err(api_error(
                StatusCode::PRECONDITION_FAILED,
                "session is not tmux-backed",
            ));
        }
        Ok(session.attach())
    })
    .await?;

    Ok(Json(session_connection_info_dto(
        &session,
        connection_host(&headers),
    )))
}

pub async fn start_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = update_session(&state, &session_id, |session| Ok(session.start())).await?;
    Ok(Json(session_dto(session)))
}

pub async fn complete_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(payload): Json<CompleteSessionRequest>,
) -> ApiResult<SessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = update_session(&state, &session_id, |session| {
        verify_completion_token(&headers, session)?;
        Ok(session.complete(payload.exit_code))
    })
    .await?;
    Ok(Json(session_dto(session)))
}

pub async fn cancel_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = update_session(&state, &session_id, |session| Ok(session.cancel())).await?;
    if session.is_tmux_backed() {
        stop_tmux_session(&session).await;
    }
    Ok(Json(session_dto(session)))
}

async fn load_session(state: &HttpState, session_id: &LfdId) -> Result<Session, ApiError> {
    state
        .store
        .get_control_session(session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "session not found"))
}

async fn store_session_update(state: &HttpState, session: &Session) -> Result<(), ApiError> {
    state
        .store
        .update_control_session(session)
        .await
        .map_err(map_store_error)?;
    state
        .event_hub
        .send(Event::session_updated(session.clone()));
    Ok(())
}

async fn update_session<F>(
    state: &HttpState,
    session_id: &LfdId,
    update: F,
) -> Result<Session, ApiError>
where
    F: FnOnce(&mut Session) -> Result<bool, ApiError>,
{
    let mut session = load_session(state, session_id).await?;
    let changed = update(&mut session)?;
    if changed {
        store_session_update(state, &session).await?;
    }
    Ok(session)
}

fn verify_completion_token(headers: &HeaderMap, session: &Session) -> Result<(), ApiError> {
    let expected = session
        .completion_token
        .as_deref()
        .ok_or_else(|| api_error(StatusCode::PRECONDITION_FAILED, "session is not attachable"))?;
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

pub(crate) fn connection_host(headers: &HeaderMap) -> String {
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

async fn stop_tmux_session(session: &Session) {
    match Command::new("tmux")
        .args(["kill-session", "-t", &session.tmux_name])
        .status()
        .await
    {
        Ok(status) if status.success() => {}
        Ok(status) => warn!(
            session_id = %session.id,
            status = ?status.code(),
            "failed to kill tmux session"
        ),
        Err(err) => warn!(
            session_id = %session.id,
            error = %err,
            "failed to kill tmux session"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        tmux_session_name, RepoWork, SessionStatus, SessionUse, Wave, WaveMode, WaveStatus,
        PALETTE_TERMINAL_SOURCE, TMUX_TERMINAL_SOURCE,
    };
    use axum::extract::{Path, Query, State};
    use axum::http::{header::HOST, HeaderValue};
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "terminal-test".to_string(),
            mode: WaveMode::Manual,
            primary_flow: "build".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_session(wave_id: LfdId, source: &str) -> Session {
        Session {
            id: LfdId::new(),
            wave_id,
            run_id: None,
            parent_session_id: None,
            session_use: if source == PALETTE_TERMINAL_SOURCE {
                SessionUse::Palette
            } else {
                SessionUse::WaveAgent
            },
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: source.to_string(),
            tmux_name: if source == TMUX_TERMINAL_SOURCE {
                tmux_session_name("test-branch")
            } else {
                String::new()
            },
            status: SessionStatus::Pending,
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
        let session = make_session(wave.id().clone(), TMUX_TERMINAL_SOURCE);
        state
            .store
            .create_control_session(&session)
            .await
            .expect("session should be created");

        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("127.0.0.1:2486"));

        let Json(response) =
            attach_session_handler(State(state.clone()), headers, Path(session.id.to_string()))
                .await
                .expect("attach should succeed");

        assert_eq!(response.session_name, "lf-test-branch");
        assert_eq!(response.host, "localhost");
        assert_eq!(response.cwd, "/tmp/repo");
        assert_eq!(response.status, "attached");

        let stored = state
            .store
            .get_control_session(&session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should still exist");
        assert_eq!(stored.status, SessionStatus::Attached);
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
        let session = make_session(wave.id().clone(), "wave_run");
        state
            .store
            .create_control_session(&session)
            .await
            .expect("session should be created");

        let error =
            attach_session_handler(State(state), HeaderMap::new(), Path(session.id.to_string()))
                .await
                .expect_err("attach should fail");

        assert_eq!(error.0, StatusCode::PRECONDITION_FAILED);
        assert_eq!(error.1 .0.error.message, "session is not tmux-backed");
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
        let mut session = make_session(wave.id().clone(), TMUX_TERMINAL_SOURCE);
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
            let mut session = make_session(wave.id().clone(), TMUX_TERMINAL_SOURCE);
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

    #[test]
    fn connection_host_normalizes_loopback_variants() {
        for raw in ["127.0.0.1:2486", "[::1]:2486", "localhost:2486"] {
            let mut headers = HeaderMap::new();
            headers.insert(
                HOST,
                HeaderValue::from_str(raw).expect("host header should be valid"),
            );
            assert_eq!(connection_host(&headers), "localhost");
        }
    }

    #[test]
    fn connection_host_preserves_remote_hostname_without_port() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("lfd.example.com:2486"));

        assert_eq!(connection_host(&headers), "lfd.example.com");
    }
}
