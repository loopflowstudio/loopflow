//! Loopback-only HTTP and WebSocket carrier for browser review pages.
//!
//! Serves static review pages under a per-server 32-byte random capability token
//! (`GET /<token>/f/<absolute path>`) and upgrades same-origin WebSockets at
//! `GET /<token>/machine` into the Machine protocol. Never writes the token to
//! disk and never exposes CORS headers, `OPTIONS`, or `/ticket`.

use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine as _;
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{
    HeaderValue, CACHE_CONTROL, CONNECTION, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN,
    REFERRER_POLICY, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE,
    X_CONTENT_TYPE_OPTIONS,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use loopflow_execution::engine::{Error, ErrorCode};
use loopflow_execution::transport::registry::Registry;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::{Role, WebSocketConfig};
use tokio_tungstenite::WebSocketStream;

use super::service::{self, Clients, Greeting, Messages};
use super::subscriptions::Subscriptions;
use super::ticket::{Admission, Tickets};
use super::wire::MAX_BULK;

const PAGE_CSP: &str = "default-src 'self' 'unsafe-inline' data: blob:; connect-src 'self'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'";
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Local-only capability minting handle for the running loopback listener.
/// Never logs or exposes the token through `Debug`.
pub(super) struct Handle {
    port: u16,
    token: String,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackHandle")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl Handle {
    /// Mint a capability URL for an existing absolute file path on this Machine.
    pub(super) fn url(&self, path: &Path) -> Result<String, Error> {
        if !path.is_absolute() {
            return Err(Error::new(
                ErrorCode::Invalid,
                format!("page path must be absolute: {}", path.display()),
            ));
        }
        let metadata = std::fs::metadata(path).map_err(|error| {
            let code = if error.kind() == std::io::ErrorKind::NotFound {
                ErrorCode::NotFound
            } else {
                ErrorCode::Unavailable
            };
            Error::new(code, format!("cannot read {}: {error}", path.display()))
        })?;
        if !metadata.is_file() {
            return Err(Error::new(
                ErrorCode::Invalid,
                format!("{} is not a file", path.display()),
            ));
        }
        let encoded = encode_absolute_path(path).ok_or_else(|| {
            Error::new(
                ErrorCode::Invalid,
                format!("page path contains invalid UTF-8: {}", path.display()),
            )
        })?;
        Ok(format!(
            "http://127.0.0.1:{}/{}/f/{encoded}",
            self.port, self.token
        ))
    }

    fn expected_host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn expected_origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn matches_token(&self, candidate: &str) -> bool {
        constant_time_eq(self.token.as_bytes(), candidate.as_bytes())
    }
}

/// A port in the dynamic range (49152–65535) derived from the server's socket path,
/// so each lf home asks for the same one every start.
fn preferred_port(socket: &Path) -> u16 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(socket.as_os_str().as_encoded_bytes());
    49152 + u16::from_be_bytes([digest[0], digest[1]]) % 16384
}

/// Bound loopback TCP listener ready to serve pages and WebSocket sessions.
pub(super) struct Loopback {
    listener: TcpListener,
    handle: Arc<Handle>,
}

