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

use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

use crate::lfd::http;
use crate::lfd::http::state::HttpState;
use crate::lfd::token_ledger::TokenLedger;

const THROTTLE_WINDOW: Duration = Duration::from_secs(60);
const MAX_BEARER_TOKEN_BYTES: usize = 4096;

/// Auth provider for lfd connections.
///
/// Selected from config (`auth.mode`). Determines how requests are
/// authenticated.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AuthProvider {
    /// Local mode: loopback only, session token required.
    Local { session_token: SecretString },
    /// CI mode: validate against a pre-shared token.
    Ci { token: SecretString },
    /// Registered with studio. Local token on loopback, connection tokens
    /// validated locally via ledger for remote clients.
    Studio {
        local_token: SecretString,
        ledger: TokenLedger,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParsedToken<'a> {
    Missing,
    Malformed,
    Present(&'a str),
}

impl AuthProvider {
    pub async fn validate(
        &self,
        provided_token: Option<&str>,
        source: IpAddr,
    ) -> Result<(), (StatusCode, &'static str)> {
        match self {
            AuthProvider::Local { session_token } => {
                authorize_expected_token(session_token, provided_token)
            }
            AuthProvider::Ci { token } => authorize_expected_token(token, provided_token),
            AuthProvider::Studio {
                local_token,
                ledger,
            } => match provided_token {
                Some(token) if source.is_loopback() && token_matches(local_token, token) => Ok(()),
                Some(token) => match ledger.validate(token).await {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        if source.is_loopback() {
                            Err((StatusCode::UNAUTHORIZED, "invalid token"))
                        } else {
                            Err((StatusCode::UNAUTHORIZED, "invalid connection token"))
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "connection token validation failed");
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "connection token validation failed",
                        ))
                    }
                },
                None => {
                    if source.is_loopback() {
                        Err((StatusCode::UNAUTHORIZED, "missing token"))
                    } else {
                        Err((StatusCode::UNAUTHORIZED, "missing connection token"))
                    }
                }
            },
        }
    }

    pub fn local_admin_authorized(&self, provided_token: Option<&str>, source: IpAddr) -> bool {
        if !source.is_loopback() {
            return false;
        }
        let Some(token) = provided_token else {
            return false;
        };

        match self {
            AuthProvider::Ci { token: local_token } => token_matches(local_token, token),
            AuthProvider::Studio { local_token, .. } => token_matches(local_token, token),
            _ => false,
        }
    }

    pub fn connection_ledger(&self) -> Option<TokenLedger> {
        match self {
            AuthProvider::Studio { ledger, .. } => Some(ledger.clone()),
            _ => None,
        }
    }
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
    source: IpAddr,
    auth_context_hash: String,
    endpoint_group: &'static str,
}

