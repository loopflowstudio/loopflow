//! Acquire observed Session identity/time outside SQL, then validate the saved inputs.

use std::collections::HashMap;

use crate::journal::{agent_process_evidence, process_identity_evidence, ProcessIdentityEvidence};
use crate::store::sqlite::sessions::{
    session_projection_inputs_in, SessionProjection, SessionProjectionInputs,
};
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

#[derive(Debug)]
pub(crate) struct Reading {
    inputs: SessionProjectionInputs,
    read_at: i64,
    session: Option<String>,
    pub values: SessionProjection,
}

impl Reading {
    pub fn acquire(store: &SqliteStore, mut now: i64, session: Option<&str>) -> StoreResult<Self> {
        let read_at = now;
        let inputs = store.session_projection_inputs(now, session)?;
        let mut evidence = HashMap::new();
        let mut live = Vec::new();
        for identity in &inputs.identities {
            let observed = agent_process_evidence(
                identity.completed_at,
                identity.pid,
                identity.os_started_at,
                Some(identity.started_at),
                process_identity_evidence,
            );
            // Summary IDs canonicalize UUID case; SQL still matches saved bytes.
            evidence.insert(identity.id.to_ascii_lowercase(), observed);
            if observed == ProcessIdentityEvidence::Live {
                live.push(identity.id.clone());
            }
        }
        if crate::observe::active() {
            for deadline in &inputs.deadlines {
                if !crate::observe::deadline(*deadline) {
                    break;
                }
                now = *deadline;
            }
        }
        live.sort_unstable();
        let live_agents = serde_json::to_string(&live)?;
        Ok(Self {
            inputs,
            read_at,
            session: session.map(str::to_owned),
            values: SessionProjection {
                now,
                live_agents,
                evidence,
            },
        })
    }

    pub fn is_current(&self, conn: &rusqlite::Connection) -> StoreResult<bool> {
        Ok(
            session_projection_inputs_in(conn, self.read_at, self.session.as_deref())?
                == self.inputs,
        )
    }

    /// The final projection uses these exact saved identities and quiet deadlines.
    /// A concurrent write restarts acquisition; no connection is lent during observation.
    pub fn pin<'a>(
        &self,
        conn: &'a mut rusqlite::Connection,
    ) -> StoreResult<Option<rusqlite::Savepoint<'a>>> {
        let snapshot = conn.savepoint()?;
        if !self.is_current(&snapshot)? {
            return Ok(None);
        }
        Ok(Some(snapshot))
    }
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;
    use crate::session::{SessionActivity, SessionFilter};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn available_limit_one_skips_dead_and_unknown_before_paging() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let pid = std::process::id();
        let started = crate::journal::process_started_at(pid).unwrap().unwrap();
        for (id, identity) in [
            ("a-dead", Some(started + 60)),
            ("b-unknown", None),
            ("c-live", Some(started)),
        ] {
            store.test_session(id, &crate::session_record::new_artifact_key());
            let attachment = attachment::claim_session_attachment(
                &store,
                id,
                None,
                &store.test_lf_process(),
                true,
            )
            .unwrap();
            if let Some(started) = identity {
                store
                    .record_agent_process_identity(id, &attachment, pid, started)
                    .unwrap();
            }
            store
                .record_session_activity(
                    id,
                    &attachment,
                    &SessionActivity {
                        preview: None,
                        observed_at: 1,
                        open_tools: 0,
                        pending_input: 1,
                        yielded: false,
                    },
                )
                .unwrap();
        }
        for after in [None, Some(String::new()), Some("a-dead".into())] {
            let filter = SessionFilter {
                available: true,
                limit: 1,
                after,
                ..Default::default()
            };
            assert_eq!(
                store.session_summaries(&filter, 500).unwrap()[0]
                    .id
                    .as_str(),
                "c-live"
            );
            assert_eq!(store.sessions(&filter).unwrap()[0].id.as_str(), "c-live");
        }
        assert_eq!(
            store
                .session_summary("b-unknown", 500)
                .unwrap()
                .unwrap()
                .agent_process_evidence,
            crate::journal::ProcessIdentityEvidence::Unknown
        );
    }

    #[test]
    fn a_changed_identity_restarts_acquisition_before_projection() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let attachment = attachment::claim_session_attachment(
            &store,
            "session",
            None,
            &store.test_lf_process(),
            true,
        )
        .unwrap();
        let pid = std::process::id();
        store
            .record_agent_process_identity(
                "session",
                &attachment,
                pid,
                crate::journal::process_started_at(pid).unwrap().unwrap(),
            )
            .unwrap();
        store
            .record_session_activity(
                "session",
                &attachment,
                &SessionActivity {
                    preview: None,
                    observed_at: 1,
                    open_tools: 0,
                    pending_input: 1,
                    yielded: false,
                },
            )
            .unwrap();
        let projection = super::Reading::acquire(&store, 500, None).unwrap();
        assert!(store.conn.lock().unwrap().is_autocommit());
        // Another connection writes after observation, before the final SQL reading.
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer
            .execute(
                "UPDATE processes SET os_started_at=os_started_at+60 WHERE id=?1",
                [&attachment.agent_process_id],
            )
            .unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            assert!(projection.pin(&mut conn).unwrap().is_none());
        }
        drop(projection);
        assert!(
            !store
                .session_summary("session", 500)
                .unwrap()
                .unwrap()
                .available
        );
    }
    #[test]
    fn identity_acquisition_is_batched_and_indexes_out_completed_history() {
        use crate::store::query_plans::{growing_scans, statements};
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("live", &crate::session_record::new_artifact_key());
        attachment::claim_session_attachment(&store, "live", None, &store.test_lf_process(), true)
            .unwrap();
        let read = || {
            let reading = super::Reading::acquire(&store, 500, None).unwrap();
            assert_eq!(reading.values.evidence.len(), 1);
            let mut conn = store.conn.lock().unwrap();
            assert!(reading.pin(&mut conn).unwrap().is_some());
        };
        let before = statements(&store, read);
        store.conn.lock().unwrap().execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<10000)
             INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,input_published,cwd,completed_at)
             SELECT printf('old-%05d',i),'Old','human',1,1,0,'/repo',2 FROM n;"
        ).unwrap();
        let after = statements(&store, read);
        assert_eq!(before.len(), after.len());
        assert_eq!(
            after.iter().filter(|sql| sql.starts_with("SELECT")).count(),
            4
        );
        let scans = growing_scans(&store, &after);
        assert!(scans.is_empty(), "{scans:?}");
        let exact = statements(&store, || {
            let reading = super::Reading::acquire(&store, 500, Some("old-10000")).unwrap();
            assert!(reading.values.evidence.is_empty());
        });
        let scans = growing_scans(&store, &exact);
        assert!(scans.is_empty(), "{scans:?}");
    }
}
