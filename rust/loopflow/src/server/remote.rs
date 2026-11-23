//! HTTPS on the host's tailnet addresses. Tailscale identifies the socket peer;
//! the mobile boundary never inherits the private Unix socket's authority.
//! One protocol message per WebSocket text frame; a bridged terminal carries
//! the owner protocol's bytes in binary frames.
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Poll};
use std::time::{Duration, SystemTime};

use super::mobile::{Configuration, Status, Tailscale};
use super::service::{self, Carrier, Clients, Greeting};
use super::subscriptions::Subscriptions;
use super::ticket::{Admission, Tickets};
use super::wire::MAX_BULK;
use anyhow::{Context, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use loopflow_execution::transport::registry::Registry;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, UnixStream};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::{
    handshake::server::{ErrorResponse, Request, Response},
    http::StatusCode,
    protocol::WebSocketConfig,
    Message,
};
use tokio_tungstenite::{accept_hdr_async_with_config, WebSocketStream};

/// Owner bytes copied per binary frame; a bound on buffering, not a protocol limit.
const CHUNK: usize = 64 * 1024;

#[derive(Debug)]
pub(super) enum Control {
    Configure {
        configuration: Option<Configuration>,
        reply: oneshot::Sender<Result<Option<Configuration>, String>>,
    },
    Status {
        reply: oneshot::Sender<Status>,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn manage(
    initial: Result<Option<Configuration>>,
    mut requests: mpsc::Receiver<Control>,
    instance: Arc<str>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    tickets: Arc<Tickets>,
    subscriptions: Arc<Subscriptions>,
    terminals: Option<Arc<Registry>>,
) -> Result<()> {
    let mut status = match initial {
        Ok(configuration) => Status {
            configuration,
            ready: false,
            error: None,
        },
        Err(error) => Status {
            configuration: None,
            ready: false,
            error: Some(error.to_string()),
        },
    };
    let mut listeners = JoinSet::new();
    let mut bindings = JoinSet::new();
    let mut setup: Option<oneshot::Sender<Result<Option<Configuration>, String>>> = None;
    let mut retry = Duration::from_secs(5);
    if let Some(saved) = status.configuration.as_ref().filter(|c| c.enabled) {
        bindings.spawn(scheduled_bind(saved.clone(), false, Duration::ZERO));
    }

    loop {
        let result = tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()) };
                let (requested, reply) = match request {
                    Control::Status { reply } => { let _ = reply.send(status.clone()); continue; }
                    Control::Configure { configuration, reply } => (configuration, reply),
                };
                if status.ready && requested.as_ref().zip(status.configuration.as_ref())
                    .is_some_and(|(requested, current)| requested.owner == current.owner && requested.endpoint == current.endpoint) {
                    let _ = reply.send(Ok(status.configuration.clone()));
                    continue;
                }
                bindings.shutdown().await;
                if let Some(previous) = setup.take() {
                    let _ = previous.send(Err("Mobile setup was superseded".into()));
                }
                listeners.shutdown().await;
                status.ready = false;
                status.error = None;
                retry = Duration::from_secs(5);
                if let Some(config) = &mut status.configuration { config.enabled = false; }
                match tokio::task::spawn_blocking(super::mobile::save_disabled).await? {
                    Ok(disabled) => status.configuration = disabled,
                    Err(error) => {
                        let error = format!("Mobile access stopped, but disabled state could not be saved: {error:#}");
                        status.error = Some(error.clone());
                        let _ = reply.send(Err(error));
                        continue;
                    }
                }
                if let Some(config) = requested {
                    bindings.spawn(scheduled_bind(config, true, Duration::ZERO));
                    setup = Some(reply);
                } else {
                    let _ = reply.send(Ok(None));
                }
                continue;
            }
            Some(bound) = bindings.join_next(), if !bindings.is_empty() => bound.map_err(anyhow::Error::from).and_then(|r| r),
            Some(ended) = listeners.join_next(), if !listeners.is_empty() => {
                status.ready = false;
                Err(match ended {
                    Ok(Err(error)) => error,
                    Err(error) => error.into(),
                    Ok(Ok(())) => anyhow::anyhow!("Mobile connection stopped; reconnecting"),
                })
            }
        };
        // Save only on explicit setup. Completing disk writes here serializes
        // them with --off, even when the CLI that requested setup has gone.
        let reply = setup.take();
        let result = match result {
            Ok(bound) if reply.is_some() => {
                let config = bound.config.clone();
                tokio::task::spawn_blocking(move || super::mobile::save(&config))
                    .await?
                    .context("Could not save mobile access; no listener was enabled")
                    .map(|()| bound)
            }
            other => other,
        };
        match result {
            Ok(bound) => {
                status.configuration = Some(bound.config.clone());
                status.ready = true;
                status.error = None;
                retry = Duration::from_secs(5);
                listeners.spawn(run(
                    bound,
                    instance.clone(),
                    executable.clone(),
                    slots.clone(),
                    clients.clone(),
                    tickets.clone(),
                    subscriptions.clone(),
                    terminals.clone(),
                ));
                if let Some(reply) = reply {
                    let _ = reply.send(Ok(status.configuration.clone()));
                }
            }
            Err(error) => {
                let message = format!("{error:#}");
                if let Some(reply) = reply {
                    let _ = reply.send(Err(message.clone()));
                } else if let Some(saved) = &status.configuration {
                    if status.error.as_ref() != Some(&message) {
                        tracing::warn!(%error, "Mobile listener unavailable; retrying the saved approval");
                    }
                    bindings.spawn(scheduled_bind(saved.clone(), false, retry));
                    retry = (2 * retry).min(Duration::from_secs(30));
                }
                status.error = Some(message);
            }
        }
    }
}

