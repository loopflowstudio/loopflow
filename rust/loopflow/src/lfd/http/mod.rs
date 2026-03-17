pub mod dto;
pub mod routes;
pub mod state;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{StatusCode, Uri};
use axum::middleware;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use tower_http::trace::TraceLayer;

use crate::lfd::auth;
use crate::lfd::http::dto::{ErrorDetail, ErrorResponse};
use crate::lfd::http::routes::{
    attention, auth as auth_routes, flows, hooks, providers, repos, sessions, system, tokens,
    usage, wave_runs, waves, worktrees, ws,
};
use crate::lfd::redaction::sanitize_operator_message;
use crate::lfd::store::StoreError;

pub use state::HttpState;

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;
const QUERY_TOKEN_ERROR: &str = "authentication credentials must not appear in query parameters";
const AUTH_LIKE_QUERY_KEYS: [&str; 8] = [
    "token",
    "access_token",
    "auth_token",
    "api_key",
    "bearer",
    "secret",
    "password",
    "credential",
];

pub fn router(state: HttpState) -> Router {
    let max_json_body_bytes = state.http_security.max_json_body_bytes;
    let max_hook_body_bytes = state.http_security.max_hook_body_bytes;

    // API routes — protected by auth middleware.
    let api_routes = Router::new()
        .route("/auth", get(auth_routes::list_auth_handler))
        .route(
            "/auth/{provider}",
            get(auth_routes::get_auth_handler)
                .post(auth_routes::start_auth_handler)
                .delete(auth_routes::disconnect_auth_handler),
        )
        .route(
            "/auth/{provider}/credential",
            put(auth_routes::configure_credential_handler),
        )
        .route("/providers", get(providers::list_providers_handler))
        .route("/flows", get(flows::list_flows_handler))
        .route(
            "/repos",
            get(repos::list_repos_handler)
                .post(repos::add_repo_handler)
                .delete(repos::remove_repo_handler),
        )
        .route(
            "/repos/{owner}/{repo}/children/{child_owner}/{child_repo}",
            post(repos::add_child_handler).delete(repos::remove_child_handler),
        )
        .route(
            "/repos/{owner}/{repo}/children",
            get(repos::list_children_handler),
        )
        .route(
            "/repos/{owner}/{repo}/parents",
            get(repos::list_parents_handler),
        )
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
            "/sessions/{id}/usage",
            get(usage::get_session_usage_handler),
        )
        .route("/attention", get(attention::list_attention_handler))
        .route(
            "/attention/history",
            get(attention::list_attention_history_handler),
        )
        .route(
            "/attention/{attention_id}",
            get(attention::get_attention_handler).patch(attention::patch_attention_handler),
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
        .route(
            "/waves/{wave_id}/diff",
            get(waves::get_wave_file_diff_handler),
        )
        .route("/waves/{wave_id}/run", post(waves::run_wave_handler))
        .route(
            "/waves/{wave_id}/triggers",
            post(waves::add_trigger_handler),
        )
        .route(
            "/waves/{wave_id}/triggers/{trigger_id}",
            delete(waves::remove_trigger_handler),
        )
        .route(
            "/waves/{wave_id}/triggers",
            get(waves::list_triggers_handler),
        )
        .route(
            "/waves/{wave_id}/activations",
            get(waves::list_activations_handler),
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
        .route("/waves/{wave_id}/usage", get(usage::get_wave_usage_handler))
        .route("/waves/{wave_id}/logs", get(wave_runs::wave_logs_handler))
        .route("/usage/summary", get(usage::get_usage_summary_handler))
        .route(
            "/usage/timeseries",
            get(usage::get_usage_timeseries_handler),
        )
        .route("/tokens/revoke", post(tokens::revoke_tokens_handler))
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

/// Distinguishes safe (known) messages from untrusted external content.
///
/// Static string literals auto-convert to `Safe` via `From<&'static str>`.
/// Dynamic strings from external systems (DB errors, path operations) should
/// use `Untrusted` to trigger sanitization.
#[derive(Debug)]
pub enum ApiMessage {
    /// Known-safe content: static strings, domain identifiers. Passed through as-is.
    Safe(String),
    /// External content that may contain secrets or paths. Sanitized before exposure.
    Untrusted(String),
}

impl From<&'static str> for ApiMessage {
    fn from(s: &'static str) -> Self {
        ApiMessage::Safe(s.to_string())
    }
}

pub fn api_error(
    status: StatusCode,
    message: impl Into<ApiMessage>,
) -> (StatusCode, Json<ErrorResponse>) {
    let message = message.into();
    if status.is_server_error() {
        let raw = match &message {
            ApiMessage::Safe(raw) | ApiMessage::Untrusted(raw) => raw,
        };
        tracing::warn!(status = %status, error = %raw, "internal API error");
    }
    let display = match message {
        ApiMessage::Safe(s) => s,
        ApiMessage::Untrusted(s) => sanitize_operator_message(&s),
    };
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                error_type: "invalid_request_error".to_string(),
                message: display,
                param: None,
            },
        }),
    )
}

