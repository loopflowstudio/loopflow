use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::lfd::http;
use crate::lfd::http::state::HttpState;
use crate::lfd::registration::ConnectionValidator;

const THROTTLE_WINDOW: Duration = Duration::from_secs(60);

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

#[derive(Clone, Debug, Default)]
pub struct AuthFailureThrottle {
    buckets: Arc<Mutex<HashMap<AuthThrottleKey, AuthThrottleBucket>>>,
}

impl AuthFailureThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_throttled(&self, key: &AuthThrottleKey, limit: u32) -> bool {
        let Ok(mut buckets) = self.buckets.lock() else {
            return false;
        };
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.window_started) < THROTTLE_WINDOW);
        buckets
            .get(key)
            .is_some_and(|bucket| bucket.failures_in_window >= limit)
    }

    pub fn record_failure(&self, key: AuthThrottleKey, limit: u32) -> bool {
        let Ok(mut buckets) = self.buckets.lock() else {
            return false;
        };
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.window_started) < THROTTLE_WINDOW);
        let bucket = buckets.entry(key).or_insert(AuthThrottleBucket {
            window_started: now,
            failures_in_window: 0,
        });
        if now.duration_since(bucket.window_started) >= THROTTLE_WINDOW {
            bucket.window_started = now;
            bucket.failures_in_window = 0;
        }
        bucket.failures_in_window = bucket.failures_in_window.saturating_add(1);
        bucket.failures_in_window > limit
    }
}

#[derive(Debug)]
struct AuthThrottleBucket {
    window_started: Instant,
    failures_in_window: u32,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AuthThrottleKey {
    source: String,
    auth_context_hash: String,
    endpoint_group: &'static str,
}

impl AuthThrottleKey {
    pub fn new(source: String, auth_context_hash: String, endpoint_group: &'static str) -> Self {
        Self {
            source,
            auth_context_hash,
            endpoint_group,
        }
    }
}

/// Axum middleware that enforces auth on all requests.
pub async fn auth_middleware(
    State(state): State<HttpState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let provided_token = extract_token(&headers);
    let throttle_limit = state.api_security.http.auth_failures_per_minute;
    let source = resolved_source(
        &request,
        &headers,
        &state.api_security.http.trusted_proxy_cidrs,
    );
    let endpoint_group = endpoint_group(request.method(), request.uri().path());
    let throttle_key = AuthThrottleKey::new(
        source.clone(),
        auth_context_hash(provided_token),
        endpoint_group,
    );

    if state
        .auth_failure_throttle
        .is_throttled(&throttle_key, throttle_limit)
    {
        return throttled_response();
    }

    let auth_result = match &state.auth {
        AuthProvider::Local => authorize_local(state.session_token.as_deref(), provided_token),
        AuthProvider::Static { token } => match provided_token {
            Some(provided) if constant_time_eq(provided, token) => Ok(()),
            Some(_) => Err((StatusCode::UNAUTHORIZED, "invalid token")),
            None => Err((StatusCode::UNAUTHORIZED, "missing token")),
        },
        AuthProvider::Studio { validator } => {
            let token = match provided_token {
                Some(token) => token,
                None => return auth_error(StatusCode::UNAUTHORIZED, "missing connection token"),
            };
            if validator.validate(token).await {
                Ok(())
            } else {
                Err((StatusCode::UNAUTHORIZED, "invalid connection token"))
            }
        }
    };

    match auth_result {
        Ok(()) => next.run(request).await,
        Err((status, message)) => {
            if state
                .auth_failure_throttle
                .record_failure(throttle_key, throttle_limit)
            {
                tracing::warn!(
                    source = source,
                    endpoint_group,
                    "auth failures exceeded limit; throttling"
                );
                throttled_response()
            } else {
                auth_error(status, message)
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

fn auth_context_hash(token: Option<&str>) -> String {
    let Some(token) = token else {
        return "missing".to_string();
    };
    if token.is_empty() {
        return "empty".to_string();
    }
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)[..16].to_string()
}

fn endpoint_group(method: &Method, path: &str) -> &'static str {
    if path == "/ws" {
        return "ws";
    }
    if method == Method::GET || method == Method::HEAD {
        "read"
    } else {
        "mutate"
    }
}

fn resolved_source(
    request: &Request,
    headers: &HeaderMap,
    trusted_proxy_cidrs: &[IpNet],
) -> String {
    let peer_ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));

    resolve_client_source(peer_ip, headers, trusted_proxy_cidrs).to_string()
}

fn resolve_client_source(
    peer_ip: IpAddr,
    headers: &HeaderMap,
    trusted_proxy_cidrs: &[IpNet],
) -> IpAddr {
    if !trusted_proxy_cidrs
        .iter()
        .any(|cidr| cidr.contains(&peer_ip))
    {
        return peer_ip;
    }

    let forwarded_for = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(forwarded_for) = forwarded_for else {
        return peer_ip;
    };
    let first_hop = forwarded_for
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(first_hop) = first_hop else {
        return peer_ip;
    };

    first_hop.parse::<IpAddr>().unwrap_or(peer_ip)
}

fn throttled_response() -> Response {
    http::api_error_response(
        StatusCode::TOO_MANY_REQUESTS,
        "too many authentication failures",
    )
}

fn auth_error(status: StatusCode, message: &str) -> Response {
    http::api_error_response(status, message)
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

    #[test]
    fn trusted_forwarded_for_is_used_for_source() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.9, 10.0.0.1".parse().unwrap());
        let source = resolve_client_source(
            IpAddr::from([127, 0, 0, 1]),
            &headers,
            &["127.0.0.1/32".parse::<IpNet>().expect("cidr")],
        );
        assert_eq!(source, IpAddr::from([203, 0, 113, 9]));
    }

    #[test]
    fn untrusted_peer_ignores_forwarded_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.9".parse().unwrap());
        let source = resolve_client_source(
            IpAddr::from([10, 0, 0, 5]),
            &headers,
            &["127.0.0.1/32".parse::<IpNet>().expect("cidr")],
        );
        assert_eq!(source, IpAddr::from([10, 0, 0, 5]));
    }

    #[test]
    fn malformed_forwarded_header_falls_back_to_peer_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "not-an-ip".parse().unwrap());
        let source = resolve_client_source(
            IpAddr::from([127, 0, 0, 1]),
            &headers,
            &["127.0.0.1/32".parse::<IpNet>().expect("cidr")],
        );
        assert_eq!(source, IpAddr::from([127, 0, 0, 1]));
    }

    #[test]
    fn auth_failure_throttle_limits_after_threshold() {
        let throttle = AuthFailureThrottle::new();
        let key = AuthThrottleKey::new("127.0.0.1".to_string(), "hash".to_string(), "mutate");

        assert!(!throttle.record_failure(key.clone(), 2));
        assert!(!throttle.record_failure(key.clone(), 2));
        assert!(throttle.record_failure(key.clone(), 2));
        assert!(throttle.is_throttled(&key, 2));
    }
}