impl Loopback {
    /// Bind the loopback port this server's socket prefers, with a fresh 32-byte random token.
    /// A stable port keeps an open page's origin, and with it the drafts and `data-lf`
    /// receipts it holds in `localStorage`, across server restarts. When something else
    /// holds that port, an ephemeral one serves instead.
    pub(super) async fn bind(socket: &Path) -> Result<(Self, Arc<Handle>)> {
        let preferred = SocketAddr::from(([127, 0, 0, 1], preferred_port(socket)));
        let listener = match TcpListener::bind(preferred).await {
            Ok(listener) => listener,
            Err(_) => TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
                .await
                .context("Could not bind loopback review page listener")?,
        };
        let port = listener.local_addr()?.port();
        let handle = Arc::new(Handle {
            port,
            token: random_token(),
        });
        Ok((
            Self {
                listener,
                handle: handle.clone(),
            },
            handle,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run(
        self,
        instance: Arc<str>,
        executable: Arc<String>,
        slots: Arc<Semaphore>,
        clients: Clients,
        tickets: Arc<Tickets>,
        subscriptions: Arc<Subscriptions>,
        terminals: Option<Arc<Registry>>,
    ) -> Result<()> {
        let state = Arc::new(State {
            handle: self.handle,
            instance,
            executable,
            slots,
            clients,
            tickets,
            subscriptions,
            terminals,
        });
        tracing::info!(
            port = state.handle.port,
            "Loopback review page listener ready"
        );
        let mut connections = JoinSet::new();
        loop {
            let (socket, _) = tokio::select! {
                accepted = self.listener.accept() => match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::warn!(%error, "Loopback accept failed; retrying");
                        tokio::time::sleep(ACCEPT_RETRY).await;
                        continue;
                    }
                },
                _ = connections.join_next(), if !connections.is_empty() => continue,
            };
            let state = state.clone();
            connections.spawn(async move {
                let io = TokioIo::new(socket);
                let service = service_fn(move |req| {
                    let state = state.clone();
                    async move { Ok::<_, hyper::Error>(handle_request(req, state).await) }
                });
                if let Err(error) = http1::Builder::new()
                    .serve_connection(io, service)
                    .with_upgrades()
                    .await
                {
                    tracing::debug!(%error, "Loopback HTTP connection ended");
                }
            });
        }
    }
}

struct State {
    handle: Arc<Handle>,
    instance: Arc<str>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    tickets: Arc<Tickets>,
    subscriptions: Arc<Subscriptions>,
    terminals: Option<Arc<Registry>>,
}

fn empty(status: StatusCode) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
        .expect("empty HTTP response is valid")
}

fn header_str(req: &Request<Incoming>, name: hyper::header::HeaderName) -> Option<&str> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

async fn handle_request(req: Request<Incoming>, state: Arc<State>) -> Response<Full<Bytes>> {
    if req.method() != Method::GET {
        return empty(StatusCode::NOT_FOUND);
    }
    // DNS rebinding defense: Host must be exactly `127.0.0.1:<port>`.
    if header_str(&req, HOST) != Some(state.handle.expected_host().as_str()) {
        return empty(StatusCode::NOT_FOUND);
    }

    // Route structure: `/<token>/machine` or `/<token>/f/<absolute path>`.
    let path = req.uri().path();
    let Some(without_leading) = path.strip_prefix('/') else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some((token, rest)) = without_leading.split_once('/') else {
        return empty(StatusCode::NOT_FOUND);
    };
    if !state.handle.matches_token(token) {
        return empty(StatusCode::NOT_FOUND);
    }

    if rest == "machine" {
        return handle_websocket(req, state);
    }
    if let Some(encoded_file) = rest.strip_prefix("f/") {
        return handle_file(encoded_file).await;
    }

    empty(StatusCode::NOT_FOUND)
}