pub fn api_error_response(status: StatusCode, message: impl Into<ApiMessage>) -> Response {
    api_error(status, message).into_response()
}

pub fn map_store_error(err: StoreError) -> (StatusCode, Json<ErrorResponse>) {
    match err {
        StoreError::NotFound => api_error(StatusCode::NOT_FOUND, "not found"),
        StoreError::InvalidData(message) => {
            api_error(StatusCode::BAD_REQUEST, ApiMessage::Safe(message))
        }
        StoreError::Serde(err) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        ),
        StoreError::Sqlite(err) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        ),
        StoreError::Postgres(err) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        ),
        StoreError::PostgresPool(err) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        ),
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
    query.split('&').map(query_key).map(str::trim).any(|key| {
        is_auth_like_query_key(key)
            || decode_query_key(key).is_some_and(|decoded| is_auth_like_query_key(&decoded))
    })
}

fn query_key(part: &str) -> &str {
    part.split_once('=').map_or(part, |(key, _)| key)
}

fn is_auth_like_query_key(key: &str) -> bool {
    AUTH_LIKE_QUERY_KEYS
        .iter()
        .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

fn decode_query_key(raw_key: &str) -> Option<String> {
    if !raw_key.contains('%') && !raw_key.contains('+') {
        return None;
    }

    let mut out = Vec::with_capacity(raw_key.len());
    let bytes = raw_key.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        match bytes[idx] {
            b'+' => {
                out.push(b' ');
                idx += 1;
            }
            b'%' => {
                if idx + 2 >= bytes.len() {
                    return None;
                }
                let hi = from_hex_digit(bytes[idx + 1])?;
                let lo = from_hex_digit(bytes[idx + 2])?;
                out.push((hi << 4) | lo);
                idx += 3;
            }
            byte => {
                out.push(byte);
                idx += 1;
            }
        }
    }

    String::from_utf8(out).ok()
}

fn from_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
    async fn auth_like_query_keys_are_rejected_when_percent_encoded() {
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
            .get(format!("http://{addr}/health?%74oken=abc"))
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    #[test]
    fn decode_query_key_decodes_percent_and_plus() {
        assert_eq!(decode_query_key("%74oken"), Some("token".to_string()));
        assert_eq!(
            decode_query_key("access%5Ftoken"),
            Some("access_token".to_string())
        );
        assert_eq!(decode_query_key("api+key"), Some("api key".to_string()));
    }

    #[test]
    fn decode_query_key_rejects_invalid_escape_sequences() {
        assert_eq!(decode_query_key("%"), None);
        assert_eq!(decode_query_key("%2"), None);
        assert_eq!(decode_query_key("%zz"), None);
    }

    #[test]
    fn api_message_safe_passes_through_unchanged() {
        let (_, json) = api_error(StatusCode::NOT_FOUND, ApiMessage::Safe("not found".into()));
        assert_eq!(json.error.message, "not found");
    }

    #[test]
    fn api_message_untrusted_sanitizes_paths() {
        let raw = "error at /tmp/private/data.db".to_string();
        let (_, json) = api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(raw),
        );
        assert!(!json.error.message.contains("/tmp/private/data.db"));
        assert!(json.error.message.contains("[REDACTED_PATH]"));
    }

    #[test]
    fn api_message_from_static_str_is_safe() {
        let msg: ApiMessage = "not found".into();
        let (_, json) = api_error(StatusCode::NOT_FOUND, msg);
        assert_eq!(json.error.message, "not found");
    }

    #[test]
    fn map_store_error_sanitizes_db_errors() {
        let err = StoreError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("no such table: /tmp/secret/db.sqlite".to_string()),
        ));
        let (status, json) = map_store_error(err);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!json.error.message.contains("/tmp/secret/db.sqlite"));
    }

    #[test]
    fn map_store_error_passes_domain_errors_through() {
        let (status, json) = map_store_error(StoreError::NotFound);
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json.error.message, "not found");

        let (status, json) =
            map_store_error(StoreError::InvalidData("invalid wave name".to_string()));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json.error.message, "invalid wave name");
    }
}
