pub mod dto;
pub mod routes;
pub mod state;

use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::auth;
use crate::http::dto::{ErrorDetail, ErrorResponse};
use crate::http::routes::{hooks, system, wave_runs, waves, worktrees, ws};
use crate::store::{SharedStore, StoreError};

pub use state::HttpState;

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

pub fn router(state: HttpState) -> Router {
    // API routes — protected by auth middleware.
    let api_routes = Router::new()
        .route(
            "/waves",
            get(waves::list_waves_handler).post(waves::create_wave_handler),
        )
        .route(
            "/waves/:wave_id",
            get(waves::get_wave_handler)
                .patch(waves::update_wave_handler)
                .delete(waves::delete_wave_handler),
        )
        .route("/waves/:wave_id/run", post(waves::run_wave_handler))
        .route("/waves/:wave_id/stop", post(waves::stop_wave_handler))
        .route(
            "/waves/:wave_id/continue",
            post(waves::continue_wave_handler),
        )
        .route("/waves/:wave_id/land", post(waves::land_wave_handler))
        .route(
            "/waves/:wave_id/runs",
            get(wave_runs::list_wave_runs_for_wave_handler),
        )
        .route(
            "/waves/:wave_id/logs",
            get(wave_runs::wave_logs_handler),
        )
        .route("/wave_runs", get(wave_runs::list_wave_runs_handler))
        .route("/worktrees", get(worktrees::list_worktrees_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Status + WebSocket — also auth-protected.
    let protected_routes = Router::new()
        .route("/status", get(system::status_handler))
        .route("/ws", get(ws::ws_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    Router::new()
        // Unauthenticated: health probes, metrics, git hooks.
        .route("/health", get(system::health_handler))
        .route("/metrics", get(system::metrics_handler))
        .nest("/v1", api_routes)
        .merge(protected_routes)
        .route("/hooks/git", post(hooks::git_hook_handler))
        .with_state(state)
}

pub fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                error_type: "invalid_request_error".to_string(),
                message: message.into(),
                param: None,
            },
        }),
    )
}

pub fn api_error_with_param(
    status: StatusCode,
    message: impl Into<String>,
    param: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                error_type: "invalid_request_error".to_string(),
                message: message.into(),
                param: Some(param.into()),
            },
        }),
    )
}

pub fn map_store_error(err: StoreError) -> (StatusCode, Json<ErrorResponse>) {
    match err {
        StoreError::NotFound => api_error(StatusCode::NOT_FOUND, "not found"),
        StoreError::InvalidData(message) => api_error(StatusCode::BAD_REQUEST, message),
        StoreError::Serde(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::Sqlite(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::Postgres(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::PostgresPool(err) => {
            api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }
    }
}

pub async fn run_store<T, F>(store: &SharedStore, f: F) -> Result<T, StoreError>
where
    F: FnOnce(&dyn crate::store::RunStore) -> Result<T, StoreError> + Send + 'static,
    T: Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(store.as_ref()))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}