async fn scheduled_bind(
    config: Configuration,
    allow_port_change: bool,
    delay: Duration,
) -> Result<Listener> {
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    bind(config, allow_port_change).await
}

struct Listener {
    sockets: Vec<TcpListener>,
    host: Tailscale,
    config: Configuration,
    tls: TlsAcceptor,
}

async fn bind(mut config: Configuration, allow_port_change: bool) -> Result<Listener> {
    let (host, observed) = Tailscale::discover().await?;
    observed.verify(&config)?;
    let tls = host.certificate(&observed.dns).await?;
    let addresses = observed.addresses;
    let port = config.port()?;
    let sockets = match bind_sockets(&addresses, port).await {
        Ok(sockets) => sockets,
        Err(error)
            if allow_port_change
                && port != 0
                && error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::AddrInUse) =>
        {
            // Only explicit setup can change the saved address. Restoration
            // keeps retrying the original port so existing phones can recover.
            bind_sockets(&addresses, 0).await?
        }
        Err(error) => return Err(error),
    };
    let port = sockets[0].local_addr()?.port();
    config.endpoint = format!("https://{}:{port}", observed.dns);
    config.enabled = true;
    Ok(Listener {
        sockets,
        host,
        config,
        tls,
    })
}

async fn bind_sockets(addresses: &[IpAddr], mut port: u16) -> Result<Vec<TcpListener>> {
    let mut sockets = Vec::new();
    for ip in addresses {
        // Never bind wildcard or LAN addresses. The exact addresses come from
        // the installed Tailscale daemon, not a caller's hostname or HTTP header.
        let socket = match TcpListener::bind((*ip, port)).await {
            Ok(socket) => socket,
            // Hosts can disable IPv6 while Tailscale still reports its node's
            // IPv6 allocation. Keep the usable family; never broaden the bind.
            Err(error) if error.kind() == std::io::ErrorKind::AddrNotAvailable => continue,
            Err(error) => {
                return Err(error).context("Could not listen on the Machine's Tailscale address")
            }
        };
        port = socket.local_addr()?.port();
        sockets.push(socket);
    }
    anyhow::ensure!(
        !sockets.is_empty(),
        "Tailscale has no available local address"
    );
    Ok(sockets)
}