fn handle_websocket(req: Request<Incoming>, state: Arc<State>) -> Response<Full<Bytes>> {
    if req.uri().query().is_some() {
        return empty(StatusCode::NOT_FOUND);
    }
    // Browser cross-origin WebSocket defense: Origin must be exact same-origin.
    if header_str(&req, ORIGIN) != Some(state.handle.expected_origin().as_str()) {
        return empty(StatusCode::NOT_FOUND);
    }

    let is_upgrade =
        header_str(&req, UPGRADE).is_some_and(|value| value.eq_ignore_ascii_case("websocket"));
    let has_connection_upgrade = req
        .headers()
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"));
    let valid_version = header_str(&req, SEC_WEBSOCKET_VERSION) == Some("13");
    let Some(key) = header_str(&req, SEC_WEBSOCKET_KEY).filter(|value| !value.is_empty()) else {
        return empty(StatusCode::NOT_FOUND);
    };

    if !is_upgrade || !has_connection_upgrade || !valid_version {
        return empty(StatusCode::NOT_FOUND);
    }

    let accept = derive_accept_key(key.as_bytes());
    let Ok(accept_header) = HeaderValue::from_str(&accept) else {
        return empty(StatusCode::NOT_FOUND);
    };

    let upgrade = hyper::upgrade::on(req);
    tokio::spawn(async move {
        let upgraded = match upgrade.await {
            Ok(upgraded) => upgraded,
            Err(error) => {
                tracing::debug!(%error, "Loopback WebSocket upgrade failed");
                return;
            }
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BULK))
            .max_frame_size(Some(MAX_BULK));
        let socket =
            WebSocketStream::from_raw_socket(TokioIo::new(upgraded), Role::Server, Some(config))
                .await;
        if let Err(error) = run_websocket(socket, &state).await {
            tracing::debug!(%error, "Loopback WebSocket session ended");
        }
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "Upgrade")
        .header(SEC_WEBSOCKET_ACCEPT, accept_header)
        .body(Full::new(Bytes::new()))
        .expect("WebSocket upgrade response is valid")
}

async fn run_websocket<S>(socket: WebSocketStream<S>, state: &State) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    let mut messages = Messages(socket);
    let first = tokio::time::timeout(service::FIRST_MESSAGE, messages.next())
        .await
        .context("no message within five seconds")?
        .context("closed before its first message")??;
    let ended = match service::welcome(
        &mut messages,
        &first,
        &state.instance,
        &state.executable,
        &state.clients,
        &state.slots,
    )
    .await?
    {
        Greeting::Ended | Greeting::Ticket(_) => Ok(()),
        Greeting::Session(listed, queue) => {
            service::session(
                &mut messages,
                state.instance.clone(),
                &listed,
                None,
                Admission::Local,
                state.tickets.clone(),
                state.terminals.clone(),
                &state.subscriptions,
                queue,
            )
            .await
        }
    };
    let _ = messages.0.close(None).await;
    ended
}

