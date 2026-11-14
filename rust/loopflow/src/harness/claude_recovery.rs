//! Claude keeps drafts, approvals and its inbox. Recovery observes the failed
//! prompt and joins the inbox UUID to a successful Stop, never a socket write.
use std::io::Write;
use std::path::Path;

use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde_json::{json, Value};

use super::recovery_gate::{read_transcript, RecoveryGate};
use crate::provider_account::{self, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::server::recovery::Client;
use crate::session_recovery::{claude_rejected_model, RecoveryState, SessionRecovery};
use crate::steer::{NativeBoundary, SteerAdapter};
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccountId, StoreError, StoreResult};

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
        if store.session_connection(&adapter.session)?.is_some() {
            crate::server::recovery::account(store, "codex", thread)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?
        } else {
            let accounts = store.list_provider_accounts(Some("claude"))?;
            input
                .pointer("/lf_recovery/claude_home")
                .and_then(Value::as_str)
                .and_then(|home| {
                    provider_account::activation::active_account(
                        Provider::Claude,
                        Path::new(home),
                        &accounts,
                    )
                })
                .map(|account| account.account_id.clone())
        }
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
    let rows = read_transcript(Path::new(path))?;
    let model = claude_rejected_model(&rows, prompt);
    let account: Option<ProviderAccountId> = store
        .recovery_boundary(&adapter.session, thread, prompt, "recovery-login")?
        .or(store.recovery_boundary(&adapter.session, thread, prompt, "started")?)
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
    Ok(())
}

/// Called by the Machine's shared recovery reader, never a native watcher.
pub(crate) fn check(store: &SqliteStore, client: &Client) -> Result<()> {
    let adapter = &client.adapter;
    let thread = client.thread.as_str();
    let Some(path) = &client.native.transcript else {
        return Ok(());
    };
    let Some(mut recovery) = store
        .session_recovery(&adapter.session)?
        .filter(|r| r.thread == client.thread)
    else {
        return Ok(());
    };
    let rows = read_transcript(path)?;
    if let Some(request) = &recovery.request {
        let mut matching = rows
            .iter()
            .filter(|row| row["type"] == "user" && row["uuid"] == *request);
        let Some(prompt) = matching.next().and_then(|row| row["promptId"].as_str()) else {
            return Ok(());
        };
        if matching.next().is_none()
            && recovery.retry_turn.as_deref() == Some(prompt)
            && store.has_recovery_phase(&adapter.session, &recovery, "admitted")?
            && store
                .recovery_boundary(&adapter.session, thread, prompt, "stopped")?
                .is_some()
        {
            recovery.state = RecoveryState::Continued;
            recovery.reason = "The native retry completed".into();
            store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                store.record_session_recovery(&adapter.session, &recovery, "settled")
            })?;
        }
        return Ok(());
    }
    check_capacity(store, client, &rows)
}

/// Broker work runs outside the native hook and keeps rotated credentials.
fn check_capacity(store: &SqliteStore, client: &Client, rows: &[Value]) -> Result<()> {
    let adapter = &client.adapter;
    let thread = client.thread.as_str();
    let Some(mut recovery) = store
        .session_recovery(&adapter.session)?
        .filter(|recovery| recovery.thread.as_str() == thread && recovery.can_retry())
    else {
        return Ok(());
    };
    let Some(model) = confirm_rejection(store, adapter, thread, rows, &mut recovery)? else {
        return Ok(());
    };
    let Some(account) = &recovery.account else {
        return Ok(());
    };
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
                provider_account::record_session_limit(Provider::Claude, account, &model, limit)
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
                &client.native.selection,
            )
            .await?,
        )
    });
    recovery.retry_at = None;
    let phase = match route {
        Ok(Some(route)) if client.native.guarded && client.native.claude_home.is_some() => {
            return dispatch_recovery(store, client, recovery, &route, &runtime);
        }
        Ok(Some(_)) => {
            recovery.state = RecoveryState::WaitingNative;
            recovery.reason =
                "This open Claude has no native recovery guard or native home; continue from the conversation"
                    .into();
            "admission"
        }
        result => {
            recovery.state = RecoveryState::WaitingCapacity;
            recovery.reason = "No eligible login is ready for this model".into();
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
            "wait"
        }
    };
    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        store.record_session_recovery(
            &adapter.session,
            &recovery,
            &format!("{phase}:{}", uuid::Uuid::new_v4()),
        )
    })?;
    Ok(())
}

