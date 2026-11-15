//! Recovery runs beside the server's shared wake reader. Holders keep the
//! terminals; this retains only native admission and Codex token callbacks.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::steer::SteerAdapter;
use crate::store::sqlite::SqliteStore;

/// Launch facts supplied by the native hook, never inherited from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeRecovery {
    pub selection: crate::provider_account::selection::AccountSelection,
    pub guarded: bool,
    pub claude_home: Option<PathBuf>,
    pub transcript: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct Client {
    pub adapter: SteerAdapter,
    pub thread: crate::id::AgentSessionId,
    pub cwd: PathBuf,
    pub endpoint: Option<PathBuf>,
    pub inbox: Option<PathBuf>,
    pub native: NativeRecovery,
}

pub(crate) fn record(
    store: &SqliteStore,
    adapter: &SteerAdapter,
    thread: &str,
    value: &serde_json::Value,
) -> Result<()> {
    let Some(value) = value.get("lf_recovery") else {
        return Ok(());
    };
    let native: NativeRecovery = serde_json::from_value(value.clone())?;
    let payload = serde_json::to_string(&native)?;
    store.with_provider_context(adapter, thread, crate::steer::NativeBoundary::Turn, || {
        let conn = store.conn.lock().expect("store mutex poisoned");
        let key = format!("quota_client:{}:{thread}", adapter.agent_process_id);
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM session_events WHERE session_id=?1 AND kind='observed' AND receipt_key=?2)", params![adapter.session,key], |row| row.get(0))?;
        if !exists {
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event) SELECT id,'observed',?2,task_id,wave_id,unixepoch(),?3,current_capture FROM agent_sessions WHERE id=?1 ON CONFLICT DO NOTHING",params![adapter.session,key,payload])?;
        }
        Ok(())
    })?;
    Ok(())
}

fn clients(store: &SqliteStore) -> Result<Vec<Client>> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    let mut query = conn.prepare("SELECT s.id,p.id,a.capability,s.provider_thread,s.cwd,p.endpoint,p.inbox,e.payload,p.pid,p.os_started_at FROM processes p INDEXED BY processes_unfinished JOIN agent_sessions s ON s.agent_process_id=p.id JOIN steer_adapters a ON a.agent_process_id=p.id JOIN session_events e ON e.session_id=s.id AND e.kind='observed' AND e.receipt_key='quota_client:'||p.id||':'||s.provider_thread WHERE p.completed_at IS NULL AND s.interactive=1 AND p.agent_interactive=1")?;
    let rows = query.query_map([], |row| {
        let live = row
            .get::<_, Option<u32>>(8)?
            .zip(row.get::<_, Option<i64>>(9)?)
            .is_some_and(|(pid, start)| {
                crate::journal::process_identity_evidence(pid, start)
                    == crate::journal::ProcessIdentityEvidence::Live
            });
        Ok(live.then_some((
            SteerAdapter {
                session: row.get(0)?,
                agent_process_id: row.get(1)?,
                capability: row.get(2)?,
            },
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
        )))
    })?;
    rows.filter_map(Result::transpose)
        .map(|row| {
            let (adapter, thread, cwd, endpoint, inbox, native) = row?;
            Ok(Client {
                adapter,
                thread: thread.into(),
                cwd: cwd.into(),
                endpoint: endpoint.map(Into::into),
                inbox: inbox.map(Into::into),
                native: serde_json::from_str(&native)?,
            })
        })
        .collect()
}

#[derive(Default)]
struct Reader {
    store: Option<SqliteStore>,
    seen: Option<(i64, i64, i64)>,
    failure: crate::store::RecordingGap,
    next: Option<SystemTime>,
    checks: HashMap<crate::id::AgentProcessId, (String, i64, SystemTime)>,
}

impl Reader {
    fn read(&mut self) -> Result<Vec<(SqliteStore, Client)>> {
        if self.store.is_none() {
            let path = crate::store::default_db_path();
            if !path.exists() {
                return Ok(Vec::new());
            }
            self.store = Some(SqliteStore::new(&path)?);
        }
        let store = self.store.as_ref().expect("opened store");
        let revisions = store.revisions()?;
        let revisions = (revisions.sessions, revisions.processes, revisions.planning);
        let now = SystemTime::now();
        if self.seen == Some(revisions) && self.next.is_some_and(|next| now < next) {
            return Ok(Vec::new());
        }
        self.seen = Some(revisions);
        self.next = Some(now + Duration::from_secs(60));
        let mut ready = Vec::new();
        let clients = clients(store)?;
        let active: HashSet<_> = clients
            .iter()
            .map(|client| client.adapter.agent_process_id.clone())
            .collect();
        self.checks.retain(|id, _| active.contains(id));
        for client in clients {
            let Some(recovery) = store.session_recovery(&client.adapter.session)? else {
                continue;
            };
            let requested = store.recovery_request_revision(&client.adapter.session)?;
            let key = recovery.key();
            if self
                .checks
                .get(&client.adapter.agent_process_id)
                .is_some_and(|(prior, revision, until)| {
                    prior == &key && *revision == requested && now < *until
                })
            {
                continue;
            }
            self.checks.insert(
                client.adapter.agent_process_id.clone(),
                (key, requested, now + Duration::from_secs(60)),
            );
            ready.push((store.clone(), client));
        }
        Ok(ready)
    }
}