impl AuthThrottleKey {
    pub fn new(source: IpAddr, auth_context_hash: String, endpoint_group: &'static str) -> Self {
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
    let throttle_limit = state.http_security.auth_failures_per_minute;
    let source = resolved_source(&request, &headers, &state.http_security.trusted_proxy_cidrs);
    let endpoint_group = endpoint_group(request.method(), request.uri().path());
    let throttle_key =
        AuthThrottleKey::new(source, auth_context_hash(provided_token), endpoint_group);

    if state
        .auth_failure_throttle
        .is_throttled(&throttle_key, throttle_limit)
    {
        return throttled_response();
    }

    let auth_result = match provided_token {
        ParsedToken::Malformed => Err(malformed_token_error(&state.auth, source)),
        ParsedToken::Missing => state.auth.validate(None, source).await,
        ParsedToken::Present(token) => state.auth.validate(Some(token), source).await,
    };

    match auth_result {
        Ok(()) => next.run(request).await,
        Err((status, message)) => {
            if state
                .auth_failure_throttle
                .record_failure(throttle_key, throttle_limit)
            {
                tracing::warn!(
                    source = %source,
                    endpoint_group,
                    "auth failures exceeded limit; throttling"
                );
                throttled_response()
            } else {
                http::api_error_response(status, message)
            }
        }
    }
}

fn authorize_expected_token(
    expected_token: &SecretString,
    provided_token: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    match provided_token {
        Some(provided) if token_matches(expected_token, provided) => Ok(()),
        Some(_) => Err((StatusCode::UNAUTHORIZED, "invalid token")),
        None => Err((StatusCode::UNAUTHORIZED, "missing token")),
    }
}

fn token_matches(expected: &SecretString, provided: &str) -> bool {
    expected
        .expose_secret()
        .as_bytes()
        .ct_eq(provided.as_bytes())
        .into()
}

fn malformed_token_error(auth: &AuthProvider, source: IpAddr) -> (StatusCode, &'static str) {
    match auth {
        AuthProvider::Local { .. } | AuthProvider::Ci { .. } => {
            (StatusCode::UNAUTHORIZED, "malformed token")
        }
        AuthProvider::Studio { .. } => {
            if source.is_loopback() {
                (StatusCode::UNAUTHORIZED, "malformed token")
            } else {
                (StatusCode::UNAUTHORIZED, "malformed connection token")
            }
        }
    }
}

pub(crate) fn extract_token(headers: &HeaderMap) -> ParsedToken<'_> {
    let Some(auth_value) = headers.get("authorization") else {
        return ParsedToken::Missing;
    };
    let Ok(auth) = auth_value.to_str() else {
        return ParsedToken::Malformed;
    };
    let Some((scheme, value)) = auth.split_once(' ') else {
        return ParsedToken::Malformed;
    };
    if !scheme.eq_ignore_ascii_case("bearer") {
        return ParsedToken::Malformed;
    }

    let token = value.trim();
    if token.is_empty() || token.len() > MAX_BEARER_TOKEN_BYTES {
        return ParsedToken::Malformed;
    }

    if token
        .chars()
        .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return ParsedToken::Malformed;
    }

    ParsedToken::Present(token)
}

pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    match extract_token(headers) {
        ParsedToken::Present(token) => Some(token.to_string()),
        ParsedToken::Missing | ParsedToken::Malformed => None,
    }
}

