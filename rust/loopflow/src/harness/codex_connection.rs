//! A native client may keep displaying a conversation after attachment transfer.
//! Its writes must still pass the Session fence at dispatch, including approval
//! replies. The same transport relays to the existing AgentProcess and inspects
//! it during attachment settlement, under the ownership fence.

use crate::id::AgentSessionId;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio_tungstenite::{accept_async, client_async, tungstenite::Message, WebSocketStream};

use crate::process::SessionAttachment;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

/// Refuse shutdown if the saved endpoint serves an unrelated conversation.
/// The caller holds the Session attachment lock through inspection and shutdown;
/// this check neither signals the provider nor changes its saved history.
pub(crate) fn validate_agent_process_close(
    endpoint: &str,
    thread: Option<&AgentSessionId>,
) -> Result<()> {
    // Use a separate runtime: exit is also reached from synchronous capture
    // settlement and signal cleanup, sometimes inside an existing runtime.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(async {
                        tokio::time::timeout(
                            Duration::from_secs(3),
                            inspect_agent_threads(endpoint, thread),
                        )
                        .await
                        .map_err(|_| anyhow!("AgentProcess inspection timed out"))?
                    })
            })
            .join()
            .map_err(|_| anyhow!("AgentProcess close worker panicked"))?
    })
}

async fn inspect_agent_threads(endpoint: &str, thread: Option<&AgentSessionId>) -> Result<()> {
    let socket = match UnixStream::connect(endpoint).await {
        Ok(socket) => socket,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(())
        }
        Err(error) => return Err(error.into()),
    };
    let (mut upstream, _) = client_async("ws://localhost", socket).await?;
    rpc_request(
        &mut upstream,
        "initialize",
        json!({"clientInfo": {"name":"loopflow_close", "version":env!("CARGO_PKG_VERSION")}}),
    )
    .await?;
    let mut cursor = Value::Null;
    loop {
        let loaded = rpc_request(
            &mut upstream,
            "thread/loaded/list",
            json!({"cursor":cursor,"limit":100}),
        )
        .await?;
        let threads = loaded["data"]
            .as_array()
            .ok_or_else(|| anyhow!("loaded threads response has no data"))?;
        for id in threads {
            let mut current = id
                .as_str()
                .ok_or_else(|| anyhow!("loaded thread has no ID"))?
                .to_owned();
            let mut ancestors = std::collections::HashSet::new();
            while Some(current.as_str()) != thread.map(AgentSessionId::as_str) {
                if !ancestors.insert(current.clone()) {
                    return Err(anyhow!("provider thread parent cycle"));
                }
                let detail =
                    rpc_request(&mut upstream, "thread/read", json!({"threadId":current})).await?;
                // Codex's own subagents are part of this AgentProcess's work. A
                // separately started conversation must survive this exit.
                current = detail
                    .pointer("/thread/parentThreadId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow!(
                            "AgentProcess still serves another conversation; leaving it running"
                        )
                    })?
                    .to_owned();
            }
        }
        let next = loaded.get("nextCursor").cloned().unwrap_or(Value::Null);
        if next.is_null() {
            break;
        }
        if next == cursor {
            return Err(anyhow!("loaded threads returned a repeated cursor"));
        }
        cursor = next;
    }
    upstream.close(None).await?;
    Ok(())
}

/// One already selected conversation. `attachment=None` is a passive display;
/// accepting its connection does not acquire a claim.
#[derive(Debug, Clone)]
pub struct CodexConnection {
    pub store: SqliteStore,
    pub session_id: String,
    pub thread_id: Option<AgentSessionId>,
    pub attachment: Option<SessionAttachment>,
    pub account_id: Option<crate::store::ProviderAccountId>,
}

#[derive(Debug)]
pub(crate) struct NativeInput {
    pub params: Option<Value>,
    pub text: Option<String>,
}

