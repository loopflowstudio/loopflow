//! Private-server recovery uses the observer connection, never the TUI's input.
use std::time::{Duration, SystemTime};

use anyhow::Result;
use futures_util::{stream::SplitSink, SinkExt};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::codex_connection::{Recorder, Upstream};
use crate::provider_account;
use crate::provider_auth::Provider;
use crate::session_recovery::{codex_rejected_input, RecoveryState, SessionRecovery};

#[derive(Debug)]
pub(super) struct Recovery {
    pending: Option<(Value, String)>,
    sent_at: SystemTime,
    check_at: SystemTime,
    requested: i64,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            pending: None,
            sent_at: SystemTime::UNIX_EPOCH,
            check_at: SystemTime::UNIX_EPOCH,
            requested: 0,
        }
    }
}

impl Recorder {
    pub(super) async fn recovery_message(
        &mut self,
        rpc: &Value,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
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
        if let Some((id, _)) = &self.recovery.pending {
            if rpc.get("id") == Some(id) {
                let (_, request) = self
                    .recovery
                    .pending
                    .take()
                    .expect("matched recovery request");
                self.recovery_reply(request, rpc).await?;
                return Ok(());
            }
        }
        if rpc["params"]["threadId"] != self.observer.thread.as_str() {
            return Ok(());
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
            .sent_at
            .elapsed()
            .is_ok_and(|elapsed| elapsed >= Duration::from_secs(15))
        {
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
                if let Some(result) = result {
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
        if let Some(recovery) =
            recovery.filter(|r| r.thread == self.observer.thread && r.can_retry())
        {
            self.read_recovery(&recovery.failed_turn, writer).await?;
        }
        Ok(())
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
        self.recovery.pending = Some((id, turn.into()));
        self.recovery.sent_at = SystemTime::now();
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

    async fn recovery_reply(&mut self, failed: String, rpc: &Value) -> Result<()> {
        let thread = &rpc["result"]["thread"];
        let Some(turns) = thread["turns"].as_array() else {
            return Ok(());
        };
        let Some(turn) = turns.iter().find(|turn| turn["id"] == failed) else {
            return Ok(());
        };
        let rejected = codex_rejected_input(turn).is_some();
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
        if !rejected
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
            recovery.account = previous.account.clone();
        }
        if previous
            .as_ref()
            .is_some_and(|old| old.key() == recovery.key() && old.request.is_some())
        {
            return Ok(());
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
            Ok(Some(_)) => {
                // Login replaces server-wide credentials and can fail active work.
                // Neither login nor turn/start conditions admission on this turn.
                recovery.state = RecoveryState::WaitingNative;
                recovery.reason = "Codex cannot condition account handoff and retry on this still-current rejected turn; continue in the conversation".into();
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
