//! Acquire observed Session identity/time outside SQL, then validate the saved inputs.

use std::collections::HashMap;

use crate::journal::{agent_process_evidence, process_identity_evidence, ProcessIdentityEvidence};
use crate::store::StoreResult;
use loopflow_storage::sqlite::sessions::{SessionProjection, SessionProjectionInputs};
use loopflow_storage::sqlite::SqliteStore;

#[derive(Debug)]
pub struct Reading {
    pub inputs: SessionProjectionInputs,
    pub values: SessionProjection,
}

impl Reading {
    pub fn acquire(store: &SqliteStore, mut now: i64, session: Option<&str>) -> StoreResult<Self> {
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
        let graph_results = inputs
            .graph_runs
            .iter()
            .map(|run| (run.id.to_string(), crate::journal::observed_result(run)))
            .collect();
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
            values: SessionProjection {
                now,
                live_agents,
                evidence,
                graph_results,
            },
        })
    }
}

pub fn session_summaries(
    store: &SqliteStore,
    filter: &crate::session::SessionFilter,
    now: i64,
) -> StoreResult<Vec<crate::session::SessionSummary>> {
    loop {
        let reading = Reading::acquire(store, now, None)?;
        if let Some(summaries) =
            store.projected_session_summaries(filter, &reading.inputs, &reading.values)?
        {
            return Ok(summaries);
        }
    }
}

pub fn session_summary(
    store: &SqliteStore,
    id: &str,
    now: i64,
) -> StoreResult<Option<crate::session::SessionSummary>> {
    let id = loopflow_model::id::canonical_session(id);
    loop {
        let reading = Reading::acquire(store, now, Some(&id))?;
        if let Some(summary) =
            store.projected_session_summary(&id, &reading.inputs, &reading.values)?
        {
            return Ok(summary);
        }
    }
}

pub fn sessions(
    store: &SqliteStore,
    filter: &crate::session::SessionFilter,
) -> StoreResult<Vec<crate::session::LfSession>> {
    loop {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let reading = filter
            .available
            .then(|| Reading::acquire(store, now, None))
            .transpose()?;
        if let Some(sessions) = store.projected_sessions(
            filter,
            reading.as_ref().map(|r| &r.inputs),
            reading.as_ref().map_or(now, |r| r.values.now),
            reading
                .as_ref()
                .map_or("[]", |r| r.values.live_agents.as_str()),
        )? {
            return Ok(sessions);
        }
    }
}

pub(crate) fn provider_lineage(
    store: &SqliteStore,
    source: &crate::session::LfSession,
) -> StoreResult<loopflow_storage::sqlite::provider_transitions::ProviderLineage> {
    let thread = source
        .artifact_key
        .as_deref()
        .map(|key| store.input_provider_session(key))
        .transpose()?
        .flatten()
        .map(|t| t.agent_session.to_string());
    let summary = session_summary(store, &source.id, loopflow_storage::rows::now_unix())?;
    let process = summary.as_ref().and_then(|s| s.agent_process_id.clone());
    let running = summary.and_then(|s| match s.agent_process_evidence {
        crate::store::ProcessIdentityEvidence::Live => Some(true),
        crate::store::ProcessIdentityEvidence::Dead => Some(false),
        crate::store::ProcessIdentityEvidence::Unknown => None,
    });
    Ok(
        loopflow_storage::sqlite::provider_transitions::ProviderLineage {
            source_session: source.id.to_string(),
            provider_session: thread,
            agent_process: process,
            original_running: running,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;
    use crate::session::{SessionActivity, SessionFilter};

    #[test]
    fn available_limit_one_skips_dead_and_unknown_before_paging() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
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
                crate::store::session_projection::session_summaries(&store, &filter, 500).unwrap()
                    [0]
                .id
                .as_str(),
                "c-live"
            );
            assert_eq!(
                crate::store::session_projection::sessions(&store, &filter).unwrap()[0]
                    .id
                    .as_str(),
                "c-live"
            );
        }
        assert_eq!(
            crate::store::session_projection::session_summary(&store, "b-unknown", 500)
                .unwrap()
                .unwrap()
                .agent_process_evidence,
            crate::store::ProcessIdentityEvidence::Unknown
        );
    }

    #[test]
    fn a_session_reads_its_agent_lifetime_from_its_own_row() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let attachment = attachment::claim_session_attachment(
            &store,
            "session",
            None,
            &store.test_lf_process(),
            true,
        )
        .unwrap();
        let lifetime = || -> (Option<u32>, bool, bool) {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT agent_pid,agent_started_at IS NOT NULL,agent_ended_at IS NOT NULL
                     FROM agent_sessions WHERE id='session'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap()
        };
        let evidence = || {
            crate::store::session_projection::session_summary(&store, "session", 500)
                .unwrap()
                .unwrap()
                .agent_process_evidence
        };
        // Reserved: started, no OS process yet, so liveness is unconfirmed.
        assert_eq!(lifetime(), (None, true, false));
        assert_eq!(evidence(), crate::store::ProcessIdentityEvidence::Unknown);
        let pid = std::process::id();
        store
            .record_agent_process_identity(
                "session",
                &attachment,
                pid,
                crate::journal::process_started_at(pid).unwrap().unwrap(),
            )
            .unwrap();
        assert_eq!(lifetime(), (Some(pid), true, false));
        assert_eq!(evidence(), crate::store::ProcessIdentityEvidence::Live);
        store
            .record_agent_process_exit("session", &attachment, true)
            .unwrap();
        assert_eq!(lifetime(), (None, true, true));
        assert_eq!(evidence(), crate::store::ProcessIdentityEvidence::Dead);
    }

    #[test]
    fn a_changed_identity_restarts_acquisition_before_projection() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = crate::store::opening::open_ephemeral(&path).unwrap();
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
            assert!(projection.inputs.pin(&mut conn).unwrap().is_none());
        }
        drop(projection);
        assert!(
            !crate::store::session_projection::session_summary(&store, "session", 500)
                .unwrap()
                .unwrap()
                .available
        );
    }
    #[test]
    fn identity_acquisition_is_batched_and_indexes_out_completed_history() {
        use loopflow_storage::query_plans::{growing_scans, statements};
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("live", &crate::session_record::new_artifact_key());
        attachment::claim_session_attachment(&store, "live", None, &store.test_lf_process(), true)
            .unwrap();
        let read = || {
            let reading = super::Reading::acquire(&store, 500, None).unwrap();
            assert_eq!(reading.values.evidence.len(), 1);
            let mut conn = store.conn.lock().unwrap();
            assert!(reading.inputs.pin(&mut conn).unwrap().is_some());
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
            6
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
