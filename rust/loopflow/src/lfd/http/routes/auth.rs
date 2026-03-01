use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::info;

use crate::lfd::http::dto::format_datetime;
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::provider_auth::{AuthError, AuthFlowResponse, Provider, ProviderAuthSnapshot};
use crate::lfd::store::CredentialType;

#[derive(Debug, Serialize)]
pub struct AuthProvidersResponse {
    pub providers: Vec<AuthProviderStatusDto>,
}

#[derive(Debug, Serialize)]
pub struct AuthProviderStatusDto {
    pub provider: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_refresh_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthFlowResponseDto {
    pub provider: String,
    pub verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
}

pub async fn list_auth_handler(State(state): State<HttpState>) -> ApiResult<AuthProvidersResponse> {
    let statuses = state
        .provider_auth
        .list_statuses()
        .await
        .map_err(map_auth_error)?;

    Ok(Json(AuthProvidersResponse {
        providers: statuses.into_iter().map(status_dto).collect(),
    }))
}

pub async fn get_auth_handler(
    State(state): State<HttpState>,
    Path(provider): Path<String>,
) -> ApiResult<AuthProviderStatusDto> {
    let provider = parse_provider(&provider)?;
    let snapshot = state
        .provider_auth
        .status(provider)
        .await
        .map_err(map_auth_error)?;
    Ok(Json(status_dto(snapshot)))
}

pub async fn start_auth_handler(
    State(state): State<HttpState>,
    Path(provider): Path<String>,
) -> ApiResult<AuthFlowResponseDto> {
    let provider = parse_provider(&provider)?;
    let response = state
        .provider_auth
        .start_auth(provider, state.event_hub.clone())
        .await
        .map_err(map_auth_error)?;

    Ok(Json(flow_dto(response)))
}

pub async fn disconnect_auth_handler(
    State(state): State<HttpState>,
    Path(provider): Path<String>,
) -> ApiResult<AuthProviderStatusDto> {
    let provider = parse_provider(&provider)?;
    state
        .provider_auth
        .disconnect(provider, state.event_hub.clone())
        .await
        .map_err(map_auth_error)?;

    let snapshot = state
        .provider_auth
        .status(provider)
        .await
        .map_err(map_auth_error)?;

    Ok(Json(status_dto(snapshot)))
}

#[derive(Debug, Deserialize)]
pub struct ConfigureCredentialRequest {
    pub api_key: String,
}

/// Store an API key for a provider. To switch back to OAuth, run the normal
/// OAuth connect flow (`lfq auth <provider>`) which overwrites the token and
/// resets credential_type to OAuth.
pub async fn configure_credential_handler(
    State(state): State<HttpState>,
    Path(provider): Path<String>,
    Json(body): Json<ConfigureCredentialRequest>,
) -> ApiResult<AuthProviderStatusDto> {
    let provider = parse_provider(&provider)?;

    let token = crate::lfd::store::ProviderToken {
        provider: provider.as_str().to_string(),
        access_token: body.api_key,
        refresh_token: None,
        expires_at: None,
        login: None,
        updated_at: crate::lfd::store::rows::now_unix(),
        credential_type: CredentialType::ApiKey,
    };
    state
        .store
        .upsert_provider_token(&token)
        .await
        .map_err(map_store_error)?;
    info!(
        provider = %provider,
        "switched to API key (pay-per-token billing)"
    );

    let snapshot = state
        .provider_auth
        .status(provider)
        .await
        .map_err(map_auth_error)?;
    Ok(Json(status_dto(snapshot)))
}

fn parse_provider(raw: &str) -> Result<Provider, ApiError> {
    raw.parse::<Provider>()
        .map_err(|_| api_error(StatusCode::NOT_FOUND, "provider not found"))
}

pub(super) fn map_auth_error(err: AuthError) -> ApiError {
    match err {
        AuthError::UnsupportedProvider(_) => {
            api_error(StatusCode::NOT_FOUND, ApiMessage::Safe(err.to_string()))
        }
        AuthError::FlowAlreadyPending(_) => {
            api_error(StatusCode::CONFLICT, ApiMessage::Safe(err.to_string()))
        }
        AuthError::CommandUnavailable { .. } => {
            api_error(StatusCode::BAD_REQUEST, ApiMessage::Safe(err.to_string()))
        }
        AuthError::CommandSpawn { .. }
        | AuthError::CommandFailed { .. }
        | AuthError::CommandIo { .. }
        | AuthError::MissingVerificationUrl { .. }
        | AuthError::Filesystem(_)
        | AuthError::CredentialSocket { .. } => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        ),
    }
}

