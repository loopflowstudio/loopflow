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
    // The login that receives this prompt, read from the client's own home.
    let home = input
        .pointer("/lf_recovery/claude_home")
        .and_then(Value::as_str)
        .filter(|_| phase == "started");
    let account = match home {
        Some(home) => {
            let accounts = store.list_provider_accounts(Some("claude"))?;
            provider_account::activation::active_account(
                Provider::Claude,
                Path::new(home),
                &accounts,
            )
            .map(|account| account.account_id.clone())
        }
        None => None,
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
    let (state, reason) = if model.is_none() {
        (
            RecoveryState::Uncertain,
            "The stopped turn's model or complete history is unavailable; inspect the conversation",
        )
    } else if account.is_none() {
        (
            RecoveryState::ManualRetry,
            "This Session’s login is not one Loopflow manages",
        )
    } else if std::env::var("LF_RECOVERY_GUARD").as_deref() != Ok("1") {
        (
            RecoveryState::ManualRetry,
            "This conversation was opened before Loopflow could resend for it",
        )
    } else {
        (
            RecoveryState::Waiting,
            "Checking eligible logins for this model",
        )
    };
    let recovery = SessionRecovery {
        agent_process_id: adapter.agent_process_id.clone(),
        thread: thread.into(),
        failed_turn: prompt.into(),
        model,
        account,
        state,
        reason: reason.into(),
        retry_at: None,
        request: None,
        retry_turn: None,
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
    if let Some(start) = rows
        .iter()
        .position(|row| row["type"] == "user" && row["promptId"] == recovery.failed_turn)
    {
        let newer = rows[start + 1..].iter().any(|row| {
            crate::transcript::is_user_input(row)
                && row["promptId"] != recovery.failed_turn
                && !row["promptId"]
                    .as_str()
                    .is_some_and(|prompt| Some(prompt) == recovery.retry_turn.as_deref())
                && !row["uuid"]
                    .as_str()
                    .is_some_and(|uuid| Some(uuid) == recovery.request.as_deref())
                && !super::recovery_gate::matches_content(
                    &row["message"]["content"],
                    &adapter.session,
                    &recovery,
                )
        });
        if newer {
            store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                store.record_session_recovery(&adapter.session, &recovery, "ended")
            })?;
            return Ok(());
        }
    }
    if recovery.state == RecoveryState::Continued {
        return Ok(());
    }
    let Some(request) = &recovery.request else {
        if recovery.can_retry() {
            check_capacity(store, client, &rows, recovery)?;
        }
        return Ok(());
    };
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
        recovery.reason = "The resent turn completed".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "settled")
        })?;
    }
    Ok(())
}