/// Capacity selection ends before dispatch. From here on the fixed attempt is
/// retained, and any failure leaves it uncertain instead of sending again.
fn dispatch_recovery(
    store: &SqliteStore,
    client: &Client,
    mut recovery: SessionRecovery,
    route: &ProviderAccountRoute,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    let adapter = &client.adapter;
    let thread = client.thread.as_str();
    let gate = RecoveryGate::new(&adapter.session, &recovery)?;
    let prompt = gate.prompt();
    let Some(key) = &client.inbox else {
        return Ok(());
    };
    let Some((mut socket, token)) = crate::server::wake::open_inbox(key, thread)? else {
        return Ok(());
    };
    recovery.request = Some(gate.attempt().into());
    recovery.state = RecoveryState::Recovering;
    recovery.reason = "Waiting for native submission to admit recovery".into();
    let dispatched = store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        store.record_session_recovery(&adapter.session, &recovery, "dispatch")
    })?;
    if !dispatched {
        return Ok(());
    }
    let admitted = (|| -> Result<()> {
        writeln!(socket, "{}", json!({"type":"auth","token":token}))?;
        writeln!(
            socket,
            "{}",
            json!({"type":"user","session_id":thread,"uuid":recovery.request,
            "priority":"next","from":"loopflow","message":{"role":"user","content":prompt}})
        )?;
        drop(socket);
        let admission = gate.accept()?;
        let rows = admission.transcript()?;
        // Some native versions append the incoming row before the hook.
        // Only this exact attempt's row can be excluded from prior history.
        let prior = rows
            .iter()
            .position(|row| {
                row["type"] == "user" && row["uuid"].as_str() == recovery.request.as_deref()
            })
            .unwrap_or(rows.len());
        anyhow::ensure!(
            claude_rejected_model(&rows[..prior], &recovery.failed_turn).as_deref()
                == recovery.model.as_deref(),
            "Newer native input superseded recovery"
        );
        let current = store.session_recovery(&adapter.session)?;
        anyhow::ensure!(
            current
                .as_ref()
                .is_some_and(|r| r.key() == recovery.key() && r.request == recovery.request),
            "Recovery was superseded"
        );
        anyhow::ensure!(
            store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                store.record_session_recovery(&adapter.session, &recovery, "admitted")
            })?,
            "Recovery attempt was already admitted"
        );
        // Claude caches its login for 30 seconds. Wait while native
        // submission is held, before taking the shared credential lock.
        let cache_expires = SystemTime::now() + Duration::from_secs(31);
        while SystemTime::now() < cache_expires {
            admission.check()?;
            std::thread::sleep(Duration::from_millis(100));
        }
        let native = client
            .native
            .claude_home
            .as_ref()
            .expect("checked native home");
        let guard = runtime.block_on(route.activate_recovery(native, &|| {
            admission
                .check()
                .map_err(|error| provider_account::ProviderAccountError::Runtime(error.to_string()))
        }))?;
        admission.check()?;
        runtime.block_on(route.pin_session(&recovery.thread))?;
        let prompt = admission.input["prompt_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Native retry has no prompt ID"))?;
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_recovery_boundary(
                &adapter.session,
                thread,
                prompt,
                "recovery-login",
                Some(route.account_id()),
            )
        })?;
        recovery.retry_turn = Some(prompt.into());
        recovery.reason = "The admitted native retry is running".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "reply")
        })?;
        admission.finish()?;
        drop(guard);
        Ok(())
    })();
    if let Err(error) = admitted {
        recovery.state = RecoveryState::Uncertain;
        recovery.reason =
            "Native recovery was not confirmed; inspect the conversation before continuing".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "unconfirmed")
        })?;
        tracing::warn!(%error, "Claude recovery admission stopped");
    }
    Ok(())
}

fn confirm_rejection(
    store: &SqliteStore,
    adapter: &SteerAdapter,
    thread: &str,
    rows: &[Value],
    recovery: &mut SessionRecovery,
) -> Result<Option<String>> {
    let Some(model) = claude_rejected_model(rows, &recovery.failed_turn)
        .filter(|model| Some(model) == recovery.model.as_ref())
    else {
        recovery.state = RecoveryState::Uncertain;
        recovery.reason =
            "Newer input or incomplete native history; continue from the conversation".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, recovery, "uncertain")
        })?;
        return Ok(None);
    };
    Ok(Some(model))
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::id::LfProcessId;
    use crate::server::recovery::{Client, NativeRecovery};
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
        store
            .record_session_recovery("reader", &fact, "admitted")
            .unwrap();
        fact.retry_turn = Some("retry".into());
        store
            .record_session_recovery("reader", &fact, "reply")
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
        let client = Client {
            adapter: adapter.clone(),
            thread: "thread".into(),
            cwd: home.path().into(),
            endpoint: None,
            inbox: None,
            native: NativeRecovery {
                selection: Default::default(),
                guarded: false,
                claude_home: None,
                transcript: Some(path),
            },
        };
        check(&reconnected, &client).unwrap();
        assert_eq!(
            reconnected.session_recovery("reader").unwrap(),
            Some(fact.clone())
        );
        store
            .record_recovery_boundary("reader", "thread", "retry", "stopped", None)
            .unwrap();
        check(&reconnected, &client).unwrap();
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