async fn check_owner(host: &Tailscale, config: &Configuration) -> Result<String> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    let observed = host.status_without_peers().await?;
    observed.verify(config)?;
    Ok(observed.dns)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    bound: Listener,
    instance: Arc<str>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    tickets: Arc<Tickets>,
    subscriptions: Arc<Subscriptions>,
    terminals: Option<Arc<Registry>>,
) -> Result<()> {
    tracing::info!(endpoint = %bound.config.endpoint, "Mobile HTTPS ready on host Tailscale");
    let mut connections = JoinSet::new();
    let mut renewed = SystemTime::now();
    let mut renewals = JoinSet::new();
    let owner = check_owner(&bound.host, &bound.config);
    tokio::pin!(owner);
    let mut tls = bound.tls;
    loop {
        let accepted = futures_util::future::select_all(
            bound.sockets.iter().map(|socket| Box::pin(socket.accept())),
        );
        let (socket, peer) = tokio::select! {
            (accepted, _, _) = accepted => accepted?,
            _ = connections.join_next(), if !connections.is_empty() => continue,
            checked = &mut owner => {
                let dns = checked?;
                owner.set(check_owner(&bound.host, &bound.config));
                if renewals.is_empty() && renewed.elapsed().unwrap_or_default() >= Duration::from_secs(6 * 60 * 60) {
                    let host = bound.host.clone();
                    renewals.spawn(async move { host.certificate(&dns).await });
                }
                continue;
            }
            Some(certificate) = renewals.join_next(), if !renewals.is_empty() => {
                tls = certificate??;
                renewed = SystemTime::now();
                continue;
            }
        };
        let slots = slots.clone();
        let subscriptions = subscriptions.clone();
        let terminals = terminals.clone();
        let (host, owner, tls, instance, executable, clients, tickets) = (
            bound.host.clone(),
            bound.config.owner.clone(),
            tls.clone(),
            instance.clone(),
            executable.clone(),
            clients.clone(),
            tickets.clone(),
        );
        connections.spawn(async move {
            let result = async {
                let admission = host.permits(peer, &owner).await?;
                let socket =
                    tokio::time::timeout(Duration::from_secs(5), tls.accept(socket)).await??;
                connection(
                    socket,
                    admission,
                    &instance,
                    &executable,
                    &clients,
                    tickets,
                    terminals,
                    &slots,
                    &subscriptions,
                )
                .await
            }
            .await;
            if let Err(error) = result {
                tracing::debug!(%error, "Mobile connection ended");
            }
        });
    }
}

fn reject(status: StatusCode) -> ErrorResponse {
    Response::builder()
        .status(status)
        .body(Some(
            "Connect using the Machine owner's Tailscale account".into(),
        ))
        .expect("fixed HTTP response is valid")
}

