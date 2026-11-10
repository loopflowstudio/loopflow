//! Claude keeps drafts, approvals and its inbox. Recovery observes the failed
//! prompt and joins the inbox UUID to a successful Stop, never a socket write.
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde_json::{json, Value};

use super::claude_inbox::Inbox;
use crate::provider_account::{self, LaunchCapacity};
use crate::provider_auth::Provider;
use crate::session_recovery::{claude_rejected_model, RecoveryState, SessionRecovery};
use crate::steer::{NativeBoundary, SteerAdapter};
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccountId, StoreError, StoreResult};

fn transcript(path: &Path) -> Result<Vec<Value>> {
    Ok(std::fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?)
}

pub(crate) fn boundary(
    store: &SqliteStore,
    adapter: &SteerAdapter,
    thread: &str,
    input: &Value,
) -> StoreResult<()> {
    let Some(prompt) = input["prompt_id"].as_str() else {
        return Ok(());
    };
    let phase = if input["hook_event_name"] == "Stop" {
        "stopped"
    } else {
        "started"
    };
    let account = if phase == "started" {
        LaunchCapacity::read(store)?
            .login("claude", None, true)
            .map(|account| account.account_id.clone())
    } else {
        None
    };
    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        store.record_recovery_boundary(&adapter.session, thread, prompt, phase, account.as_ref())
    })
}

