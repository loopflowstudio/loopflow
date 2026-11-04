//! Host Tailscale Serve terminates HTTPS. This loopback-only carrier admits
//! explicit device grants, then dispatches the same bounded Machine stream.
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use secrecy::{ExposeSecret, SecretString};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio_tungstenite::tungstenite::{
    handshake::server::{ErrorResponse, Request, Response},
    http::StatusCode,
    protocol::WebSocketConfig,
    Message,
};
use tokio_tungstenite::{accept_hdr_async_with_config, WebSocketStream};

use super::admission::{self, DeviceId};
use crate::store::sqlite::SqliteStore;

const CHUNK: usize = 64 * 1024;

pub(super) async fn run(
    port: u16,
    instance: Arc<String>,
    executable: Arc<String>,
    slots: Arc<Semaphore>,
) -> Result<()> {
    let store = Arc::new(admission::store().await?);
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    tracing::info!(address = %listener.local_addr()?, "Mobile carrier ready for host Tailscale Serve");
    loop {
        let (socket, _) = listener.accept().await?;
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            continue;
        };
        let (store, instance, executable) = (store.clone(), instance.clone(), executable.clone());
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(socket, store, &instance, &executable).await {
                tracing::debug!(%error, "Mobile connection ended");
            }
        });
    }
}

fn reject(status: StatusCode) -> ErrorResponse {
    Response::builder()
        .status(status)
        .body(Some("Machine admission rejected".into()))
        .expect("fixed HTTP response is valid")
}

// Tungstenite's handshake callback requires an unboxed HTTP ErrorResponse.
#[allow(clippy::result_large_err)]
async fn connection(
    socket: TcpStream,
    store: Arc<SqliteStore>,
    instance: &str,
    executable: &str,
) -> Result<()> {
    let mut credential: Option<SecretString> = None;
    let mut pairing = false;
    let config = WebSocketConfig::default()
        .max_message_size(Some(CHUNK))
        .max_frame_size(Some(CHUNK));
    let mut socket = tokio::time::timeout(
        Duration::from_secs(5),
        accept_hdr_async_with_config(
            socket,
            |request: &Request, response: Response| {
                // Native clients carry an Authorization header. Browser origins and
                // URL/query credentials are intentionally not an admission path.
                if request.headers().contains_key("origin") || request.uri().query().is_some() {
                    return Err(reject(StatusCode::FORBIDDEN));
                }
                pairing = match request.uri().path() {
                    "/v1/pair" => true,
                    "/v1/machine" => false,
                    _ => return Err(reject(StatusCode::NOT_FOUND)),
                };
                let value = request
                    .headers()
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.strip_prefix("Bearer "))
                    .filter(|value| {
                        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    .ok_or_else(|| reject(StatusCode::UNAUTHORIZED))?;
                credential = Some(SecretString::new(value.into()));
                Ok(response)
            },
            Some(config),
        ),
    )
    .await??;
    let hash = admission::hash(
        credential
            .context("Missing mobile credential")?
            .expose_secret(),
    );
    if pairing {
        let token = admission::secret();
        let token_hash = admission::hash(token.expose_secret());
        let id = DeviceId(uuid::Uuid::new_v4().to_string());
        let device = tokio::task::spawn_blocking(move || {
            store.redeem_machine_pairing(
                &hash,
                &token_hash,
                &id,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
        })
        .await??;
        if let Some(device) = device {
            let reply = serde_json::to_vec(&serde_json::json!({
                "version": 1, "device": device, "token": token.expose_secret()
            }))?;
            tokio::time::timeout(
                Duration::from_secs(5),
                socket.send(Message::Binary(reply.into())),
            )
            .await??;
        }
        tokio::time::timeout(Duration::from_secs(5), socket.close(None)).await??;
        return Ok(());
    }
    let authority = store.clone();
    let device =
        tokio::task::spawn_blocking(move || authority.admitted_machine_device(&hash)).await??;
    let Some(device) = device else {
        tokio::time::timeout(Duration::from_secs(5), socket.close(None)).await??;
        return Ok(());
    };
    let (client, server) = tokio::io::duplex(CHUNK);
    // No proxy connection to the trusted Unix listener. Admission above is the
    // authority for this dispatch. Cancellation drops only this viewer's stream.
    tokio::select! {
        result = async {
            tokio::try_join!(super::service::connection(server, instance, executable), bridge(socket, client))?;
            Ok(())
        } => result,
        result = until_revoked(store, device) => result,
    }
}

async fn until_revoked(store: Arc<SqliteStore>, device: DeviceId) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let (store, device) = (store.clone(), device.clone());
        let check = tokio::task::spawn_blocking(move || store.machine_device_active(&device));
        let active = tokio::time::timeout(Duration::from_secs(5), check).await???;
        anyhow::ensure!(active, "Device grant revoked");
    }
}

