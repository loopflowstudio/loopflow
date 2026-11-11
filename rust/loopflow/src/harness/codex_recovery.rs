//! Private-server recovery uses the observer connection, never the TUI's input.
use std::time::{Duration, SystemTime};

use anyhow::Result;
use futures_util::{stream::SplitSink, SinkExt};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::codex_connection::{CodexObserver, Recorder, Upstream};
use super::recovery_gate::{Admission, RecoveryGate};
use crate::provider_account;
use crate::provider_auth::Provider;
use crate::session_recovery::{codex_rejected_turn, RecoveryState, SessionRecovery};

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
        // Native exit can outlive a refresh response. Keep this observer until
        // the broker has stored any rotated grant; dropping admission then
        // closes its socket without releasing the native prompt.
        if let Some(worker) = self.preparing.take() {
            if worker.join().is_err() {
                tracing::warn!("Recovery credential worker panicked during observer exit");
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

/// Keep credential preparation off the observer loop. This worker owns the
/// admission until the observer can install the prepared login on its connection.
fn prepare(
    gate: RecoveryGate,
    observer: CodexObserver,
    route: provider_account::ProviderAccountRoute,
    mut recovery: SessionRecovery,
) -> Result<Prepared> {
    let admission = gate.accept()?;
    let turn = super::recovery_gate::codex_current(&admission, &recovery)?;
    let store = crate::store::sqlite::SqliteStore::new(&observer.database)?;
    let current = store.session_recovery(&observer.session)?;
    anyhow::ensure!(
        current
            .as_ref()
            .is_some_and(|r| r.key() == recovery.key() && r.request == recovery.request),
        "Recovery was superseded"
    );
    anyhow::ensure!(
        store.with_session_attachment(&observer.session, &observer.attachment, || {
            store.record_session_recovery(&observer.session, &recovery, "admitted")
        })?,
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

impl Recorder {
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
            if rpc.get("error").is_some() {
                recovery.state = RecoveryState::Uncertain;
                recovery.reason =
                    "Native account installation was not confirmed; inspect the conversation"
                        .into();
                self.save_recovery(&recovery, "unconfirmed")?;
                return Ok(());
            }
            prepared.route.pin_session(&self.observer.thread).await?;
            self.observer.account = Some(prepared.route.account_id().clone());
            self.route = Some(prepared.route.clone());
            if let Some((_, stream)) = self.stream() {
                stream.writer.set_route(prepared.route);
            }
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
        if rpc["params"]["threadId"] != self.observer.thread.as_str() {
            return Ok(());
        }
        if rpc["method"] == "item/completed" {
            let session = self.observer.session.clone();
            let recovery = match self.stream() {
                Some((store, _)) => store.session_recovery(&session)?,
                None => None,
            };
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
                    agent_process_id: self.observer.attachment.agent_process_id.clone(),
                    thread: self.observer.thread.clone(),
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
                        prepared.recovery.thread == self.observer.thread,
                        "Recovery thread changed"
                    );
                    let session = self.observer.session.clone();
                    let current = self
                        .stream()
                        .ok_or_else(|| anyhow::anyhow!("Session history unavailable"))?
                        .0
                        .session_recovery(&session)?;
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
        let session = self.observer.session.clone();
        if let Some((store, _)) = self.stream() {
            let requested = store.recovery_request_revision(&session)?;
            if requested != self.recovery.requested {
                self.recovery.requested = requested;
                self.recovery.check_at = SystemTime::UNIX_EPOCH;
            }
        }
        // Completion can precede the RPC reply or arrive during a disconnected
        // observer. Settle from the same retained native receipt in either case.
        let mut recovery = match self.stream() {
            Some((store, _)) => store.session_recovery(&session)?,
            None => None,
        };
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
                recovery.reason = "The observer reconnected without a native retry reply; inspect the conversation before continuing".into();
                self.save_recovery(recovery, "unconfirmed")?;
            }
            if let Some(turn) = &recovery.retry_turn {
                let result = match self.stream() {
                    Some((store, _)) => {
                        store.session_turn_result(&session, &recovery.thread, turn)?
                    }
                    None => None,
                };
                let received = self
                    .stream()
                    .map(|(store, _)| store.has_recovery_phase(&session, recovery, "input"))
                    .transpose()?
                    .unwrap_or(false);
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
            .filter(|r| r.thread == self.observer.thread && (r.can_retry() || r.request.is_some()))
        {
            self.read_recovery(&recovery.failed_turn, writer).await?;
        }
        Ok(())
    }

    async fn has_recovery_guard(&self) -> Result<bool> {
        let mut upstream = super::codex_connection::initialized(
            &self.observer.endpoint,
            "loopflow_recovery_hooks",
        )
        .await?;
        let hooks = super::codex_connection::rpc_request(
            &mut upstream,
            "hooks/list",
            json!({"threadId":self.observer.thread}),
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
            "params":{"threadId":self.observer.thread,"includeTurns":true}});
        writer.send(Message::Text(rpc.to_string().into())).await?;
        self.recovery.pending = Some(HistoryRead {
            id,
            failed_turn: turn.into(),
            sent_at: SystemTime::now(),
        });
        Ok(())
    }

    fn save_recovery(&mut self, recovery: &SessionRecovery, phase: &str) -> Result<bool> {
        let (session, attachment) = (
            self.observer.session.clone(),
            self.observer.attachment.clone(),
        );
        let (store, _) = self
            .stream()
            .ok_or_else(|| anyhow::anyhow!("Session history unavailable"))?;
        Ok(store.with_session_attachment(&session, &attachment, || {
            store.record_session_recovery(&session, recovery, phase)
        })?)
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
        let session = self.observer.session.clone();
        let retained = match self.stream() {
            Some((store, _)) => store.session_recovery(&session)?,
            None => None,
        };
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
            agent_process_id: self.observer.attachment.agent_process_id.clone(),
            thread: self.observer.thread.clone(),
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
        let session = self.observer.session.clone();
        let Some((store, _)) = self.stream() else {
            return Ok(());
        };
        let previous = store.session_recovery(&session)?;
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
            .stream()
            .ok_or_else(|| anyhow::anyhow!("Session history unavailable"))?
            .0
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
            .stream()
            .and_then(|(store, _)| store.session(&session).ok().flatten())
            .and_then(|session| session.wave_id);
        let route = provider_account::session_recovery_route(
            Provider::Codex,
            &model,
            &self.observer.cwd,
            wave.as_ref(),
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
                let observer = self.observer.clone();
                self.recovery.preparing = Some(std::thread::spawn(move || {
                    prepare(gate, observer, route, recovery)
                }));
                // The reply can name earlier work. Only the hook supplies retry_turn.
                let rpc = json!({"id":uuid::Uuid::new_v4().to_string(),"method":"turn/start",
                    "params":{"threadId":self.observer.thread,"input":[{"type":"text","text":prompt}]}});
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
mod tests {
    use super::{Prepared, RecoveryGate};
    use crate::harness::codex_connection::tests::recovery_fixture;
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
        for reply in ["success", "error", "lost"] {
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
                agent_process_id: recorder.observer.attachment.agent_process_id.clone(),
                thread: recorder.observer.thread.clone(),
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
                } else {
                    json!({"id":sent["id"],"error":{"code":-32603,"message":"failed"}})
                };
                recorder.recovery_message(&rpc, &mut writer).await.unwrap();
                let mut response = String::new();
                hook.read_to_string(&mut response).unwrap();
                assert!(response.is_empty(), "cancelled retry was released");
            }
            let pinned = shared
                .provider_session_account(Provider::Codex, &recorder.observer.thread)
                .await
                .unwrap();
            assert_eq!(pinned, (reply == "success").then_some(account));
            for _ in 0..2 {
                recorder.recovery_tick(&mut writer).await.unwrap();
            }
            let retained = store.session_recovery("conversation").unwrap().unwrap();
            assert_eq!(retained.state, RecoveryState::Uncertain);
            assert!(retained.request.is_some());
            assert!(
                tokio::time::timeout(Duration::from_millis(50), native.next())
                    .await
                    .is_err(),
                "installation or input was resent"
            );
        }
    }
}
