//! Provider observations survive the client that happened to receive them.
//! These receipts confer no conversational or Flow mutation authority.

use serde_json::{json, Value};

use crate::exec::SessionDriver;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

pub(super) fn record(
    store: &SqliteStore,
    session: &str,
    driver: Option<&SessionDriver>,
    expected_thread: Option<&str>,
    rpc: &Value,
) -> StoreResult<()> {
    let method = rpc
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(
        method,
        "turn/started" | "turn/completed" | "thread/tokenUsage/updated"
    ) && rpc.pointer("/result/turn").is_none()
        && rpc.pointer("/result/thread/turns").is_none()
        && rpc.pointer("/result/data").is_none()
    {
        return Ok(());
    }
    let params = &rpc["params"];
    let result = &rpc["result"];
    let stored_thread = if expected_thread.is_none() {
        store.session_connection(session)?.map(|(_, thread)| thread)
    } else {
        None
    };
    let observed_thread = params["threadId"]
        .as_str()
        .or(result["thread"]["id"].as_str());
    let thread = expected_thread
        .or(stored_thread.as_deref())
        .or(observed_thread);
    let Some(thread) = thread else { return Ok(()) };
    if observed_thread.is_some_and(|observed| observed != thread) {
        return Ok(());
    }
    let turn = super::codex_mapping::extract_turn_id(params);
    if let Some(turn) = turn {
        match method {
            "turn/started" => {
                store.record_session_event(
                    session,
                    thread,
                    &turn,
                    SessionEventKind::Started,
                    &json!({}),
                )?;
            }
            "thread/tokenUsage/updated" => {
                // Keep the provider's cumulative and last-request counters.
                // Missing baseline is not permission to attribute all lifetime
                // usage to the client which reconnected halfway through a turn.
                store.record_session_event(
                    session,
                    thread,
                    &turn,
                    SessionEventKind::Usage,
                    &params["tokenUsage"],
                )?;
            }
            "turn/completed" => {
                completion(store, session, thread, &params["turn"])?;
            }
            _ => {}
        }
    }
    if let Some(turn) = result["turn"]["id"].as_str() {
        if let Some(driver) = driver {
            if let Some(exec) = &driver.exec_id {
                store.record_session_turn_origin(
                    session,
                    thread,
                    turn,
                    driver.provider_generation,
                    exec,
                )?;
            }
        }
    }
    if let Some(turns) = result["thread"]["turns"]
        .as_array()
        .or(result["data"].as_array())
    {
        for turn in turns {
            // Snapshot recovery knows completion, not which client started the
            // turn or which provider generation was active at that earlier time.
            completion(store, session, thread, turn)?;
        }
    }
    Ok(())
}

fn completion(store: &SqliteStore, session: &str, thread: &str, turn: &Value) -> StoreResult<()> {
    let Some(id) = turn["id"].as_str() else {
        return Ok(());
    };
    let Some(status @ ("completed" | "failed" | "interrupted")) = turn["status"].as_str() else {
        return Ok(());
    };
    store.record_session_event(session, thread, id, SessionEventKind::Completed,
        &json!({"status": status, "error": turn["error"], "started_at": turn["startedAt"], "completed_at": turn["completedAt"], "duration_ms": turn["durationMs"]}))?;
    Ok(())
}
