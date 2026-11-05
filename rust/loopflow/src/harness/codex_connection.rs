//! A native client may keep displaying a conversation after attachment transfer.
//! Its writes must still pass the Session fence at dispatch, including approval
//! replies. The same transport relays to the existing AgentProcess and inspects
//! it during attachment settlement, under the ownership fence.

use crate::id::AgentSessionId;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
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
    pub session_id: crate::id::LfSessionId,
    pub thread_id: Option<AgentSessionId>,
    pub attachment: Option<SessionAttachment>,
    /// The login this conversation runs on; the relay pins the thread to it
    /// as soon as Codex reports it, so a move while it is open finds its home.
    pub(crate) account_route: Option<crate::provider_account::ProviderAccountRoute>,
}

impl CodexConnection {
    pub fn new(
        store: SqliteStore,
        session_id: crate::id::LfSessionId,
        thread_id: Option<AgentSessionId>,
        attachment: Option<SessionAttachment>,
    ) -> Self {
        Self {
            store,
            session_id,
            thread_id,
            attachment,
            account_route: None,
        }
    }
}

/// The saved first input every connection in one launch shares, and the native
/// conversation Codex reported for it: a reconnect reads that here when the store
/// cannot answer.
#[derive(Debug)]
pub(crate) struct NativeInput {
    pub params: Option<Value>,
    pub text: Option<String>,
    pub context: Option<crate::context_block::ContextDelivery>,
    pub thread: Option<AgentSessionId>,
}

/// Receipts belong to the immutable captured input, alongside its saved prompt.
/// A dispatch receipt precedes the write: interruption can never authorize replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeSubmission {
    ThreadDispatched,
    ThreadRejected,
    InputDispatched,
    InputRejected,
    InputAccepted,
}

#[derive(Debug, thiserror::Error)]
#[error("{direction}: {operation}")]
struct RelayOperation {
    operation: &'static str,
    direction: &'static str,
}

impl RelayOperation {
    fn new(operation: &'static str, direction: &'static str) -> Self {
        Self {
            operation,
            direction,
        }
    }
}

struct RelayDiagnostics<'a> {
    connection: &'a CodexConnection,
    /// None when the store could not name the Session's input before the
    /// handshake; the relay then serves without receipts or retained failures.
    input: Option<String>,
    attachment: Option<SessionAttachment>,
}

impl RelayDiagnostics<'_> {
    fn retain(&self, error: &anyhow::Error) {
        let operation = error.downcast_ref::<RelayOperation>();
        let mut message = format!("{error:#}");
        if message.len() > 2048 {
            let mut end = 2045;
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            message.truncate(end);
            message.push_str("...");
        }
        let Some(input) = &self.input else {
            tracing::warn!(session = %self.connection.session_id, "codex relay failed without a store to retain it: {message}");
            return;
        };
        // A failed diagnostic write must never end the listener or reach the TUI.
        let _ = super::dispatch::off_reactor(|| {
            self.connection.store.retain_codex_relay_failure(
                input,
                self.attachment.as_ref(),
                &json!({
                    "source": "codex_relay", "input_id": input,
                    "operation": operation.map_or("process", |op| op.operation),
                    "direction": operation.map_or("relay", |op| op.direction),
                    "error": message,
                }),
            )
        });
    }

    fn client_result<T>(
        &self,
        result: std::result::Result<T, Error>,
        operation: RelayOperation,
    ) -> Result<Option<T>> {
        let error = match result {
            Ok(value) => return Ok(Some(value)),
            Err(error) => error,
        };
        // Renderer resets end this channel, while outstanding provider replies
        // still drain. Explicit close frames never reach this error path.
        let disconnected = match &error {
            Error::ConnectionClosed
            | Error::AlreadyClosed
            | Error::Protocol(ProtocolError::ResetWithoutClosingHandshake) => true,
            Error::Io(error) => matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ),
            _ => false,
        };
        let error = anyhow::Error::new(error).context(operation);
        if disconnected {
            self.retain(&error);
            Ok(None)
        } else {
            Err(error)
        }
    }
}

impl CodexConnection {
    pub(crate) fn ensure_thread_resolved(&self) -> Result<()> {
        if self.submission()? == Some(NativeSubmission::ThreadDispatched)
            && self.store.session_thread(&self.session_id)?.is_none()
        {
            return Err(anyhow!("Codex thread creation outcome is unknown; saved input is retained; inspect provider history before starting another conversation"));
        }
        Ok(())
    }

    pub(crate) fn submission(&self) -> Result<Option<NativeSubmission>> {
        let session = self
            .store
            .session(&self.session_id)?
            .ok_or_else(|| anyhow!("Session disappeared during native startup"))?;
        self.store
            .session_history(&self.session_id, 0, 0)?
            .iter()
            .rev()
            .find(|event| {
                session
                    .artifact_key
                    .as_deref()
                    .is_some_and(|key| event.payload["input_id"] == key)
                    && event.payload["source"] == "codex_submission"
            })
            .map(|event| {
                serde_json::from_value(event.payload["evidence"].clone()).map_err(Into::into)
            })
            .transpose()
    }

    pub(crate) fn recover_input(&self, prompt: &str) -> Result<Option<String>> {
        let retry = match self.submission()? {
            Some(NativeSubmission::InputDispatched | NativeSubmission::InputAccepted) => false,
            Some(_) => true,
            None => self
                .store
                .session_thread(&self.session_id)?
                .or_else(|| self.thread_id.clone())
                .is_none(),
        };
        Ok((retry && !prompt.is_empty()).then(|| prompt.to_owned()))
    }

