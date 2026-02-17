use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
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
    /// Local auth with a generated session token.
    Local,
    /// Validate against a pre-shared static token.
    Static { token: String },
    /// Validate via loopflow.studio registration.
    Studio { validator: ConnectionValidator },
}

/// Axum middleware that enforces auth based on route tier:
/// - Loopback reads: allowed without token
/// - Mutations: token required
/// - Remote reads in local mode: token required
pub async fn auth_middleware(
    State(state): State<HttpState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let is_loopback = connect_info
        .map(|ConnectInfo(addr)| addr.ip().is_loopback())
        .unwrap_or(false);

    if should_bypass_auth(is_loopback, &request) {
        return next.run(request).await;
    }

    match &state.auth {
        AuthProvider::Local => {
            match authorize_local(
                state.session_token.as_deref(),
                extract_token(&headers),
                is_loopback,
            ) {
                Ok(()) => next.run(request).await,
                Err((status, message)) => auth_error(status, message),
            }
        }
        AuthProvider::Static { token } => match extract_token(&headers) {
            Some(provided) if constant_time_eq(provided, token) => next.run(request).await,
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

            if validator.validate(token).await {
                next.run(request).await
            } else {
                auth_error(StatusCode::UNAUTHORIZED, "invalid connection token")
            }
        }
    }
}

fn should_bypass_auth(is_loopback: bool, request: &Request) -> bool {
    is_loopback && !is_mutation(request)
}

fn authorize_local(
    session_token: Option<&str>,
    provided_token: Option<&str>,
    is_loopback: bool,
) -> Result<(), (StatusCode, &'static str)> {
    match (session_token, provided_token) {
        (Some(expected), Some(provided)) if constant_time_eq(provided, expected) => Ok(()),
        (Some(_), Some(_)) => Err((StatusCode::UNAUTHORIZED, "invalid token")),
        (Some(_), None) if is_loopback => {
            Err((StatusCode::FORBIDDEN, "mutations require session token"))
        }
        _ => Err((
            StatusCode::FORBIDDEN,
            "remote access requires auth configuration",
        )),
    }
}

fn is_mutation(request: &Request) -> bool {
    !matches!(
        request.method(),
        &Method::GET | &Method::HEAD | &Method::OPTIONS
    )
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
    use axum::body::Body;

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

    #[test]
    fn loopback_get_bypasses_auth() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/v0/waves")
            .body(Body::empty())
            .unwrap();

        assert!(should_bypass_auth(true, &request));
    }

    #[test]
    fn loopback_post_without_token_is_forbidden() {
        let result = authorize_local(Some("session-token"), None, true);
        assert_eq!(
            result,
            Err((StatusCode::FORBIDDEN, "mutations require session token"))
        );
    }

    #[test]
    fn loopback_post_with_valid_token_is_allowed() {
        let result = authorize_local(Some("session-token"), Some("session-token"), true);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn remote_get_without_token_is_forbidden() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/v0/waves")
            .body(Body::empty())
            .unwrap();

        assert!(!should_bypass_auth(false, &request));

        let result = authorize_local(Some("session-token"), None, false);
        assert_eq!(
            result,
            Err((
                StatusCode::FORBIDDEN,
                "remote access requires auth configuration",
            ))
        );
    }

    #[test]
    fn remote_get_with_valid_token_is_allowed() {
        let result = authorize_local(Some("session-token"), Some("session-token"), false);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn remote_post_with_valid_token_is_allowed() {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v0/waves")
            .body(Body::empty())
            .unwrap();

        assert!(!should_bypass_auth(true, &request));

        let result = authorize_local(Some("session-token"), Some("session-token"), false);
        assert_eq!(result, Ok(()));
    }
}