/// `/v1/machine` is the control connection; `/v1/ticket` carries what a ticket
/// names: a terminal's bytes or one bulk message.
// Tungstenite's handshake callback requires an unboxed HTTP ErrorResponse.
#[allow(clippy::result_large_err, clippy::too_many_arguments)]
async fn connection<S>(
    socket: S,
    admission: Option<Admission>,
    instance: &str,
    executable: &str,
    clients: &Clients,
    tickets: Arc<Tickets>,
    terminals: Option<Arc<Registry>>,
    slots: &Arc<Semaphore>,
    subscriptions: &Arc<Subscriptions>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_BULK))
        .max_frame_size(Some(MAX_BULK));
    let permitted = admission.is_some();
    let mut ticket_path = false;
    let socket = tokio::time::timeout(
        Duration::from_secs(5),
        accept_hdr_async_with_config(
            socket,
            |request: &Request, response: Response| {
                if !permitted
                    || request.headers().contains_key("origin")
                    || request.uri().query().is_some()
                {
                    return Err(reject(StatusCode::FORBIDDEN));
                }
                match request.uri().path() {
                    "/v1/machine" => {}
                    "/v1/ticket" => ticket_path = true,
                    _ => return Err(reject(StatusCode::NOT_FOUND)),
                }
                Ok(response)
            },
            Some(config),
        ),
    )
    .await??;
    let admission = admission.expect("the handshake refused an unadmitted peer");
    let mut messages = Messages(socket);
    let first = tokio::time::timeout(service::FIRST_MESSAGE, messages.next())
        .await
        .context("no message within five seconds")?
        .context("closed before its first message")??;
    let ended = match service::welcome(&mut messages, &first, instance, executable, clients, slots)
        .await?
    {
        Greeting::Ended => Ok(()),
        Greeting::Ticket(ticket) if ticket_path => {
            match service::ticketed(&mut messages, &ticket, &admission, &tickets).await {
                Ok(Some(destination)) => return bridge(messages.0, destination).await,
                ended => ended.map(|_| ()),
            }
        }
        // A ticket belongs on `/v1/ticket` and a session on `/v1/machine`.
        Greeting::Ticket(_) => Ok(()),
        Greeting::Session(..) if ticket_path => Ok(()),
        // Remote connections never configure mobile access.
        Greeting::Session(listed, queue) => {
            service::session(
                &mut messages,
                instance.into(),
                &listed,
                None,
                admission,
                tickets,
                terminals,
                subscriptions,
                queue,
            )
            .await
        }
    };
    // The control connection ends with a close frame, not a reset.
    let _ = messages.0.close(None).await;
    ended
}

