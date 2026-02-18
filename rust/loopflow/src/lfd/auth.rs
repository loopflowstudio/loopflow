use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;

use crate::lfd::http::dto::{ErrorDetail, ErrorResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::registration::ConnectionValidator;

/// Auth provider for lfd connections.
///
/// Selected from config (`auth.provider`). Determines how requests are
/// authenticated.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthProvider {
    /// Local mode: loopback only, no remote access.
    Local,
    /// Validate against a pre-shared static token.
    Static { token: String },
    /// Validate via loopflow.studio registration.
    Studio { validator: ConnectionValidator },
}

/// Axum middleware that enforces auth.
///
/// Loopback connections bypass auth entirely. Remote connections
/// require a valid token (static or studio) — Local mode rejects them.
pub async fn auth_middleware(
    State(state): State<HttpState>,
    request: Request,
    next: Next,
) -> Response {
    // Loopback connections bypass auth entirely, regardless of provider.
    if let Some(ConnectInfo(addr)) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        if addr.ip().is_loopback() {
            return next.run(request).await;
        }
    }

    let headers = request.headers();
    match &state.auth {
        AuthProvider::Local => auth_error(
            StatusCode::FORBIDDEN,
            "remote access requires auth configuration",
        ),
        AuthProvider::Static { token } => match extract_token(headers) {
            Some(provided) if constant_time_eq(provided, token) => next.run(request).await,
            Some(_) => auth_error(StatusCode::UNAUTHORIZED, "invalid token"),
            None => auth_error(StatusCode::UNAUTHORIZED, "missing token"),
        },
        AuthProvider::Studio { validator } => {
            let token = match extract_token(headers) {
                Some(t) => t,
                None => {
                    return auth_error(StatusCode::UNAUTHORIZED, "missing connection token");
                }
            };

            if validator.validate(token).await {
                next.run(request).await
            } else {
                auth_error(StatusCode::UNAUTHORIZED, "invalid connection token")
            }
        }
    }
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

fn extract_token(headers: &HeaderMap) -> Option<&str> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let token = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))?;
    Some(token.trim())
}

fn auth_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                error_type: "invalid_request_error".to_string(),
                message: message.to_string(),
                param: None,
            },
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "def"));
        assert!(!constant_time_eq("abc", "ab"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn extract_token_from_bearer_header() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer test-token-123".parse().unwrap());
        assert_eq!(extract_token(&headers), Some("test-token-123"));
    }

    #[test]
    fn extract_token_from_bearer_lowercase() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "bearer test-token".parse().unwrap());
        assert_eq!(extract_token(&headers), Some("test-token"));
    }

    #[test]
    fn extract_token_missing() {
        let headers = HeaderMap::new();
        assert_eq!(extract_token(&headers), None);
    }
}