async fn handle_file(encoded: &str) -> Response<Full<Bytes>> {
    let Some(path) = decode_absolute_path(encoded) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(file) = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
        .await
    else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(metadata) = file.metadata().await else {
        return empty(StatusCode::NOT_FOUND);
    };
    if !metadata.is_file() {
        return empty(StatusCode::NOT_FOUND);
    }
    if metadata.len() > MAX_BULK as u64 {
        return empty(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let mut bytes = Vec::new();
    if file
        .take(MAX_BULK as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .is_err()
    {
        return empty(StatusCode::NOT_FOUND);
    }
    if bytes.len() > MAX_BULK {
        return empty(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let content_type = content_type_for(&path);
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(REFERRER_POLICY, "no-referrer")
        .header(CACHE_CONTROL, "no-store")
        .header(X_CONTENT_TYPE_OPTIONS, "nosniff");
    if content_type.starts_with("text/html") || content_type == "image/svg+xml" {
        builder = builder.header(CONTENT_SECURITY_POLICY, PAGE_CSP);
    }
    builder
        .body(Full::new(Bytes::from(bytes)))
        .expect("file HTTP response is valid")
}

fn content_type_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    match ext.as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

fn encode_absolute_path(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let trimmed = text.strip_prefix('/')?;
    let mut out = String::with_capacity(trimmed.len());
    for byte in trimmed.bytes() {
        if byte == b'/' || byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
        {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte & 0x0f));
        }
    }
    Some(out)
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

fn decode_absolute_path(encoded: &str) -> Option<PathBuf> {
    if encoded.is_empty() {
        return None;
    }
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len() + 1);
    decoded.push(b'/');
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi = from_hex(bytes[i + 1])?;
            let lo = from_hex(bytes[i + 2])?;
            let byte = (hi << 4) | lo;
            if byte == 0 {
                return None;
            }
            decoded.push(byte);
            i += 3;
        } else {
            if bytes[i] == 0 {
                return None;
            }
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let text = String::from_utf8(decoded).ok()?;
    let path = PathBuf::from(text);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }
    Some(path)
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        constant_time_eq, decode_absolute_path, encode_absolute_path, preferred_port, Loopback,
        PAGE_CSP,
    };
    use crate::server::service::Clients;
    use crate::server::subscriptions::Subscriptions;
    use crate::server::ticket::Tickets;
    use crate::server::wire::{
        ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation,
        MAX_CONNECTIONS, REVISION,
    };
    use futures_util::{SinkExt, StreamExt};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Semaphore;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;

    fn in_empty_home(body: impl std::future::Future<Output = ()>) {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = loopflow_execution::config::environment::EnvVarGuard::set(
            "LF_HOME",
            home.path().to_str().unwrap(),
        );
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(body);
    }

    fn home_socket() -> std::path::PathBuf {
        loopflow_execution::store::lf_home_dir().join("server.sock")
    }

    #[test]
    fn loopback_keeps_its_port_across_restarts_and_yields_a_taken_one() {
        in_empty_home(async {
            let socket = home_socket();
            let (first, handle) = Loopback::bind(&socket).await.unwrap();
            let port = handle.port;
            assert_eq!(port, preferred_port(&socket));
            // Held by the first server: the second still binds, elsewhere.
            let (_second, other) = Loopback::bind(&socket).await.unwrap();
            assert_ne!(other.port, port);
            drop(first);
            let (_restarted, again) = Loopback::bind(&socket).await.unwrap();
            assert_eq!(again.port, port);
            assert_ne!(again.token, handle.token);
        });
    }

    #[test]
    fn path_encoding_round_trips_spaces_and_rejects_parent_traversal() {
        let path = Path::new("/tmp/my repo/scratch/ship review #1.html");
        let encoded = encode_absolute_path(path).unwrap();
        assert_eq!(encoded, "tmp/my%20repo/scratch/ship%20review%20%231.html");
        assert_eq!(decode_absolute_path(&encoded).as_deref(), Some(path));
        assert!(decode_absolute_path("tmp/../etc/passwd").is_none());
        assert!(decode_absolute_path("tmp/%2e%2e/etc/passwd").is_none());
        assert!(decode_absolute_path("tmp/%00foo").is_none());
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"short"));
    }

    #[test]
    fn loopback_serves_html_with_security_headers_and_enforces_websocket_admission() {
        in_empty_home(async {
            let (loopback, handle) = Loopback::bind(&home_socket()).await.unwrap();
            let port = handle.port;
            let token = handle.token.clone();
            let server = tokio::spawn(loopback.run(
                Arc::from("test-instance"),
                Arc::new("test-executable".into()),
                Arc::new(Semaphore::new(MAX_CONNECTIONS)),
                Clients::default(),
                Arc::new(Tickets::default()),
                Arc::new(Subscriptions::default()),
                None,
            ));

            let dir = tempfile::tempdir().unwrap();
            let page = dir.path().join("ship-review.html");
            std::fs::write(&page, "<!doctype html><h1>Hello</h1>").unwrap();
            let url = handle.url(&page).unwrap();

            let client = reqwest::Client::new();
            let response = client.get(&url).send().await.unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(
                response.headers().get("content-security-policy").unwrap(),
                PAGE_CSP
            );
            assert_eq!(
                response.headers().get("referrer-policy").unwrap(),
                "no-referrer"
            );
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert_eq!(
                response.headers().get("x-content-type-options").unwrap(),
                "nosniff"
            );
            assert!(response
                .headers()
                .get("access-control-allow-origin")
                .is_none());
            assert_eq!(
                response.text().await.unwrap(),
                "<!doctype html><h1>Hello</h1>"
            );

            // Wrong token returns 404 with empty body.
            let bad_token = url.replace(&token, "wrong-token");
            let rejected = client.get(&bad_token).send().await.unwrap();
            assert_eq!(rejected.status(), reqwest::StatusCode::NOT_FOUND);
            assert!(rejected.text().await.unwrap().is_empty());

            // Wrong Host header (DNS rebinding) returns 404 with empty body.
            let rebound = client
                .get(&url)
                .header("host", format!("evil.test:{port}"))
                .send()
                .await
                .unwrap();
            assert_eq!(rebound.status(), reqwest::StatusCode::NOT_FOUND);
            assert!(rebound.text().await.unwrap().is_empty());

            // Upgrade without Origin, foreign Origin, wrong Host, or wrong token → 404 each.
            let ws_url = format!("ws://127.0.0.1:{port}/{token}/machine");
            let good_origin = format!("http://127.0.0.1:{port}");
            let cases: Vec<(String, Option<&str>, Option<String>)> = vec![
                (ws_url.clone(), None, None),
                (ws_url.clone(), Some("https://evil.example"), None),
                (
                    ws_url.clone(),
                    Some(good_origin.as_str()),
                    Some(format!("localhost:{port}")),
                ),
                (
                    format!("ws://127.0.0.1:{port}/wrong-token/machine"),
                    Some(good_origin.as_str()),
                    None,
                ),
                (
                    format!("ws://127.0.0.1:{port}/{token}/ticket"),
                    Some(good_origin.as_str()),
                    None,
                ),
            ];
            for (target, origin, host_override) in cases {
                let mut req = target.into_client_request().unwrap();
                if let Some(origin) = origin {
                    req.headers_mut().insert("origin", origin.parse().unwrap());
                }
                if let Some(host) = host_override {
                    req.headers_mut().insert("host", host.parse().unwrap());
                }
                let err = tokio_tungstenite::connect_async(req).await.unwrap_err();
                let tokio_tungstenite::tungstenite::Error::Http(resp) = err else {
                    panic!("expected HTTP rejection, got {err:?}");
                };
                assert_eq!(resp.status(), 404);
            }

            // Valid Host + Origin + token upgrades and speaks the Machine protocol,
            // while refusing local-socket-only operations (`page.url`, `mobile.status`).
            let mut req = ws_url.into_client_request().unwrap();
            req.headers_mut().insert(
                "origin",
                format!("http://127.0.0.1:{port}").parse().unwrap(),
            );
            let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
            let hello = ClientMessage::Hello(Hello {
                revision: REVISION,
                min_revision: REVISION,
                client: ClientIdentity {
                    kind: "page".into(),
                    platform: "browser".into(),
                    name: "review-page".into(),
                    version: "0".into(),
                },
            });
            ws.send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
                .await
                .unwrap();
            let welcome = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let Message::Text(text) = welcome else {
                panic!("expected text welcome");
            };
            let parsed: ServerMessage = serde_json::from_str(&text).unwrap();
            assert!(matches!(parsed, ServerMessage::Welcome(w) if w.instance == "test-instance"));

            // `page.url` is refused on the loopback carrier itself.
            let mint = ClientMessage::Request {
                id: "mint-1".into(),
                operation: Operation::Server(ServerOperation::PageUrl { path: page.clone() }),
                select: None,
                sender: None,
                request_id: None,
            };
            ws.send(Message::Text(serde_json::to_string(&mint).unwrap().into()))
                .await
                .unwrap();
            let answer = tokio::time::timeout(Duration::from_secs(5), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let Message::Text(text) = answer else {
                panic!("expected text error");
            };
            let parsed: ServerMessage = serde_json::from_str(&text).unwrap();
            assert!(matches!(
                parsed,
                ServerMessage::Error {
                    id: Some(ref id),
                    ..
                } if id == "mint-1"
            ));

            let _ = ws.close(None).await;
            server.abort();
        });
    }
}
