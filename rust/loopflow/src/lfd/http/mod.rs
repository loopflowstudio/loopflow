pub mod dto;
pub mod routes;
pub mod state;

use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;

use crate::lfd::auth;
use crate::lfd::http::dto::{ErrorDetail, ErrorResponse};
use crate::lfd::http::routes::{
    flows, hooks, repos, system, wave_runs, wave_schemas, waves, worktrees, ws,
};
use crate::lfd::store::{SharedStore, StoreError};

pub use state::HttpState;

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

pub fn router(state: HttpState) -> Router {
    // API routes — protected by auth middleware.
    let api_routes = Router::new()
        .route("/flows", get(flows::list_flows_handler))
        .route("/repos", get(repos::list_repos_handler))
        .route(
            "/wave/schemas",
            get(wave_schemas::list_wave_schemas_handler),
        )
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
        .route(
            "/waves/:wave_id/stimulus",
            post(waves::add_stimulus_handler),
        )
        .route(
            "/waves/:wave_id/stimulus/:stimulus_id",
            delete(waves::remove_stimulus_handler),
        )
        .route("/waves/:wave_id/stimuli", get(waves::list_stimuli_handler))
        .route(
            "/waves/:wave_id/memory-blocks",
            get(waves::list_memory_blocks_handler),
        )
        .route(
            "/waves/:wave_id/memory-blocks/:name",
            put(waves::upsert_memory_block_handler).delete(waves::delete_memory_block_handler),
        )
        .route("/waves/:wave_id/stop", post(waves::stop_wave_handler))
        .route(
            "/waves/:wave_id/restart-step",
            post(waves::restart_step_handler),
        )
        .route(
            "/waves/:wave_id/continue",
            post(waves::continue_wave_handler),
        )
        .route("/waves/:wave_id/land", post(waves::land_wave_handler))
        .route("/waves/:wave_id/next", post(waves::next_wave_handler))
        .route(
            "/waves/:wave_id/check-ci",
            post(waves::check_wave_ci_handler),
        )
        .route("/waves/:wave_id/combine", post(waves::combine_wave_handler))
        .route(
            "/waves/:wave_id/runs",
            get(wave_runs::list_wave_runs_for_wave_handler),
        )
        .route("/waves/:wave_id/logs", get(wave_runs::wave_logs_handler))
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
        .nest("/v0", api_routes)
        .merge(protected_routes)
        .route("/hooks/git", post(hooks::git_hook_handler))
        .route("/v0/hooks/github", post(hooks::github_webhook_handler))
        .layer(TraceLayer::new_for_http())
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
    F: FnOnce(&dyn crate::lfd::store::RunStore) -> Result<T, StoreError> + Send + 'static,
    T: Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(store.as_ref()))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}