fn auth_context_hash(token: ParsedToken<'_>) -> String {
    match token {
        ParsedToken::Missing => "missing".to_string(),
        ParsedToken::Malformed => "malformed".to_string(),
        ParsedToken::Present(value) => {
            let digest = Sha256::digest(value.as_bytes());
            hex::encode(digest)[..16].to_string()
        }
    }
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
) -> IpAddr {
    let peer_ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));

    resolve_client_source(peer_ip, headers, trusted_proxy_cidrs)
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lfd::token_ledger::TokenLedger;
    use axum::http::HeaderValue;
    use tempfile::tempdir;

    fn token(value: &str) -> SecretString {
        SecretString::new(value.to_string())
    }

    #[test]
    fn extract_token_classifies_authorization_headers() {
        struct Case {
            name: &'static str,
            authorization: Option<&'static str>,
            expected: ParsedToken<'static>,
        }

        let cases = [
            Case {
                name: "missing header",
                authorization: None,
                expected: ParsedToken::Missing,
            },
            Case {
                name: "bearer token",
                authorization: Some("Bearer test-token-123"),
                expected: ParsedToken::Present("test-token-123"),
            },
            Case {
                name: "bearer lowercase",
                authorization: Some("bearer test-token"),
                expected: ParsedToken::Present("test-token"),
            },
            Case {
                name: "bearer uppercase",
                authorization: Some("BEARER upper-token"),
                expected: ParsedToken::Present("upper-token"),
            },
            Case {
                name: "trimmed token",
                authorization: Some("Bearer   trimmed-token   "),
                expected: ParsedToken::Present("trimmed-token"),
            },
            Case {
                name: "empty bearer token",
                authorization: Some("Bearer "),
                expected: ParsedToken::Malformed,
            },
            Case {
                name: "wrong auth scheme",
                authorization: Some("Basic dXNlcjpwYXNz"),
                expected: ParsedToken::Malformed,
            },
            Case {
                name: "embedded spaces in token",
                authorization: Some("Bearer token with-space"),
                expected: ParsedToken::Malformed,
            },
            Case {
                name: "embedded tab in token",
                authorization: Some("Bearer token\tinjected"),
                expected: ParsedToken::Malformed,
            },
        ];

        for case in cases {
            let mut headers = HeaderMap::new();
            if let Some(authorization) = case.authorization {
                headers.insert(
                    "authorization",
                    authorization.parse().expect("header value"),
                );
            }
            assert_eq!(
                extract_token(&headers),
                case.expected,
                "case {:?} failed",
                case.name
            );
        }
    }

    #[test]
    fn extract_token_rejects_non_utf8_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_bytes(&[b'B', b'e', b'a', b'r', b'e', b'r', b' ', 0x80])
                .expect("header value"),
        );
        assert_eq!(extract_token(&headers), ParsedToken::Malformed);
    }

    #[test]
    fn extract_token_rejects_overlength() {
        let mut headers = HeaderMap::new();
        let long_token = format!("Bearer {}", "a".repeat(MAX_BEARER_TOKEN_BYTES + 1));
        headers.insert("authorization", long_token.parse().expect("header value"));
        assert_eq!(extract_token(&headers), ParsedToken::Malformed);
    }

    #[test]
    fn extract_token_allows_max_length() {
        let mut headers = HeaderMap::new();
        let token = "a".repeat(MAX_BEARER_TOKEN_BYTES);
        let header = format!("Bearer {token}");
        headers.insert("authorization", header.parse().expect("header value"));
        assert_eq!(
            extract_token(&headers),
            ParsedToken::Present(token.as_str())
        );
    }

    #[test]
    fn valid_token_is_allowed() {
        let expected = token("session-token");
        let result = authorize_expected_token(&expected, Some("session-token"));
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn invalid_token_is_rejected() {
        let expected = token("session-token");
        let result = authorize_expected_token(&expected, Some("wrong"));
        assert_eq!(result, Err((StatusCode::UNAUTHORIZED, "invalid token")));
    }

    #[test]
    fn missing_token_is_rejected() {
        let expected = token("session-token");
        let result = authorize_expected_token(&expected, None);
        assert_eq!(result, Err((StatusCode::UNAUTHORIZED, "missing token")));
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
        let key = AuthThrottleKey::new(IpAddr::from([127, 0, 0, 1]), "hash".to_string(), "mutate");

        assert!(!throttle.record_failure(key.clone(), 2));
        assert!(!throttle.record_failure(key.clone(), 2));
        assert!(throttle.record_failure(key.clone(), 2));
        assert!(throttle.is_throttled(&key, 2));
    }

    #[tokio::test]
    async fn studio_accepts_local_static_token_on_loopback() {
        let tmp = tempdir().expect("tempdir");
        let ledger = TokenLedger::new(tmp.path().join("tokens.db"))
            .await
            .expect("ledger");
        let auth = AuthProvider::Studio {
            local_token: token("local-static"),
            ledger,
        };

        let result = auth
            .validate(Some("local-static"), IpAddr::from([127, 0, 0, 1]))
            .await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn studio_accepts_connection_token_for_remote_source() {
        let tmp = tempdir().expect("tempdir");
        let ledger = TokenLedger::new(tmp.path().join("tokens.db"))
            .await
            .expect("ledger");
        let mut minted = ledger.mint(1).await.expect("mint");
        let connection_token = minted.pop().expect("token");
        let auth = AuthProvider::Studio {
            local_token: token("local-static"),
            ledger,
        };

        let result = auth
            .validate(Some(&connection_token), IpAddr::from([203, 0, 113, 7]))
            .await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn studio_rejects_static_token_for_remote_source() {
        let tmp = tempdir().expect("tempdir");
        let ledger = TokenLedger::new(tmp.path().join("tokens.db"))
            .await
            .expect("ledger");
        let auth = AuthProvider::Studio {
            local_token: token("local-static"),
            ledger,
        };

        let result = auth
            .validate(Some("local-static"), IpAddr::from([203, 0, 113, 7]))
            .await;
        assert_eq!(
            result,
            Err((StatusCode::UNAUTHORIZED, "invalid connection token"))
        );
    }
}