async fn bridge(socket: WebSocketStream<TcpStream>, stream: DuplexStream) -> Result<()> {
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
    tokio::select! {
        result = upstream => result,
        result = downstream => result,
    }
}

#[cfg(test)]
mod tests {
    use super::{admission, connection, until_revoked, DeviceId, TcpListener, TcpStream};
    use futures_util::{SinkExt, StreamExt};
    use secrecy::ExposeSecret;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    use tokio_tungstenite::WebSocketStream;

    use crate::machine_service::wire::{ErrorCode, Operation, Reply, Request, Response, VERSION};
    use crate::store::sqlite::SqliteStore;

    async fn connect(
        store: Arc<SqliteStore>,
        path: &str,
        credential: &str,
        origin: bool,
    ) -> anyhow::Result<WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let mut request = format!("ws://{}{path}", listener.local_addr()?).into_client_request()?;
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {credential}").parse()?);
        if origin {
            request
                .headers_mut()
                .insert("origin", "https://example.com".parse()?);
        }
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            connection(socket, store, "test-instance", "test-executable").await
        });
        let client = tokio_tungstenite::connect_async(request).await;
        // Keep the server alive after connection; it owns only this test's socket.
        drop(server);
        Ok(client?.0)
    }

    #[tokio::test]
    async fn one_use_pairing_admits_framed_requests_and_invalid_grants_get_no_work() {
        let temp = tempfile::tempdir().unwrap();
        let store = Arc::new(SqliteStore::open_ephemeral(&temp.path().join("test.db")).unwrap());
        let ticket = admission::secret();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        store
            .create_machine_pairing(
                &admission::hash(ticket.expose_secret()),
                "Test phone",
                now,
                now + 600,
            )
            .unwrap();
        let mut pair = connect(store.clone(), "/v1/pair", ticket.expose_secret(), false)
            .await
            .unwrap();
        let bytes = pair.next().await.unwrap().unwrap().into_data();
        let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = reply["token"].as_str().unwrap();
        assert_eq!(reply["version"], 1);
        assert_eq!(reply["device"]["name"], "Test phone");
        drop(pair);

        let mut reused = connect(store.clone(), "/v1/pair", ticket.expose_secret(), false)
            .await
            .unwrap();
        assert!(matches!(reused.next().await, Some(Ok(Message::Close(_)))));
        let mut denied = connect(store.clone(), "/v1/machine", ticket.expose_secret(), false)
            .await
            .unwrap();
        assert!(matches!(denied.next().await, Some(Ok(Message::Close(_)))));
        assert!(connect(store.clone(), "/v1/machine", token, true)
            .await
            .is_err());

        let mut client = connect(store.clone(), "/v1/machine", token, false)
            .await
            .unwrap();
        let request = serde_json::to_vec(&Request {
            version: VERSION + 1,
            id: "fragmented".into(),
            operation: Operation::Bootstrap,
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
        assert!(matches!(
            response.result,
            Reply::Error {
                code: ErrorCode::Incompatible,
                ..
            }
        ));

        let id = DeviceId(reply["device"]["id"].as_str().unwrap().into());
        store.revoke_machine_device(&id, now).unwrap();
        let mut revoked = connect(store.clone(), "/v1/machine", token, false)
            .await
            .unwrap();
        assert!(matches!(revoked.next().await, Some(Ok(Message::Close(_)))));
        assert!(until_revoked(store, id).await.is_err());
    }
}