fn status_dto(snapshot: ProviderAuthSnapshot) -> AuthProviderStatusDto {
    AuthProviderStatusDto {
        provider: snapshot.provider.as_str().to_string(),
        status: snapshot.status.as_str().to_string(),
        login: snapshot.status.login(),
        expires_at: format_unix_timestamp(snapshot.expires_at),
        next_refresh_at: format_unix_timestamp(snapshot.next_refresh_at),
        credential_type: snapshot.credential_type.map(|ct| ct.as_str().to_string()),
    }
}

fn format_unix_timestamp(timestamp: Option<i64>) -> Option<String> {
    let timestamp = timestamp?;
    let datetime = OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    format_datetime(Some(datetime))
}

fn flow_dto(response: AuthFlowResponse) -> AuthFlowResponseDto {
    AuthFlowResponseDto {
        provider: response.provider.as_str().to_string(),
        verification_uri: response.verification_uri,
        verification_uri_complete: response.verification_uri_complete,
        user_code: response.user_code,
        expires_in: response.expires_in,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_provider_supports_aliases() {
        assert_eq!(parse_provider("github").expect("github"), Provider::GitHub);
        assert_eq!(parse_provider("gh").expect("gh"), Provider::GitHub);
        assert_eq!(parse_provider("claude").expect("claude"), Provider::Claude);
        assert_eq!(parse_provider("codex").expect("codex"), Provider::Codex);
        assert_eq!(
            parse_provider("opencodezen").expect("opencodezen"),
            Provider::OpenCodeZen
        );
        assert_eq!(parse_provider("zen").expect("zen"), Provider::OpenCodeZen);
    }

    #[test]
    fn parse_provider_rejects_unknown_values() {
        let (status, body) = parse_provider("gemini").expect_err("unknown provider");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error.message, "provider not found");
    }

    #[test]
    fn map_auth_error_returns_expected_status_codes() {
        let (pending_status, _) = map_auth_error(AuthError::FlowAlreadyPending(Provider::Claude));
        assert_eq!(pending_status, StatusCode::CONFLICT);

        let (missing_cli_status, _) = map_auth_error(AuthError::CommandUnavailable {
            provider: Provider::Codex,
            command: "codex".to_string(),
        });
        assert_eq!(missing_cli_status, StatusCode::BAD_REQUEST);

        let (command_failed_status, _) = map_auth_error(AuthError::CommandFailed {
            provider: Provider::GitHub,
            message: "boom".to_string(),
        });
        assert_eq!(command_failed_status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn status_dto_preserves_provider_and_login() {
        let dto = status_dto(ProviderAuthSnapshot {
            provider: Provider::GitHub,
            status: crate::lfd::provider_auth::AuthStatus::Active {
                login: Some("jackdanger".to_string()),
            },
            expires_at: Some(1_893_456_000),
            next_refresh_at: Some(1_893_454_800),
            credential_type: Some(crate::lfd::store::CredentialType::OAuth),
        });

        assert_eq!(dto.provider, "github");
        assert_eq!(dto.status, "active");
        assert_eq!(dto.login, Some("jackdanger".to_string()));
        assert_eq!(dto.expires_at, Some("2030-01-01T00:00:00Z".to_string()));
        assert_eq!(
            dto.next_refresh_at,
            Some("2029-12-31T23:40:00Z".to_string())
        );
    }
}
