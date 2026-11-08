//! Observed provider changes. Reservations and viewer handoffs are not launches.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::store::StoreResult;

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
    pub from_agent_process: Option<crate::id::AgentProcessId>,
    pub to_agent_process: crate::id::AgentProcessId,
    pub original_running: Option<bool>,
    pub seed: Option<String>,
    pub observed_at: i64,
}

pub(crate) fn latest_in(
    conn: &Connection,
    session: &str,
) -> StoreResult<Option<ProviderTransition>> {
    let payload: Option<String> = conn.query_row(
        "SELECT json_extract(payload,'$.transition') FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.type')='provider_transition' ORDER BY seq DESC LIMIT 1",
        [session], |row| row.get(0)).optional()?;
    Ok(payload.map(|p| serde_json::from_str(&p)).transpose()?)
}

pub(crate) fn observe_in(
    conn: &Connection,
    session: &str,
    thread: &str,
    source: &str,
) -> StoreResult<()> {
    let process: Option<crate::id::AgentProcessId> = conn
        .query_row(
            "SELECT p.id FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id
         WHERE s.id=?1 AND p.pid IS NOT NULL AND p.completed_at IS NULL",
            [session],
            |row| row.get(0),
        )
        .optional()?;
    let Some(process) = process else {
        return Ok(());
    };
    let previous = latest_in(conn, session)?;
    if previous
        .as_ref()
        .is_some_and(|p| p.to_agent_process == process && p.to_provider_session == thread)
    {
        return Ok(());
    }
    let lineage: Option<String> = conn.query_row(
        "SELECT e.payload FROM session_events e JOIN agent_sessions s ON s.id=e.session_id
         WHERE s.id=?1 AND e.captured_event=s.current_capture AND json_extract(e.payload,'$.type')='provider_lineage'
         AND NOT EXISTS(SELECT 1 FROM session_events changed WHERE changed.session_id=s.id AND changed.captured_event=s.current_capture AND json_extract(changed.payload,'$.type')='provider_transition')
         ORDER BY e.seq DESC LIMIT 1", [session], |row| row.get(0)).optional()?;
    let lineage: Option<serde_json::Value> =
        lineage.map(|s| serde_json::from_str(&s)).transpose()?;
    let from_thread = lineage
        .as_ref()
        .and_then(|p| p["provider_session"].as_str())
        .map(str::to_owned)
        .or_else(|| previous.as_ref().map(|p| p.to_provider_session.clone()));
    let from_process = lineage
        .as_ref()
        .map(|p| {
            serde_json::from_value::<Option<crate::id::AgentProcessId>>(p["agent_process"].clone())
        })
        .transpose()?
        .flatten()
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
        original_running: lineage
            .as_ref()
            .and_then(|p| p["original_running"].as_bool()),
        seed: forked.then(|| if source == "fork" { "native" } else { "log" }.into()),
        observed_at: crate::store::rows::now_unix(),
    };
    conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload,captured_event)
        SELECT s.id,'observed',?2,p.attached_lf_process_id,?3,?4,s.current_capture FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
        params![session,format!("provider-transition:{}:{}",process,uuid::Uuid::new_v4()),transition.observed_at,
        serde_json::to_string(&serde_json::json!({"type":"provider_transition","source":source,"transition":transition}))?])?;
    Ok(())
}

impl SqliteStore {
    pub fn provider_transition(&self, session: &str) -> StoreResult<Option<ProviderTransition>> {
        latest_in(&self.conn.lock().expect("store mutex poisoned"), session)
    }

    pub(crate) fn record_provider_lineage(
        &self,
        next: &crate::session::LfSession,
        source: &crate::session::LfSession,
    ) -> StoreResult<()> {
        let thread = self.session_thread(&source.id)?.map(|t| t.to_string());
        let summary = self.session_summary(&source.id, crate::store::rows::now_unix())?;
        let process = summary.as_ref().and_then(|s| s.agent_process_id.clone());
        let running = summary.and_then(|s| match s.agent_process_evidence {
            crate::journal::ProcessIdentityEvidence::Live => Some(true),
            crate::journal::ProcessIdentityEvidence::Dead => Some(false),
            crate::journal::ProcessIdentityEvidence::Unknown => None,
        });
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
            SELECT id,'observed',?2,unixepoch(),?3,current_capture FROM agent_sessions WHERE id=?1",
            params![next.id,format!("lineage:{}",next.artifact_key.as_deref().unwrap_or("unknown")),
                serde_json::to_string(&serde_json::json!({"type":"provider_lineage","source_session":source.id,
                    "provider_session":thread,"agent_process":process,"original_running":running}))?])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{observe_in, TransitionKind};
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn reservations_do_not_report_success_and_observed_replacements_keep_lineage() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let owner = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'test',1)",
                [&owner],
            )
            .unwrap();
        let first = store
            .claim_session_attachment("conversation", None, &owner, false)
            .unwrap();
        store
            .record_session_agent_session("conversation", &first, &"native".into())
            .unwrap();
        assert!(store.provider_transition("conversation").unwrap().is_none());
        store
            .record_agent_process_identity("conversation", &first, std::process::id(), 1)
            .unwrap();
        store
            .record_session_agent_session("conversation", &first, &"native".into())
            .unwrap();
        let started = store.provider_transition("conversation").unwrap().unwrap();
        assert_eq!(started.kind, TransitionKind::Started);
        store
            .record_session_agent_session("conversation", &first, &"native".into())
            .unwrap();
        assert_eq!(
            store.provider_transition("conversation").unwrap().unwrap(),
            started
        );
        let next = store
            .replace_session_agent_process("conversation", &first, Some(&"native".into()), || {
                Ok(true)
            })
            .unwrap();
        assert_eq!(
            store.provider_transition("conversation").unwrap().unwrap(),
            started
        );
        store
            .record_agent_process_identity("conversation", &next, std::process::id(), 1)
            .unwrap();
        store
            .record_session_agent_session("conversation", &next, &"native".into())
            .unwrap();
        let resumed = store.provider_transition("conversation").unwrap().unwrap();
        assert_eq!(resumed.kind, TransitionKind::Resumed);
        assert_eq!(resumed.from_agent_process, Some(first.agent_process_id));
        assert_eq!(resumed.to_agent_process, next.agent_process_id);
        observe_in(&store.conn.lock().unwrap(), "conversation", "fork", "fork").unwrap();
        let forked = store.provider_transition("conversation").unwrap().unwrap();
        assert_eq!(forked.kind, TransitionKind::Forked);
        assert_eq!(forked.seed.as_deref(), Some("native"));
        assert_eq!(forked.from_provider_session.as_deref(), Some("native"));
        observe_in(&store.conn.lock().unwrap(), "conversation", "empty", "new").unwrap();
        let empty = store.provider_transition("conversation").unwrap().unwrap();
        assert_eq!(empty.kind, TransitionKind::Started);
        assert_eq!(empty.from_provider_session.as_deref(), Some("fork"));
    }
}
