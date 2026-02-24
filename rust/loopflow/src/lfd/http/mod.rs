pub mod dto;
pub mod routes;
pub mod state;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{StatusCode, Uri};
use axum::middleware;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;

use crate::lfd::auth;
use crate::lfd::http::dto::{ErrorDetail, ErrorResponse};
use crate::lfd::http::routes::{
    flows, hooks, repos, sessions, system, wave_runs, waves, worktrees, ws,
};
use crate::lfd::redaction::sanitize_operator_message;
use crate::lfd::store::StoreError;

pub use state::HttpState;

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;
const QUERY_TOKEN_ERROR: &str = "authentication credentials must not appear in query parameters";

pub fn router(state: HttpState) -> Router {
    let max_json_body_bytes = state.http_security.max_json_body_bytes;
    let max_hook_body_bytes = state.http_security.max_hook_body_bytes;

    // API routes — protected by auth middleware.
    let api_routes = Router::new()
        .route("/flows", get(flows::list_flows_handler))
        .route("/repos", get(repos::list_repos_handler))
        .route("/sessions", post(sessions::create_session_handler))
        .route(
            "/sessions/{id}",
            get(sessions::get_session_handler).delete(sessions::delete_session_handler),
        )
        .route(
            "/sessions/{id}/input",
            post(sessions::send_session_input_handler),
        )
        .route(
            "/sessions/{id}/events",
            get(sessions::stream_session_events_handler),
        )
        .route(
            "/waves",
            get(waves::list_waves_handler).post(waves::create_wave_handler),
        )
        .route(
            "/waves/{wave_id}",
            get(waves::get_wave_handler)
                .patch(waves::update_wave_handler)
                .delete(waves::delete_wave_handler),
        )
        .route("/waves/{wave_id}/run", post(waves::run_wave_handler))
        .route(
            "/waves/{wave_id}/stimulus",
            post(waves::add_stimulus_handler),
        )
        .route(
            "/waves/{wave_id}/stimulus/{stimulus_id}",
            delete(waves::remove_stimulus_handler),
        )
        .route("/waves/{wave_id}/stimuli", get(waves::list_stimuli_handler))
        .route("/waves/{wave_id}/stop", post(waves::stop_wave_handler))
        .route(
            "/waves/{wave_id}/restart-step",
            post(waves::restart_step_handler),
        )
        .route(
            "/waves/{wave_id}/continue",
            post(waves::continue_wave_handler),
        )
        .route("/waves/{wave_id}/land", post(waves::land_wave_handler))
        .route("/waves/{wave_id}/next", post(waves::next_wave_handler))
        .route(
            "/waves/{wave_id}/check-ci",
            post(waves::check_wave_ci_handler),
        )
        .route(
            "/waves/{wave_id}/combine",
            post(waves::combine_wave_handler),
        )
        .route(
            "/waves/{wave_id}/runs",
            get(wave_runs::list_wave_runs_for_wave_handler),
        )
        .route("/waves/{wave_id}/logs", get(wave_runs::wave_logs_handler))
        .route("/wave_runs", get(wave_runs::list_wave_runs_handler))
        .route("/worktrees", get(worktrees::list_worktrees_handler))
        .layer(DefaultBodyLimit::max(max_json_body_bytes))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Status + WebSocket — also auth-protected.
    let protected_routes = Router::new()
        .route("/status", get(system::status_handler))
        .route("/ws", get(ws::ws_handler))
        .layer(DefaultBodyLimit::max(max_json_body_bytes))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    let hook_routes = Router::new()
        .route("/hooks/git", post(hooks::git_hook_handler))
        .route("/v0/hooks/github", post(hooks::github_webhook_handler))
        .layer(DefaultBodyLimit::max(max_hook_body_bytes));

    Router::new()
        // Unauthenticated: health probes, metrics, git hooks.
        .route("/health", get(system::health_handler))
        .route("/metrics", get(system::metrics_handler))
        .nest("/v0", api_routes)
        .merge(protected_routes)
        .merge(hook_routes)
        .layer(middleware::from_fn(normalize_payload_too_large))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(reject_auth_query_params))
        .with_state(state)
}

pub fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    let raw = message.into();
    let sanitized = sanitize_operator_message(&raw);
    if status.is_server_error() {
        tracing::warn!(status = %status, error = %raw, "internal API error");
    }
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                error_type: "invalid_request_error".to_string(),
                message: sanitized,
                param: None,
            },
        }),
    )
}

pub fn api_error_response(status: StatusCode, message: impl Into<String>) -> Response {
    api_error(status, message).into_response()
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

async fn normalize_payload_too_large(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return api_error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
    }
    response
}

async fn reject_auth_query_params(request: Request, next: Next) -> Response {
    if has_auth_like_query_param(request.uri()) {
        return api_error_response(StatusCode::BAD_REQUEST, QUERY_TOKEN_ERROR);
    }
    next.run(request).await
}

fn has_auth_like_query_param(uri: &Uri) -> bool {
    let Some(query) = uri.query() else {
        return false;
    };
    query
        .split('&')
        .filter_map(|part| part.split_once('=').map(|(key, _)| key).or(Some(part)))
        .map(str::trim)
        .any(is_auth_like_query_key)
}

fn is_auth_like_query_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "token"
            | "access_token"
            | "auth_token"
            | "api_key"
            | "bearer"
            | "secret"
            | "password"
            | "credential"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use axum::routing::get;
    use axum::routing::post;
    use axum::Router;
    use tokio::net::TcpListener;

    async fn add_trace_header(request: Request, next: Next) -> Response {
        let mut response = next.run(request).await;
        response
            .headers_mut()
            .insert("x-trace-hit", HeaderValue::from_static("1"));
        response
    }

    #[tokio::test]
    async fn oversized_payload_returns_json_413() {
        let app = Router::new()
            .route("/limited", post(|_body: bytes::Bytes| async { "ok" }))
            .layer(DefaultBodyLimit::max(8))
            .layer(middleware::from_fn(normalize_payload_too_large));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/limited"))
            .body("0123456789abcdef")
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let payload: serde_json::Value = response.json().await.expect("json response");
        assert_eq!(
            payload["error"]["message"],
            serde_json::Value::String("request body too large".to_string())
        );
    }

    #[tokio::test]
    async fn auth_like_query_keys_are_rejected_case_insensitively() {
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(middleware::from_fn(reject_auth_query_params));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });

        let response = reqwest::Client::new()
            .get(format!("http://{addr}/health?ToKeN=abc"))
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload: serde_json::Value = response.json().await.expect("json response");
        assert_eq!(
            payload["error"]["message"],
            serde_json::Value::String(QUERY_TOKEN_ERROR.to_string())
        );
    }

    #[tokio::test]
    async fn auth_like_query_rejection_happens_before_trace_layer() {
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(middleware::from_fn(add_trace_header))
            .layer(middleware::from_fn(reject_auth_query_params));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });

        let response = reqwest::Client::new()
            .get(format!("http://{addr}/health?token=abc"))
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.headers().get("x-trace-hit").is_none());
    }

    #[test]
    fn auth_like_query_key_list_is_case_insensitive() {
        for key in [
            "token",
            "access_token",
            "auth_token",
            "api_key",
            "bearer",
            "secret",
            "password",
            "credential",
            "ToKeN",
            "API_KEY",
        ] {
            assert!(
                is_auth_like_query_key(key),
                "expected key {key} to be blocked"
            );
        }
        assert!(!is_auth_like_query_key("page"));
    }
}
