//! Observed provider changes. Reservations and viewer handoffs are not launches.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::StoreResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Retained,
    Resumed,
    Forked,
    Started,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderTransition {
    pub kind: TransitionKind,
    pub from_provider_session: Option<String>,
    pub to_provider_session: String,
    pub from_agent_process: Option<loopflow_model::id::AgentProcessId>,
    pub to_agent_process: loopflow_model::id::AgentProcessId,
    pub original_running: Option<bool>,
    pub seed: Option<String>,
    pub observed_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderLineage {
    pub source_session: String,
    pub provider_session: Option<String>,
    pub agent_process: Option<loopflow_model::id::AgentProcessId>,
    pub original_running: Option<bool>,
}

/// The latest transition, kept on the Session as state.
pub(crate) fn latest_in(
    conn: &Connection,
    session: &str,
) -> StoreResult<Option<ProviderTransition>> {
    let transition: Option<String> = conn
        .query_row(
            "SELECT transition FROM agent_sessions WHERE id=?1",
            [session],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(transition
        .map(|transition| serde_json::from_str(&transition))
        .transpose()?)
}

// Released history keeps its original payload; only its reader translates it.
fn decode(payload: &str, at: i64) -> StoreResult<ProviderTransition> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    if value["type"] == "provider_transition" {
        return Ok(serde_json::from_value(value["transition"].clone())?);
    }
    let process: loopflow_model::id::AgentProcessId =
        serde_json::from_value(value["agent_process_id"].clone())?;
    let forked = value["source"] == "fork";
    Ok(ProviderTransition {
        kind: if forked {
            TransitionKind::Forked
        } else {
            TransitionKind::Started
        },
        from_provider_session: serde_json::from_value(value["provider_thread"].clone())?,
        to_provider_session: serde_json::from_value(value["next_provider_thread"].clone())?,
        from_agent_process: value["provider_thread"].as_str().map(|_| process.clone()),
        to_agent_process: process,
        original_running: None,
        seed: forked.then(|| "native".into()),
        observed_at: at,
    })
}

fn live_process(
    conn: &Connection,
    session: &str,
) -> StoreResult<Option<loopflow_model::id::AgentProcessId>> {
    Ok(conn
        .query_row(
            "SELECT agent_process_id FROM agent_sessions
         WHERE id=?1 AND agent_pid IS NOT NULL AND agent_ended_at IS NULL",
            [session],
            |row| row.get(0),
        )
        .optional()?)
}

fn is_latest(
    previous: Option<&ProviderTransition>,
    process: &loopflow_model::id::AgentProcessId,
    thread: &str,
) -> bool {
    previous.is_some_and(|p| p.to_agent_process == *process && p.to_provider_session == thread)
}

/// Whether [`observe_in`] would record nothing: no live provider, or `thread`
/// in it is already the latest transition.
pub(crate) fn observed_in(conn: &Connection, session: &str, thread: &str) -> StoreResult<bool> {
    let Some(process) = live_process(conn, session)? else {
        return Ok(true);
    };
    Ok(is_latest(
        latest_in(conn, session)?.as_ref(),
        &process,
        thread,
    ))
}

pub(crate) fn observe_in(
    conn: &Connection,
    session: &str,
    thread: &str,
    source: &str,
) -> StoreResult<Option<crate::history::Row>> {
    let Some(process) = live_process(conn, session)? else {
        return Ok(None);
    };
    let previous = latest_in(conn, session)?;
    if is_latest(previous.as_ref(), &process, thread) {
        return Ok(None);
    }
    // The lineage seeds only the input's first transition, which takes it.
    let lineage: Option<String> = conn
        .query_row(
            "SELECT c.provider_lineage FROM agent_sessions s
             JOIN session_events c ON c.seq=s.current_capture WHERE s.id=?1",
            [session],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let lineage: Option<ProviderLineage> = lineage.map(|s| serde_json::from_str(&s)).transpose()?;
    let from_thread = lineage
        .as_ref()
        .and_then(|p| p.provider_session.clone())
        .or_else(|| previous.as_ref().map(|p| p.to_provider_session.clone()));
    let from_process = lineage
        .as_ref()
        .and_then(|p| p.agent_process.clone())
        .or_else(|| previous.as_ref().map(|p| p.to_agent_process.clone()));
    let forked = source == "fork" || lineage.is_some();
    let kind = if from_thread.as_deref() == Some(thread) {
        if from_process.as_ref() == Some(&process) {
            TransitionKind::Retained
        } else {
            TransitionKind::Resumed
        }
    } else if forked {
        TransitionKind::Forked
    } else {
        TransitionKind::Started
    };
    let transition = ProviderTransition {
        kind,
        from_provider_session: from_thread,
        to_provider_session: thread.into(),
        from_agent_process: from_process,
        to_agent_process: process.clone(),
        original_running: lineage.as_ref().and_then(|p| p.original_running),
        seed: forked.then(|| if source == "fork" { "native" } else { "log" }.into()),
        observed_at: crate::rows::now_unix(),
    };
    conn.execute(
        "UPDATE agent_sessions SET transition=?2 WHERE id=?1",
        params![session, serde_json::to_string(&transition)?],
    )?;
    if lineage.is_some() {
        conn.execute(
            "UPDATE session_events SET provider_lineage=NULL
             WHERE seq=(SELECT current_capture FROM agent_sessions WHERE id=?1)",
            [session],
        )?;
    }
    let (attached, captured): (Option<String>, Option<i64>) = conn.query_row(
        "SELECT attached_lf_process_id,current_capture FROM agent_sessions WHERE id=?1",
        [session],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut row = crate::history::Row::observed(
        session,
        format!("provider-transition:{}:{}", process, uuid::Uuid::new_v4()),
        captured,
        transition.observed_at,
        serde_json::to_string(
            &serde_json::json!({"type":"provider_transition","source":source,"transition":transition}),
        )?,
    );
    row.lf_process_id = attached;
    Ok(Some(row))
}

impl SqliteStore {
    pub fn provider_transition(&self, session: &str) -> StoreResult<Option<ProviderTransition>> {
        latest_in(&self.conn.lock().expect("store mutex poisoned"), session)
    }

    /// Every transition `session` made, oldest first, from its history.
    pub fn provider_transitions(&self, session: &str) -> StoreResult<Vec<ProviderTransition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare_cached(
            "SELECT payload,observed_at FROM history.session_history
             WHERE session_id=?1 AND kind='transition' ORDER BY seq",
        )?;
        let rows = statement.query_map([session], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.map(|row| {
            let (payload, at) = row?;
            decode(&payload, at)
        })
        .collect()
    }

    pub fn record_provider_lineage(
        &self,
        next: &crate::session::LfSession,
        source: &ProviderLineage,
    ) -> StoreResult<()> {
        let mut payload = serde_json::to_value(source)?;
        payload["type"] = "provider_lineage".into();
        let payload = serde_json::to_string(&payload)?;
        let captured: Option<i64> = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let captured = conn
                .query_row(
                    "SELECT current_capture FROM agent_sessions WHERE id=?1",
                    [&next.id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or(crate::StoreError::NotFound)?;
            conn.execute(
                "UPDATE session_events SET provider_lineage=?2 WHERE seq=?1 AND kind='captured'",
                params![captured, payload],
            )?;
            captured
        };
        self.append_history(
            "provider lineage",
            &[crate::history::Row::observed(
                &next.id,
                format!(
                    "lineage:{}",
                    next.artifact_key.as_deref().unwrap_or("unknown")
                ),
                captured,
                crate::rows::now_unix(),
                payload,
            )],
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::{observe_in, TransitionKind};

    #[test]
    fn transition_fixture_and_released_history_keep_their_meaning() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/provider_transition.json"
        ));
        let transition: super::ProviderTransition = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&transition).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        let legacy = super::decode(r#"{"type":"agent_session_selected","agent_process_id":"44444444-4444-4444-8444-444444444444","provider_thread":"earlier","next_provider_thread":"fork","source":"fork"}"#, 42).unwrap();
        assert_eq!(legacy.kind, TransitionKind::Forked);
        assert_eq!(legacy.from_provider_session.as_deref(), Some("earlier"));
        assert_eq!(legacy.observed_at, 42);
        assert_eq!(legacy.original_running, None);
    }

    #[test]
    fn explicit_provider_changes_keep_the_original_thread() {
        let (_home, store, adapter) = crate::test_fixture::steer();
        let attachment = store.session_attachment("session").unwrap().unwrap();
        assert_eq!(attachment.agent_process_id, adapter.agent_process_id);
        store
            .record_session_agent_session("session", &attachment, &"native".into())
            .unwrap();
        let observe = |thread: &str, source: &str| {
            let row = observe_in(&store.conn.lock().unwrap(), "session", thread, source).unwrap();
            store.append_history("transition", &Vec::from_iter(row));
        };
        observe("fork", "fork");
        let forked = store.provider_transition("session").unwrap().unwrap();
        assert_eq!(forked.kind, TransitionKind::Forked);
        assert_eq!(forked.seed.as_deref(), Some("native"));
        assert_eq!(forked.from_provider_session.as_deref(), Some("native"));
        observe("empty", "new");
        let empty = store.provider_transition("session").unwrap().unwrap();
        assert_eq!(empty.kind, TransitionKind::Started);
        assert_eq!(empty.from_provider_session.as_deref(), Some("fork"));
        assert_eq!(store.provider_transitions("session").unwrap().len(), 3);
    }
}
