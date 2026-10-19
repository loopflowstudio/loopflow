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
use tokio_tungstenite::{
    accept_async_with_config, client_async_with_config,
    tungstenite::{error::ProtocolError, protocol::WebSocketConfig, Error, Message},
    WebSocketStream,
};

// Codex sends single frames larger than tungstenite's defaults: plugin/list
// carries the whole curated marketplace (7,000+ plugins, over 16 MiB). The
// relay forwards between two Codex endpoints and imposes no limit of its own.
fn relay_config() -> Option<WebSocketConfig> {
    Some(
        WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None),
    )
}

pub(super) async fn connect_upstream(
    socket: UnixStream,
) -> Result<WebSocketStream<UnixStream>, Error> {
    Ok(
        client_async_with_config("ws://localhost", socket, relay_config())
            .await?
            .0,
    )
}

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
    let mut upstream = connect_upstream(socket).await?;
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
    pub context: Option<crate::context_block::ContextDelivery>,
}

impl CodexConnection {
    /// Read all native turn pages before displaying the conversation. This
    /// connection never subscribes, answers approvals, or acquires an attachment.
    pub async fn recover_history(&self, endpoint: &Path) -> Result<()> {
        let thread = self
            .thread_id
            .as_ref()
            .ok_or_else(|| anyhow!("Conversation has no native history yet"))?;
        let mut upstream = connect_upstream(UnixStream::connect(endpoint).await?).await?;
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
            result["data"]
                .as_array()
                .ok_or_else(|| anyhow!("Native history response has no turns"))?;
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
        Ok(())
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
        let mut client = accept_async_with_config(client, relay_config()).await?;
        let mut upstream = connect_upstream(UnixStream::connect(endpoint).await?).await?;
        let mut history = super::codex_history::History::default();
        let mut resume_request = None;
        let mut bind_thread = false;
        let mut initial_request = None;
        let mut thread = self.thread_id.clone();
        loop {
            tokio::select! {
                incoming = client.next() => {
                    let Some(incoming) = incoming else { break };
                    let text = match client_result(incoming)? {
                        Some(Message::Text(text)) => text,
                        None | Some(Message::Close(_)) => break,
                        _ => continue,
                    };
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
                            // Keep native tools and explicitly selected UI settings;
                            // prepared instructions and launch configuration win.
                            // A passive or stale UI can only subscribe.
                            rpc["params"] = match params {
                                Some(params) => native_start_params(&rpc["params"], params),
                                None => json!({"threadId":thread}),
                            };
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
                                if let (Some(initial), Some(attachment)) = (&initial, &self.attachment) {
                                    let mut initial = initial.lock().await;
                                    if let Some(context) = &initial.context {
                                        // Hooks run on the app-server, which never inherits the
                                        // terminal's launch file. Bind before the UI can send a turn.
                                        self.store.with_session_attachment(&self.session_id, attachment, || {
                                            Ok(context.bind_codex_session(selected))
                                        })??;
                                        initial.context = None;
                                    }
                                }
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
                                        &crate::session_record::capture_dir(session.input_key()?)?, thread, self.account_id.clone())?;
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
                            if client_result(client.send(Message::Text(text)).await)?.is_none() { break; }
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
                        message => {
                            if client_result(client.send(message).await)?.is_none() { break; }
                        }
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

fn native_start_params(client: &Value, prepared: Value) -> Value {
    let mut params = client.as_object().cloned().unwrap_or_default();
    let mut config = client["config"].as_object().cloned().unwrap_or_default();
    let mut prepared = prepared
        .as_object()
        .expect("prepared thread parameters are an object")
        .clone();
    if let Some(overrides) = prepared.remove("config") {
        config.extend(
            overrides
                .as_object()
                .expect("prepared configuration is an object")
                .clone(),
        );
    }
    params.extend(prepared);
    params.insert("config".into(), Value::Object(config));
    Value::Object(params)
}

// The retained native client may explicitly exit without a WebSocket close handshake. This applies
// only to its side of the relay: provider disconnects still propagate as failures.
fn client_result<T>(result: std::result::Result<T, Error>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(
            Error::ConnectionClosed
            | Error::AlreadyClosed
            | Error::Protocol(ProtocolError::ResetWithoutClosingHandshake),
        ) => Ok(None),
        Err(Error::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
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

#[cfg(test)]
mod tests {
    use super::{CodexConnection, NativeInput};
    use crate::context_block::{codex_delivery_path, ContextDelivery};
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::net::{UnixListener, UnixStream};
    use tokio_tungstenite::{accept_async, client_async, tungstenite::Message};

    #[tokio::test]
    async fn native_start_and_reconnect_restore_context_before_client_receives_thread() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        rusqlite::Connection::open(database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, true)
            .unwrap();
        let context = ContextDelivery {
            repository: None,
            repo: home.path().to_owned(),
            home: home.path().to_owned(),
            wave: None,
            skill_file: None,
            references: Vec::new(),
        };
        let binding = codex_delivery_path(home.path(), "native-thread");
        for resume in [false, true] {
            let endpoint = home
                .path()
                .join(if resume { "resume.sock" } else { "start.sock" });
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let request = socket.next().await.unwrap().unwrap();
                let request: serde_json::Value =
                    serde_json::from_str(request.to_text().unwrap()).unwrap();
                socket
                    .send(Message::Text(
                        json!({"id":request["id"],"result":{
                            "thread":{"id":"native-thread","turns":[]}
                        }})
                        .to_string()
                        .into(),
                    ))
                    .await
                    .unwrap();
                let _ = socket.next().await;
            });
            let connection = CodexConnection {
                store: store.clone(),
                session_id: "conversation".into(),
                thread_id: resume.then(|| "native-thread".into()),
                attachment: Some(attachment.clone()),
                account_id: None,
            };
            let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
                params: (!resume).then(|| json!({})),
                text: None,
                context: Some(context.clone()),
            }));
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay = tokio::spawn(async move {
                connection
                    .serve_with_input(relay, &endpoint, Some(input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            terminal
                .send(Message::Text(
                    json!({"id":1,
                        "method":if resume {"thread/resume"} else {"thread/start"},
                        "params":if resume {json!({"threadId":"native-thread"})} else {json!({})}
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            let response = tokio::time::timeout(std::time::Duration::from_secs(2), terminal.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(response.to_text().unwrap().contains("native-thread"));
            let restored: ContextDelivery =
                serde_json::from_slice(&std::fs::read(&binding).unwrap()).unwrap();
            assert_eq!(restored, context);
            std::fs::remove_file(&binding).unwrap();
            terminal.close(None).await.unwrap();
            drop(terminal);
            relay.await.unwrap().unwrap();
            provider.await.unwrap();
        }
    }

    #[tokio::test]
    async fn native_client_exit_with_or_without_close_handshake_ends_its_channel_cleanly() {
        for handshake in [false, true] {
            let home = tempfile::tempdir_in("/tmp").unwrap();
            let endpoint = home.path().join("provider.sock");
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let input = socket.next().await.unwrap().unwrap();
                socket.send(input).await.unwrap();
                let _ = socket.next().await;
            });
            let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
            store.test_session("conversation", "input");
            let connection = CodexConnection {
                store,
                session_id: "conversation".into(),
                thread_id: None,
                attachment: None,
                account_id: None,
            };
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay = tokio::spawn(async move { connection.serve(relay, &endpoint).await });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            terminal
                .send(Message::Text(
                    r#"{"method":"initialize","params":{}}"#.into(),
                ))
                .await
                .unwrap();
            assert!(terminal.next().await.unwrap().unwrap().is_text());
            if handshake {
                terminal.close(None).await.unwrap();
            }
            drop(terminal);
            tokio::time::timeout(std::time::Duration::from_secs(2), relay)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            provider.await.unwrap();
        }
    }
}