/// Broker work runs outside the native hook and keeps rotated credentials.
fn check_capacity(
    store: &SqliteStore,
    client: &Client,
    rows: &[Value],
    mut recovery: SessionRecovery,
) -> Result<()> {
    let adapter = &client.adapter;
    let thread = client.thread.as_str();
    let Some(model) = claude_rejected_model(rows, &recovery.failed_turn)
        .filter(|model| Some(model) == recovery.model.as_ref())
    else {
        recovery.state = RecoveryState::Uncertain;
        recovery.reason = "Incomplete history; continue from the conversation".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "uncertain")
        })?;
        return Ok(());
    };
    // `record_limit` already classified a turn with no managed login.
    let Some(account) = &recovery.account else {
        return Ok(());
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    // Each observer attributes its rejection to the login captured at
    // prompt submission, even if another Session switched it meanwhile.
    if !store.has_recovery_phase(&adapter.session, &recovery, "limited")? {
        if let Some(limit) = rows.iter().rev().find_map(|row| {
            let text = row
                .pointer("/message/content")?
                .as_array()?
                .iter()
                .filter_map(|item| item["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            crate::agent::subscription_limit_text(&text)
        }) {
            runtime.block_on(provider_account::record_session_limit(
                Provider::Claude,
                account,
                &model,
                limit,
            ))?;
            store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
                store.record_session_recovery(&adapter.session, &recovery, "limited")
            })?;
        }
    }
    // The guard is a launch fact, so the rejection's classification stands:
    // the spent limit still routes other Sessions, and nothing is resent here.
    if recovery.state == RecoveryState::ManualRetry {
        return Ok(());
    }
    let session = store
        .session(&adapter.session)?
        .ok_or(StoreError::NotFound)?;
    recovery.retry_at = None;
    match runtime.block_on(provider_account::session_recovery_route(
        Provider::Claude,
        &model,
        Path::new(&session.cwd),
        session.wave_id.as_ref(),
        &client.native.selection,
    )) {
        Ok(Some(route)) => return dispatch_recovery(store, client, recovery, &route, &runtime),
        Ok(None) => recovery.reason = "No eligible login is ready for this model".into(),
        Err(provider_account::ProviderAccountError::NoEligibleAccount { retry_at, .. }) => {
            recovery.reason = "No eligible login is ready for this model".into();
            recovery.retry_at = retry_at;
        }
        Err(_) => {
            recovery.reason = "The chosen login could not be signed in; checking again".into();
        }
    }
    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        store.record_session_recovery(
            &adapter.session,
            &recovery,
            &format!("wait:{}", uuid::Uuid::new_v4()),
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
    let (Some(key), Some(native)) = (&client.inbox, &client.native.claude_home) else {
        return Ok(());
    };
    let Some((mut socket, token)) = crate::server::wake::open_inbox(key, thread)? else {
        return Ok(());
    };
    recovery.request = Some(gate.attempt().into());
    recovery.state = RecoveryState::Recovering;
    recovery.reason = "Preparing to resend the stopped turn".into();
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
        recovery.reason = "The resent turn is running".into();
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
            "The resend was not confirmed; inspect the conversation before continuing".into();
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            store.record_session_recovery(&adapter.session, &recovery, "unconfirmed")
        })?;
        tracing::warn!(%error, "Claude recovery admission stopped");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::id::LfProcessId;
    use crate::server::recovery::{Client, NativeRecovery};
    use crate::session_recovery::{RecoveryState, SessionRecovery};
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;
    use std::io::Write;

    #[test]
    fn recovery_classifies_the_rejection_without_waiting_for_capacity() {
        let _lock = crate::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::clear(&[
            "LF_HOME",
            crate::steer::PROVIDER_CONTEXT_ENV,
            "LF_RECOVERY_GUARD",
        ]);
        for (managed, guarded, complete, expected) in [
            (true, true, true, RecoveryState::Waiting),
            (true, false, true, RecoveryState::ManualRetry),
            (false, true, true, RecoveryState::ManualRetry),
            (false, false, false, RecoveryState::Uncertain),
        ] {
            let home = tempfile::tempdir().unwrap();
            std::env::set_var("LF_HOME", home.path());
            std::env::set_var("LF_RECOVERY_GUARD", if guarded { "1" } else { "0" });
            let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
            store.test_session("reader", &crate::session_record::new_artifact_key());
            let process = store.test_lf_process();
            let attachment = store
                .claim_session_attachment("reader", None, &process, true)
                .unwrap();
            store
                .record_session_connection("reader", &attachment, "fixture", &"thread".into())
                .unwrap();
            let adapter = store.reserve_steer_adapter("reader", &attachment).unwrap();
            std::env::set_var(
                crate::steer::PROVIDER_CONTEXT_ENV,
                serde_json::to_string(&adapter).unwrap(),
            );
            let account =
                managed.then(|| crate::provider_account::parse_account_id("spent").unwrap());
            store
                .record_recovery_boundary("reader", "thread", "prompt", "started", account.as_ref())
                .unwrap();
            let mut rows = vec![
                json!({"type":"user","isSidechain":false,"uuid":"user","promptId":"prompt"}),
                json!({"type":"api-request","params":{"model":"sonnet"}}),
                json!({"type":"assistant","isSidechain":false,"parentUuid":"user",
                    "isApiErrorMessage":true,"failedResponseHadNoToolCall":true,"error":"rate_limit",
                    "message":{"content":[{"type":"text","text":"You've hit your limit"}]}}),
            ];
            if !complete {
                rows[1]["params"] = json!({});
            }
            let transcript = home.path().join("transcript");
            std::fs::write(
                &transcript,
                rows.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
            super::record_limit(
                &json!({"session_id":"thread","prompt_id":"prompt","transcript_path":transcript}),
            )
            .unwrap();
            let recovery = store.session_recovery("reader").unwrap().unwrap();
            assert_eq!(recovery.state, expected);
            assert_eq!(recovery.can_retry(), complete);
            assert!(recovery.request.is_none());
            assert!(recovery.retry_at.is_none());
            if complete {
                let client = Client {
                    adapter,
                    thread: "thread".into(),
                    cwd: home.path().into(),
                    endpoint: None,
                    inbox: None,
                    native: NativeRecovery {
                        selection: Default::default(),
                        guarded,
                        claude_home: Some(home.path().into()),
                        transcript: Some(transcript),
                    },
                };
                // Isolate the classification from the broker: its spent-limit
                // receipt already exists and this fixture has no eligible login.
                store
                    .record_session_recovery("reader", &recovery, "limited")
                    .unwrap();
                if !guarded || !managed {
                    check(&store, &client).unwrap();
                    assert_eq!(
                        store.session_recovery("reader").unwrap().unwrap().state,
                        RecoveryState::ManualRetry
                    );
                    rows.push(
                        json!({"type":"user","userType":"external","isSidechain":false,"uuid":"new","promptId":"new","message":{"content":"next"}}),
                    );
                    std::fs::write(
                        client.native.transcript.as_ref().unwrap(),
                        rows.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .unwrap();
                    check(&store, &client).unwrap();
                    assert!(store.session_recovery("reader").unwrap().is_none());
                }
            }
        }
    }

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
            state: RecoveryState::Waiting,
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
        let path = home.path().join("transcript");
        std::fs::write(
            &path,
            [
                json!({"type":"user","uuid":"original","promptId":"rejected"}),
                json!({"type":"user","userType":"external","uuid":"inbox-uuid","promptId":"retry","message":{"content":"retry"}}),
                json!({"type":"user","userType":"external","uuid":"result","message":{"content":[{"type":"tool_result","tool_use_id":"tool","content":"done"}]}}),
                json!({"type":"user","userType":"external","uuid":"meta","isMeta":true,"message":{"content":"injected"}}),
            ]
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
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
            Some(SessionRecovery {
                retry_turn: None,
                ..fact.clone()
            })
        );
        store
            .record_session_recovery("reader", &fact, "reply")
            .unwrap();
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
        let path = client.native.transcript.as_ref().unwrap();
        writeln!(
            std::fs::OpenOptions::new().append(true).open(path).unwrap(),
            "\n{}",
            json!({"type":"user","userType":"external","uuid":"typed","promptId":"next","message":{"content":"next"}})
        )
        .unwrap();
        check(&reconnected, &client).unwrap();
        assert!(reconnected.session_recovery("reader").unwrap().is_none());
        store
            .record_session_recovery("reader", &late, "wait:after-input")
            .unwrap();
        assert!(reconnected.session_recovery("reader").unwrap().is_none());
    }
}