/// One Machine clock; connections wait on native traffic between these ticks.
pub(super) fn start() {
    tokio::spawn(async {
        let (tick, _) = watch::channel(0_u64);
        let mut jobs = HashMap::<crate::id::AgentProcessId, tokio::task::JoinHandle<()>>::new();
        let mut reader = Reader::default();
        let mut clock = tokio::time::interval(Duration::from_secs(1));
        loop {
            clock.tick().await;
            tick.send_modify(|revision| *revision = revision.wrapping_add(1));
            jobs.retain(|_, job| !job.is_finished());
            let (next, read) = crate::journal::request::spawn_blocking(move || {
                let ready = reader.read();
                if ready.is_err() {
                    reader.store = None;
                    reader.seen = None;
                }
                (reader, ready)
            })
            .await
            .expect("recovery reader returns");
            reader = next;
            let ready = match read {
                Ok(ready) => {
                    reader.failure.recovered();
                    ready
                }
                Err(error) => {
                    reader.failure.note("session recovery", &error);
                    continue;
                }
            };
            for (store, client) in ready {
                let key = client.adapter.agent_process_id.clone();
                if jobs.contains_key(&key) {
                    continue;
                }
                let ticks = tick.subscribe();
                jobs.insert(
                    key,
                    tokio::spawn(async move {
                        let result = if client.endpoint.is_some() {
                            crate::harness::codex_recovery::run(store, client, ticks).await
                        } else {
                            crate::journal::request::spawn_blocking(move || {
                                crate::harness::claude_recovery::check(&store, &client)
                            })
                            .await
                            .map_err(anyhow::Error::from)
                            .and_then(|r| r)
                        };
                        if let Err(error) = result {
                            tracing::warn!(%error,"native recovery remains waiting or uncertain");
                        }
                    }),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{clients, record, NativeRecovery, Reader};
    use crate::provider_account::selection::AccountSelection;
    use crate::session_recovery::{RecoveryState, SessionRecovery};
    use crate::store::sqlite::SqliteStore;
    use serde_json::json;

    #[test]
    fn recovery_keeps_each_clients_restrictions_after_a_server_reopen() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("limited", &crate::session_record::new_artifact_key());
        let process = store.test_lf_process();
        let attachment = store
            .claim_session_attachment("limited", None, &process, true)
            .unwrap();
        let adapter = store.reserve_steer_adapter("limited", &attachment).unwrap();
        let pid = std::process::id();
        store
            .record_agent_process_identity(
                "limited",
                &attachment,
                pid,
                crate::journal::process_started_at(pid).unwrap().unwrap(),
            )
            .unwrap();
        store
            .record_session_connection("limited", &attachment, "native.sock", &"native".into())
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='limited'",
                [],
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET agent_interactive=1 WHERE id=?1",
                [&adapter.agent_process_id],
            )
            .unwrap();
        let selection = AccountSelection::from_flags(&[], &["codex=restricted".into()]).unwrap();
        let native = NativeRecovery {
            selection: selection.clone(),
            guarded: true,
            claude_home: Some(home.path().join("native-claude")),
            transcript: None,
        };
        let callback = json!({"lf_recovery":native});
        record(&store, &adapter, "native", &callback).unwrap();
        record(&store, &adapter, "native", &callback).unwrap();
        let recovery = SessionRecovery {
            agent_process_id: attachment.agent_process_id.clone(),
            thread: "native".into(),
            failed_turn: "failed".into(),
            model: Some("model".into()),
            account: None,
            state: RecoveryState::Waiting,
            reason: "quota".into(),
            retry_at: None,
            request: None,
            retry_turn: None,
        };
        store
            .record_session_recovery("limited", &recovery, "rejected")
            .unwrap();
        let reopened = SqliteStore::new(&path).unwrap();
        let retained = clients(&reopened).unwrap();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].native.selection, selection);
        assert_eq!(
            retained[0].native.claude_home,
            Some(home.path().join("native-claude"))
        );
        assert_eq!(
            retained[0].adapter.agent_process_id,
            attachment.agent_process_id
        );
        let mut reader = Reader {
            store: Some(reopened),
            ..Default::default()
        };
        assert_eq!(reader.read().unwrap().len(), 1);
        assert!(
            reader.read().unwrap().is_empty(),
            "unchanged waits do no capacity work"
        );
        store.request_session_recovery("limited").unwrap();
        assert_eq!(
            reader.read().unwrap().len(),
            1,
            "Desktop bypasses capacity backoff"
        );
        let conn = store.conn.lock().unwrap();
        let rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM session_events WHERE receipt_key LIKE 'quota_client:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "repeated native boundaries keep one launch record");
        drop(conn);
        for state in [RecoveryState::Continued, RecoveryState::Uncertain] {
            let mut fact = recovery.clone();
            fact.failed_turn = format!("later-{state:?}");
            fact.state = state;
            store
                .record_session_recovery("limited", &fact, "rejected")
                .unwrap();
            assert_eq!(
                reader.read().unwrap().len(),
                1,
                "{state:?} still checks for later input"
            );
            store
                .record_session_recovery("limited", &fact, "ended")
                .unwrap();
            assert!(reader.read().unwrap().is_empty());
        }
    }
}