    fn record_submission(&self, input: &str, state: NativeSubmission) -> StoreResult<()> {
        let session = self.store.session(&self.session_id)?.ok_or_else(|| {
            StoreError::InvalidData("Session disappeared during native startup".into())
        })?;
        let observation = crate::session::SessionObservation {
            artifact_key: input.to_owned(),
            source: format!("codex_submission:{}", uuid::Uuid::new_v4()),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task_id: session.task_id.clone(),
            wave_id: session.wave_id.clone(),
            payload: json!({"input_id":input,"source":"codex_submission","evidence":state}),
        };
        self.store
            .retain_session_observation(&session, &observation)
    }

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
        let mut history = super::codex::CodexStream::new(super::fact_writer::FactWriter::new(
            self.store.clone(),
            self.session_id.to_string(),
            None,
            None,
            Default::default(),
        ));
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
            history
                .inbound(&json!({"result":result}), Some(thread))
                .await
                .1?;
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
        // A reconnecting client is accepted whatever the store says: its
        // records describe the conversation and never gate carrying it.
        let input = match self.store.session(&self.session_id) {
            Ok(Some(session)) => Some(session.input_key()?.clone()),
            Ok(None) => return Err(anyhow!("Session disappeared during native startup")),
            Err(error) => {
                tracing::warn!(%error, session = %self.session_id, "store unavailable before the client handshake; serving the conversation without its records");
                None
            }
        };
        let attachment = self.attachment.clone().or_else(|| {
            self.store
                .session_attachment(&self.session_id)
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, session = %self.session_id, "store unavailable; relay failures will not name their attachment");
                    None
                })
        });
        let diagnostics = RelayDiagnostics {
            connection: self,
            input,
            attachment,
        };
        let result = self
            .serve_relay(client, endpoint, initial, &diagnostics)
            .await;
        if let Err(error) = &result {
            diagnostics.retain(error);
        }
        result
    }

    async fn serve_relay(
        &self,
        client: UnixStream,
        endpoint: &Path,
        initial: Option<Arc<tokio::sync::Mutex<NativeInput>>>,
        diagnostics: &RelayDiagnostics<'_>,
    ) -> Result<()> {
        let mut client = accept_async_with_config(client, relay_config())
            .await
            .context(RelayOperation::new("handshake", "client_to_relay"))?;
        let socket = UnixStream::connect(endpoint)
            .await
            .context(RelayOperation::new("connect", "relay_to_provider"))?;
        let mut upstream = connect_upstream(socket)
            .await
            .context(RelayOperation::new("handshake", "relay_to_provider"))?;
        let input_key = diagnostics.input.as_deref();
        let receipt_key = || input_key.expect("saved input dispatches only with its receipt key");
        // The store records this conversation; it never carries it. Every store
        // failure below except a lost attachment is logged once and relaying
        // continues. Only the saved initial input keeps waiting for its receipt.
        let mut store_failure = crate::store::RecordingGap::default();
        let mut history = super::codex::CodexStream::new(super::fact_writer::FactWriter::new(
            self.store.clone(),
            self.session_id.to_string(),
            self.attachment.clone(),
            self.account_route.clone(),
            Default::default(),
        ));
        let mut startup_request = None;
        let mut initial_request = None;
        let mut client_open = true;
        let mut subscribed = false;
        let mut busy = true;
        let mut client_turn_request = None;
        let mut updates = super::task_updates::TaskUpdates::default();
        let mut update_request: Option<(Value, super::task_updates::TaskUpdate)> = None;
        let mut update_clock = tokio::time::interval(Duration::from_secs(1));
        if let Some(attachment) = &self.attachment {
            match self
                .store
                .with_session_attachment(&self.session_id, attachment, || {
                    updates.pending(&self.store, &self.session_id)
                }) {
                Ok(_) | Err(StoreError::InvalidAuthority(_)) => {}
                Err(error) => store_failure.note("task updates", &error),
            }
        }
        let disconnected_deadline = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(disconnected_deadline);
        // The conversation's identity: the store's record, then the thread this
        // launch already saw Codex select, then the connection's own.
        let saved = match self.store.session_thread(&self.session_id) {
            Ok(saved) => saved,
            Err(error) => {
                store_failure.note("conversation lookup", &error);
                None
            }
        };
        let launched = match &initial {
            Some(initial) => initial.lock().await.thread.clone(),
            None => None,
        };
        let mut thread = saved.or(launched).or_else(|| self.thread_id.clone());
        loop {
            // A disconnected renderer cannot cancel work already dispatched.
            // Drain only outstanding startup replies, saving their native identity.
            if !client_open
                && startup_request.is_none()
                && initial_request.is_none()
                && update_request.is_none()
            {
                break;
            }
            tokio::select! {
                _ = update_clock.tick(), if subscribed && client_open && !busy && initial_request.is_none() && update_request.is_none() => {
                    // An idle tick is also when a reading the store refused catches
                    // up (the tracker logs its own misses) and when a closed gap
                    // enters the Session's history.
                    let caught_up = matches!(
                        history.writer.catch_up(),
                        Ok(()) | Err(StoreError::InvalidAuthority(_))
                    );
                    let pending = match &self.attachment {
                        Some(attachment) => self.store.with_session_attachment(&self.session_id, attachment,
                            || updates.pending(&self.store, &self.session_id)),
                        None => Ok(None),
                    };
                    let pending = match pending {
                        Ok(pending) => {
                            if caught_up {
                                if let Err(error) = self.store.close_recording_gap(&self.session_id, &mut store_failure) {
                                    store_failure.note("recording gap", &error);
                                }
                            }
                            pending
                        }
                        Err(StoreError::InvalidAuthority(_)) => None,
                        Err(error) => {
                            store_failure.note("task updates", &error);
                            None
                        }
                    };
                    if let Some(update) = pending {
                        let id = json!(format!("lf-task-update-{}", uuid::Uuid::new_v4()));
                        let request = json!({"id":id,"method":"turn/start","params":{
                            "threadId":thread,"input":[],
                            "toolOutput":{"name":"loopflow_task_update","output":&update.text,"retain":true}
                        }});
                        if let Err(error) = history.outbound(&request).await {
                            if !matches!(error.downcast_ref::<StoreError>(), Some(StoreError::InvalidAuthority(_))) {
                                store_failure.note("task update history", &error);
                            }
                            continue;
                        }
                        let sent;
                        (upstream, sent) = self.send_task_update(upstream, &request, &update).await?;
                        match sent {
                            Ok(true) => {
                                busy = true;
                                update_request = Some((id, update));
                            }
                            Ok(false) | Err(StoreError::InvalidAuthority(_)) => {}
                            Err(error) => store_failure.note("task update dispatch", &error),
                        }
                    }
                }
                _ = &mut disconnected_deadline, if !client_open => {
                    return Err(anyhow!("Codex startup response was lost; submission outcome is unknown; reconnect to inspect the saved conversation").context(RelayOperation::new("startup_reply_timeout", "provider_to_relay")));
                }
                incoming = client.next(), if client_open => {
                    let message = incoming.map(|result| diagnostics.client_result(result, RelayOperation::new("read", "client_to_relay"))).transpose()?.flatten();
                    let text = match message {
                        Some(Message::Text(text)) => text,
                        None | Some(Message::Close(_)) => {
                            client_open = false;
                            disconnected_deadline.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
                            continue;
                        }
                        _ => continue,
                    };
                    let mut rpc: Value = serde_json::from_str(&text).context(RelayOperation::new("decode", "client_to_relay"))?;
                    // A reconnecting fresh TUI still asks to start. Its Session
                    // may already have received the original start response.
                    if rpc["method"] == "thread/start" && !subscribed {
                        thread = match self.store.session_thread(&self.session_id) {
                            Ok(saved) => saved.or(thread),
                            Err(error) => {
                                store_failure.note("conversation lookup", &error);
                                thread
                            }
                        };
                        if let Some(thread) = &thread {
                            rpc["method"] = json!("thread/resume");
                            rpc["params"]["threadId"] = json!(thread);
                        }
                    }
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
                            let mut saved = match &initial {
                                Some(initial) => Some(initial.lock().await),
                                None => None,
                            };
                            // Saved input dispatches only with its receipt key. Without
                            // the store it stays saved for the next reconnect.
                            if input_key.is_none()
                                && saved.as_ref().is_some_and(|saved| saved.params.is_some() || saved.text.is_some())
                            {
                                drop(saved);
                                reject(&mut client, &rpc, "Loopflow store is unavailable; saved input is retained; reconnect to retry").await?;
                                continue;
                            }
                            let params = saved.as_mut().and_then(|saved| saved.params.take());
                            drop(saved);
                            if starting && params.is_none() {
                                reject(&mut client, &rpc, "Conversation startup was already submitted").await?;
                                continue;
                            }
                            startup_request = rpc.get("id").cloned().map(|id| (id, params.clone()));
                            let passive = params.is_none();
                            // Keep native tools and explicitly selected UI settings;
                            // prepared instructions and launch configuration win.
                            // A passive or stale UI can only subscribe.
                            rpc["params"] = match params {
                                Some(params) => native_start_params(&rpc["params"], params),
                                None => json!({"threadId":thread}),
                            };
                            passive
                        }
                        _ => false,
                    };
                    if let Err(error) = history.outbound(&rpc).await {
                        match error.downcast_ref::<StoreError>() {
                            Some(StoreError::InvalidAuthority(reason)) => {
                                reject(&mut client, &rpc, reason).await?;
                                continue;
                            }
                            _ => store_failure.note("client request history", &error),
                        }
                    }
                    if rpc["method"] == "turn/start" { busy = true; client_turn_request = rpc.get("id").cloned(); }
                    let message = Message::Text(serde_json::to_string(&rpc)?.into());
                    if passive {
                        upstream.send(message).await.context(RelayOperation::new("write", "relay_to_provider"))?;
                    } else {
                        let outcome;
                        (upstream, outcome) = self.send(upstream, message,
                            starting.then(|| (receipt_key(), NativeSubmission::ThreadDispatched))).await?;
                        match outcome {
                            Ok(()) => {},
                            Err(StoreError::InvalidAuthority(reason)) => {
                                reject(&mut client, &rpc, &reason).await?;
                            }
                            Err(error) => return Err(anyhow::Error::new(error).context(RelayOperation::new("dispatch", "relay_to_provider"))),
                        }
                    }
                }
                outgoing = upstream.next() => {
                    let outgoing = outgoing.ok_or_else(|| anyhow!("Codex provider connection closed; resume saved history before retrying input")).and_then(|message| message.map_err(Into::into)).context(RelayOperation::new("read", "provider_to_relay"))?;
                    let resumed = match &outgoing {
                        Message::Close(_) => return Err(anyhow!("Codex provider closed the connection; resume saved history before retrying input").context(RelayOperation::new("close", "provider_to_relay"))),
                        Message::Text(text) => {
                            let rpc: Value = serde_json::from_str(text).context(RelayOperation::new("decode", "provider_to_relay"))?;
                            let bind_thread = startup_request.as_ref().is_some_and(|(_, params)| params.is_some());
                            let startup_reply = startup_request.as_ref().is_some_and(|(id, _)| rpc.get("id") == Some(id));
                            if startup_reply {
                                let (_, params) = startup_request.take().expect("matched startup request");
                                if rpc.get("error").is_some() && thread.is_none() {
                                    if let Some(key) = input_key {
                                        if let Err(error) = self.record_submission(key, NativeSubmission::ThreadRejected) {
                                            store_failure.note("thread rejection receipt", &error);
                                        }
                                    }
                                    if let Some(initial) = &initial {
                                        initial.lock().await.params = params;
                                    }
                                }
                            }
                            let resumed = startup_reply && rpc.get("result").is_some();
                            // Reply-first and notification-first creation share one identity
                            // check and receipt, even after the renderer disappears.
                            let selected = if resumed {
                                Some(rpc.pointer("/result/thread/id").and_then(Value::as_str)
                                    .ok_or_else(|| anyhow!("Codex returned no native conversation ID"))?)
                            } else if bind_thread && rpc["method"] == "thread/started" {
                                rpc.pointer("/params/thread/id").and_then(Value::as_str)
                            } else {
                                None
                            };
                            if let Some(selected) = selected {
                                if thread.as_ref().is_some_and(|thread| thread.as_str() != selected) {
                                    return Err(anyhow!("Codex selected another conversation"));
                                }
                                let selected_thread = AgentSessionId::from(selected);
                                if let Some(initial) = &initial {
                                    initial.lock().await.thread = Some(selected_thread.clone());
                                }
                                if bind_thread {
                                    if let Some(attachment) = &self.attachment {
                                        if let Err(error) = self.store.record_session_connection(&self.session_id, attachment,
                                            &endpoint.to_string_lossy(), &selected_thread) {
                                            store_failure.note("conversation selection", &error);
                                        }
                                    }
                                }
                                thread = Some(selected_thread);
                                if resumed {
                                    subscribed = true;
                                    if let (Some(initial), Some(attachment)) = (&initial, &self.attachment) {
                                        let mut initial = initial.lock().await;
                                        if let Some(context) = &initial.context {
                                            // Hooks run on the app-server, which never inherits the
                                            // terminal's launch file. Bind before the UI can send a turn.
                                            match self.store.with_session_attachment(&self.session_id, attachment, || {
                                                Ok(context.bind_codex_session(selected))
                                            }) {
                                                Ok(bound) => {
                                                    bound?;
                                                    initial.context = None;
                                                }
                                                Err(error @ StoreError::InvalidAuthority(_)) => return Err(error.into()),
                                                Err(error) => store_failure.note("hook context binding", &error),
                                            }
                                        }
                                    }
                                }
                            }
                            if rpc["method"] == "turn/started" && initial.is_some() {
                                if let Some(thread) = &thread {
                                    let written = (|| -> Result<()> {
                                        let session = self.store.session(&self.session_id)?
                                            .ok_or_else(|| anyhow!("Session disappeared during native startup"))?;
                                        crate::session_record::write_provider_session(
                                            &crate::session_record::capture_dir(session.input_key()?)?, thread, self.account_route.as_ref().map(|route| route.account_id().clone()))?;
                                        Ok(())
                                    })();
                                    if let Err(error) = written {
                                        store_failure.note("provider session record", &error);
                                    }
                                }
                            }
                            if client_turn_request.as_ref().is_some_and(|id| rpc.get("id") == Some(id)) {
                                client_turn_request = None;
                                if rpc.get("error").is_some() { busy = false; }
                            }
                            if rpc["method"] == "turn/started" { busy = true; }
                            if rpc["method"] == "turn/completed" { busy = false; }
                            if resumed {
                                busy = rpc.pointer("/result/thread/turns").and_then(Value::as_array)
                                    .is_none_or(|turns| turns.iter().any(|turn| turn["status"] == "inProgress"));
                            }
                            if let Err(error) = history.inbound(&rpc, thread.as_ref()).await.1 {
                                store_failure.note("provider history", &error);
                            }
                            if update_request.as_ref().is_some_and(|(id, _)| rpc.get("id") == Some(id)) {
                                let (_, update) = update_request.take().expect("matched update request");
                                let state = if rpc.get("error").is_some() { busy = false; "rejected" } else { "accepted" };
                                if let Err(error) = self.store.record_task_update(&self.session_id, &update.key, state, &update.observation) {
                                    store_failure.note("task update receipt", &error);
                                }
                                continue;
                            }
                            if initial_request.as_ref().is_some_and(|(id, _)| rpc.get("id") == Some(id)) {
                                let (_, text) = initial_request.take().expect("matched initial request");
                                let key = receipt_key();
                                if let Some(error) = rpc.get("error") {
                                    if let Err(error) = self.record_submission(key, NativeSubmission::InputRejected) {
                                        store_failure.note("input rejection receipt", &error);
                                    }
                                    if let Some(initial) = &initial {
                                        initial.lock().await.text = Some(text);
                                    }
                                    return Err(anyhow!("Codex rejected the initial request: {error}; saved input is available on reconnect"));
                                }
                                if let Err(error) = self.record_submission(key, NativeSubmission::InputAccepted) {
                                    store_failure.note("input acceptance receipt", &error);
                                }
                                continue;
                            }
                            resumed.then_some(rpc)
                        }
                        _ => None,
                    };
                    if !client_open { continue; }
                    if diagnostics.client_result(client.send(outgoing).await, RelayOperation::new("write", "relay_to_client"))?.is_none() {
                        client_open = false;
                        disconnected_deadline.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
                        continue;
                    }
                    if let (Some(rpc), Some(initial)) = (resumed, &initial) {
                        let mut initial = initial.lock().await;
                        let turns = rpc.pointer("/result/thread/turns").and_then(Value::as_array)
                            .ok_or_else(|| anyhow!("Codex returned no native turn history"))?;
                        if !turns.is_empty() {
                            initial.text = None;
                            match (self.submission(), input_key) {
                                (Ok(Some(NativeSubmission::InputDispatched)), Some(key)) => {
                                    if let Err(error) = self.record_submission(key, NativeSubmission::InputAccepted) {
                                        store_failure.note("input acceptance receipt", &error);
                                    }
                                }
                                (Ok(_), _) => {}
                                (Err(error), _) => store_failure.note("submission receipt", &error),
                            }
                        }
                        if let Some(text) = initial.text.take() {
                            let id = json!(format!("lf-initial-{}", uuid::Uuid::new_v4()));
                            let request = json!({"id":id,"method":"turn/start","params":{
                                "threadId":thread,"input":[{"type":"text","text":&text}]
                            }});
                            if let Err(error) = history.outbound(&request).await {
                                store_failure.note("initial request history", &error);
                            }
                            let outcome;
                            (upstream, outcome) = self.send(upstream, Message::Text(request.to_string().into()),
                                Some((receipt_key(), NativeSubmission::InputDispatched))).await?;
                            outcome.context(RelayOperation::new("dispatch", "relay_to_provider"))?;
                            initial_request = Some((id, text));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// One admitted socket write and its uncertainty receipt share the Session
    /// fence, including overlapping reconnects using the same attachment.
    async fn send_task_update(
        &self,
        mut upstream: WebSocketStream<UnixStream>,
        request: &Value,
        update: &super::task_updates::TaskUpdate,
    ) -> Result<(WebSocketStream<UnixStream>, StoreResult<bool>)> {
        let Some(attachment) = self.attachment.clone() else {
            return Ok((upstream, Ok(false)));
        };
        let connection = self.clone();
        let update = update.clone();
        let message = Message::Text(request.to_string().into());
        tokio::task::spawn_blocking(move || {
            let result =
                connection
                    .store
                    .with_session_dispatch(&connection.session_id, &attachment, || {
                        if !connection.store.record_task_update(
                            &connection.session_id,
                            &update.key,
                            "dispatched",
                            &update.observation,
                        )? {
                            return Ok(false);
                        }
                        super::dispatch::send_fenced(&mut upstream, message)?;
                        Ok(true)
                    });
            (upstream, result)
        })
        .await
        .map_err(|error| anyhow!("Native dispatch task failed: {error}"))
    }

    async fn send(
        &self,
        mut upstream: WebSocketStream<UnixStream>,
        message: Message,
        submission: Option<(&str, NativeSubmission)>,
    ) -> Result<(WebSocketStream<UnixStream>, StoreResult<()>)> {
        let Some(attachment) = self.attachment.clone() else {
            return Ok((
                upstream,
                Err(StoreError::InvalidAuthority(
                    "Passive display has no attachment claim".into(),
                )),
            ));
        };
        let connection = self.clone();
        let submission = submission.map(|(input, state)| (input.to_owned(), state));
        // A second connection can transfer the attachment in another process.
        // Keep the attachment comparison and bounded socket dispatch under the
        // Session lock. History and other Sessions can still use the database.
        tokio::task::spawn_blocking(move || {
            let mut message = Some(message);
            let outcome =
                connection
                    .store
                    .with_session_dispatch(&connection.session_id, &attachment, || {
                        // The admitted write and its durable uncertainty receipt share
                        // the attachment fence. A rejected stale writer records no dispatch.
                        if let Some((input, state)) = &submission {
                            connection.record_submission(input, *state)?;
                        }
                        let message = message.take().expect("one dispatch per send");
                        super::dispatch::send_fenced(&mut upstream, message)
                    });
            // `message` is still here only when the fence never ran the dispatch.
            // A person's own turn is then sent unfenced rather than held back by
            // the store; saved initial input keeps waiting for its receipt.
            let outcome = match (outcome, message) {
                (Err(error), Some(message))
                    if submission.is_none()
                        && !matches!(error, StoreError::InvalidAuthority(_)) =>
                {
                    tracing::warn!(%error, session = %connection.session_id, "store unavailable; dispatching the turn without its fence");
                    super::dispatch::send_fenced(&mut upstream, message)
                }
                (outcome, _) => outcome,
            };
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
            .await
            .context(RelayOperation::new("reject", "relay_to_client"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CodexConnection, NativeInput};
    use crate::context_block::{native_delivery_path, ContextDelivery};
    use crate::id::LfProcessId;
    use crate::provider_account::ProviderAccountRoute;
    use crate::provider_auth::Provider;
    use crate::store::sqlite::SqliteStore;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::net::{UnixListener, UnixStream};
    use tokio_tungstenite::{accept_async, client_async, tungstenite::Message};

    fn attached_connection(home: &std::path::Path) -> CodexConnection {
        let database = home.join("store.db");
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
        CodexConnection {
            store,
            session_id: "conversation".into(),
            thread_id: None,
            attachment: Some(attachment),
            account_route: None,
        }
    }

    #[tokio::test]
    async fn task_update_wakes_idle_native_client_on_its_existing_thread() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let mut connection = attached_connection(home.path());
        connection.thread_id = Some("native-thread".into());
        let store = connection.store.clone();
        let task = crate::durable::TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/task-updates").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Updates".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/task-updates',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='conversation'",
                [],
            )
            .unwrap();
        let captured = store.session("conversation").unwrap().unwrap().captured;
        store.bind_session("conversation", captured, &task).unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let resume = fixture_rpc(&mut socket).await;
            fixture_send(
                &mut socket,
                json!({"id":resume["id"],"result":{"thread":{"id":"native-thread","turns":[]}}}),
            )
            .await;
            let request =
                tokio::time::timeout(std::time::Duration::from_secs(8), fixture_rpc(&mut socket))
                    .await
                    .unwrap();
            assert_eq!(request["method"], "turn/start");
            assert_eq!(request["params"]["threadId"], "native-thread");
            assert_eq!(request["params"]["input"], json!([]));
            assert_eq!(
                request["params"]["toolOutput"]["name"],
                "loopflow_task_update"
            );
            assert_eq!(request["params"]["toolOutput"]["retain"], true);
            let progress = request["params"]["toolOutput"]["output"].as_str().unwrap();
            assert!(progress.contains("gate ("));
            assert!(progress.contains("): failed"));
            fixture_send(&mut socket, json!({"id":request["id"],"result":{"turn":{"id":"report","status":"inProgress","items":[]}}})).await;
            fixture_send(&mut socket, json!({"method":"turn/started","params":{"threadId":"native-thread","turn":{"id":"report","status":"inProgress","items":[]}}})).await;
            let _ = socket.next().await;
        });
        let (terminal, relay) = UnixStream::pair().unwrap();
        let relay = tokio::spawn(async move { connection.serve(relay, &endpoint).await });
        let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        fixture_send(
            &mut terminal,
            json!({"id":1,"method":"thread/resume","params":{"threadId":"native-thread"}}),
        )
        .await;
        let resumed = fixture_rpc(&mut terminal).await;
        assert_eq!(resumed["result"]["thread"]["id"], "native-thread");
        store.test_graph(
            "feature",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        let notification = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            fixture_rpc(&mut terminal),
        )
        .await
        .unwrap();
        assert_eq!(notification["method"], "turn/started");
        assert_eq!(notification["params"]["threadId"], "native-thread");
        assert!(store
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .any(|event| event.payload["source"] == "task_update"
                && event.payload["state"] == "accepted"));
        terminal.close(None).await.unwrap();
        drop(terminal);
        relay.await.unwrap().unwrap();
        provider.await.unwrap();
    }

    #[tokio::test]
    async fn idle_relay_survives_store_failure_and_accepts_a_reconnect() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let store = connection.store.clone();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='conversation'",
                [],
            )
            .unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let request = fixture_rpc(&mut socket).await;
                fixture_send(
                    &mut socket,
                    json!({"id":request["id"],"result":{"thread":{"id":"native-thread","turns":[]}}}),
                )
                .await;
                while let Some(Ok(Message::Text(text))) = socket.next().await {
                    let rpc: serde_json::Value = serde_json::from_str(&text).unwrap();
                    if rpc["method"] == "thread/read" {
                        fixture_send(
                            &mut socket,
                            json!({"id":rpc["id"],"result":{"thread":{"id":"native-thread"}}}),
                        )
                        .await;
                    }
                }
            }
        });
        let initial = Arc::new(tokio::sync::Mutex::new(NativeInput {
            params: Some(json!({})),
            text: None,
            context: None,
            thread: None,
        }));
        let (terminal, relay) = UnixStream::pair().unwrap();
        let (relay_connection, relay_endpoint, relay_initial) =
            (connection.clone(), endpoint.clone(), initial.clone());
        let relay = tokio::spawn(async move {
            relay_connection
                .serve_with_input(relay, &relay_endpoint, Some(relay_initial))
                .await
        });
        let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        fixture_send(
            &mut terminal,
            json!({"id":1,"method":"thread/start","params":{}}),
        )
        .await;
        let started = fixture_rpc(&mut terminal).await;
        assert_eq!(started["result"]["thread"]["id"], "native-thread");
        // The store the idle relay polls every second disappears under it.
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("ALTER TABLE agent_sessions RENAME TO agent_sessions_gone")
            .unwrap();
        assert!(store.session("conversation").is_err());
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        assert!(!relay.is_finished(), "an idle poll ended the conversation");
        fixture_send(
            &mut terminal,
            json!({"id":2,"method":"thread/read","params":{"threadId":"native-thread"}}),
        )
        .await;
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            fixture_rpc(&mut terminal),
        )
        .await
        .unwrap();
        assert_eq!(read["id"], 2);
        // The store answers again: the relay's gap enters the Session's history
        // once, at the idle tick, with nothing typed.
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("ALTER TABLE agent_sessions_gone RENAME TO agent_sessions")
            .unwrap();
        let observations = || -> i64 {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM session_events WHERE session_id='conversation'
                     AND kind='observed' AND json_extract(payload,'$.source')='store-gap'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while observations() == 0 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the restored store never heard of the relay's gap"
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        assert_eq!(observations(), 1, "a closed gap is recorded once");
        terminal.close(None).await.unwrap();
        drop(terminal);
        relay.await.unwrap().unwrap();
        // A reconnecting client is accepted without the store and lands on the
        // thread this launch already selected.
        let (terminal, relay) = UnixStream::pair().unwrap();
        let relay = tokio::spawn(async move {
            connection
                .serve_with_input(relay, &endpoint, Some(initial))
                .await
        });
        let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        fixture_send(
            &mut terminal,
            json!({"id":1,"method":"thread/start","params":{}}),
        )
        .await;
        let resumed = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            fixture_rpc(&mut terminal),
        )
        .await
        .unwrap();
        assert_eq!(resumed["result"]["thread"]["id"], "native-thread");
        terminal.close(None).await.unwrap();
        drop(terminal);
        relay.await.unwrap().unwrap();
        provider.await.unwrap();
    }

    #[tokio::test]
    async fn task_update_dispatch_survives_reconnect_and_fences_takeover() {
        let home = tempfile::tempdir().unwrap();
        let connection = attached_connection(home.path());
        let (sender, receiver) = UnixStream::pair().unwrap();
        let (sender, receiver) =
            tokio::join!(super::connect_upstream(sender), accept_async(receiver));
        let sender = sender.unwrap();
        let mut receiver = receiver.unwrap();
        let update = crate::harness::task_updates::TaskUpdate {
            key: "transition".into(),
            observation: serde_json::json!({}),
            text: "At demo".into(),
        };
        let request = json!({"id":"update","method":"turn/start","params":{"threadId":"saved", "input":[{"type":"text","text":update.text}]}});
        let (sender, sent) = connection
            .send_task_update(sender, &request, &update)
            .await
            .unwrap();
        assert!(sent.unwrap());
        assert_eq!(fixture_rpc(&mut receiver).await, request);
        // No reply: reconnect must retain uncertainty instead of replaying.
        let (sender, sent) = connection
            .clone()
            .send_task_update(sender, &request, &update)
            .await
            .unwrap();
        assert!(!sent.unwrap());
        let process = LfProcessId::new();
        connection
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'replacement',2)",
                [process.as_str()],
            )
            .unwrap();
        connection
            .store
            .claim_session_attachment(
                "conversation",
                connection.attachment.as_ref(),
                &process,
                true,
            )
            .unwrap();
        let newer = crate::harness::task_updates::TaskUpdate {
            key: "next-transition".into(),
            observation: serde_json::json!({}),
            text: "Stopped".into(),
        };
        let (_, sent) = connection
            .send_task_update(sender, &request, &newer)
            .await
            .unwrap();
        assert!(matches!(
            sent,
            Err(crate::store::StoreError::InvalidAuthority(_))
        ));
        assert!(!connection
            .store
            .session_history("conversation", 0, 0)
            .unwrap()
            .iter()
            .any(|event| event.payload["key"] == "next-transition"));
    }

    #[tokio::test]
    async fn relay_retains_connect_and_client_handshake_failures() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let endpoint = home.path().join("missing.sock");
        let (terminal, relay) = UnixStream::pair().unwrap();
        let task_connection = connection.clone();
        let task = tokio::spawn(async move { task_connection.serve(relay, &endpoint).await });
        let (_terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        assert!(task.await.unwrap().is_err());
        let (terminal, relay) = UnixStream::pair().unwrap();
        drop(terminal);
        assert!(connection
            .serve(relay, &home.path().join("unused.sock"))
            .await
            .is_err());
        let history = connection
            .store
            .session_history(&connection.session_id, 0, 0)
            .unwrap();
        let diagnostics: Vec<_> = history
            .iter()
            .filter(|event| event.payload["source"] == "codex_relay")
            .collect();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].payload["operation"], "connect");
        assert_eq!(diagnostics[0].payload["direction"], "relay_to_provider");
        assert_eq!(diagnostics[1].payload["operation"], "handshake");
        assert_eq!(diagnostics[1].payload["direction"], "client_to_relay");
        assert_eq!(
            diagnostics[0].agent_process_id.as_deref(),
            Some(
                connection
                    .attachment
                    .as_ref()
                    .unwrap()
                    .agent_process_id
                    .as_str()
            )
        );
    }

    #[test]
    fn relay_diagnostics_bound_reconnect_storms_and_unicode_errors() {
        let home = tempfile::tempdir().unwrap();
        let connection = attached_connection(home.path());
        let input = connection
            .store
            .session(&connection.session_id)
            .unwrap()
            .unwrap()
            .artifact_key
            .unwrap();
        let diagnostics = super::RelayDiagnostics {
            connection: &connection,
            input: Some(input),
            attachment: connection.attachment.clone(),
        };
        for index in 0..32 {
            diagnostics.retain(
                &anyhow::anyhow!("{index}: {}", "雪".repeat(2000))
                    .context(super::RelayOperation::new("read", "provider_to_relay")),
            );
        }
        let history = connection
            .store
            .session_history(&connection.session_id, 0, 0)
            .unwrap();
        let diagnostics: Vec<_> = history
            .iter()
            .filter(|event| event.payload["source"] == "codex_relay")
            .collect();
        assert_eq!(diagnostics.len(), 16);
        assert!(diagnostics[0].payload["error"]
            .as_str()
            .unwrap()
            .contains("0:"));
        assert!(diagnostics
            .iter()
            .all(|event| event.payload["error"].as_str().unwrap().len() <= 2048));
    }

    #[tokio::test]
    async fn native_start_and_reconnect_restore_context_before_client_receives_thread() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let context = ContextDelivery {
            repository: None,
            repo: home.path().to_owned(),
            home: home.path().to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let binding = native_delivery_path(home.path(), "native-thread");
        for resume in [false, true] {
            let endpoint = home
                .path()
                .join(if resume { "resume.sock" } else { "start.sock" });
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let request = fixture_rpc(&mut socket).await;
                fixture_send(
                    &mut socket,
                    json!({"id":request["id"],"result":{
                        "thread":{"id":"native-thread","turns":[]}
                    }}),
                )
                .await;
                let _ = socket.next().await;
            });
            let connection = CodexConnection {
                thread_id: resume.then(|| "native-thread".into()),
                ..connection.clone()
            };
            let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
                params: (!resume).then(|| json!({})),
                text: None,
                context: Some(context.clone()),
                thread: None,
            }));
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay = tokio::spawn(async move {
                connection
                    .serve_with_input(relay, &endpoint, Some(input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            fixture_send(
                &mut terminal,
                json!({"id":1,
                    "method":if resume {"thread/resume"} else {"thread/start"},
                    "params":if resume {json!({"threadId":"native-thread"})} else {json!({})}
                }),
            )
            .await;
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
    async fn lost_start_response_reconnects_to_same_thread_and_submits_input_once() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
            params: Some(json!({})),
            text: Some("saved skill".into()),
            context: None,
            thread: None,
        }));
        for reconnect in [false, true] {
            let endpoint = home.path().join(if reconnect {
                "resume.sock"
            } else {
                "start.sock"
            });
            let listener = UnixListener::bind(&endpoint).unwrap();
            let (received, ready) = tokio::sync::oneshot::channel();
            let (release, wait) = tokio::sync::oneshot::channel();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let request = fixture_rpc(&mut socket).await;
                assert_eq!(
                    request["method"],
                    if reconnect {
                        "thread/resume"
                    } else {
                        "thread/start"
                    }
                );
                if reconnect {
                    assert_eq!(request["params"]["threadId"], "native-thread");
                }
                received.send(()).unwrap();
                wait.await.unwrap();
                fixture_send(
                    &mut socket,
                    json!({"method":"thread/started",
                        "params":{"thread":{"id":"native-thread"}}
                    }),
                )
                .await;
                fixture_send(
                    &mut socket,
                    json!({"id":request["id"],"result":{
                        "thread":{"id":"native-thread","turns":[]}
                    }}),
                )
                .await;
                if reconnect {
                    let turn = fixture_rpc(&mut socket).await;
                    assert_eq!(turn["method"], "turn/start");
                    assert_eq!(turn["params"]["input"][0]["text"], "saved skill");
                    fixture_send(
                        &mut socket,
                        json!({"id":turn["id"],"result":{
                            "turn":{"id":"one-turn"}
                        }}),
                    )
                    .await;
                    fixture_send(&mut socket, json!({"method":"item/agentMessage/delta",
                        "params":{"threadId":"native-thread","turnId":"one-turn","delta":"visible output"}
                    })).await;
                }
                let _ = socket.next().await;
            });
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay_connection = connection.clone();
            let input = input.clone();
            let relay = tokio::spawn(async move {
                relay_connection
                    .serve_with_input(relay, &endpoint, Some(input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            fixture_send(
                &mut terminal,
                json!({"id":1,"method":"thread/start","params":{}}),
            )
            .await;
            ready.await.unwrap();
            if reconnect {
                release.send(()).unwrap();
                let notification = terminal.next().await.unwrap().unwrap();
                assert!(notification.to_text().unwrap().contains("thread/started"));
                let response = terminal.next().await.unwrap().unwrap();
                assert!(response.to_text().unwrap().contains("native-thread"));
                let output = terminal.next().await.unwrap().unwrap();
                assert!(output.to_text().unwrap().contains("visible output"));
                fixture_send(
                    &mut terminal,
                    json!({"id":9,"method":"thread/start","params":{}}),
                )
                .await;
                let rejected = terminal.next().await.unwrap().unwrap();
                let rejected: serde_json::Value =
                    serde_json::from_str(rejected.to_text().unwrap()).unwrap();
                assert_eq!(rejected["id"], 9);
                assert_eq!(rejected["error"]["code"], -32001);
                terminal.close(None).await.unwrap();
                drop(terminal);
            } else {
                drop(terminal);
                release.send(()).unwrap();
            }
            tokio::time::timeout(std::time::Duration::from_secs(2), relay)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            provider.await.unwrap();
            assert_eq!(
                connection
                    .store
                    .session_thread("conversation")
                    .unwrap()
                    .unwrap()
                    .as_str(),
                "native-thread"
            );
        }
        assert!(input.lock().await.text.is_none());
    }

    async fn fixture_rpc(
        socket: &mut tokio_tungstenite::WebSocketStream<UnixStream>,
    ) -> serde_json::Value {
        let message = socket.next().await.unwrap().unwrap();
        serde_json::from_str(message.to_text().unwrap()).unwrap()
    }

    async fn fixture_send(
        socket: &mut tokio_tungstenite::WebSocketStream<UnixStream>,
        rpc: serde_json::Value,
    ) {
        socket
            .send(Message::Text(rpc.to_string().into()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn provider_loss_retains_creation_uncertainty_or_notification_identity() {
        for outcome in ["lost-before-creation", "lost-after-creation", "rejected"] {
            let created = outcome == "lost-after-creation";
            let home = tempfile::tempdir_in("/tmp").unwrap();
            let connection = attached_connection(home.path());
            let endpoint = home.path().join("provider.sock");
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let start = fixture_rpc(&mut socket).await;
                assert_eq!(start["method"], "thread/start");
                if outcome == "rejected" {
                    fixture_send(
                        &mut socket,
                        json!({"id":start["id"],"error":{
                        "code":-32000,"message":"startup rejected"}}),
                    )
                    .await;
                }
                if created {
                    fixture_send(
                        &mut socket,
                        json!({"method":"thread/started",
                        "params":{"thread":{"id":"saved-thread"}}}),
                    )
                    .await;
                }
                socket.close(None).await.unwrap();
            });
            let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
                params: Some(json!({})),
                text: Some("retained draft".into()),
                context: None,
                thread: None,
            }));
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay_connection = connection.clone();
            let relay_input = input.clone();
            let relay = tokio::spawn(async move {
                relay_connection
                    .serve_with_input(relay, &endpoint, Some(relay_input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            fixture_send(
                &mut terminal,
                json!({"id":1,"method":"thread/start","params":{}}),
            )
            .await;
            let error = format!("{:#}", relay.await.unwrap().unwrap_err());
            assert!(error.contains("provider closed"), "{error}");
            provider.await.unwrap();
            let diagnostic = connection
                .store
                .session_history(&connection.session_id, 0, 0)
                .unwrap()
                .into_iter()
                .find(|event| event.payload["source"] == "codex_relay")
                .unwrap();
            assert_eq!(diagnostic.payload["direction"], "provider_to_relay");
            let reopened = CodexConnection {
                store: SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap(),
                ..connection
            };
            assert_eq!(
                reopened.submission().unwrap(),
                Some(if outcome == "rejected" {
                    super::NativeSubmission::ThreadRejected
                } else {
                    super::NativeSubmission::ThreadDispatched
                })
            );
            assert_eq!(
                reopened
                    .store
                    .session_thread("conversation")
                    .unwrap()
                    .map(|id| id.to_string()),
                created.then(|| "saved-thread".to_owned())
            );
            assert_eq!(input.lock().await.text.as_deref(), Some("retained draft"));
            assert_eq!(input.lock().await.params.is_some(), outcome == "rejected");
            assert_eq!(
                reopened.recover_input("retained draft").unwrap().as_deref(),
                Some("retained draft")
            );
            assert_eq!(
                reopened.ensure_thread_resolved().is_err(),
                outcome == "lost-before-creation"
            );
        }
    }

    #[tokio::test]
    async fn an_open_conversation_is_pinned_to_its_login_when_codex_reports_it() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let mut connection = attached_connection(home.path());
        rusqlite::Connection::open(home.path().join("store.db"))
            .unwrap()
            .execute(
                "INSERT INTO provider_accounts(provider,account_id,credential_state,
                    created_at,updated_at)
                 VALUES('codex','engineering','connected',1,1)",
                [],
            )
            .unwrap();
        connection.account_route = Some(ProviderAccountRoute::shared(
            Provider::Codex,
            crate::store::ProviderAccountId::parse("engineering").unwrap(),
            Arc::new(crate::store::Store {
                sqlite: connection.store.clone(),
            }),
            home.path().join("engineering"),
            true,
        ));
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let start = fixture_rpc(&mut socket).await;
            assert_eq!(start["method"], "thread/start");
            fixture_send(
                &mut socket,
                json!({"method":"thread/started","params":{"thread":{"id":"open-thread"}}}),
            )
            .await;
            fixture_send(
                &mut socket,
                json!({"id":start["id"],"result":{"thread":{"id":"open-thread","turns":[]}}}),
            )
            .await;
            let _ = socket.next().await;
        });
        let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
            params: Some(json!({})),
            text: None,
            context: None,
            thread: None,
        }));
        let (terminal, relay) = UnixStream::pair().unwrap();
        let relay_connection = connection.clone();
        let relay = tokio::spawn(async move {
            relay_connection
                .serve_with_input(relay, &endpoint, Some(input))
                .await
        });
        let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        fixture_send(
            &mut terminal,
            json!({"id":1,"method":"thread/start","params":{}}),
        )
        .await;
        let within = std::time::Duration::from_secs(5);
        let started = tokio::time::timeout(within, fixture_rpc(&mut terminal))
            .await
            .unwrap();
        assert_eq!(started["method"], "thread/started", "{started}");
        let response = tokio::time::timeout(within, fixture_rpc(&mut terminal))
            .await
            .unwrap();
        assert_eq!(
            response["result"]["thread"]["id"], "open-thread",
            "{response}"
        );

        // Still open: the pin exists before the terminal exits, in the
        // shared home.
        assert!(connection
            .store
            .provider_session_routed(Provider::Codex, &"open-thread".into())
            .unwrap());
        assert_eq!(
            connection
                .store
                .provider_session_account_home(Provider::Codex, &"open-thread".into())
                .unwrap(),
            None
        );
        terminal.close(None).await.unwrap();
        tokio::time::timeout(within, relay)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        provider.await.unwrap();
    }

    #[tokio::test]
    async fn rejected_and_unacknowledged_initial_input_keep_distinct_durable_receipts() {
        for outcome in [
            "rejected",
            "lost-before-acceptance",
            "lost-after-acceptance",
            "accepted",
        ] {
            let home = tempfile::tempdir_in("/tmp").unwrap();
            let connection = attached_connection(home.path());
            let endpoint = home.path().join("provider.sock");
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let start = fixture_rpc(&mut socket).await;
                // Reply-first ordering is valid; no thread/started is required.
                fixture_send(
                    &mut socket,
                    json!({"id":start["id"],"result":{
                    "thread":{"id":"saved-thread","turns":[]}}}),
                )
                .await;
                let turn = fixture_rpc(&mut socket).await;
                assert_eq!(turn["method"], "turn/start");
                assert_eq!(turn["params"]["input"][0]["text"], "retained draft");
                match outcome {
                    "rejected" => {
                        fixture_send(
                            &mut socket,
                            json!({"id":turn["id"],
                        "error":{"code":-32000,"message":"not accepted"}}),
                        )
                        .await
                    }
                    "accepted" => {
                        fixture_send(
                            &mut socket,
                            json!({"id":turn["id"],
                        "result":{"turn":{"id":"only-turn"}}}),
                        )
                        .await
                    }
                    // A provider may accept the turn yet lose both acknowledgement
                    // and notifications. Neither transport loss authorizes replay.
                    _ => {}
                }
                socket.close(None).await.unwrap();
            });
            let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
                params: Some(json!({})),
                text: Some("retained draft".into()),
                context: None,
                thread: None,
            }));
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay_connection = connection.clone();
            let relay_input = input.clone();
            let relay = tokio::spawn(async move {
                relay_connection
                    .serve_with_input(relay, &endpoint, Some(relay_input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            fixture_send(
                &mut terminal,
                json!({"id":1,"method":"thread/start","params":{}}),
            )
            .await;
            assert_eq!(
                fixture_rpc(&mut terminal).await["result"]["thread"]["id"],
                "saved-thread"
            );
            let error = format!("{:#}", relay.await.unwrap().unwrap_err());
            assert!(
                error.contains(if outcome == "rejected" {
                    "rejected"
                } else {
                    "provider closed"
                }),
                "{error}"
            );
            provider.await.unwrap();
            let reopened = CodexConnection {
                store: SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap(),
                ..connection
            };
            let expected = match outcome {
                "rejected" => super::NativeSubmission::InputRejected,
                "accepted" => super::NativeSubmission::InputAccepted,
                _ => super::NativeSubmission::InputDispatched,
            };
            assert_eq!(reopened.submission().unwrap(), Some(expected));
            assert_eq!(
                input.lock().await.text.as_deref(),
                (outcome == "rejected").then_some("retained draft")
            );
            assert_eq!(
                reopened
                    .store
                    .session_thread("conversation")
                    .unwrap()
                    .unwrap()
                    .as_str(),
                "saved-thread"
            );
            let recovered = reopened.recover_input("retained draft").unwrap();
            assert_eq!(
                recovered.as_deref(),
                (outcome == "rejected").then_some("retained draft")
            );
            let endpoint = home.path().join("resume.sock");
            let listener = UnixListener::bind(&endpoint).unwrap();
            let provider = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let start = fixture_rpc(&mut socket).await;
                assert_eq!(start["method"], "thread/resume");
                assert_eq!(start["params"]["threadId"], "saved-thread");
                let turns = if matches!(outcome, "lost-after-acceptance" | "accepted") {
                    json!([{"id":"only-turn","status":"completed","items":[{
                        "type":"agentMessage","text":"retained answer"}]}])
                } else {
                    json!([])
                };
                fixture_send(
                    &mut socket,
                    json!({"id":start["id"],"result":{
                    "thread":{"id":"saved-thread","turns":turns}}}),
                )
                .await;
                if outcome == "rejected" {
                    let turn = fixture_rpc(&mut socket).await;
                    assert_eq!(turn["params"]["input"][0]["text"], "retained draft");
                    fixture_send(
                        &mut socket,
                        json!({"id":turn["id"],"result":{
                        "turn":{"id":"only-turn"}}}),
                    )
                    .await;
                }
                // A barrier after the possible turn proves there was no replay.
                let barrier = fixture_rpc(&mut socket).await;
                assert_eq!(barrier["method"], "initialize");
                fixture_send(&mut socket, json!({"id":barrier["id"],"result":{}})).await;
                let _ = socket.next().await;
            });
            let (terminal, relay) = UnixStream::pair().unwrap();
            let relay_connection = reopened.clone();
            let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
                params: Some(json!({"threadId":"saved-thread"})),
                text: recovered,
                context: None,
                thread: None,
            }));
            let relay = tokio::spawn(async move {
                relay_connection
                    .serve_with_input(relay, &endpoint, Some(input))
                    .await
            });
            let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            fixture_send(
                &mut terminal,
                json!({"id":2,"method":"thread/start","params":{}}),
            )
            .await;
            assert_eq!(
                fixture_rpc(&mut terminal).await["result"]["thread"]["id"],
                "saved-thread"
            );
            fixture_send(
                &mut terminal,
                json!({"id":3,"method":"initialize","params":{}}),
            )
            .await;
            assert_eq!(fixture_rpc(&mut terminal).await["id"], 3);
            terminal.close(None).await.unwrap();
            drop(terminal);
            relay.await.unwrap().unwrap();
            provider.await.unwrap();
            if outcome == "lost-after-acceptance" {
                assert_eq!(
                    reopened.submission().unwrap(),
                    Some(super::NativeSubmission::InputAccepted)
                );
                assert!(reopened
                    .store
                    .session_history("conversation", 0, 0)
                    .unwrap()
                    .iter()
                    .any(|event| event.payload["text"] == "retained answer"));
            }
        }
    }

    #[tokio::test]
    async fn renderer_loss_during_initial_turn_retains_acknowledgement_without_replay() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let (dispatched, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let start = fixture_rpc(&mut socket).await;
            fixture_send(
                &mut socket,
                json!({"id":start["id"],"result":{
                    "thread":{"id":"saved-thread","turns":[]}}}),
            )
            .await;
            let turn = fixture_rpc(&mut socket).await;
            assert_eq!(turn["method"], "turn/start");
            assert_eq!(turn["params"]["input"][0]["text"], "saved skill");
            dispatched.send(()).unwrap();
            wait.await.unwrap();
            fixture_send(
                &mut socket,
                json!({"id":turn["id"],"result":{"turn":{"id":"only-turn"}}}),
            )
            .await;
            // The vanished renderer cannot cause another submission.
            assert!(!matches!(socket.next().await, Some(Ok(Message::Text(_)))));
        });
        let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
            params: Some(json!({})),
            text: Some("saved skill".into()),
            context: None,
            thread: None,
        }));
        let (terminal, relay) = UnixStream::pair().unwrap();
        let relay_connection = connection.clone();
        let relay = tokio::spawn(async move {
            relay_connection
                .serve_with_input(relay, &endpoint, Some(input))
                .await
        });
        let (mut terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
        fixture_send(
            &mut terminal,
            json!({"id":1,"method":"thread/start","params":{}}),
        )
        .await;
        assert_eq!(
            fixture_rpc(&mut terminal).await["result"]["thread"]["id"],
            "saved-thread"
        );
        ready.await.unwrap();
        drop(terminal);
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), relay)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        provider.await.unwrap();
        let reopened = CodexConnection {
            store: SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap(),
            ..connection
        };
        assert_eq!(
            reopened.submission().unwrap(),
            Some(super::NativeSubmission::InputAccepted)
        );
        assert_eq!(reopened.recover_input("saved skill").unwrap(), None);
        assert_eq!(
            reopened
                .store
                .session_thread("conversation")
                .unwrap()
                .unwrap()
                .as_str(),
            "saved-thread"
        );
    }

    #[tokio::test]
    async fn overlapping_reconnect_waits_for_original_identity_without_second_creation() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut first = accept_async(stream).await.unwrap();
            let start = fixture_rpc(&mut first).await;
            assert_eq!(start["method"], "thread/start");
            started.send(()).unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut second = accept_async(stream).await.unwrap();
            wait.await.unwrap();
            fixture_send(
                &mut first,
                json!({"id":start["id"],"result":{
                "thread":{"id":"only-thread","turns":[]}}}),
            )
            .await;
            let resume = fixture_rpc(&mut second).await;
            assert_eq!(resume["method"], "thread/resume");
            fixture_send(
                &mut second,
                json!({"id":resume["id"],"result":{
                "thread":{"id":"only-thread","turns":[]}}}),
            )
            .await;
            assert_eq!(fixture_rpc(&mut second).await["method"], "turn/start");
            // No first turn was sent to the vanished renderer's connection.
            let _ = first.next().await;
        });
        let input = Arc::new(tokio::sync::Mutex::new(NativeInput {
            params: Some(json!({})),
            text: Some("once".into()),
            context: None,
            thread: None,
        }));
        let mut relays = Vec::new();
        let mut terminals = Vec::new();
        for _ in 0..2 {
            let (terminal, relay) = UnixStream::pair().unwrap();
            let connection = connection.clone();
            let endpoint = endpoint.clone();
            let input = input.clone();
            relays.push(tokio::spawn(async move {
                connection
                    .serve_with_input(relay, &endpoint, Some(input))
                    .await
            }));
            let (terminal, _) = client_async("ws://localhost", terminal).await.unwrap();
            terminals.push(terminal);
        }
        fixture_send(
            &mut terminals[0],
            json!({"id":1,"method":"thread/start","params":{}}),
        )
        .await;
        ready.await.unwrap();
        fixture_send(
            &mut terminals[1],
            json!({"id":2,"method":"thread/start","params":{}}),
        )
        .await;
        assert_eq!(
            fixture_rpc(&mut terminals[1]).await["error"]["code"],
            -32001
        );
        let second = terminals.pop().unwrap();
        drop(terminals); // Original reply is still outstanding.
        release.send(()).unwrap();
        relays.remove(0).await.unwrap().unwrap();
        let mut second = second;
        fixture_send(
            &mut second,
            json!({"id":3,"method":"thread/start","params":{}}),
        )
        .await;
        assert_eq!(
            fixture_rpc(&mut second).await["result"]["thread"]["id"],
            "only-thread"
        );
        // Provider exit after receiving the one input is an uncertain acknowledgement.
        assert!(relays.remove(0).await.unwrap().is_err());
        provider.await.unwrap();
        assert!(input.lock().await.text.is_none());
    }

    #[tokio::test]
    async fn stale_attachment_cannot_record_a_dispatch_or_send_initial_input() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let connection = attached_connection(home.path());
        let attachment = connection.attachment.as_ref().unwrap();
        connection
            .store
            .claim_session_attachment(
                "conversation",
                Some(attachment),
                attachment.lf_process_id.as_ref().unwrap(),
                false,
            )
            .unwrap();
        let (client, server) = UnixStream::pair().unwrap();
        let provider = tokio::spawn(async move {
            let mut socket = accept_async(server).await.unwrap();
            // Closing an unused transport must be the only provider observation.
            assert!(!matches!(socket.next().await, Some(Ok(Message::Text(_)))));
        });
        let (upstream, _) = client_async("ws://localhost", client).await.unwrap();
        let input = connection
            .store
            .session("conversation")
            .unwrap()
            .unwrap()
            .artifact_key
            .unwrap();
        let (upstream, result) = connection
            .send(
                upstream,
                Message::Text(json!({"id":1,"method":"turn/start"}).to_string().into()),
                Some((&input, super::NativeSubmission::InputDispatched)),
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            Err(crate::store::StoreError::InvalidAuthority(_))
        ));
        assert_eq!(connection.submission().unwrap(), None);
        drop(upstream);
        provider.await.unwrap();
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
            store.test_session("conversation", &crate::session_record::new_artifact_key());
            let connection = CodexConnection {
                store,
                session_id: "conversation".into(),
                thread_id: None,
                attachment: None,
                account_route: None,
            };
            let (terminal, relay) = UnixStream::pair().unwrap();
            let store = connection.store.clone();
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
            let diagnostics: Vec<_> = store
                .session_history("conversation", 0, 0)
                .unwrap()
                .into_iter()
                .filter(|event| event.payload["source"] == "codex_relay")
                .collect();
            assert_eq!(diagnostics.len(), usize::from(!handshake));
        }
    }

    #[tokio::test]
    async fn relay_carries_provider_messages_beyond_sixteen_mebibytes() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let catalog = json!({"id": 1, "result": {"plugins": "x".repeat(17 << 20)}}).to_string();
        let reply = catalog.clone();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            socket.next().await.unwrap().unwrap();
            socket.send(Message::Text(reply.into())).await.unwrap();
            let _ = socket.next().await;
        });
        let connection = attached_connection(home.path());
        let (terminal, relay) = UnixStream::pair().unwrap();
        let relay = tokio::spawn(async move { connection.serve(relay, &endpoint).await });
        let mut terminal = super::connect_upstream(terminal).await.unwrap();
        terminal
            .send(Message::Text(
                r#"{"id":1,"method":"plugin/list","params":{}}"#.into(),
            ))
            .await
            .unwrap();
        let received = terminal.next().await.unwrap().unwrap();
        assert_eq!(received.into_text().unwrap().len(), catalog.len());
        terminal.close(None).await.unwrap();
        drop(terminal);
        tokio::time::timeout(std::time::Duration::from_secs(5), relay)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        provider.await.unwrap();
    }
}
