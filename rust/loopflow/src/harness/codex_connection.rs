//! A native client may keep displaying a conversation after attachment transfer.
//! Its writes must still pass the Session fence at dispatch, including approval
//! replies. The same transport relays to the existing AgentProcess and inspects
//! it during attachment settlement, under the ownership fence.

use crate::id::AgentSessionId;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio_tungstenite::{accept_async, client_async, tungstenite::Message, WebSocketStream};

use crate::process::SessionAttachment;
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

/// Called under the Session attachment lock, so takeover cannot race the provider
/// shutdown. Saved history and the provider thread ID survive. The endpoint and thread
/// must come from the record; an AgentProcess that also serves an unrelated
/// conversation is left running, and that is an error.
pub(crate) fn close_agent_process(
    (endpoint, thread): (&str, &AgentSessionId),
    pid: u32,
    started: i64,
) -> Result<()> {
    let same_process = || -> Result<bool> {
        match crate::journal::process_identity_evidence(pid, started) {
            crate::journal::ProcessIdentityEvidence::Live => Ok(true),
            crate::journal::ProcessIdentityEvidence::Dead => Ok(false),
            crate::journal::ProcessIdentityEvidence::Unknown => {
                Err(anyhow!("AgentProcess OS identity is unavailable"))
            }
        }
    };
    if !same_process()? {
        return Ok(());
    }
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
    })?;
    if !same_process()? {
        return Ok(());
    }
    let group = i32::try_from(pid)?;
    // SAFETY: getpgid reads process metadata. Only the exact recorded process
    // leading the group that Loopflow created may authorize a group signal.
    let owner = unsafe { libc::getpgid(group) };
    if owner == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        // It exited on its own since the check above; reap it if it is ours.
        // SAFETY: WNOHANG only reaps our own exited child.
        unsafe {
            libc::waitpid(group, std::ptr::null_mut(), libc::WNOHANG);
        }
        return Ok(());
    }
    if owner != group {
        return Err(anyhow!(
            "recorded process does not lead its own process group"
        ));
    }
    // The leader may exit before its helpers. Use the same group-wide death
    // judgment as scheduled settlement rather than ending on leader death.
    if crate::os_process::terminate_process_group(pid) {
        Ok(())
    } else {
        Err(anyhow!("AgentProcess group {pid} death is unresolved"))
    }
}

async fn inspect_agent_threads(endpoint: &str, thread: &AgentSessionId) -> Result<()> {
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
            while current != thread.as_str() {
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
    pub thread_id: AgentSessionId,
    pub attachment: Option<SessionAttachment>,
}

impl CodexConnection {
    /// Read all native turn pages before displaying the conversation. This
    /// connection never subscribes, answers approvals, or acquires an attachment.
    pub async fn recover_history(&self, endpoint: &Path) -> Result<()> {
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
            super::dispatch::off_reactor(|| {
                history.record(
                    &self.store,
                    &self.session_id,
                    None,
                    Some(&self.thread_id),
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
        let mut client = accept_async(client).await?;
        let (mut upstream, _) =
            client_async("ws://localhost", UnixStream::connect(endpoint).await?).await?;
        let mut history = super::codex_history::History::default();
        loop {
            tokio::select! {
                incoming = client.next() => {
                    let Some(incoming) = incoming else { break };
                    let Message::Text(text) = incoming? else { continue };
                    let mut rpc: Value = serde_json::from_str(&text)?;
                    let method = rpc.get("method").and_then(Value::as_str).unwrap_or_default();
                    let target = rpc.pointer("/params/threadId").and_then(Value::as_str);
                    let wrong_thread = target.is_some_and(|id| id != self.thread_id.as_str());
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
                    history.request(&rpc, self.attachment.as_ref().map(|attachment| (&self.store,self.session_id.as_str(),attachment)))?;
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
                            super::dispatch::off_reactor(|| {
                                history.record(&self.store, &self.session_id,
                                    self.attachment.as_ref(), Some(&self.thread_id), &rpc)
                            })?;
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
            let outcome = store.with_session_attachment(&session, &attachment, || {
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

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};

    struct Group(Child);

    impl Drop for Group {
        fn drop(&mut self) {
            crate::os_process::terminate_process_group(self.0.id());
            let _ = self.0.wait();
        }
    }

    #[test]
    fn close_waits_for_helpers_after_the_leader_exits() {
        let home = tempfile::tempdir().unwrap();
        let mut group = Group(
            Command::new("/bin/sh")
                .env_clear()
                .args([
                    "-c",
                    "trap 'exit 0' TERM; /bin/sh -c 'trap \"\" TERM; printf \"%s\\n\" $$; exec /bin/sleep 60' & wait",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        // The helper has installed its TERM handler before close can signal it.
        let mut ready = String::new();
        BufReader::new(group.0.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        let helper: u32 = ready.trim().parse().unwrap();
        let leader = crate::journal::OsProcess::read(group.0.id())
            .unwrap()
            .unwrap();
        let child = crate::journal::OsProcess::read(helper).unwrap().unwrap();
        super::close_agent_process(
            (
                home.path().join("absent.sock").to_str().unwrap(),
                &"saved".into(),
            ),
            leader.pid,
            leader.started_at,
        )
        .unwrap();
        assert!(!crate::journal::OsProcess::group_is_alive(leader.pid).unwrap());
        assert_eq!(
            crate::journal::process_identity_evidence(helper, child.started_at),
            crate::journal::ProcessIdentityEvidence::Dead
        );
    }
}
