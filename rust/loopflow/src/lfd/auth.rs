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

    if should_bypass_auth(is_loopback, request.method()) {
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

fn should_bypass_auth(is_loopback: bool, method: &Method) -> bool {
    is_loopback && !is_mutation(method)
}

fn authorize_local(
    session_token: Option<&str>,
    provided_token: Option<&str>,
    is_loopback: bool,
) -> Result<(), (StatusCode, &'static str)> {
    let Some(expected) = session_token else {
        return Err((
            StatusCode::FORBIDDEN,
            "remote access requires auth configuration",
        ));
    };

    match provided_token {
        Some(provided) if constant_time_eq(provided, expected) => Ok(()),
        Some(_) => Err((StatusCode::UNAUTHORIZED, "invalid token")),
        None if is_loopback => Err((StatusCode::FORBIDDEN, "mutations require session token")),
        None => Err((
            StatusCode::FORBIDDEN,
            "remote access requires auth configuration",
        )),
    }
}

fn is_mutation(method: &Method) -> bool {
    !matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
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

    #[test]
    fn loopback_get_bypasses_auth() {
        assert!(should_bypass_auth(true, &Method::GET));
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
        assert!(!should_bypass_auth(false, &Method::GET));

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
        assert!(!should_bypass_auth(true, &Method::POST));

        let result = authorize_local(Some("session-token"), Some("session-token"), false);
        assert_eq!(result, Ok(()));
    }
}
