//! lf's side connections to a Codex app-server. The native client talks to the
//! app-server directly: lf opens the conversation, and the Machine server reads
//! its turns when a turn-end hook says one finished. Nothing here carries the
//! person's bytes, so a store failure is a recording gap and never reaches the
//! conversation.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio_tungstenite::{
    client_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Error, Message},
    WebSocketStream,
};

use super::codex::CodexStream;
use super::fact_writer::FactWriter;
use crate::id::AgentSessionId;
use crate::store::sqlite::SqliteStore;

type Upstream = WebSocketStream<UnixStream>;

const REPLY_LIMIT: Duration = Duration::from_secs(15);
/// Starting a thread loads MCP servers and plugins.
const STARTUP_LIMIT: Duration = Duration::from_secs(60);

// Codex sends single frames larger than tungstenite's defaults: plugin/list
// carries the whole curated marketplace (7,000+ plugins, over 16 MiB).
fn connection_config() -> Option<WebSocketConfig> {
    Some(
        WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None),
    )
}

pub(super) async fn connect_upstream(socket: UnixStream) -> Result<Upstream, Error> {
    Ok(
        client_async_with_config("ws://localhost", socket, connection_config())
            .await?
            .0,
    )
}

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

async fn initialized(endpoint: &Path, name: &str) -> Result<Upstream> {
    let mut upstream = connect_upstream(UnixStream::connect(endpoint).await?).await?;
    rpc_request(
        &mut upstream,
        "initialize",
        json!({
            "clientInfo": {"name":name, "version":env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi":true}
        }),
    )
    .await?;
    upstream
        .send(Message::Text(
            json!({"method":"initialized"}).to_string().into(),
        ))
        .await?;
    Ok(upstream)
}

/// Read all native turn pages before displaying the conversation, and say how
/// many there are. A thread with turns has received its first input, so history
/// settles what a dispatch receipt could only guess at.
pub(crate) async fn recover_history(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    endpoint: &Path,
) -> Result<usize> {
    let mut upstream = initialized(endpoint, "loopflow_history").await?;
    let mut history = CodexStream::new(FactWriter::new(
        store.clone(),
        session.to_owned(),
        None,
        None,
        Default::default(),
    ));
    let mut cursor = Value::Null;
    let mut count = 0;
    loop {
        let result = rpc_request(
            &mut upstream,
            "thread/turns/list",
            json!({
                "threadId":thread, "cursor":cursor, "limit":100,
                "sortDirection":"asc", "itemsView":"notLoaded"
            }),
        )
        .await?;
        count += result["data"]
            .as_array()
            .ok_or_else(|| anyhow!("Native history response has no turns"))?
            .len();
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
    Ok(count)
}

/// Send the launch's first turn, recorded as lf's input, before the native
/// client joins. Codex creates the turn's rollout before answering.
pub(crate) async fn start_turn(
    store: &SqliteStore,
    session: &str,
    attachment: &crate::process::SessionAttachment,
    endpoint: &Path,
    thread: &AgentSessionId,
    text: &str,
) -> Result<()> {
    let mut upstream = initialized(endpoint, "loopflow_start").await?;
    let mut history = CodexStream::new(FactWriter::new(
        store.clone(),
        session.to_owned(),
        Some(attachment.clone()),
        None,
        Default::default(),
    ));
    let id = format!("lf-initial-{}", uuid::Uuid::new_v4());
    let request = json!({"id":id,"method":"turn/start",
        "params":{"threadId":thread,"input":[{"type":"text","text":text}]}});
    if let Err(error) = history.outbound(&request).await {
        tracing::warn!(error = %format!("{error:#}"), session, "Codex first turn unrecorded");
    }
    upstream
        .send(Message::Text(request.to_string().into()))
        .await?;
    let reply = reply_within(&mut upstream, &id, "turn/start", REPLY_LIMIT).await?;
    if let Err(error) = history
        .inbound(&json!({"id":id,"result":reply}), Some(thread))
        .await
        .1
    {
        tracing::warn!(error = %format!("{error:#}"), session, "Codex first turn unrecorded");
    }
    upstream.close(None).await?;
    Ok(())
}

/// Wake an idle conversation with steers and a Task update, as a tool result Codex
/// keeps in its history and answers in a turn of its own.
pub(crate) async fn deliver_update(endpoint: &Path, thread: &str, text: &str) -> Result<()> {
    let mut upstream = initialized(endpoint, "loopflow_update").await?;
    rpc_request(
        &mut upstream,
        "turn/start",
        json!({"threadId":thread,"input":[],
            "toolOutput":{"name":"loopflow_wake","output":text,"retain":true}}),
    )
    .await?;
    upstream.close(None).await?;
    Ok(())
}

/// Start or resume the conversation with lf's prepared configuration, so the
/// native client only has to join it. Codex cannot resume a thread with no
/// rollout, and creates one on the first turn or a name (verified on 0.166.0);
/// naming it lets the client attach before the first turn.
pub(crate) async fn open_thread(
    endpoint: &Path,
    params: Value,
    name: &str,
) -> Result<AgentSessionId> {
    let mut upstream = initialized(endpoint, "loopflow_start").await?;
    let mut params = params;
    let thread = match params["threadId"].as_str() {
        Some(thread) => {
            let thread = AgentSessionId::from(thread);
            rpc_within(&mut upstream, "thread/resume", params, STARTUP_LIMIT).await?;
            thread
        }
        None => {
            // What Codex's own terminal asks for. Its `codex_tui` dynamic tools
            // stay out: only the client that declares them can describe them.
            params["threadSource"] = json!("user");
            params["historyMode"] = json!("paginated");
            let started = rpc_within(&mut upstream, "thread/start", params, STARTUP_LIMIT).await?;
            let thread = started
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .map(AgentSessionId::from)
                .ok_or_else(|| anyhow!("Codex returned no native conversation ID"))?;
            rpc_request(
                &mut upstream,
                "thread/name/set",
                json!({"threadId":thread, "name":name}),
            )
            .await?;
            thread
        }
    };
    upstream.close(None).await?;
    Ok(thread)
}

async fn rpc_request(upstream: &mut Upstream, method: &str, params: Value) -> Result<Value> {
    rpc_within(upstream, method, params, REPLY_LIMIT).await
}

async fn rpc_within(
    upstream: &mut Upstream,
    method: &str,
    params: Value,
    limit: Duration,
) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    upstream
        .send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))
        .await?;
    reply_within(upstream, &id, method, limit).await
}

