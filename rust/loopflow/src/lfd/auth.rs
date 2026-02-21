use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
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

/// Axum middleware that enforces auth on all requests.
pub async fn auth_middleware(
    State(state): State<HttpState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let provided_token = extract_token(&headers);

    match &state.auth {
        AuthProvider::Local => {
            match authorize_local(state.session_token.as_deref(), provided_token) {
                Ok(()) => next.run(request).await,
                Err((status, message)) => auth_error(status, message),
            }
        }
        AuthProvider::Static { token } => match provided_token {
            Some(provided) if constant_time_eq(provided, token) => next.run(request).await,
            Some(_) => auth_error(StatusCode::UNAUTHORIZED, "invalid token"),
            None => auth_error(StatusCode::UNAUTHORIZED, "missing token"),
        },
        AuthProvider::Studio { validator } => {
            let token = match provided_token {
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

fn authorize_local(
    session_token: Option<&str>,
    provided_token: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    let Some(expected) = session_token else {
        return Err((StatusCode::FORBIDDEN, "session token not configured"));
    };

    match provided_token {
        Some(provided) if constant_time_eq(provided, expected) => Ok(()),
        Some(_) => Err((StatusCode::UNAUTHORIZED, "invalid token")),
        None => Err((StatusCode::UNAUTHORIZED, "missing token")),
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

    #[test]
    fn valid_token_is_allowed() {
        let result = authorize_local(Some("session-token"), Some("session-token"));
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn invalid_token_is_rejected() {
        let result = authorize_local(Some("session-token"), Some("wrong"));
        assert_eq!(result, Err((StatusCode::UNAUTHORIZED, "invalid token")));
    }

    #[test]
    fn missing_token_is_rejected() {
        let result = authorize_local(Some("session-token"), None);
        assert_eq!(result, Err((StatusCode::UNAUTHORIZED, "missing token")));
    }

    #[test]
    fn unconfigured_session_token_is_forbidden() {
        let result = authorize_local(None, Some("any-token"));
        assert_eq!(
            result,
            Err((StatusCode::FORBIDDEN, "session token not configured"))
        );
    }
}
