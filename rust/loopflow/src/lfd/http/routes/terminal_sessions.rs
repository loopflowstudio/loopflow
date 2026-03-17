use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use secrecy::ExposeSecret;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::lfd::auth::AuthProvider;
use crate::lfd::http::dto::{
    terminal_session_dto, ListResponse, TerminalLaunchSpecDto, TerminalSessionDto,
};
use crate::lfd::http::routes::{parse_lfd_id, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::types::{Event, TerminalSession, TerminalSessionStatus};

const COMPLETION_TOKEN_HEADER: &str = "x-terminal-completion-token";

#[derive(Debug, Deserialize)]
pub struct ListTerminalSessionsQuery {
    pub repo: Option<String>,
    pub wave_id: Option<String>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTerminalSessionRequest {
    pub wave_id: String,
    pub wave_run_id: Option<String>,
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

pub async fn create_terminal_session_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateTerminalSessionRequest>,
) -> ApiResult<TerminalSessionDto> {
    let wave_id = resolve_wave_id(&state, &payload.wave_id).await?;
    let wave_run_id = payload
        .wave_run_id
        .as_deref()
        .map(|value| parse_lfd_id(value, "invalid wave run id"))
        .transpose()?;
    let session = state
        .executor
        .create_terminal_session_for_waiting_wave(&wave_id, wave_run_id.as_ref())
        .await
        .map_err(|error| {
            api_error(
                StatusCode::PRECONDITION_FAILED,
                ApiMessage::Untrusted(error),
            )
        })?;
    Ok(Json(terminal_session_dto(session)))
}

pub async fn get_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let session = state
        .store
        .get_terminal_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))?;
    Ok(Json(terminal_session_dto(session)))
}

pub async fn attach_terminal_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalLaunchSpecDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let mut session = state
        .store
        .get_terminal_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))?;

    if session.status == TerminalSessionStatus::Pending {
        session.status = TerminalSessionStatus::Attached;
        session.attached_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_terminal_session(&session)
            .await
            .map_err(map_store_error)?;
        state
            .event_hub
            .send(Event::terminal_session_updated(session.clone()));
    }

    let completion_token = session.completion_token.clone().ok_or_else(|| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "missing completion token",
        )
    })?;
    let complete_url = completion_url(&headers, &session_id);
    let auth_header = local_auth_header(&state.auth).ok_or_else(|| {
        api_error(
            StatusCode::PRECONDITION_FAILED,
            "terminal embedding unavailable",
        )
    })?;
    let launch_argv = vec![
        "/bin/zsh".to_string(),
        "-lc".to_string(),
        build_wrapped_command(&session, &complete_url, &auth_header, &completion_token),
    ];

    Ok(Json(TerminalLaunchSpecDto {
        session_id: session.id.to_string(),
        wave_id: session.wave_id.to_string(),
        step: session.step,
        agent: session.agent,
        cwd: session.cwd,
        argv: launch_argv,
        env: session.env,
        completion_token,
    }))
}

pub async fn start_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let mut session = state
        .store
        .get_terminal_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))?;
    if !session.status.is_terminal() {
        if session.attached_at.is_none() {
            session.attached_at = Some(OffsetDateTime::now_utc());
        }
        session.status = TerminalSessionStatus::Running;
        session.started_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_terminal_session(&session)
            .await
            .map_err(map_store_error)?;
        state
            .event_hub
            .send(Event::terminal_session_updated(session.clone()));
    }
    Ok(Json(terminal_session_dto(session)))
}

pub async fn complete_terminal_session_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(payload): Json<CompleteTerminalSessionRequest>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let mut session = state
        .store
        .get_terminal_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))?;

    verify_completion_token(&headers, &session)?;

    if !session.status.is_terminal() {
        session.status = if payload.exit_code == 0 {
            TerminalSessionStatus::Succeeded
        } else {
            TerminalSessionStatus::Failed
        };
        session.completed_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_terminal_session(&session)
            .await
            .map_err(map_store_error)?;
        state
            .event_hub
            .send(Event::terminal_session_updated(session.clone()));
    }
    Ok(Json(terminal_session_dto(session)))
}

pub async fn cancel_terminal_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<TerminalSessionDto> {
    let session_id = parse_lfd_id(&session_id, "invalid terminal session id")?;
    let mut session = state
        .store
        .get_terminal_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "terminal session not found"))?;
    if !session.status.is_terminal() {
        session.status = TerminalSessionStatus::Canceled;
        session.completed_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_terminal_session(&session)
            .await
            .map_err(map_store_error)?;
        state
            .event_hub
            .send(Event::terminal_session_updated(session.clone()));
    }
    Ok(Json(terminal_session_dto(session)))
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

fn completion_url(headers: &HeaderMap, session_id: &crate::lfd::id::LfdId) -> String {
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("127.0.0.1");
    format!("http://{host}/v0/terminal-sessions/{session_id}/complete")
}

fn local_auth_header(auth: &AuthProvider) -> Option<String> {
    match auth {
        AuthProvider::Local { session_token } => Some(format!(
            "Authorization: Bearer {}",
            session_token.expose_secret()
        )),
        AuthProvider::Static { token } => {
            Some(format!("Authorization: Bearer {}", token.expose_secret()))
        }
        AuthProvider::Studio { local_token, .. } => Some(format!(
            "Authorization: Bearer {}",
            local_token.expose_secret()
        )),
    }
}

fn build_wrapped_command(
    session: &TerminalSession,
    complete_url: &str,
    auth_header: &str,
    completion_token: &str,
) -> String {
    let env_prefix = session
        .env
        .iter()
        .map(|(key, value)| format!("{key}={} ", shell_escape(value)))
        .collect::<String>();
    let agent_command = session
        .argv
        .iter()
        .map(|value| shell_escape(value))
        .collect::<Vec<_>>()
        .join(" ");
    let exit_payload = format!("{{\\\"exit_code\\\":%s}}");
    format!(
        "{env_prefix}{agent_command}; EXIT_CODE=$?; /usr/bin/curl -fsS -X POST -H {} -H 'Content-Type: application/json' -H {} --data \"$(printf '{}' \\\"$EXIT_CODE\\\")\" {} >/dev/null 2>&1 || true; exit \"$EXIT_CODE\"",
        shell_escape(auth_header),
        shell_escape(&format!("{COMPLETION_TOKEN_HEADER}: {completion_token}")),
        exit_payload,
        shell_escape(complete_url),
    )
}

fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}