/// The result of request `id`, skipping notifications and other replies.
async fn reply_within(
    upstream: &mut Upstream,
    id: &str,
    method: &str,
    limit: Duration,
) -> Result<Value> {
    tokio::time::timeout(limit, async {
        while let Some(message) = upstream.next().await {
            let Message::Text(text) = message? else {
                continue;
            };
            let rpc: Value = serde_json::from_str(&text)?;
            if rpc["id"].as_str() != Some(id) {
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

#[cfg(test)]
mod tests {
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::time::Duration;
    use tokio::net::{UnixListener, UnixStream};
    use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

    type Provider = WebSocketStream<UnixStream>;

    async fn receive(socket: &mut Provider) -> Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(8), socket.next())
                .await
                .expect("fixture provider waited too long")
                .unwrap()
                .unwrap();
            if let Message::Text(text) = message {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    async fn send(socket: &mut Provider, value: Value) {
        socket
            .send(Message::Text(value.to_string().into()))
            .await
            .unwrap();
    }

    /// An idle conversation wakes on a turn whose input is the update, kept as
    /// a tool result in its history.
    #[tokio::test]
    async fn an_update_starts_a_turn_on_the_conversations_app_server() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let initialize = receive(&mut socket).await;
            send(&mut socket, json!({"id":initialize["id"],"result":{}})).await;
            assert_eq!(receive(&mut socket).await["method"], "initialized");
            let start = receive(&mut socket).await;
            send(
                &mut socket,
                json!({"id":start["id"],"result":{"turn":{"id":"report"}}}),
            )
            .await;
            let _ = socket.next().await;
            start
        });
        super::deliver_update(&endpoint, "native-thread", "gate: failed")
            .await
            .unwrap();
        let start = provider.await.unwrap();
        assert_eq!(start["method"], "turn/start");
        assert_eq!(start["params"]["threadId"], "native-thread");
        assert_eq!(start["params"]["input"], json!([]));
        assert_eq!(
            start["params"]["toolOutput"],
            json!({"name":"loopflow_wake","output":"gate: failed","retain":true})
        );
    }

    #[tokio::test]
    async fn an_opened_thread_is_named_so_a_client_can_resume_it() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let initialize = receive(&mut socket).await;
            send(&mut socket, json!({"id":initialize["id"],"result":{}})).await;
            assert_eq!(receive(&mut socket).await["method"], "initialized");
            let start = receive(&mut socket).await;
            assert_eq!(start["method"], "thread/start");
            assert_eq!(start["params"]["cwd"], "/work");
            send(
                &mut socket,
                json!({"id":start["id"],"result":{"thread":{"id":"created"}}}),
            )
            .await;
            let name = receive(&mut socket).await;
            assert_eq!(name["method"], "thread/name/set");
            assert_eq!(name["params"]["threadId"], "created");
            send(&mut socket, json!({"id":name["id"],"result":{}})).await;
            let _ = socket.next().await;
        });
        let thread = super::open_thread(&endpoint, json!({"cwd":"/work"}), "Fix login")
            .await
            .unwrap();
        assert_eq!(thread.as_str(), "created");
        provider.await.unwrap();
    }
}
