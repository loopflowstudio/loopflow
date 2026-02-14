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
/// Selected from config (`auth.provider`). Determines how non-loopback
/// requests are authenticated.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthProvider {
    /// Loopback connections only. Non-loopback requests get 403.
    Local,
    /// Validate against a pre-shared static token.
    Static { token: String },
    /// Validate via loopflow.studio registration.
    Studio { validator: ConnectionValidator },
}

/// Axum middleware that enforces auth on non-loopback requests.
pub async fn auth_middleware(
    State(state): State<HttpState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Loopback connections bypass auth entirely, regardless of provider.
    if let Some(ConnectInfo(addr)) = connect_info {
        if addr.ip().is_loopback() {
            return next.run(request).await;
        }
    }

    match &state.auth {
        AuthProvider::Local => auth_error(
            StatusCode::FORBIDDEN,
            "remote access requires auth configuration",
        ),
        AuthProvider::Static { token } => match extract_token(&headers) {
            Some(provided) if constant_time_eq(&provided, token) => next.run(request).await,
            Some(_) => auth_error(StatusCode::UNAUTHORIZED, "invalid token"),
            None => auth_error(StatusCode::UNAUTHORIZED, "missing token"),
        },
        AuthProvider::Studio { validator } => {
            let token = match extract_token(&headers) {
                Some(t) => t,
                None => {
                    return auth_error(StatusCode::UNAUTHORIZED, "missing connection token");
                }
            };

            if validator.validate(&token).await {
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

fn extract_token(headers: &HeaderMap) -> Option<String> {
    // x-loopflow-connection-token header (preferred).
    if let Some(value) = headers.get("x-loopflow-connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }

    // connection-token header (legacy).
    if let Some(value) = headers.get("connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }

    // Authorization: Bearer <token>.
    if let Some(value) = headers.get("authorization") {
        if let Ok(auth) = value.to_str() {
            if let Some(token) = auth
                .strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
            {
                return Some(token.trim().to_string());
            }
        }
    }

    None
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
        assert_eq!(extract_token(&headers), Some("test-token-123".to_string()));
    }

    #[test]
    fn extract_token_from_bearer_lowercase() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "bearer test-token".parse().unwrap());
        assert_eq!(extract_token(&headers), Some("test-token".to_string()));
    }

    #[test]
    fn extract_token_from_connection_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-loopflow-connection-token", "conn-token".parse().unwrap());
        assert_eq!(extract_token(&headers), Some("conn-token".to_string()));
    }

    #[test]
    fn extract_token_missing() {
        let headers = HeaderMap::new();
        assert_eq!(extract_token(&headers), None);
    }

    #[test]
    fn extract_token_prefers_connection_header_over_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-loopflow-connection-token", "conn-token".parse().unwrap());
        headers.insert("authorization", "Bearer bearer-token".parse().unwrap());
        assert_eq!(extract_token(&headers), Some("conn-token".to_string()));
    }
}
