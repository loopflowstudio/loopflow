//! A native client may keep displaying a conversation after driver transfer.
//! Its writes must still pass the Session fence at dispatch, including approval
//! replies. This connection forwards to the existing engine; it never starts,
//! stops or claims one. The caller owns the private socket and its lifetime.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio_tungstenite::{accept_async, client_async, tungstenite::Message, WebSocketStream};

use crate::exec::SessionDriver;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

/// One already selected conversation. `driver=None` is a passive display;
/// accepting its connection does not acquire a claim.
#[derive(Debug, Clone)]
pub struct CodexConnection {
    pub store: SqliteStore,
    pub session_id: String,
    pub thread_id: String,
    pub driver: Option<SessionDriver>,
}

impl CodexConnection {
    pub async fn serve(&self, client: UnixStream, engine: &Path) -> Result<()> {
        let mut client = accept_async(client).await?;
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(engine).await?).await?;
        loop {
            tokio::select! {
                incoming = client.next() => {
                    let Some(incoming) = incoming else { break };
                    let Message::Text(text) = incoming? else { continue };
                    let mut rpc: Value = serde_json::from_str(&text)?;
                    let method = rpc.get("method").and_then(Value::as_str).unwrap_or_default();
                    let target = rpc.pointer("/params/threadId").and_then(Value::as_str);
                    let wrong_thread = target.is_some_and(|id| id != self.thread_id);
                    // Creating another thread is another conversation admission.
                    let admission = matches!(method, "thread/start" | "thread/fork");
                    if wrong_thread || admission {
                        reject(&mut client, &rpc, "Client belongs to another conversation").await?;
                        continue;
                    }
                    let passive = match method {
                        "initialize" | "initialized" => true,
                        "thread/read" => target == Some(self.thread_id.as_str()),
                        "thread/resume" => {
                            // Subscribe to the existing thread. Do not forward
                            // config/cwd overrides from a passive or stale UI.
                            rpc["params"] = json!({"threadId": self.thread_id});
                            true
                        }
                        _ => false,
                    };
                    let message = Message::Text(serde_json::to_string(&rpc)?.into());
                    if passive {
                        upstream.send(message).await?;
                    } else {
                        let outcome;
                        (upstream, outcome) = self.send(upstream, message).await?;
                        match outcome {
                            Ok(()) => {},
                            Err(StoreError::InvalidAuthority(reason)) => {
                                reject(&mut client, &rpc, &reason).await?;
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                }
                outgoing = upstream.next() => {
                    let Some(outgoing) = outgoing else { break };
                    match outgoing? {
                        Message::Close(_) => break,
                        message => client.send(message).await?,
                    }
                }
            }
        }
        Ok(())
    }

    async fn send(
        &self,
        mut upstream: WebSocketStream<UnixStream>,
        message: Message,
    ) -> Result<(WebSocketStream<UnixStream>, StoreResult<()>)> {
        let Some(driver) = self.driver.clone() else {
            return Ok((
                upstream,
                Err(StoreError::InvalidAuthority(
                    "Passive display has no driver claim".into(),
                )),
            ));
        };
        let store = self.store.clone();
        let session = self.session_id.clone();
        let runtime = tokio::runtime::Handle::current();
        // A second connection can transfer the driver in another process.
        // Keep the SQLite comparison and bounded socket dispatch in one
        // transaction, rather than checking before an asynchronous queue.
        tokio::task::spawn_blocking(move || {
            let outcome = store.with_session_driver(&session, &driver, || {
                runtime.block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), upstream.send(message))
                        .await
                        .map_err(|_| {
                            StoreError::InvalidData(
                                "Native dispatch timed out; outcome is unknown".into(),
                            )
                        })?
                        .map_err(|error| {
                            StoreError::InvalidData(format!("Native dispatch failed: {error}"))
                        })
                })
            });
            (upstream, outcome)
        })
        .await
        .map_err(|error| anyhow!("Native dispatch task failed: {error}"))
    }
}

async fn reject(client: &mut WebSocketStream<UnixStream>, rpc: &Value, reason: &str) -> Result<()> {
    // Approval replies and notifications have no request to answer. Dropping
    // them is essential: forwarding an old approval would still mutate a turn.
    if rpc.get("method").is_some() && rpc.get("id").is_some() {
        client
            .send(Message::Text(
                json!({
                    "id": rpc["id"],
                    "error": {"code": -32001, "message": reason},
                })
                .to_string()
                .into(),
            ))
            .await?;
    }
    Ok(())
}
