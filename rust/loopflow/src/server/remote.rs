//! HTTPS on the host's tailnet addresses. Tailscale identifies the socket peer;
//! the mobile boundary never inherits the private Unix socket's authority.
//! One protocol message per WebSocket text frame; a bridged terminal carries
//! the owner protocol's bytes in binary frames.
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Poll};
use std::time::{Duration, SystemTime};

use super::mobile::{Configuration, Host, Status};
use super::service::{self, Carrier, Clients, Greeting};
use super::ticket::{Admission, Tickets};
use super::wire::MAX_BULK;
use anyhow::{Context, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
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

pub(super) async fn manage(
    initial: Result<Option<Configuration>>,
    mut requests: mpsc::Receiver<Control>,
    instance: Arc<str>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    tickets: Arc<Tickets>,
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
    let mut reported_failure = false;
    let mut retry = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()) };
                let (requested, reply) = match request {
                    Control::Status { reply } => { let _ = reply.send(status.clone()); continue; }
                    Control::Configure { configuration, reply } => (configuration, reply),
                };
                if status.ready {
                    if let (Some(requested), Some(current)) = (&requested, &status.configuration) {
                        if requested.owner == current.owner && requested.endpoint == current.endpoint {
                            let _ = reply.send(Ok(status.configuration.clone()));
                            continue;
                        }
                    }
                }
                // Persist off before replacing a listener. Close admission even
                // if persistence fails, and report that failure to the caller.
                let disabled = tokio::task::spawn_blocking(super::mobile::save_disabled).await?;
                listeners.abort_all();
                while listeners.join_next().await.is_some() {}
                status.ready = false;
                if let Some(config) = &mut status.configuration { config.enabled = false; }
                reported_failure = false;
                let result: Result<Option<Configuration>> = async {
                    status.configuration = disabled.context("Mobile access stopped, but disabled state could not be saved")?;
                    let Some(requested) = requested else { return Ok(None) };
                    let bound = bind(requested, true).await?;
                    let selected = bound.config.clone();
                    tokio::task::spawn_blocking(move || super::mobile::save(&selected)).await?
                        .context("Could not save mobile access; no listener was enabled")?;
                    status.configuration = Some(bound.config.clone());
                    status.ready = true;
                    listeners.spawn(run(bound, instance.clone(), executable.clone(), slots.clone(), clients.clone(), tickets.clone()));
                    Ok(status.configuration.clone())
                }.await;
                status.error = result.as_ref().err().map(|error| format!("{error:#}"));
                let _ = reply.send(result.map_err(|error| format!("{error:#}")));
            }
            ended = listeners.join_next(), if !listeners.is_empty() => {
                let error = match ended {
                    Some(Ok(Err(error))) => error.to_string(),
                    Some(Err(error)) => error.to_string(),
                    _ => "Mobile connection stopped; reconnecting".into(),
                };
                tracing::warn!(%error, "Mobile listener stopped; local service remains ready");
                status.ready = false;
                status.error = Some(error);
                reported_failure = false;
            }
            _ = retry.tick(), if !status.ready && status.configuration.as_ref().is_some_and(|config| config.enabled) => {
                if let Some(config) = status.configuration.clone() {
                    match bind(config, false).await {
                        Ok(bound) => {
                            status.configuration = Some(bound.config.clone());
                            status.ready = true;
                            status.error = None;
                            reported_failure = false;
                            listeners.spawn(run(bound, instance.clone(), executable.clone(), slots.clone(), clients.clone(), tickets.clone()));
                        }
                        Err(error) => {
                            if !reported_failure {
                                tracing::warn!(%error, "Mobile listener could not start; retrying the saved approval");
                                reported_failure = true;
                            }
                            status.error = Some(error.to_string());
                        }
                    }
                }
            }
        }
    }
}

struct Listener {
    sockets: Vec<TcpListener>,
    host: Host,
    config: Configuration,
    tls: TlsAcceptor,
}

