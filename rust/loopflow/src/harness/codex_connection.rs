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
    /// Read all native turn pages before displaying the conversation. This
    /// connection never subscribes, answers approvals, or acquires a driver.
    pub async fn recover_history(&self, engine: &Path) -> Result<()> {
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(engine).await?).await?;
        read_rpc(
            &mut upstream,
            "initialize",
            json!({
                "clientInfo": {"name":"loopflow_history", "version":env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi":true}
            }),
        )
        .await?;
        upstream
            .send(Message::Text(
                json!({"method":"initialized"}).to_string().into(),
            ))
            .await?;
        let mut history = super::codex_history::History::default();
        let mut cursor = Value::Null;
        loop {
            let result = read_rpc(
                &mut upstream,
                "thread/turns/list",
                json!({
                    "threadId":self.thread_id, "cursor":cursor, "limit":100,
                    "sortDirection":"asc", "itemsView":"notLoaded"
                }),
            )
            .await?;
            history.record(
                &self.store,
                &self.session_id,
                None,
                Some(&self.thread_id),
                &json!({"result":result}),
            )?;
            let next = result.get("nextCursor").cloned().unwrap_or(Value::Null);
            if next.is_null() {
                break;
            }
            if next == cursor {
                return Err(anyhow!("Native history returned a repeated cursor"));
            }
            cursor = next;
        }
        upstream.close(None).await?;
        Ok(())
    }

    pub async fn serve(&self, client: UnixStream, engine: &Path) -> Result<()> {
        let mut client = accept_async(client).await?;
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(engine).await?).await?;
        let mut history = super::codex_history::History::default();
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
                    history.request(&rpc);
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
                        Message::Text(text) => {
                            let rpc: Value = serde_json::from_str(&text)?;
                            history.record(&self.store, &self.session_id,
                                self.driver.as_ref(), Some(&self.thread_id), &rpc)?;
                            client.send(Message::Text(text)).await?;
                        }
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

async fn read_rpc(
    upstream: &mut WebSocketStream<UnixStream>,
    method: &str,
    params: Value,
) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    upstream
        .send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))
        .await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(message) = upstream.next().await {
            let Message::Text(text) = message? else {
                continue;
            };
            let rpc: Value = serde_json::from_str(&text)?;
            if rpc["id"].as_str() != Some(&id) {
                continue;
            }
            if let Some(error) = rpc.get("error") {
                return Err(anyhow!("Native history {method} failed: {error}"));
            }
            return rpc
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("Native history reply has no result"));
        }
        Err(anyhow!("Native history connection ended"))
    })
    .await
    .map_err(|_| anyhow!("Native history {method} timed out"))?
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
