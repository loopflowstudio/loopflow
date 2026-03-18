use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::secrets::{self, DopplerSecretsProvider, SecretsProviderStatus};
use crate::lfd::store::SecretsProviderConfig;
use crate::lfd::types::Event;

pub async fn secrets_status_handler(
    State(state): State<HttpState>,
) -> ApiResult<SecretsProviderStatus> {
    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

#[derive(Debug, Deserialize)]
pub struct ConnectSecretsRequest {
    pub provider: String,
    pub token: String,
    pub project: String,
    pub config: String,
}

pub async fn connect_secrets_handler(
    State(state): State<HttpState>,
    Json(body): Json<ConnectSecretsRequest>,
) -> ApiResult<SecretsProviderStatus> {
    if body.provider != "doppler" {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "only 'doppler' is supported as a secrets provider",
        ));
    }

    let config = SecretsProviderConfig {
        provider: body.provider.clone(),
        access_token: body.token,
        project: Some(body.project),
        config: Some(body.config),
        updated_at: crate::lfd::store::rows::now_unix(),
    };

    state
        .store
        .upsert_secrets_provider_config(&config)
        .await
        .map_err(map_store_error)?;

    let provider = DopplerSecretsProvider;
    match secrets::sync_secrets(&state.store, &provider, &config, Some(&state.event_hub)).await {
        Ok(_) => {
            state
                .event_hub
                .send(Event::secrets_connected(body.provider));
        }
        Err(err) => {
            return Err(map_secrets_error(err));
        }
    }

    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

pub async fn sync_secrets_handler(
    State(state): State<HttpState>,
) -> ApiResult<SecretsProviderStatus> {
    let configs = state
        .store
        .list_secrets_provider_configs()
        .await
        .map_err(map_store_error)?;

    let Some(config) = configs.into_iter().next() else {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "no secrets provider configured",
        ));
    };

    let provider = resolve_provider(&config.provider)?;
    secrets::sync_secrets(
        &state.store,
        provider.as_ref(),
        &config,
        Some(&state.event_hub),
    )
    .await
    .map_err(map_secrets_error)?;

    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSecretsConfigRequest {
    pub project: Option<String>,
    pub config: Option<String>,
}

pub async fn update_secrets_config_handler(
    State(state): State<HttpState>,
    Json(body): Json<UpdateSecretsConfigRequest>,
) -> ApiResult<SecretsProviderStatus> {
    let configs = state
        .store
        .list_secrets_provider_configs()
        .await
        .map_err(map_store_error)?;

    let Some(mut config) = configs.into_iter().next() else {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "no secrets provider configured",
        ));
    };

    if let Some(project) = body.project {
        config.project = Some(project);
    }
    if let Some(config_name) = body.config {
        config.config = Some(config_name);
    }
    config.updated_at = crate::lfd::store::rows::now_unix();

    state
        .store
        .upsert_secrets_provider_config(&config)
        .await
        .map_err(map_store_error)?;

    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

pub async fn disconnect_secrets_handler(
    State(state): State<HttpState>,
) -> ApiResult<SecretsProviderStatus> {
    let configs = state
        .store
        .list_secrets_provider_configs()
        .await
        .map_err(map_store_error)?;

    for config in &configs {
        state
            .store
            .delete_secrets_provider_config(&config.provider)
            .await
            .map_err(map_store_error)?;
    }

    secrets::clear_secrets_credentials(&state.store, Some(&state.event_hub)).await;
    state.event_hub.send(Event::secrets_disconnected());

    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

fn resolve_provider(name: &str) -> Result<Box<dyn secrets::SecretsProvider>, ApiError> {
    match name {
        "doppler" => Ok(Box::new(DopplerSecretsProvider)),
        _ => Err(api_error(
            StatusCode::BAD_REQUEST,
            "unsupported secrets provider",
        )),
    }
}

fn map_secrets_error(err: secrets::SecretsError) -> ApiError {
    match err {
        secrets::SecretsError::Unauthorized => api_error(
            StatusCode::UNAUTHORIZED,
            "secrets provider token is invalid or expired",
        ),
        secrets::SecretsError::Http(msg) => api_error(
            StatusCode::BAD_GATEWAY,
            crate::lfd::http::ApiMessage::Untrusted(msg),
        ),
        secrets::SecretsError::InvalidResponse(msg) => api_error(
            StatusCode::BAD_GATEWAY,
            crate::lfd::http::ApiMessage::Untrusted(msg),
        ),
    }
}