impl CodexConnection {
    /// Read all native turn pages before displaying the conversation. This
    /// connection never subscribes, answers approvals, or acquires an attachment.
    pub async fn recover_history(&self, endpoint: &Path) -> Result<bool> {
        let thread = self
            .thread_id
            .as_ref()
            .ok_or_else(|| anyhow!("Conversation has no native history yet"))?;
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(endpoint).await?).await?;
        rpc_request(
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
        let mut empty = true;
        loop {
            let result = rpc_request(
                &mut upstream,
                "thread/turns/list",
                json!({
                    "threadId":self.thread_id, "cursor":cursor, "limit":100,
                    "sortDirection":"asc", "itemsView":"notLoaded"
                }),
            )
            .await?;
            empty &= result["data"]
                .as_array()
                .ok_or_else(|| anyhow!("Native history response has no turns"))?
                .is_empty();
            super::dispatch::off_reactor(|| {
                history.record(
                    &self.store,
                    &self.session_id,
                    None,
                    Some(thread),
                    &json!({"result":result}),
                )
            })?;
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
        Ok(empty)
    }

    pub async fn serve(&self, client: UnixStream, endpoint: &Path) -> Result<()> {
        self.serve_with_input(client, endpoint, None).await
    }

    /// Submit the saved first input on the native client's connection, after it
    /// subscribes. Approval requests then belong to that client, never a headless
    /// auto-approver. All connections in this launch share the one pending input.
    pub(crate) async fn serve_with_input(
        &self,
        client: UnixStream,
        endpoint: &Path,
        initial: Option<Arc<tokio::sync::Mutex<NativeInput>>>,
    ) -> Result<()> {
        let mut client = accept_async(client).await?;
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(endpoint).await?).await?;
        let mut history = super::codex_history::History::default();
        let mut resume_request = None;
        let mut bind_thread = false;
        let mut initial_request = None;
        let mut thread = self.thread_id.clone();
        loop {
            tokio::select! {
                incoming = client.next() => {
                    let Some(incoming) = incoming else { break };
                    let Message::Text(text) = incoming? else { continue };
                    let mut rpc: Value = serde_json::from_str(&text)?;
                    let method = rpc.get("method").and_then(Value::as_str).unwrap_or_default();
                    let target = rpc.pointer("/params/threadId").and_then(Value::as_str);
                    let wrong_thread = target.is_some_and(|id| Some(id) != thread.as_ref().map(AgentSessionId::as_str));
                    let starting = method == "thread/start" && thread.is_none();
                    if wrong_thread || method == "thread/fork" || (method == "thread/start" && !starting) {
                        reject(&mut client, &rpc, "Client belongs to another conversation").await?;
                        continue;
                    }
                    let passive = match method {
                        "initialize" | "initialized" => true,
                        "thread/read" => target == thread.as_ref().map(AgentSessionId::as_str),
                        "thread/start" | "thread/resume" => {
                            let params = match &initial {
                                Some(initial) => initial.lock().await.params.take(),
                                None => None,
                            };
                            if starting && params.is_none() {
                                reject(&mut client, &rpc, "Conversation startup was already submitted").await?;
                                continue;
                            }
                            let passive = params.is_none();
                            bind_thread = !passive;
                            // A passive or stale UI can only subscribe. An owned
                            // startup uses the prepared payload, never argv data.
                            rpc["params"] = params.unwrap_or_else(|| json!({"threadId":thread}));
                            resume_request = rpc.get("id").cloned();
                            passive
                        }
                        _ => false,
                    };
                    match history.request(&rpc, self.attachment.as_ref().map(|attachment| (&self.store,self.session_id.as_str(),attachment))) {
                        Ok(()) => {},
                        Err(StoreError::InvalidAuthority(reason)) => {
                            reject(&mut client, &rpc, &reason).await?;
                            continue;
                        }
                        Err(error) => return Err(error.into()),
                    }
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
                            let resumed = resume_request.as_ref().is_some_and(|id| rpc.get("id") == Some(id))
                                && rpc.get("result").is_some();
                            if resumed {
                                let selected = rpc.pointer("/result/thread/id").and_then(Value::as_str)
                                    .ok_or_else(|| anyhow!("Codex returned no native conversation ID"))?;
                                if thread.as_ref().is_some_and(|thread| thread.as_str() != selected) {
                                    return Err(anyhow!("Codex resumed another conversation"));
                                }
                                thread = Some(selected.into());
                                if bind_thread {
                                    if let Some(attachment) = &self.attachment {
                                        self.store.record_session_connection(&self.session_id, attachment,
                                            &endpoint.to_string_lossy(), thread.as_ref().expect("selected thread"))?;
                                    }
                                    bind_thread = false;
                                }
                            }
                            if rpc["method"] == "turn/started" && initial.is_some() {
                                if let Some(thread) = &thread {
                                    let session = self.store.session(&self.session_id)?
                                        .ok_or_else(|| anyhow!("Session disappeared during native startup"))?;
                                    crate::session_record::write_provider_session(
                                        &crate::session_record::capture_dir(&session.artifact_key)?, thread, self.account_id.clone())?;
                                }
                            }
                            super::dispatch::off_reactor(|| {
                                history.record(&self.store, &self.session_id,
                                    self.attachment.as_ref(), thread.as_ref(), &rpc)
                            })?;
                            if initial_request.as_ref().is_some_and(|id| rpc.get("id") == Some(id)) {
                                initial_request = None;
                                if let Some(error) = rpc.get("error") {
                                    return Err(anyhow!("Codex rejected the initial request: {error}"));
                                }
                                continue;
                            }
                            client.send(Message::Text(text)).await?;
                            if resumed {
                                resume_request = None;
                                if let Some(initial) = &initial {
                                    let mut initial = initial.lock().await;
                                    let turns = rpc.pointer("/result/thread/turns").and_then(Value::as_array)
                                        .ok_or_else(|| anyhow!("Codex returned no native turn history"))?;
                                    if !turns.is_empty() { initial.text = None; }
                                    if let Some(text) = initial.text.take() {
                                        let id = json!(format!("lf-initial-{}", uuid::Uuid::new_v4()));
                                        let request = json!({"id":id,"method":"turn/start","params":{
                                            "threadId":thread,"input":[{"type":"text","text":text}]
                                        }});
                                        history.request(&request, self.attachment.as_ref().map(|attachment| (&self.store,self.session_id.as_str(),attachment)))?;
                                        let outcome;
                                        (upstream, outcome) = self.send(upstream, Message::Text(request.to_string().into())).await?;
                                        outcome?;
                                        initial_request = Some(id);
                                    }
                                }
                            }
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
        let Some(attachment) = self.attachment.clone() else {
            return Ok((
                upstream,
                Err(StoreError::InvalidAuthority(
                    "Passive display has no attachment claim".into(),
                )),
            ));
        };
        let store = self.store.clone();
        let session = self.session_id.clone();
        // A second connection can transfer the attachment in another process.
        // Keep the attachment comparison and bounded socket dispatch under the
        // Session lock. History and other Sessions can still use the database.
        tokio::task::spawn_blocking(move || {
            let outcome = store.with_session_dispatch(&session, &attachment, || {
                super::dispatch::send_fenced(&mut upstream, message)
            });
            (upstream, outcome)
        })
        .await
        .map_err(|error| anyhow!("Native dispatch task failed: {error}"))
    }
}

pub(crate) async fn rpc_request(
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
                return Err(anyhow!("Codex {method} failed: {error}"));
            }
            return rpc
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("Codex {method} reply has no result"));
        }
        Err(anyhow!("Codex connection ended during {method}"))
    })
    .await
    .map_err(|_| anyhow!("Codex {method} timed out"))?
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
