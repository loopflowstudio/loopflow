use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::secrets::{self, DopplerConfig, DopplerProject, SecretsProviderStatus};
use crate::lfd::store::SecretsProviderConfig;
use crate::lfd::types::Event;

pub async fn secrets_status_handler(
    State(state): State<HttpState>,
) -> ApiResult<SecretsProviderStatus> {
    let status = secrets::secrets_status(&state.store).await;
    Ok(Json(status))
}

pub async fn list_projects_handler(
    State(state): State<HttpState>,
) -> ApiResult<Vec<DopplerProject>> {
    let projects = secrets::list_projects(&state.store)
        .await
        .map_err(map_secrets_error)?;
    Ok(Json(projects))
}

#[derive(Debug, Deserialize)]
pub struct ConfigsQuery {
    pub project: String,
}

pub async fn list_configs_handler(
    State(state): State<HttpState>,
    Query(query): Query<ConfigsQuery>,
) -> ApiResult<Vec<DopplerConfig>> {
    let configs = secrets::list_configs(&state.store, &query.project)
        .await
        .map_err(map_secrets_error)?;
    Ok(Json(configs))
}

#[derive(Debug, Deserialize)]
pub struct SelectSecretsRequest {
    pub project: String,
    pub config: String,
}

pub async fn select_secrets_handler(
    State(state): State<HttpState>,
    Json(body): Json<SelectSecretsRequest>,
) -> ApiResult<SecretsProviderStatus> {
    let config = SecretsProviderConfig {
        provider: "doppler".to_string(),
        project: Some(body.project),
        config: Some(body.config),
        updated_at: crate::lfd::store::rows::now_unix(),
    };

    state
        .store
        .upsert_secrets_provider_config(&config)
        .await
        .map_err(map_store_error)?;

    match secrets::sync_secrets(&state.store, &config, Some(&state.event_hub)).await {
        Ok(_) => {
            state
                .event_hub
                .send(Event::secrets_connected("doppler".to_string()));
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

    secrets::sync_secrets(&state.store, &config, Some(&state.event_hub))
        .await
        .map_err(map_secrets_error)?;

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

fn map_secrets_error(err: secrets::SecretsError) -> ApiError {
    match err {
        secrets::SecretsError::Unauthorized => api_error(
            StatusCode::UNAUTHORIZED,
            "secrets provider token is invalid or expired",
        ),
        secrets::SecretsError::NotConnected => api_error(
            StatusCode::PRECONDITION_FAILED,
            "Doppler is not connected — authenticate first",
        ),
        secrets::SecretsError::Http(msg) | secrets::SecretsError::InvalidResponse(msg) => {
            api_error(
                StatusCode::BAD_GATEWAY,
                crate::lfd::http::ApiMessage::Untrusted(msg),
            )
        }
    }
}