pub(crate) fn record_limit(input: &Value) -> Result<()> {
    let adapter: SteerAdapter =
        serde_json::from_str(&std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)?)?;
    let store = SqliteStore::new(&crate::store::lf_home_dir().join("loopflow.db"))?;
    if !store
        .session(&adapter.session)?
        .is_some_and(|session| session.interactive)
    {
        return Ok(());
    }
    let (Some(thread), Some(prompt), Some(path)) = (
        input["session_id"].as_str(),
        input["prompt_id"].as_str(),
        input["transcript_path"].as_str(),
    ) else {
        return Ok(());
    };
    let rows = transcript(Path::new(path))?;
    let model = claude_rejected_model(&rows, prompt);
    let account: Option<ProviderAccountId> = store
        .recovery_boundary(&adapter.session, thread, prompt, "started")?
        .and_then(|value| serde_json::from_value(value["account"].clone()).ok())
        .flatten();
    let safe = model.is_some() && account.is_some();
    let recovery = SessionRecovery {
        agent_process_id: adapter.agent_process_id.clone(),
        thread: thread.into(), failed_turn: prompt.into(), model, account,
        state: if safe { RecoveryState::WaitingCapacity } else { RecoveryState::Uncertain },
        reason: if safe { input["last_assistant_message"].as_str().unwrap_or("Usage limit reached") } else { "The failed turn's model, login, or complete history is unavailable; inspect the conversation" }.into(),
        retry_at: None, request: None, retry_turn: None,
    };
    store.with_provider_context(&adapter, thread, NativeBoundary::Turn, || {
        store.record_session_recovery(&adapter.session, &recovery, "rejected")
    })?;
    if let (Ok(socket), Ok(token)) = (
        std::env::var("CLAUDE_CODE_MESSAGING_SOCKET"),
        std::env::var("CLAUDE_CODE_MESSAGING_TOKEN"),
    ) {
        let mut worker = Recovery {
            detached: true,
            ..Default::default()
        };
        worker.tick(&store, &adapter, thread, Path::new(path), &socket, &token)?;
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct Recovery {
    check_at: SystemTime,
    requested: i64,
    detached: bool,
}
impl Default for Recovery {
    fn default() -> Self {
        Self {
            check_at: SystemTime::UNIX_EPOCH,
            requested: 0,
            detached: false,
        }
    }
}

impl Recovery {
    pub(super) fn tick(
        &mut self,
        store: &SqliteStore,
        adapter: &SteerAdapter,
        thread: &str,
        path: &Path,
        socket: &str,
        token: &str,
    ) -> Result<()> {
        let Some(mut recovery) = store
            .session_recovery(&adapter.session)?
            .filter(|r| r.thread.as_str() == thread)
        else {
            return Ok(());
        };
        let requested = store.recovery_request_revision(&adapter.session)?;
        if requested != self.requested {
            self.requested = requested;
            self.check_at = SystemTime::UNIX_EPOCH;
        }
        if recovery.state == RecoveryState::Continued
            || (recovery.state == RecoveryState::Uncertain && recovery.request.is_none())
        {
            return Ok(());
        }
        let rows = transcript(path)?;
        if let Some(request) = &recovery.request {
            let matching: Vec<_> = rows
                .iter()
                .filter(|row| row["type"] == "user" && row["uuid"] == *request)
                .collect();
            if matching.len() == 1 {
                if let Some(prompt) = matching[0]["promptId"].as_str() {
                    if store
                        .recovery_boundary(&adapter.session, thread, prompt, "stopped")?
                        .is_some()
                    {
                        recovery.retry_turn = Some(prompt.into());
                        recovery.state = RecoveryState::Continued;
                        recovery.reason = "The native retry completed".into();
                        store.with_provider_context(
                            adapter,
                            thread,
                            NativeBoundary::Turn,
                            || {
                                store.record_session_recovery(
                                    &adapter.session,
                                    &recovery,
                                    "settled",
                                )
                            },
                        )?;
                    }
                }
            }
            return Ok(());
        }
        if SystemTime::now() < self.check_at {
            return Ok(());
        }
        self.check_at = SystemTime::now() + Duration::from_secs(60);
        let Some(model) = claude_rejected_model(&rows, &recovery.failed_turn)
            .filter(|model| Some(model) == recovery.model.as_ref())
        else {
            recovery.state = RecoveryState::Uncertain;
            recovery.reason =
                "Newer input or incomplete native history; continue from the conversation".into();
            store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                store.record_session_recovery(&adapter.session, &recovery, "uncertain")
            })?;
            return Ok(());
        };
        let Some(account) = &recovery.account else {
            return Ok(());
        };
        if !self.detached {
            let reason = rows
                .iter()
                .rev()
                .find(|row| row["isApiErrorMessage"] == true)
                .and_then(|row| row.pointer("/message/content"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            if let Some(reason) = reason {
                provider_account::start_limit_recording(&json!({"session_id":thread,
                    "prompt_id":recovery.failed_turn,"transcript_path":path,"last_assistant_message":reason}).to_string())?;
            }
            return Ok(());
        }
        let session = store
            .session(&adapter.session)?
            .ok_or(StoreError::NotFound)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let route = runtime.block_on(async {
            // Each observer attributes its rejection to the login captured at
            // prompt submission, even if another Session switched it meanwhile.
            if !store.has_recovery_phase(&adapter.session, &recovery, "limited")? {
                if let Some(limit) = crate::agent::subscription_limit_text(&recovery.reason) {
                    provider_account::record_session_limit(
                        Provider::Claude,
                        account,
                        &model,
                        limit,
                    )
                    .await?;
                    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                        store.record_session_recovery(&adapter.session, &recovery, "limited")
                    })?;
                }
            }
            let route = provider_account::session_recovery_route(
                Provider::Claude,
                &model,
                Path::new(&session.cwd),
                session.wave_id.as_ref(),
            )
            .await?;
            if let Some(route) = &route {
                // Same owner/locks as an ordinary launch, without starting one.
                let _lock = route
                    .launch_as(&mut std::process::Command::new("claude"))
                    .await?;
                route.pin_session(&recovery.thread).await?;
            }
            Ok::<_, anyhow::Error>(route)
        });
        let route = match route {
            Ok(Some(route)) => route,
            result => {
                recovery.state = RecoveryState::WaitingCapacity;
                recovery.reason = "No eligible login is ready for this model".into();
                recovery.retry_at = None;
                if let Err(error) = result {
                    if let Some(provider_account::ProviderAccountError::NoEligibleAccount {
                        retry_at,
                        ..
                    }) = error.downcast_ref()
                    {
                        recovery.retry_at = *retry_at;
                    } else {
                        recovery.state = RecoveryState::WaitingNative;
                        recovery.reason = "The selected login could not prepare native credentials; recovery will check again".into();
                    }
                }
                store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                    store.record_session_recovery(
                        &adapter.session,
                        &recovery,
                        &format!("wait:{}", uuid::Uuid::new_v4()),
                    )
                })?;
                return Ok(());
            }
        };
        recovery.retry_at = None;
        recovery.state = RecoveryState::WaitingNative;
        recovery.reason = "Waiting for Claude to reload the selected login".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "reload")
        })?;
        // Claude's native Keychain reader caches a login for 30 seconds. Inbox
        // turns can reuse that cached token immediately after a shared switch.
        // No credential lock is held while the native cache expires.
        std::thread::sleep(Duration::from_secs(31));
        if LaunchCapacity::read(store)?
            .login("claude", None, true)
            .is_none_or(|account| &account.account_id != route.account_id())
        {
            return Ok(());
        }
        // Account work may have taken time. A newer prompt supersedes this retry.
        if claude_rejected_model(&transcript(path)?, &recovery.failed_turn).as_deref()
            != Some(&model)
        {
            return Ok(());
        }
        let mut inbox = Inbox::connect(socket)?;
        let request = uuid::Uuid::new_v4().to_string();
        recovery.request = Some(request.clone());
        recovery.state = RecoveryState::Uncertain;
        recovery.reason = "Retry delivery is not yet confirmed by the native conversation".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            if store.record_session_recovery(&adapter.session, &recovery, "dispatch")? {
                inbox.send(token, thread, &request,
                    "Continue the existing conversation after its usage-limit rejection. Use the work already recorded in this conversation; do not repeat completed actions or answer pending approvals.")?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