async fn bind(mut config: Configuration, allow_port_change: bool) -> Result<Listener> {
    let expected = config.clone();
    let (host, addresses, tls) = tokio::task::spawn_blocking(move || {
        let host = Host::discover()?;
        let (owner, addresses) = host.identity()?;
        anyhow::ensure!(
            owner == expected.owner,
            "Tailscale account changed. Restore the intended account, or choose Set up again to approve the current account."
        );
        anyhow::ensure!(
            reqwest::Url::parse(&expected.endpoint)?.host_str() == Some(&host.dns),
            "Tailscale address changed. Choose Set up again, then scan the new QR code."
        );
        let tls = host.certificate()?;
        Ok::<_, anyhow::Error>((host, addresses, tls))
    })
    .await??;
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
    config.endpoint = format!("https://{}:{port}", host.dns);
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

async fn maintain(host: Host, config: Configuration, tls: mpsc::Sender<TlsAcceptor>) -> Result<()> {
    let mut renewed = SystemTime::now();
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let host = host.clone();
        let refresh = renewed.elapsed().unwrap_or_default() >= Duration::from_secs(6 * 60 * 60);
        let (owner, certificate) = tokio::task::spawn_blocking(move || {
            let (owner, _) = host.identity()?;
            let certificate = if refresh {
                Some(host.certificate()?)
            } else {
                None
            };
            Ok::<_, anyhow::Error>((owner, certificate))
        })
        .await??;
        anyhow::ensure!(
            owner == config.owner,
            "Tailscale account changed. Restore the intended account, or choose Set up again to approve the current account."
        );
        if let Some(certificate) = certificate {
            tls.send(certificate).await?;
            renewed = SystemTime::now();
        }
    }
}

async fn run(
    bound: Listener,
    instance: Arc<str>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    tickets: Arc<Tickets>,
) -> Result<()> {
    tracing::info!(endpoint = %bound.config.endpoint, "Mobile HTTPS ready on host Tailscale");
    let mut connections = JoinSet::new();
    let (renewed, mut renewals) = mpsc::channel(1);
    let maintenance = maintain(bound.host.clone(), bound.config.clone(), renewed);
    tokio::pin!(maintenance);
    let mut tls = bound.tls;
    loop {
        let accepted = futures_util::future::select_all(
            bound.sockets.iter().map(|socket| Box::pin(socket.accept())),
        );
        let (socket, peer) = tokio::select! {
            (accepted, _, _) = accepted => accepted?,
            _ = connections.join_next(), if !connections.is_empty() => continue,
            result = &mut maintenance => return result,
            Some(certificate) = renewals.recv() => { tls = certificate; continue; }
        };
        let slots = slots.clone();
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
                let admission =
                    tokio::task::spawn_blocking(move || host.permits(peer, &owner)).await??;
                let socket =
                    tokio::time::timeout(Duration::from_secs(5), tls.accept(socket)).await??;
                connection(
                    socket,
                    admission,
                    &instance,
                    &executable,
                    &clients,
                    tickets,
                    &slots,
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
#[allow(clippy::result_large_err)]
async fn connection<S>(
    socket: S,
    admission: Option<Admission>,
    instance: &str,
    executable: &str,
    clients: &Clients,
    tickets: Arc<Tickets>,
    slots: &Arc<Semaphore>,
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
        Greeting::Session(_) if ticket_path => Ok(()),
        // Remote connections never configure mobile access.
        Greeting::Session(listed) => {
            service::session(
                &mut messages,
                instance.into(),
                &listed,
                None,
                admission,
                tickets,
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

/// A plain byte copy between the owner and the WebSocket's binary frames.
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
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, UnixStream};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

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
        // The Machine may keep its read half alive while the owner has ended.
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
                &Arc::new(tokio::sync::Semaphore::new(
                    crate::server::wire::MAX_CONNECTIONS,
                )),
            )
            .await;
        });
        Ok(tokio_tungstenite::client_async(request, client).await?.0)
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

    #[tokio::test]
    async fn only_owner_identity_reaches_shared_operations_on_one_connection() {
        let tickets = Arc::new(Tickets::default());
        for (permitted, path, origin) in [
            (false, "/v1/machine", false),
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
                ServerOperation::ConfigureRemote {
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
                }))
                .await
                .unwrap();
            let answer = receive(&mut client).await.unwrap();
            assert!(
                matches!(&answer, ServerMessage::Error { id: Some(of), .. } if of == id),
                "{answer:?}"
            );
        }
        // A Session without an owner is refused without starting a provider.
        client
            .send(text(&ClientMessage::Request {
                select: None,
                id: "terminal".into(),
                operation: Operation::Server(ServerOperation::SessionAttach {
                    session: crate::id::LfSessionId::from("session_missing"),
                    owner_lf_process_id: None,
                }),
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
            trace_id: crate::id::TraceId::default(),
            result: serde_json::Value::String("x".repeat(2 * MAX_MESSAGE)),
        })
        .unwrap();
        let ticket = tickets.issue(phone.clone(), Purpose::Download(big.clone()));
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
            Purpose::Download(big),
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
}
