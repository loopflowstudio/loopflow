//! Private-server recovery uses the observer connection, never the TUI's input.
use std::time::{Duration, SystemTime};

use anyhow::Result;
use futures_util::{stream::SplitSink, SinkExt};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::codex_connection::{Recorder, Upstream};
use crate::provider_account::{self, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::session_recovery::{codex_rejected_input, RecoveryState, SessionRecovery};

#[derive(Debug)]
pub(super) struct Recovery {
    pending: Option<(Value, Request)>,
    sent_at: SystemTime,
    check_at: SystemTime,
    requested: i64,
}

#[derive(Debug)]
enum Request {
    Read {
        turn: String,
        authenticated: bool,
    },
    Login {
        recovery: SessionRecovery,
        route: ProviderAccountRoute,
    },
    Start(SessionRecovery),
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

impl Recovery {
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
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
                Some(route) => route.codex_auth_tokens(true).await.ok(),
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
                self.recovery_reply(request, rpc, writer).await?;
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
                && self.recovery.pending.is_none()
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
                self.read_recovery(id, false, writer).await?;
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
            if let Some((_, Request::Start(recovery) | Request::Login { recovery, .. })) =
                &self.recovery.pending
            {
                let mut recovery = recovery.clone();
                recovery.state = RecoveryState::Uncertain;
                recovery.reason = "The native recovery request was not confirmed; inspect the conversation before continuing".into();
                self.save_recovery(&recovery, "unconfirmed")?;
            } else if matches!(self.recovery.pending, Some((_, Request::Read { .. }))) {
                // A history read has no native effect and can be repeated.
                self.recovery.pending = None;
            }
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
                && self.recovery.pending.is_none()
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
            self.read_recovery(&recovery.failed_turn, false, writer)
                .await?;
        }
        Ok(())
    }

    async fn read_recovery(
        &mut self,
        turn: &str,
        authenticated: bool,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        self.send_recovery(
            "thread/read",
            json!({"threadId":self.observer.thread,"includeTurns":true}),
            Request::Read {
                turn: turn.into(),
                authenticated,
            },
            writer,
        )
        .await
    }

    async fn send_recovery(
        &mut self,
        method: &str,
        params: Value,
        request: Request,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        let id = json!(format!("lf-recovery-{}", uuid::Uuid::new_v4()));
        // Once queued, loss of a reply remains pending; never infer rejection.
        let rpc = json!({"id":id,"method":method,"params":params});
        if method == "turn/start" {
            if let Some((_, history)) = self.stream() {
                history.outbound(&rpc).await?;
            }
        }
        let (session, attachment) = (
            self.observer.session.clone(),
            self.observer.attachment.clone(),
        );
        let thread = self.observer.thread.clone();
        let (store, _) = self
            .stream()
            .ok_or_else(|| anyhow::anyhow!("Session history unavailable"))?;
        let message = Message::Text(rpc.to_string().into());
        let sent = super::dispatch::off_reactor(|| {
            store.with_session_dispatch(&session, &attachment, || {
                if store.session_thread(&session)?.as_ref() != Some(&thread) {
                    return Ok(false);
                }
                if let Request::Start(recovery) = &request {
                    if !store.record_session_recovery(&session, recovery, "dispatch")? {
                        return Ok(false);
                    }
                }
                super::dispatch::send_fenced(writer, message)?;
                Ok(true)
            })
        });
        if !matches!(sent, Ok(false)) {
            self.recovery.pending = Some((id, request));
            self.recovery.sent_at = SystemTime::now();
        }
        sent?;
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
        request: Request,
        rpc: &Value,
        writer: &mut SplitSink<Upstream, Message>,
    ) -> Result<()> {
        match request {
            Request::Read {
                turn: failed,
                authenticated,
            } => {
                let thread = &rpc["result"]["thread"];
                let Some(turns) = thread["turns"].as_array() else {
                    return Ok(());
                };
                let Some(turn) = turns.iter().find(|turn| turn["id"] == failed) else {
                    return Ok(());
                };
                let input = codex_rejected_input(turn);
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
                if input.is_none()
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
                    recovery.reason =
                        "The native thread has not reported its selected model".into();
                    self.save_recovery(&recovery, "model")?;
                    return Ok(());
                };
                let input = input.expect("complete rejected input checked above");
                let session = self.observer.session.clone();
                let Some((store, _)) = self.stream() else {
                    return Ok(());
                };
                let previous = store.session_recovery(&session)?;
                if let Some(previous) = previous.as_ref().filter(|old| old.key() == recovery.key())
                {
                    recovery.account = previous.account.clone();
                }
                if authenticated
                    && previous
                        .as_ref()
                        .is_some_and(|old| old.model != recovery.model)
                {
                    recovery.state = RecoveryState::WaitingNative;
                    recovery.reason = "The selected model changed during recovery; continue with the native conversation".into();
                    self.save_recovery(&recovery, "model_changed")?;
                    return Ok(());
                }
                if previous
                    .as_ref()
                    .is_some_and(|old| old.key() == recovery.key() && old.request.is_some())
                {
                    return Ok(());
                }
                if authenticated {
                    recovery.request = Some(uuid::Uuid::new_v4().to_string());
                    recovery.state = RecoveryState::Recovering;
                    recovery.reason = "Retry dispatched; waiting for native completion".into();
                    self.busy = true;
                    self.send_recovery(
                        "turn/start",
                        json!({"threadId":recovery.thread,"input":input,"model":model}),
                        Request::Start(recovery),
                        writer,
                    )
                    .await?;
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
                    provider_account::record_session_limit(
                        Provider::Codex,
                        &account,
                        &model,
                        limit,
                    )
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
                let route = match route {
                    Ok(Some(route)) => route,
                    result => {
                        recovery.reason = "No eligible login has capacity for this model".into();
                        if let Err(provider_account::ProviderAccountError::NoEligibleAccount {
                            retry_at,
                            ..
                        }) = result
                        {
                            recovery.retry_at = retry_at;
                        }
                        self.save_recovery(&recovery, &format!("wait:{}", uuid::Uuid::new_v4()))?;
                        self.recovery.check_at = SystemTime::now() + Duration::from_secs(60);
                        return Ok(());
                    }
                };
                let tokens = match route.codex_auth_tokens(false).await {
                    Ok(tokens) => tokens,
                    Err(error) => {
                        recovery.state = RecoveryState::WaitingNative;
                        recovery.reason = "The selected login could not prepare native credentials; recovery will check again".into();
                        self.save_recovery(
                            &recovery,
                            &format!("credentials:{}", uuid::Uuid::new_v4()),
                        )?;
                        return Err(error.into());
                    }
                };
                let mut params: Value = serde_json::from_str(tokens.expose_secret())?;
                params["type"] = json!("chatgptAuthTokens");
                self.send_recovery(
                    "account/login/start",
                    params,
                    Request::Login { recovery, route },
                    writer,
                )
                .await?;
            }
            Request::Login {
                mut recovery,
                route,
            } => {
                if rpc.get("error").is_some() {
                    recovery.state = RecoveryState::WaitingNative;
                    recovery.reason =
                        "Native account handoff failed; retry when the login is available".into();
                    self.save_recovery(&recovery, "login_failed")?;
                    return Ok(());
                }
                self.adopt_route(route);
                if let Some(route) = &self.route {
                    route.pin_session(&self.observer.thread).await?;
                }
                // Read again after credential work: input may have arrived while
                // the broker was busy. Never retry an older turn over newer work.
                self.read_recovery(&recovery.failed_turn, true, writer)
                    .await?;
            }
            Request::Start(mut recovery) => {
                if let Some(turn) = rpc.pointer("/result/turn/id").and_then(Value::as_str) {
                    recovery.retry_turn = Some(turn.into());
                } else {
                    recovery.state = RecoveryState::Uncertain;
                    recovery.reason =
                        "Native retry was not confirmed; inspect the conversation".into();
                }
                self.save_recovery(&recovery, "reply")?;
            }
        }
        Ok(())
    }
}
