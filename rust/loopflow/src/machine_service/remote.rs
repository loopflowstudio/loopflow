//! HTTPS on the host's tailnet addresses. Tailscale identifies the socket peer;
//! the mobile boundary never inherits the private Unix socket's authority.
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use super::mobile::{Configuration, Host};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
use tokio::net::TcpListener;
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

const CHUNK: usize = 64 * 1024;

#[derive(Debug)]
pub(super) struct Control {
    pub configuration: Option<Configuration>,
    pub reply: oneshot::Sender<Result<Option<Configuration>, String>>,
}

pub(super) async fn manage(
    initial: Option<Configuration>,
    mut requests: mpsc::Receiver<Control>,
    instance: Arc<String>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
) -> Result<()> {
    let mut desired = initial;
    let mut active: Option<Configuration> = None;
    let mut listeners = JoinSet::new();
    let mut reported_failure = false;
    let mut retry = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else { return Ok(()) };
                if let (Some(requested), Some(current)) = (&request.configuration, &active) {
                    if requested.owner == current.owner && requested.endpoint == current.endpoint {
                        let _ = request.reply.send(Ok(active.clone()));
                        continue;
                    }
                }
                listeners.abort_all();
                while listeners.join_next().await.is_some() {}
                active = None;
                desired = request.configuration;
                reported_failure = false;
                let result = if let Some(config) = desired.clone() {
                    match bind(config).await {
                        Ok(bound) => {
                            active = Some(bound.config.clone());
                            desired = active.clone();
                            listeners.spawn(run(bound, instance.clone(), executable.clone(), slots.clone()));
                            Ok(active.clone())
                        }
                        Err(error) => { desired = None; Err(error.to_string()) }
                    }
                } else { Ok(None) };
                let _ = request.reply.send(result);
            }
            ended = listeners.join_next(), if !listeners.is_empty() => {
                tracing::warn!(?ended, "Mobile listener stopped; local service remains ready");
                active = None;
                reported_failure = false;
            }
            _ = retry.tick(), if active.is_none() && desired.is_some() => {
                if let Some(config) = desired.clone() {
                    match bind(config).await {
                        Ok(bound) => {
                            active = Some(bound.config.clone());
                            desired = active.clone();
                            reported_failure = false;
                            listeners.spawn(run(bound, instance.clone(), executable.clone(), slots.clone()));
                        }
                        Err(error) if !reported_failure => {
                            tracing::warn!(%error, "Mobile listener could not start; retrying the saved approval");
                            reported_failure = true;
                        }
                        Err(_) => {}
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

async fn bind(mut config: Configuration) -> Result<Listener> {
    let expected = config.clone();
    let (host, addresses, tls) = tokio::task::spawn_blocking(move || {
        let host = Host::discover()?;
        let (owner, addresses) = host.identity()?;
        anyhow::ensure!(
            owner == expected.owner,
            "Tailscale account changed; enable mobile access again"
        );
        anyhow::ensure!(
            reqwest::Url::parse(&expected.endpoint)?.host_str() == Some(&host.dns),
            "Tailscale address changed; enable mobile access again"
        );
        let tls = host.certificate()?;
        Ok::<_, anyhow::Error>((host, addresses, tls))
    })
    .await??;
    let mut port = config.port()?;
    let mut sockets = Vec::new();
    for ip in addresses {
        // Never bind wildcard or LAN addresses. The exact addresses come from
        // the installed Tailscale daemon, not a caller's hostname or HTTP header.
        let socket = match TcpListener::bind((ip, port)).await {
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
    config.endpoint = format!("https://{}:{port}", host.dns);
    config.enabled = true;
    Ok(Listener {
        sockets,
        host,
        config,
        tls,
    })
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
            "Tailscale account changed; enable mobile access again"
        );
        if let Some(certificate) = certificate {
            tls.send(certificate).await?;
            renewed = SystemTime::now();
        }
    }
}

async fn run(
    bound: Listener,
    instance: Arc<String>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
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
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            continue;
        };
        let (host, owner, tls, instance, executable) = (
            bound.host.clone(),
            bound.config.owner.clone(),
            tls.clone(),
            instance.clone(),
            executable.clone(),
        );
        connections.spawn(async move {
            let _permit = permit;
            let result = async {
                let permitted =
                    tokio::task::spawn_blocking(move || host.permits(peer, &owner)).await??;
                let socket =
                    tokio::time::timeout(Duration::from_secs(5), tls.accept(socket)).await??;
                connection(socket, permitted, &instance, &executable).await
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

// Tungstenite's handshake callback requires an unboxed HTTP ErrorResponse.
#[allow(clippy::result_large_err)]
async fn connection<S>(socket: S, permitted: bool, instance: &str, executable: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let config = WebSocketConfig::default()
        .max_message_size(Some(CHUNK))
        .max_frame_size(Some(CHUNK));
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
                if request.uri().path() != "/v1/machine" {
                    return Err(reject(StatusCode::NOT_FOUND));
                }
                Ok(response)
            },
            Some(config),
        ),
    )
    .await??;
    let (client, server) = tokio::io::duplex(CHUNK);
    tokio::try_join!(
        super::service::connection(server, instance, executable, None),
        bridge(socket, client)
    )?;
    Ok(())
}

async fn bridge<S>(socket: WebSocketStream<S>, stream: DuplexStream) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sender, mut receiver) = socket.split();
    let (mut reader, mut writer) = tokio::io::split(stream);
    let upstream = async {
        while let Some(message) = receiver.next().await {
            match message? {
                Message::Binary(bytes) => writer.write_all(&bytes).await?,
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {}
                _ => anyhow::bail!("Machine carrier requires binary messages"),
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
    result?;
    closed??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bridge, connection};
    use crate::machine_service::wire::{Operation, Request, Response, VERSION};
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, UnixStream};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    #[tokio::test]
    async fn completed_machine_stream_sends_websocket_close_after_its_last_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stream, mut machine) = tokio::io::duplex(1024);
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
            bridge(websocket, stream).await.unwrap();
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
        permitted: bool,
        path: &str,
        origin: bool,
    ) -> anyhow::Result<tokio_tungstenite::WebSocketStream<UnixStream>> {
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
            let _ = connection(server, permitted, "test-instance", "test-executable").await;
        });
        Ok(tokio_tungstenite::client_async(request, client).await?.0)
    }

    #[tokio::test]
    async fn only_owner_identity_can_reach_shared_operations() {
        for (permitted, path, origin) in [
            (false, "/v1/machine", false),
            (true, "/v1/pair", false),
            (true, "/v1/machine?token=old", false),
            (true, "/v1/machine", true),
        ] {
            assert!(connect(permitted, path, origin).await.is_err());
        }
        for (version, operation, expected) in [
            (VERSION + 1, Operation::Bootstrap, "incompatible"),
            (
                VERSION,
                Operation::ConfigureRemote {
                    configuration: None,
                },
                "unsupported",
            ),
        ] {
            let mut client = connect(true, "/v1/machine", false).await.unwrap();
            let request = serde_json::to_vec(&Request {
                version,
                id: "fragmented".into(),
                operation,
            })
            .unwrap();
            let mut framed = (request.len() as u32).to_be_bytes().to_vec();
            framed.extend(request);
            for chunk in framed.chunks(3) {
                client
                    .send(Message::Binary(chunk.to_vec().into()))
                    .await
                    .unwrap();
            }
            let mut received = Vec::new();
            loop {
                if received.len() >= 4 {
                    let length = u32::from_be_bytes(received[..4].try_into().unwrap()) as usize;
                    if received.len() == length + 4 {
                        break;
                    }
                }
                let message = tokio::time::timeout(Duration::from_secs(5), client.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert!(matches!(message, Message::Binary(_)));
                received.extend(message.into_data());
            }
            let response: Response = serde_json::from_slice(&received[4..]).unwrap();
            assert_eq!(response.id, "fragmented");
            assert_eq!(response.instance, "test-instance");
            assert_eq!(
                serde_json::to_value(&response).unwrap()["result"]["code"],
                expected
            );
        }
    }
}
