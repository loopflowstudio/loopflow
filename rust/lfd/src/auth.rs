use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::net::SocketAddr;

use crate::http::dto::{ErrorDetail, ErrorResponse};
use crate::http::state::HttpState;
use crate::registration::ConnectionValidator;

/// Auth context held in HttpState.
///
/// When `active` is true (non-loopback bind), all non-exempt routes require
/// a valid connection token from loopflow.studio.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub active: bool,
    pub registered: bool,
    pub validator: Option<ConnectionValidator>,
}

impl AuthContext {
    pub fn new(registered: bool, validator: ConnectionValidator) -> Self {
        Self {
            active: true,
            registered,
            validator: Some(validator),
        }
    }

    pub fn inactive() -> Self {
        Self {
            active: false,
            registered: false,
            validator: None,
        }
    }
}

/// Axum middleware that enforces connection-token auth on non-loopback requests.
pub async fn auth_middleware(
    State(state): State<HttpState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Loopback connections bypass auth entirely.
    if let Some(ConnectInfo(addr)) = connect_info {
        if addr.ip().is_loopback() {
            return next.run(request).await;
        }
    }

    // Auth not active (localhost bind) — pass through.
    if !state.auth.active {
        return next.run(request).await;
    }

    // Auth active but registration failed — reject.
    if !state.auth.registered {
        return auth_error(
            StatusCode::FORBIDDEN,
            "remote access requires successful loopflow.studio registration",
        );
    }

    // Extract token from headers.
    let token = match extract_token(&headers) {
        Some(t) => t,
        None => {
            return auth_error(StatusCode::UNAUTHORIZED, "missing connection token");
        }
    };

    // Validate token.
    let Some(validator) = &state.auth.validator else {
        return next.run(request).await;
    };

    if validator.validate(&token).await {
        next.run(request).await
    } else {
        auth_error(StatusCode::UNAUTHORIZED, "invalid connection token")
    }
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