/// Protocol messages as WebSocket text frames. Binary frames belong to a
/// bridged terminal and are refused here; the carrier answers pings itself.
struct Messages<S>(WebSocketStream<S>);

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Stream for Messages<S> {
    type Item = Result<String>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            let Some(message) = ready!(self.0.poll_next_unpin(cx)) else {
                return Poll::Ready(None);
            };
            return Poll::Ready(match message {
                Ok(Message::Text(text)) => Some(Ok(text.to_string())),
                Ok(Message::Close(_)) => None,
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Binary(_)) => Some(Err(anyhow::anyhow!(
                    "the control connection carries text messages"
                ))),
                Err(error) => Some(Err(error.into())),
            });
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Sink<String> for Messages<S> {
    type Error = anyhow::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<()>> {
        self.0.poll_ready_unpin(cx).map_err(Into::into)
    }

    fn start_send(mut self: Pin<&mut Self>, item: String) -> Result<()> {
        Ok(self.0.start_send_unpin(Message::Text(item.into()))?)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<()>> {
        self.0.poll_flush_unpin(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<()>> {
        self.0.poll_close_unpin(cx).map_err(Into::into)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Carrier for Messages<S> {
    type Failure = anyhow::Error;
}

/// A plain byte copy between the held terminal and the WebSocket's binary frames.
/// Closing either side releases the owner claim.
async fn bridge<S>(socket: WebSocketStream<S>, mut destination: UnixStream) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut sender, mut receiver) = socket.split();
    let (mut reader, mut writer) = destination.split();
    let upstream = async {
        while let Some(message) = receiver.next().await {
            match message? {
                Message::Binary(bytes) => writer.write_all(&bytes).await?,
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
                Message::Text(_) => anyhow::bail!("a bridged terminal carries binary messages"),
            }
        }
        Ok(())
    };
    let downstream = async {
        let mut bytes = vec![0; CHUNK];
        loop {
            let count = reader.read(&mut bytes).await?;
            if count == 0 {
                break;
            }
            tokio::time::timeout(
                Duration::from_secs(5),
                sender.send(Message::Binary(bytes[..count].to_vec().into())),
            )
            .await??;
        }
        Ok(())
    };
    let result = tokio::select! {
        result = upstream => result,
        result = downstream => result,
    };
    // End the WebSocket protocol as well as the byte stream. A TCP EOF without
    // its close frame can leave native clients waiting after buffered output.
    // A stalled peer gets the same bounded write grace, never an unbounded drain.
    let closed = tokio::time::timeout(Duration::from_secs(5), sender.close()).await;
    if let Err(error) = result {
        tracing::debug!(%error, "Server owner bridge disconnected");
    }
    closed??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bridge, connection};
    use crate::server::service::Clients;
    use crate::server::ticket::{Admission, Purpose, Tickets};
    use crate::server::wire::{
        ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation,
        MAX_MESSAGE, MIN_REVISION, REVISION,
    };
    use futures_util::{SinkExt, StreamExt};
    use loopflow_execution::engine::ErrorCode;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, UnixStream};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

    // Kernel socket I/O and the manager's control replies should finish at the
    // current tick. Keep Tokio from auto-advancing to an HTTP timeout meanwhile.
    async fn at_current_tick<T>(event: impl std::future::Future<Output = T>) -> T {
        let (release, held) = std::sync::mpsc::channel::<()>();
        let mut watchdog =
            tokio::task::spawn_blocking(move || held.recv_timeout(Duration::from_secs(5)));
        let result = tokio::select! {
            result = event => result,
            _ = &mut watchdog => panic!("expected progress without advancing the clock"),
        };
        drop(release);
        let _ = watchdog.await.unwrap();
        result
    }

    async fn mobile_status(
        requests: &tokio::sync::mpsc::Sender<super::Control>,
    ) -> crate::server::mobile::Status {
        let (reply, answer) = tokio::sync::oneshot::channel();
        requests
            .send(super::Control::Status { reply })
            .await
            .unwrap();
        answer.await.unwrap()
    }

    async fn next_restore_error(
        requests: &tokio::sync::mpsc::Sender<super::Control>,
        previous: Option<&str>,
    ) -> String {
        at_current_tick(async {
            loop {
                if let Some(error) = mobile_status(requests).await.error {
                    if Some(error.as_str()) != previous {
                        return error;
                    }
                }
            }
        })
        .await
    }

    #[test]
    fn saved_restores_back_off_from_completion_and_off_cancels_a_pending_request() {
        use crate::server::mobile::{
            configured, save,
            tests::{approval, start_manager},
        };
        use axum::{routing::get, Json, Router};
        use loopflow_execution::config::environment::EnvVarGuard;
        use serde_json::{json, Value};
        use tokio::net::UnixListener;
        use tokio::sync::{mpsc, oneshot};
        use tokio::time::{advance, timeout};

        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let path = dir.path().join("tailscaled.sock");
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let _socket = EnvVarGuard::set("TS_SOCKET", path.to_str().unwrap());
        save(&approval(8443)).unwrap();

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(async {
                let (requests, manager) = start_manager();
                let mut previous = next_restore_error(&requests, None).await;
                // The first attempt found no socket. Hold each subsequent status
                // request at the same address until the test supplies its answer.
                let daemon = UnixListener::bind(path).unwrap();
                let (incoming, mut attempts) = mpsc::unbounded_channel();
                let status = Router::new().route(
                    "/localapi/v0/status",
                    get(move || {
                        let (respond, answer) = oneshot::channel::<Value>();
                        incoming.send(respond).unwrap();
                        async move { Json(answer.await.unwrap_or_default()) }
                    }),
                );
                let server =
                    tokio::spawn(async move { axum::serve(daemon, status).await.unwrap() });
                let tick = Duration::from_millis(1);
                for (i, seconds) in [5, 10, 20, 30, 30].into_iter().enumerate() {
                    assert!(
                        timeout(Duration::from_secs(seconds) - tick, attempts.recv())
                            .await
                            .is_err(),
                        "retried before {seconds}s"
                    );
                    advance(tick).await;
                    let mut respond = at_current_tick(attempts.recv()).await.unwrap();
                    advance(Duration::from_secs(2)).await;
                    let pending = at_current_tick(mobile_status(&requests)).await;
                    assert!(!pending.ready);
                    assert_eq!(pending.error.as_deref(), Some(previous.as_str()));

                    if i == 4 {
                        let (reply, answer) = tokio::sync::oneshot::channel();
                        requests
                            .send(super::Control::Configure {
                                configuration: None,
                                reply,
                            })
                            .await
                            .unwrap();
                        assert!(at_current_tick(answer).await.unwrap().unwrap().is_none());
                        at_current_tick(respond.closed()).await;
                        break;
                    }

                    // Distinct errors acknowledge completion before timing the next retry.
                    let node = if i % 2 == 0 {
                        json!({"Tags":["tag:ci"]})
                    } else {
                        json!({})
                    };
                    respond
                        .send(json!({"BackendState":"Running", "Self":node}))
                        .unwrap();
                    previous = next_restore_error(&requests, Some(&previous)).await;
                }
                assert!(!configured().unwrap().unwrap().enabled);
                assert!(
                    timeout(Duration::from_secs(60), attempts.recv())
                        .await
                        .is_err(),
                    "disable cancels future retries"
                );
                drop(requests);
                manager.await.unwrap().unwrap();
                server.abort();
                assert!(server.await.unwrap_err().is_cancelled());
            });
    }

    #[tokio::test]
    async fn completed_terminal_stream_sends_websocket_close_after_its_last_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (owner, mut machine) = UnixStream::pair().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
            bridge(websocket, owner).await.unwrap();
        });
        let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
            .await
            .unwrap();
        client
            .send(Message::Ping(b"alive".to_vec().into()))
            .await
            .unwrap();
        let pong = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await
            .unwrap();
        assert!(
            matches!(pong, Some(Ok(Message::Pong(_)))),
            "An idle stream must answer liveness checks"
        );
        machine.write_all(b"last screen").await.unwrap();
        // The Machine may keep its read half alive while the held terminal has ended.
        machine.shutdown().await.unwrap();
        let last = client.next().await.unwrap().unwrap();
        assert_eq!(last.into_data(), b"last screen"[..]);
        let close = tokio::time::timeout(Duration::from_secs(2), client.next())
            .await
            .unwrap();
        assert!(
            matches!(close, Some(Ok(Message::Close(_)))),
            "Expected WebSocket close, got {close:?}"
        );
        server.await.unwrap();
    }

    async fn connect(
        tickets: &Arc<Tickets>,
        permitted: bool,
        path: &str,
        origin: bool,
    ) -> anyhow::Result<tokio_tungstenite::WebSocketStream<UnixStream>> {
        let tickets = tickets.clone();
        let (client, server) = UnixStream::pair()?;
        let mut request = format!("ws://localhost{path}").into_client_request()?;
        // Caller-supplied identity cannot override the host's WhoIs result.
        request
            .headers_mut()
            .insert("tailscale-user-login", "owner@example.com".parse()?);
        if origin {
            request
                .headers_mut()
                .insert("origin", "https://example.com".parse()?);
        }
        tokio::spawn(async move {
            let admission = permitted.then(|| Admission::Remote {
                node: "phone".into(),
                user: 1,
            });
            let _ = connection(
                server,
                admission,
                "test-instance",
                "test-executable",
                &Clients::default(),
                tickets,
                None,
                &Arc::new(tokio::sync::Semaphore::new(
                    crate::server::wire::MAX_CONNECTIONS,
                )),
                &Arc::new(super::Subscriptions::default()),
            )
            .await;
        });
        Ok(tokio_tungstenite::client_async(request, client).await?.0)
    }

    fn download(message: String) -> Purpose {
        let (outbox, _queue) = crate::server::outbox::Outbox::new();
        Purpose::Download {
            message,
            reservation: outbox.reserve().unwrap(),
        }
    }

    fn text(message: &ClientMessage) -> Message {
        Message::Text(serde_json::to_string(message).unwrap().into())
    }

    async fn receive(
        client: &mut tokio_tungstenite::WebSocketStream<UnixStream>,
    ) -> Option<ServerMessage> {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), client.next())
                .await
                .unwrap()?
                .unwrap();
            match message {
                Message::Text(text) => return Some(serde_json::from_str(&text).unwrap()),
                Message::Close(_) => return None,
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Binary(_) => panic!("unexpected binary message"),
            }
        }
    }

    fn hello(revision: u32) -> ClientMessage {
        ClientMessage::Hello(Hello {
            revision,
            min_revision: revision,
            client: ClientIdentity {
                kind: "test".into(),
                platform: "test".into(),
                name: "fixture".into(),
                version: "0".into(),
            },
        })
    }

    /// Runs `body` with this test's own `LF_HOME`: the welcome reads the
    /// Machine's store, which other tests swap under the env lock.
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

    #[test]
    fn only_owner_identity_reaches_shared_operations_on_one_connection() {
        in_empty_home(async {
            let tickets = Arc::new(Tickets::default());
            for (permitted, path, origin) in [
                (false, "/v1/machine", false),
                (false, "/v1/ticket", false),
                (true, "/v1/pair", false),
                (true, "/v1/machine?token=old", false),
                (true, "/v1/machine", true),
            ] {
                assert!(connect(&tickets, permitted, path, origin).await.is_err());
            }
            // A client speaking only a newer revision is refused by name.
            let mut client = connect(&tickets, true, "/v1/machine", false).await.unwrap();
            client.send(text(&hello(REVISION + 1))).await.unwrap();
            let refusal = receive(&mut client).await.unwrap();
            assert!(
                matches!(refusal, ServerMessage::Error { id: None, .. }),
                "{refusal:?}"
            );
            assert!(receive(&mut client).await.is_none(), "refusal closes");

            let mut client = connect(&tickets, true, "/v1/machine", false).await.unwrap();
            client.send(text(&hello(REVISION))).await.unwrap();
            let ServerMessage::Welcome(welcome) = receive(&mut client).await.unwrap() else {
                panic!("expected welcome");
            };
            assert_eq!(welcome.revision, REVISION.min(REVISION));
            assert!(welcome.min_revision <= MIN_REVISION);
            assert_eq!(welcome.instance, "test-instance");
            // Mobile setup stays on the Machine's private socket; the remote
            // carrier answers it as an ordinary error and keeps the connection.
            for (id, operation) in [
                ("status", ServerOperation::MobileStatus),
                (
                    "configure",
                    ServerOperation::MobileConfigure {
                        configuration: None,
                    },
                ),
            ] {
                let operation = Operation::Server(operation);
                client
                    .send(text(&ClientMessage::Request {
                        select: None,
                        id: id.into(),
                        operation,
                        sender: None,
                        request_id: None,
                    }))
                    .await
                    .unwrap();
                let answer = receive(&mut client).await.unwrap();
                assert!(
                    matches!(&answer, ServerMessage::Error { id: Some(of), .. } if of == id),
                    "{answer:?}"
                );
            }
            // A Session without a held terminal is refused without starting a provider.
            client
                .send(text(&ClientMessage::Request {
                    select: None,
                    id: "terminal".into(),
                    operation: Operation::Server(ServerOperation::SessionAttach {
                        session: loopflow_model::id::LfSessionId::from("session_missing"),
                        terminal_id: None,
                    }),
                    sender: None,
                    request_id: None,
                }))
                .await
                .unwrap();
            assert!(matches!(
                receive(&mut client).await.unwrap(),
                ServerMessage::Error { id: Some(id), .. } if id == "terminal"
            ));
            // Binary frames belong to a bridged terminal, never to the control connection.
            client
                .send(Message::Binary(b"\x11\x00\x00\x00\x00".to_vec().into()))
                .await
                .unwrap();
            assert!(receive(&mut client).await.is_none(), "connection ends");
        });
    }

    #[tokio::test]
    async fn ticket_endpoint_delivers_one_bulk_message_to_the_issuing_device() {
        let tickets = Arc::new(Tickets::default());
        let phone = Admission::Remote {
            node: "phone".into(),
            user: 1,
        };
        let big = serde_json::to_string(&ServerMessage::Response {
            id: "file".into(),
            trace_id: loopflow_model::id::TraceId::default(),
            result: serde_json::Value::String("x".repeat(2 * MAX_MESSAGE)),
            revisions: None,
        })
        .unwrap();
        let ticket = tickets.issue(phone.clone(), download(big.clone()));
        // A ticket presented on the control endpoint admits nothing.
        let mut wrong = connect(&tickets, true, "/v1/machine", false).await.unwrap();
        wrong
            .send(text(&ClientMessage::Attach {
                ticket: ticket.clone(),
            }))
            .await
            .unwrap();
        assert!(receive(&mut wrong).await.is_none());
        let mut bulk = connect(&tickets, true, "/v1/ticket", false).await.unwrap();
        bulk.send(text(&ClientMessage::Attach { ticket }))
            .await
            .unwrap();
        assert!(matches!(
            receive(&mut bulk).await.unwrap(),
            ServerMessage::Admitted
        ));
        let Some(Ok(Message::Text(delivered))) = bulk.next().await else {
            panic!("expected the bulk message")
        };
        assert_eq!(delivered.as_str(), big);
        assert!(
            receive(&mut bulk).await.is_none(),
            "one message, then close"
        );
        // Another device's ticket is refused even on the right endpoint.
        let foreign = tickets.issue(
            Admission::Remote {
                node: "laptop".into(),
                user: 1,
            },
            download(big),
        );
        let mut refused = connect(&tickets, true, "/v1/ticket", false).await.unwrap();
        refused
            .send(text(&ClientMessage::Attach { ticket: foreign }))
            .await
            .unwrap();
        assert!(matches!(
            receive(&mut refused).await.unwrap(),
            ServerMessage::Error { id: None, .. }
        ));
        assert!(receive(&mut refused).await.is_none());
    }

    #[test]
    fn ticket_endpoint_bridges_one_terminal_and_any_foreign_attempt_spends_it() {
        in_empty_home(async {
            let tickets = Arc::new(Tickets::default());
            let phone = Admission::Remote {
                node: "phone".into(),
                user: 1,
            };
            let held = loopflow_model::id::TerminalId::new();
            let listener = tokio::net::UnixListener::bind(
                loopflow_execution::transport::registry::entrance().unwrap(),
            )
            .unwrap();
            let entrance = tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let (mut stream, _) = listener.accept().await.unwrap();
                while stream.read_u8().await.unwrap() != b'\n' {}
                let answer = format!(
                    "{{\"answer\":\"hosted\",\"revision\":{}}}\n",
                    loopflow_execution::transport::registry::REVISION
                );
                stream.write_all(answer.as_bytes()).await.unwrap();
                stream
            });
            let attach = |ticket: String| text(&ClientMessage::Attach { ticket });
            let ticket = tickets.issue(phone.clone(), Purpose::Terminal(held.clone()));
            let mut terminal = connect(&tickets, true, "/v1/ticket", false).await.unwrap();
            terminal.send(attach(ticket.clone())).await.unwrap();
            assert!(matches!(
                receive(&mut terminal).await.unwrap(),
                ServerMessage::Bridged { terminal_id } if terminal_id == held
            ));
            let _held = entrance.await.unwrap();
            // Replay, another node and a local ticket are refused and spent.
            let laptop = Admission::Remote {
                node: "laptop".into(),
                user: 1,
            };
            let foreign = tickets.issue(laptop.clone(), Purpose::Terminal(held.clone()));
            let local = tickets.issue(Admission::Local, Purpose::Terminal(held.clone()));
            for presented in [ticket, foreign.clone(), local.clone()] {
                let mut refused = connect(&tickets, true, "/v1/ticket", false).await.unwrap();
                refused.send(attach(presented)).await.unwrap();
                assert!(matches!(
                    receive(&mut refused).await.unwrap(),
                    ServerMessage::Error {
                        id: None,
                        code: ErrorCode::NotFound,
                        ..
                    }
                ));
                assert!(receive(&mut refused).await.is_none());
            }
            assert!(tickets.redeem(&foreign, &laptop).is_none());
            assert!(tickets.redeem(&local, &Admission::Local).is_none());
        });
    }
}
