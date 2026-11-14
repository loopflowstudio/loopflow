//! The Machine server retains native recovery admission and token callbacks.
//! The TUI and holder stay connected directly to the provider.
use std::time::{Duration, SystemTime};

use anyhow::Result;
use futures_util::{stream::SplitSink, SinkExt};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::codex::CodexStream;
use super::codex_connection::Upstream;
use super::fact_writer::FactWriter;
use super::recovery_gate::{Admission, RecoveryGate};
use crate::provider_account;
use crate::provider_auth::Provider;
use crate::server::recovery::Client;
use crate::session_recovery::{codex_rejected_turn, RecoveryState, SessionRecovery};
use crate::store::sqlite::SqliteStore;
use futures_util::StreamExt;

struct Connection {
    client: Client,
    route: Option<provider_account::ProviderAccountRoute>,
    pin_pending: bool,
    failure: crate::store::RecordingGap,
    store: SqliteStore,
    history: CodexStream,
    busy: bool,
    recovery: Recovery,
}

pub(crate) async fn run(
    store: SqliteStore,
    client: Client,
    mut ticks: tokio::sync::watch::Receiver<u64>,
) -> Result<()> {
    store.with_provider_context(
        &client.adapter,
        client.thread.as_str(),
        crate::steer::NativeBoundary::Turn,
        || Ok(()),
    )?;
    let account = crate::server::recovery::account(&store, "codex", client.thread.as_str())?;
    let route = match account {
        Some(account) => {
            provider_account::session_account_route(Provider::Codex, &client.thread, &account)
                .await?
        }
        None => None,
    };
    let mut upstream = super::codex_connection::initialized(
        client.endpoint.as_ref().expect("Codex endpoint"),
        "loopflow_recovery",
    )
    .await?;
    let result = super::codex_connection::rpc_request(
        &mut upstream,
        "thread/resume",
        json!({"threadId":client.thread}),
    )
    .await?;
    let busy = result["thread"]["status"]["type"] == "active";
    let history = CodexStream::new(FactWriter::new(
        store.clone(),
        client.adapter.session.clone(),
        None,
        route.clone(),
        Default::default(),
    ));
    let mut connection = Connection {
        client,
        route,
        pin_pending: false,
        failure: Default::default(),
        store,
        history,
        busy,
        recovery: Default::default(),
    };
    let (mut writer, mut reader) = upstream.split();
    connection.recovery_tick(&mut writer).await?;
    loop {
        tokio::select! {
            changed = ticks.changed() => {
                if changed.is_err() { break; }
                let current = connection.store.with_provider_context(&connection.client.adapter,connection.client.thread.as_str(),crate::steer::NativeBoundary::Turn, || Ok(()));
                if matches!(current, Err(crate::store::StoreError::InvalidAuthority(_))) { break; }
                match connection.recovery_tick(&mut writer).await {
                    Ok(()) => connection.failure.recovered(),
                    Err(error) => connection.failure.note("Codex recovery", &error),
                }
            }
            message = reader.next() => {
                let Some(message) = message else { break };
                let Message::Text(text) = message? else { continue };
                let rpc: Value = serde_json::from_str(&text)?;
                let ours = rpc["params"]["threadId"].as_str() == Some(connection.client.thread.as_str());
                if ours && rpc["method"] == "turn/started" { connection.busy = true; }
                if ours && rpc["method"] == "turn/completed" { connection.busy = false; }
                if let Err(error) = connection.history.inbound(&rpc, Some(&connection.client.thread)).await.1 {
                    connection.failure.note("Codex recovery history", &error);
                }
                if let Err(error) = connection.recovery_message(&rpc,&mut writer).await {
                    connection.failure.note("Codex recovery", &error);
                }
            }
        }
        // Once no native login was installed, a settled attempt has no connection to retain.
        let Ok(retained) = connection
            .store
            .session_recovery(&connection.client.adapter.session)
        else {
            continue;
        };
        if retained.is_none_or(|r| !r.can_retry() && r.request.is_none())
            && connection.recovery.installing.is_none()
            && connection.recovery.preparing.is_none()
        {
            break;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct HistoryRead {
    id: Value,
    failed_turn: String,
    sent_at: SystemTime,
}

#[derive(Debug)]
pub(super) struct Recovery {
    pending: Option<HistoryRead>,
    check_at: SystemTime,
    requested: i64,
    preparing: Option<std::thread::JoinHandle<Result<Prepared>>>,
    installing: Option<(Value, Prepared)>,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            pending: None,
            check_at: SystemTime::UNIX_EPOCH,
            requested: 0,
            preparing: None,
            installing: None,
        }
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        // Native exit can outlive a refresh response. Keep this connection until
        // the broker has stored any rotated grant; dropping admission then
        // closes its socket without releasing the native prompt.
        if let Some(worker) = self.preparing.take() {
            if worker.join().is_err() {
                tracing::warn!("Recovery credential worker panicked during connection shutdown");
            }
        }
    }
}

#[derive(Debug)]
struct Prepared {
    admission: Admission,
    route: provider_account::ProviderAccountRoute,
    tokens: secrecy::SecretString,
    recovery: SessionRecovery,
}

/// Keep credential preparation off the native connection. This worker owns the
/// admission until the server can install the prepared login on its connection.
fn prepare(
    gate: RecoveryGate,
    client: Client,
    store: SqliteStore,
    route: provider_account::ProviderAccountRoute,
    mut recovery: SessionRecovery,
) -> Result<Prepared> {
    let admission = gate.accept()?;
    let turn = super::recovery_gate::codex_current(&admission, &recovery)?;
    let current = store.session_recovery(&client.adapter.session)?;
    anyhow::ensure!(
        current
            .as_ref()
            .is_some_and(|r| r.key() == recovery.key() && r.request == recovery.request),
        "Recovery was superseded"
    );
    anyhow::ensure!(
        store.with_provider_context(
            &client.adapter,
            client.thread.as_str(),
            crate::steer::NativeBoundary::Turn,
            || { store.record_session_recovery(&client.adapter.session, &recovery, "admitted") }
        )?,
        "Recovery attempt was already admitted"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let tokens = runtime.block_on(route.prepare_codex_auth_tokens(&|| {
        admission
            .check()
            .map_err(|error| provider_account::ProviderAccountError::Runtime(error.to_string()))
    }))?;
    // Never cancel the broker future: it owns any rotated grant.
    admission.check()?;
    recovery.retry_turn = Some(turn);
    Ok(Prepared {
        admission,
        route,
        tokens,
        recovery,
    })
}

impl Connection {
    async fn pin_route(&mut self) {
        if !self.pin_pending {
            return;
        }
        if let Some(route) = &self.route {
            match route.pin_session(&self.client.thread).await {
                Ok(()) => self.pin_pending = false,
                Err(error) => self.failure.note("native recovery login", &error),
            }
        }
    }

    pub(super) async fn recovery_message(
        &mut self,
        rpc: &Value,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        if self
            .recovery
            .installing
            .as_ref()
            .is_some_and(|(id, _)| rpc.get("id") == Some(id))
        {
            let (_, prepared) = self
                .recovery
                .installing
                .take()
                .expect("matched installation");
            let mut recovery = prepared.recovery;
            if rpc.get("error").is_some()
                || rpc.pointer("/result/type").and_then(Value::as_str) != Some("chatgptAuthTokens")
            {
                recovery.state = RecoveryState::Uncertain;
                recovery.reason =
                    "Native account installation was not confirmed; inspect the conversation"
                        .into();
                self.save_recovery(&recovery, "unconfirmed")?;
                return Ok(());
            }
            // Native identity advances before best-effort recording. A store
            // gap cannot leave token callbacks refreshing the displaced login.
            self.route = Some(prepared.route.clone());
            self.pin_pending = true;
            self.history.writer.set_route(prepared.route);
            self.pin_route().await;
            if prepared.admission.check().is_err() {
                recovery.state = RecoveryState::Uncertain;
                recovery.reason = "The login changed but native retry admission ended; continue from the conversation".into();
                self.save_recovery(&recovery, "unconfirmed")?;
                return Ok(());
            }
            recovery.state = RecoveryState::Recovering;
            recovery.reason = "The admitted native retry is running".into();
            self.save_recovery(&recovery, "reply")?;
            prepared.admission.finish()?;
            return Ok(());
        }
        if rpc["method"] == "account/chatgptAuthTokens/refresh" {
            let response = match &self.route {
                Some(route) => route.refresh_codex_auth_tokens().await.ok(),
                None => None,
            };
            let reply = match response {
                Some(tokens) => {
                    let tokens: Value = serde_json::from_str(tokens.expose_secret())?;
                    if tokens["chatgptAccountId"] != rpc["params"]["previousAccountId"] {
                        json!({"id":rpc["id"],"error":{"code":-32603,"message":"The requested login changed; refresh was not applied"}})
                    } else {
                        json!({"id":rpc["id"], "result":tokens})
                    }
                }
                None => {
                    json!({"id":rpc["id"],"error":{"code":-32603,"message":"Loopflow could not refresh this login"}})
                }
            };
            writer.send(Message::Text(reply.to_string().into())).await?;
            return Ok(());
        }
        if let Some(pending) = &self.recovery.pending {
            if rpc.get("id") == Some(&pending.id) {
                let pending = self
                    .recovery
                    .pending
                    .take()
                    .expect("matched recovery request");
                self.recovery_reply(pending.failed_turn, rpc, writer)
                    .await?;
                return Ok(());
            }
        }
        if rpc["params"]["threadId"] != self.client.thread.as_str() {
            return Ok(());
        }
        if rpc["method"] == "item/completed" {
            let session = self.client.adapter.session.clone();
            let recovery = self.store.session_recovery(&session)?;
            if let Some(recovery) = recovery.filter(|r| {
                r.retry_turn.as_deref() == rpc["params"]["turnId"].as_str()
                    && super::recovery_gate::matches_input(&rpc["params"]["item"], &session, r)
            }) {
                self.save_recovery(&recovery, "input")?;
            }
        }
        if rpc["method"] == "turn/completed" {
            let turn = &rpc["params"]["turn"];
            let Some(id) = turn["id"].as_str() else {
                return Ok(());
            };
            if turn
                .pointer("/error/codexErrorInfo")
                .and_then(Value::as_str)
                == Some("usageLimitExceeded")
            {
                let recovery = SessionRecovery {
                    agent_process_id: self.client.adapter.agent_process_id.clone(),
                    thread: self.client.thread.clone(),
                    failed_turn: id.into(),
                    model: None,
                    account: self.route.as_ref().map(|route| route.account_id().clone()),
                    state: RecoveryState::WaitingNative,
                    reason: "Reading the rejected turn's native history".into(),
                    retry_at: None,
                    request: None,
                    retry_turn: None,
                };
                self.save_recovery(&recovery, "reported")?;
                if self.recovery.pending.is_none() {
                    self.read_recovery(id, writer).await?;
                }
            }
        }
        Ok(())
    }

    pub(super) async fn recovery_tick(
        &mut self,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        self.pin_route().await;
        if self
            .recovery
            .preparing
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            let worker = self
                .recovery
                .preparing
                .take()
                .expect("finished preparation");
            match worker
                .join()
                .map_err(|_| anyhow::anyhow!("Recovery preparation panicked"))?
            {
                Ok(prepared) => {
                    prepared.admission.check()?;
                    anyhow::ensure!(
                        prepared.recovery.thread == self.client.thread,
                        "Recovery thread changed"
                    );
                    let session = self.client.adapter.session.clone();
                    let current = self.store.session_recovery(&session)?;
                    anyhow::ensure!(
                        current
                            .as_ref()
                            .is_some_and(|r| r.key() == prepared.recovery.key()
                                && r.request == prepared.recovery.request),
                        "Recovery was superseded"
                    );
                    let id = json!(format!("lf-login-{}", uuid::Uuid::new_v4()));
                    let mut tokens: Value = serde_json::from_str(prepared.tokens.expose_secret())?;
                    tokens["type"] = json!("chatgptAuthTokens");
                    let rpc = json!({"id":id,"method":"account/login/start","params":tokens});
                    writer.send(Message::Text(rpc.to_string().into())).await?;
                    self.recovery.installing = Some((id, prepared));
                }
                Err(error) => tracing::warn!(%error, "Native recovery preparation stopped"),
            }
        }
        if let Some((_, prepared)) = self.recovery.installing.as_mut() {
            if prepared.recovery.state != RecoveryState::Uncertain
                && prepared.admission.check().is_err()
            {
                prepared.recovery.state = RecoveryState::Uncertain;
                prepared.recovery.reason = "Native installation or admission did not finish; continue from the conversation".into();
                let recovery = prepared.recovery.clone();
                // A late successful installation reply still updates the broker's
                // route for subsequent typing, but can never release this gate.
                self.save_recovery(&recovery, "unconfirmed")?;
            }
        }
        if self.recovery.pending.as_ref().is_some_and(|pending| {
            pending
                .sent_at
                .elapsed()
                .is_ok_and(|elapsed| elapsed >= Duration::from_secs(15))
        }) {
            // History reads have no native effect and can be repeated.
            self.recovery.pending = None;
        }
        let session = self.client.adapter.session.clone();
        let requested = self.store.recovery_request_revision(&session)?;
        if requested != self.recovery.requested {
            self.recovery.requested = requested;
            self.recovery.check_at = SystemTime::UNIX_EPOCH;
        }
        // Completion can precede the RPC reply or arrive during a disconnected
        // connection. Settle from the same retained native receipt in either case.
        let mut recovery = self.store.session_recovery(&session)?;
        if let Some(recovery) = recovery
            .as_mut()
            .filter(|r| r.state != RecoveryState::Continued)
        {
            if recovery.state == RecoveryState::Recovering
                && recovery.request.is_some()
                && recovery.retry_turn.is_none()
                && self.recovery.preparing.is_none()
                && self.recovery.installing.is_none()
            {
                recovery.state = RecoveryState::Uncertain;
                recovery.reason = "The server reconnected without a native retry reply; inspect the conversation before continuing".into();
                self.save_recovery(recovery, "unconfirmed")?;
            }
            if let Some(turn) = &recovery.retry_turn {
                let result = self
                    .store
                    .session_turn_result(&session, &recovery.thread, turn)?;
                let received = self.store.has_recovery_phase(&session, recovery, "input")?;
                if let Some(result) = result.filter(|_| received) {
                    recovery.state = if result["status"] == "completed" {
                        RecoveryState::Continued
                    } else {
                        RecoveryState::Uncertain
                    };
                    recovery.reason = if recovery.state == RecoveryState::Continued {
                        "The native retry completed"
                    } else {
                        "The retry stopped; inspect its history before continuing"
                    }
                    .into();
                    self.save_recovery(recovery, "settled")?;
                }
            }
        }
        if self.busy
            || self.recovery.pending.is_some()
            || SystemTime::now() < self.recovery.check_at
        {
            return Ok(());
        }
        self.recovery.check_at = SystemTime::now() + Duration::from_secs(60);
        if let Some(recovery) = recovery
            .filter(|r| r.thread == self.client.thread && (r.can_retry() || r.request.is_some()))
        {
            self.read_recovery(&recovery.failed_turn, writer).await?;
        }
        Ok(())
    }

    async fn has_recovery_guard(&self) -> Result<bool> {
        let mut upstream = super::codex_connection::initialized(
            self.client
                .endpoint
                .as_ref()
                .expect("Codex has an endpoint"),
            "loopflow_recovery_hooks",
        )
        .await?;
        let hooks = super::codex_connection::rpc_request(
            &mut upstream,
            "hooks/list",
            json!({"threadId":self.client.thread}),
        )
        .await?;
        let expected = super::recovery_gate::hook_command(&crate::store::lf_home_dir())?;
        Ok(hooks["data"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|entry| entry["hooks"].as_array().into_iter().flatten())
            .any(|hook| {
                hook["eventName"] == "userPromptSubmit"
                    && hook["enabled"] == true
                    && hook["trustStatus"] == "trusted"
                    && hook["command"] == expected
                    && hook["timeoutSec"]
                        .as_u64()
                        .is_some_and(|timeout| timeout >= 60)
            }))
    }

    async fn read_recovery(
        &mut self,
        turn: &str,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        let id = json!(format!("lf-recovery-{}", uuid::Uuid::new_v4()));
        let rpc = json!({"id":id,"method":"thread/read",
            "params":{"threadId":self.client.thread,"includeTurns":true}});
        writer.send(Message::Text(rpc.to_string().into())).await?;
        self.recovery.pending = Some(HistoryRead {
            id,
            failed_turn: turn.into(),
            sent_at: SystemTime::now(),
        });
        Ok(())
    }

    fn save_recovery(&mut self, recovery: &SessionRecovery, phase: &str) -> Result<bool> {
        Ok(self.store.with_provider_context(
            &self.client.adapter,
            self.client.thread.as_str(),
            crate::steer::NativeBoundary::Turn,
            || {
                self.store
                    .record_session_recovery(&self.client.adapter.session, recovery, phase)
            },
        )?)
    }

    async fn recovery_reply(
        &mut self,
        failed: String,
        rpc: &Value,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        let thread = &rpc["result"]["thread"];
        let Some(turns) = thread["turns"].as_array() else {
            return Ok(());
        };
        let session = self.client.adapter.session.clone();
        let retained = self.store.session_recovery(&session)?;
        if let Some(mut recovery) =
            retained.filter(|r| r.failed_turn == failed && r.request.is_some())
        {
            if let Some(turn) = turns
                .iter()
                .find(|turn| turn["id"].as_str() == recovery.retry_turn.as_deref())
            {
                if turn["itemsView"] == "full"
                    && turn["status"] == "completed"
                    && turn["items"].as_array().is_some_and(|items| {
                        items.iter().any(|item| {
                            super::recovery_gate::matches_input(item, &session, &recovery)
                        })
                    })
                {
                    recovery.state = RecoveryState::Continued;
                    recovery.reason = "The admitted native retry completed".into();
                    self.save_recovery(&recovery, "settled")?;
                }
            }
            return Ok(());
        }
        let Some(turn) = turns.iter().find(|turn| turn["id"] == failed) else {
            return Ok(());
        };
        let mut recovery = SessionRecovery {
            agent_process_id: self.client.adapter.agent_process_id.clone(),
            thread: self.client.thread.clone(),
            failed_turn: failed,
            model: thread["model"].as_str().map(str::to_owned),
            account: self.route.as_ref().map(|route| route.account_id().clone()),
            state: RecoveryState::WaitingCapacity,
            reason: "Checking eligible logins for this model".into(),
            retry_at: None,
            request: None,
            retry_turn: None,
        };
        if !codex_rejected_turn(turn)
            || turns.last() != Some(turn)
            || !matches!(
                thread["status"]["type"].as_str(),
                Some("idle" | "systemError")
            )
        {
            recovery.state = RecoveryState::Uncertain;
            recovery.reason = "Newer input, earlier work, or incomplete native history; continue from the conversation".into();
            self.save_recovery(&recovery, "uncertain")?;
            return Ok(());
        }
        let Some(model) = recovery.model.clone() else {
            recovery.state = RecoveryState::WaitingNative;
            recovery.reason = "The native thread has not reported its selected model".into();
            self.save_recovery(&recovery, "model")?;
            return Ok(());
        };
        let session = self.client.adapter.session.clone();
        let previous = self.store.session_recovery(&session)?;
        if let Some(previous) = previous.as_ref().filter(|old| old.key() == recovery.key()) {
            if previous.request.is_some() {
                return Ok(());
            }
            recovery.account = previous.account.clone();
        }
        let Some(account) = recovery.account.clone() else {
            recovery.reason = "This Session has no managed login to recover".into();
            self.save_recovery(&recovery, "unmanaged")?;
            return Ok(());
        };
        self.save_recovery(&recovery, "rejected")?;
        let limited = self
            .store
            .has_recovery_phase(&session, &recovery, "limited")?;
        if !limited {
            let reason = turn
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("usage limit reached");
            // The native error code already established spent quota;
            // localized display text is not another admission check.
            let limit = crate::agent::SpentLimit {
                reason: reason.into(),
                model: None,
                credits: false,
            };
            provider_account::record_session_limit(Provider::Codex, &account, &model, limit)
                .await?;
            self.save_recovery(&recovery, "limited")?;
        }
        let wave = self
            .store
            .session(&session)?
            .and_then(|session| session.wave_id);
        let route = provider_account::session_recovery_route(
            Provider::Codex,
            &model,
            &self.client.cwd,
            wave.as_ref(),
            &self.client.native.selection,
        )
        .await;
        let phase = match route {
            Ok(Some(route)) if self.has_recovery_guard().await.unwrap_or(false) => {
                if self.recovery.preparing.is_some() || self.recovery.installing.is_some() {
                    recovery.state = RecoveryState::WaitingNative;
                    recovery.reason = "The earlier native recovery attempt is settling".into();
                    self.save_recovery(&recovery, &format!("wait:{}", uuid::Uuid::new_v4()))?;
                    return Ok(());
                }
                let gate = RecoveryGate::new(&session, &recovery)?;
                let prompt = gate.prompt();
                recovery.request = Some(gate.attempt().into());
                recovery.state = RecoveryState::Recovering;
                recovery.reason = "Waiting for native submission to admit recovery".into();
                if !self.save_recovery(&recovery, "dispatch")? {
                    return Ok(());
                }
                let client = self.client.clone();
                let store = self.store.clone();
                self.recovery.preparing = Some(std::thread::spawn(move || {
                    prepare(gate, client, store, route, recovery)
                }));
                // The reply can name earlier work. Only the hook supplies retry_turn.
                let rpc = json!({"id":uuid::Uuid::new_v4().to_string(),"method":"turn/start",
                    "params":{"threadId":self.client.thread,"input":[{"type":"text","text":prompt}]}});
                writer.send(Message::Text(rpc.to_string().into())).await?;
                return Ok(());
            }
            Ok(Some(_)) => {
                recovery.state = RecoveryState::WaitingNative;
                recovery.reason = "This open Codex has no enabled native recovery guard; continue from the conversation".into();
                "admission"
            }
            result => {
                recovery.reason = "No eligible login has capacity for this model".into();
                if let Err(provider_account::ProviderAccountError::NoEligibleAccount {
                    retry_at,
                    ..
                }) = result
                {
                    recovery.retry_at = retry_at;
                }
                "wait"
            }
        };
        self.save_recovery(&recovery, &format!("{phase}:{}", uuid::Uuid::new_v4()))?;
        self.recovery.check_at = SystemTime::now() + Duration::from_secs(60);
        Ok(())
    }
}

#[cfg(test)]
async fn recovery_fixture(home: &std::path::Path) -> (SqliteStore, Connection) {
    let store = SqliteStore::open_ephemeral(&home.join("db")).unwrap();
    store.test_session("conversation", &crate::session_record::new_artifact_key());
    let process = store.test_lf_process();
    let attachment = store
        .claim_session_attachment("conversation", None, &process, true)
        .unwrap();
    let adapter = store
        .reserve_steer_adapter("conversation", &attachment)
        .unwrap();
    store
        .record_session_connection("conversation", &attachment, "fixture", &"thread".into())
        .unwrap();
    let client = Client {
        adapter,
        thread: "thread".into(),
        cwd: home.into(),
        endpoint: Some(home.join("native.sock")),
        inbox: None,
        native: crate::server::recovery::NativeRecovery {
            selection: Default::default(),
            guarded: true,
            transcript: None,
        },
    };
    let history = CodexStream::new(FactWriter::new(
        store.clone(),
        "conversation".into(),
        Some(attachment.clone()),
        None,
        Default::default(),
    ));
    (
        store.clone(),
        Connection {
            client,
            route: None,
            pin_pending: false,
            failure: Default::default(),
            store,
            history,
            busy: false,
            recovery: Default::default(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::recovery_fixture;
    use super::{Prepared, RecoveryGate};
    use crate::provider_account::{new_account, parse_account_id, ProviderAccountRoute};
    use crate::provider_auth::Provider;
    use crate::session_recovery::{RecoveryState, SessionRecovery};
    use crate::store::Store;
    use futures_util::StreamExt;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    #[tokio::test]
    async fn cancelled_installation_retains_late_or_lost_reply_without_releasing_retry() {
        for reply in ["success", "error", "lost", "malformed"] {
            let home = tempfile::tempdir().unwrap();
            let (store, mut recorder) = recovery_fixture(home.path()).await;
            let shared = Arc::new(Store::from_sqlite_for_test(store.clone()));
            let account = parse_account_id("replacement").unwrap();
            shared
                .upsert_provider_account(&new_account(
                    Provider::Codex,
                    account.clone(),
                    home.path().join("login"),
                    None,
                ))
                .await
                .unwrap();
            let route = ProviderAccountRoute::shared(
                Provider::Codex,
                account.clone(),
                Arc::clone(&shared),
                home.path().join("login"),
                false,
            );
            let mut recovery = SessionRecovery {
                agent_process_id: recorder.client.adapter.agent_process_id.clone(),
                thread: recorder.client.thread.clone(),
                failed_turn: "rejected".into(),
                model: Some("model".into()),
                account: None,
                state: RecoveryState::WaitingNative,
                reason: "waiting".into(),
                retry_at: None,
                request: None,
                retry_turn: None,
            };
            store
                .record_session_recovery("conversation", &recovery, "rejected")
                .unwrap();
            let gate = RecoveryGate::new("conversation", &recovery).unwrap();
            let prompt = gate.prompt();
            // The envelope carries a disposable socket, never a real login.
            let encoded = prompt
                .strip_prefix("<lf-recovery:")
                .unwrap()
                .split_once('>')
                .unwrap()
                .0;
            let envelope: Value = serde_json::from_str(encoded).unwrap();
            let mut hook =
                std::os::unix::net::UnixStream::connect(envelope["socket"].as_str().unwrap())
                    .unwrap();
            writeln!(hook, "{}", json!({"prompt":prompt,"session_id":recovery.thread,"hook_event_name":"UserPromptSubmit"})).unwrap();
            recovery.request = Some(gate.attempt().into());
            recovery.state = RecoveryState::Recovering;
            store
                .record_session_recovery("conversation", &recovery, "dispatch")
                .unwrap();
            let admission = gate.accept().unwrap();
            recovery.retry_turn = Some("retry".into());
            recorder.recovery.preparing = Some(std::thread::spawn(move || {
                Ok(Prepared {
                    admission,
                    route,
                    recovery,
                    tokens: secrecy::SecretString::new(
                        json!({"accessToken":"synthetic","chatgptAccountId":"replacement"})
                            .to_string(),
                    ),
                })
            }));
            while !recorder.recovery.preparing.as_ref().unwrap().is_finished() {
                tokio::task::yield_now().await;
            }
            let (client, server) = tokio::net::UnixStream::pair().unwrap();
            let (mut writer, _) = WebSocketStream::from_raw_socket(client, Role::Client, None)
                .await
                .split();
            let mut native = WebSocketStream::from_raw_socket(server, Role::Server, None).await;
            recorder.recovery_tick(&mut writer).await.unwrap();
            let sent = native.next().await.unwrap().unwrap();
            let sent: Value = serde_json::from_str(sent.to_text().unwrap()).unwrap();
            assert_eq!(sent["method"], "account/login/start");
            // Native cancellation happens after the account installation was sent.
            hook.shutdown(std::net::Shutdown::Write).unwrap();
            recorder.recovery_tick(&mut writer).await.unwrap();
            assert_eq!(
                store
                    .session_recovery("conversation")
                    .unwrap()
                    .unwrap()
                    .state,
                RecoveryState::Uncertain
            );
            if reply != "lost" {
                let rpc = if reply == "success" {
                    json!({"id":sent["id"],"result":{"type":"chatgptAuthTokens"}})
                } else if reply == "malformed" {
                    json!({"id":sent["id"],"result":{}})
                } else {
                    json!({"id":sent["id"],"error":{"code":-32603,"message":"failed"}})
                };
                recorder.recovery_message(&rpc, &mut writer).await.unwrap();
                let mut response = String::new();
                hook.read_to_string(&mut response).unwrap();
                assert!(response.is_empty(), "cancelled retry was released");
            }
            let pinned = shared
                .provider_session_account(Provider::Codex, &recorder.client.thread)
                .await
                .unwrap();
            assert_eq!(pinned, (reply == "success").then_some(account));
            for _ in 0..2 {
                recorder.recovery_tick(&mut writer).await.unwrap();
            }
            let retained = store.session_recovery("conversation").unwrap().unwrap();
            assert_eq!(retained.state, RecoveryState::Uncertain);
            assert!(retained.request.is_some());
            // Reconciliation may read history; neither login nor input can repeat.
            while let Ok(Some(message)) =
                tokio::time::timeout(Duration::from_millis(50), native.next()).await
            {
                let message = message.unwrap();
                let rpc: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                assert_eq!(
                    rpc["method"], "thread/read",
                    "installation or input was resent"
                );
            }
        }
    }
}
