//! Claude keeps drafts, approvals and its inbox. Recovery observes the failed
//! prompt and joins the inbox UUID to a successful Stop, never a socket write.
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde_json::{json, Value};

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
    let mut worker = Recovery {
        detached: true,
        ..Default::default()
    };
    worker.tick(&store, &adapter, thread, Path::new(path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Recovery;
    use crate::id::LfProcessId;
    use crate::session_recovery::{RecoveryState, SessionRecovery};
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn reconnect_settles_the_dispatched_prompt_despite_a_late_wait() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        store.test_session("reader", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("reader", None, &process, true)
            .unwrap();
        store
            .record_session_connection("reader", &attachment, "fixture", &"thread".into())
            .unwrap();
        let adapter = store.reserve_steer_adapter("reader", &attachment).unwrap();
        let mut fact = SessionRecovery {
            agent_process_id: attachment.agent_process_id,
            thread: "thread".into(),
            failed_turn: "rejected".into(),
            model: Some("sonnet".into()),
            account: None,
            state: RecoveryState::WaitingCapacity,
            reason: "waiting".into(),
            retry_at: None,
            request: None,
            retry_turn: None,
        };
        store
            .record_session_recovery("reader", &fact, "rejected")
            .unwrap();
        let late = fact.clone();
        fact.request = Some("inbox-uuid".into());
        fact.state = RecoveryState::Uncertain;
        store
            .record_session_recovery("reader", &fact, "dispatch")
            .unwrap();
        store
            .record_session_recovery("reader", &late, "wait:late")
            .unwrap();
        let path = home.path().join("transcript");
        std::fs::write(
            &path,
            json!({"type":"user","uuid":"inbox-uuid","promptId":"retry"}).to_string(),
        )
        .unwrap();
        store
            .record_recovery_boundary("reader", "thread", "unrelated", "stopped", None)
            .unwrap();
        let reconnected = SqliteStore::new(&database).unwrap();
        let mut observer = Recovery::default();
        observer
            .tick(&reconnected, &adapter, "thread", &path)
            .unwrap();
        assert_eq!(
            reconnected.session_recovery("reader").unwrap(),
            Some(fact.clone())
        );
        store
            .record_recovery_boundary("reader", "thread", "retry", "stopped", None)
            .unwrap();
        observer
            .tick(&reconnected, &adapter, "thread", &path)
            .unwrap();
        let completed = reconnected.session_recovery("reader").unwrap().unwrap();
        assert_eq!(completed.state, RecoveryState::Continued);
        assert_eq!(completed.request, fact.request);
        assert_eq!(completed.retry_turn.as_deref(), Some("retry"));
        store
            .record_session_recovery("reader", &late, "wait:after-stop")
            .unwrap();
        assert_eq!(
            reconnected.session_recovery("reader").unwrap(),
            Some(completed)
        );
    }
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
            Ok::<_, anyhow::Error>(
                provider_account::session_recovery_route(
                    Provider::Claude,
                    &model,
                    Path::new(&session.cwd),
                    session.wave_id.as_ref(),
                )
                .await?,
            )
        });
        match route {
            Ok(Some(_)) => (),
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
        // The inbox queues behind native input and has no failed-prompt
        // condition. A transcript read before sending cannot reserve that
        // position, so neither switch the shared login nor enqueue a retry.
        recovery.retry_at = None;
        recovery.state = RecoveryState::WaitingNative;
        recovery.reason = "Claude cannot condition an inbox retry on this still-current rejected prompt; continue in the conversation".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(
                &adapter.session,
                &recovery,
                &format!("admission:{}", uuid::Uuid::new_v4()),
            )
        })?;
        Ok(())
    }
}
