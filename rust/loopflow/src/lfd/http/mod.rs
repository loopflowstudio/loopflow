pub mod dto;
pub mod routes;
pub mod state;

use std::path::Path;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::StatusCode;
use axum::middleware;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;

use crate::lfd::auth;
use crate::lfd::http::dto::{ErrorDetail, ErrorResponse};
use crate::lfd::http::routes::{
    chat, flows, hooks, repos, sessions, system, wave_runs, wave_schemas, waves, worktrees, ws,
};
use crate::lfd::store::StoreError;

pub use state::HttpState;

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

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
            "/wave/schemas",
            get(wave_schemas::list_wave_schemas_handler),
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
        .route(
            "/waves/{wave_id}/memory-blocks",
            get(chat::list_memory_blocks_handler),
        )
        .route(
            "/waves/{wave_id}/memory-blocks/{name}",
            put(chat::upsert_memory_block_handler).delete(chat::delete_memory_block_handler),
        )
        .route("/waves/{wave_id}/chat", post(chat::start_chat_handler))
        .route(
            "/waves/{wave_id}/chat/events",
            get(chat::stream_chat_events_handler),
        )
        .route(
            "/waves/{wave_id}/chat/messages",
            get(chat::list_chat_messages_handler),
        )
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
        .with_state(state)
}

pub fn api_error(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    let raw = message.into();
    let sanitized = sanitize_error_message(&raw);
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

fn sanitize_error_message(message: &str) -> String {
    let mut sanitized = redact_known_paths(message);
    sanitized = redact_bearer_token_segments(&sanitized);
    sanitized = redact_long_secret_segments(&sanitized);
    sanitized = redact_internal_identifiers(&sanitized);
    sanitized
}

fn redact_known_paths(message: &str) -> String {
    let mut sanitized = message.to_string();
    if let Some(home) = dirs::home_dir() {
        let home = home.to_string_lossy().to_string();
        if !home.is_empty() {
            sanitized = sanitized.replace(&home, "[REDACTED_PATH]");
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = cwd.to_string_lossy().to_string();
        if !cwd.is_empty() {
            sanitized = sanitized.replace(&cwd, "[REDACTED_PATH]");
        }
    }

    for token in extract_absolute_path_tokens(message) {
        sanitized = sanitized.replace(&token, "[REDACTED_PATH]");
    }
    sanitized
}

fn redact_bearer_token_segments(message: &str) -> String {
    let mut result = String::new();
    let mut remaining = message;
    while let Some(idx) = remaining.find("Bearer ") {
        result.push_str(&remaining[..idx]);
        let after = &remaining[idx + "Bearer ".len()..];
        let token = after.split_whitespace().next().unwrap_or_default();
        if token.is_empty() {
            result.push_str("Bearer");
            remaining = after;
        } else {
            result.push_str("Bearer [REDACTED_TOKEN]");
            remaining = &after[token.len()..];
        }
    }
    result.push_str(remaining);
    result
}

fn redact_long_secret_segments(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_word_if_secret)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_word_if_secret(word: &str) -> String {
    let trimmed = word.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ',');
    let looks_secret = trimmed.len() >= 24
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.');
    if looks_secret {
        word.replace(trimmed, "[REDACTED_TOKEN]")
    } else {
        word.to_string()
    }
}

fn redact_internal_identifiers(message: &str) -> String {
    let mut sanitized = message.to_string();
    for marker in [
        "docker://",
        "/var/lib/docker/volumes/",
        "volume ",
        "container ",
    ] {
        if sanitized.contains(marker) {
            sanitized = sanitized.replace(marker, "[REDACTED_INTERNAL] ");
        }
    }
    sanitized
}

fn extract_absolute_path_tokens(message: &str) -> Vec<String> {
    message
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ','))
        .filter(|word| is_absolute_path_token(word))
        .map(str::to_string)
        .collect()
}

fn is_absolute_path_token(word: &str) -> bool {
    !word.is_empty() && Path::new(word).is_absolute()
}

async fn normalize_payload_too_large(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return api_error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use axum::Router;
    use tokio::net::TcpListener;

    #[test]
    fn sanitize_error_message_redacts_paths_and_tokens() {
        let raw = "failed at /tmp/worktree with Bearer abcdef0123456789abcdef0123456789";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("/tmp/worktree"));
        assert!(!sanitized.contains("abcdef0123456789abcdef0123456789"));
        assert!(sanitized.contains("[REDACTED_PATH]"));
        assert!(sanitized.contains("[REDACTED_TOKEN]"));
    }

    #[test]
    fn sanitize_error_message_redacts_home_path() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let raw = format!("cannot open {}", home.display());
        let sanitized = sanitize_error_message(&raw);
        assert!(!sanitized.contains(&home.to_string_lossy().to_string()));
        assert!(sanitized.contains("[REDACTED_PATH]"));
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
}
