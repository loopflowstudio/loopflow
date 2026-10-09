//! Passive terminal observation shares session_activity; no driver claim.
use rusqlite::params;

use crate::program_status::Records;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn begin_program_status(
        &self,
        session: &str,
        provider_generation: i64,
        stream: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "INSERT INTO session_activity(session_id,attachment_token,observed_at,open_tools,pending_input,yielded,provider_generation,status_stream,status_sequence)
             SELECT s.id,NULL,0,0,0,0,p.provider_generation,?3,0 FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid
             WHERE s.id=?1 AND p.provider_generation=?2 AND s.completed_at IS NULL
             ON CONFLICT(session_id) DO UPDATE SET
                attachment_token=CASE WHEN session_activity.provider_generation=?2 THEN session_activity.attachment_token ELSE NULL END,
                program_status=CASE WHEN session_activity.provider_generation=?2 THEN session_activity.program_status END,
                provider_generation=?2,status_stream=?3,status_sequence=0",
            params![session, provider_generation, stream],
        )? == 1)
    }

    pub(crate) fn record_program_status(
        &self,
        session: &str,
        provider_generation: i64,
        stream: &str,
        sequence: i64,
        records: &Records,
    ) -> StoreResult<bool> {
        if sequence <= 0 || !records.seen || !records.validate() {
            return Err(StoreError::InvalidData(
                "invalid Program Status snapshot".into(),
            ));
        }
        let json = serde_json::to_string(records)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE session_activity SET program_status=?5,status_sequence=?4
             WHERE session_id=?1 AND provider_generation=?2 AND status_stream=?3 AND status_sequence<?4
             AND EXISTS(SELECT 1 FROM agent_sessions s JOIN processes p ON p.lfid=s.agent_process_lfid WHERE s.id=?1 AND p.provider_generation=?2 AND s.completed_at IS NULL)",
            params![session, provider_generation, stream, sequence, json],
        )? == 1)
    }
}

#[cfg(test)]
mod tests {
    use crate::id::ProcessLfid;
    use crate::program_status::{Kind, Records, Report, State};
    use crate::session::{SessionActivity, SessionFilter};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn program_status_overrides_inference_before_paging_and_survives_driver_handoff() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("session", "run_00000000000000000000000000000001");
        let process_lfid = ProcessLfid::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process_lfid],
            )
            .unwrap();
        let first = store
            .claim_session_attachment("session", None, &process_lfid, true)
            .unwrap();
        store
            .record_session_activity(
                "session",
                &first,
                &SessionActivity {
                    observed_at: 1,
                    open_tools: 0,
                    pending_input: 0,
                    yielded: false,
                },
            )
            .unwrap();
        let filter = SessionFilter {
            waiting: true,
            limit: 1,
            interactive: None,
            ..SessionFilter::default()
        };
        assert_eq!(store.session_summaries(&filter, 500).unwrap().len(), 1);
        assert!(store
            .begin_program_status("session", first.provider_generation, "surface-a")
            .unwrap());
        let mut records = Records {
            seen: true,
            records: vec![Report {
                state: State::Working,
                id: None,
                kind: None,
                progress: None,
                app: None,
                title: None,
                msg: None,
            }],
        };
        assert!(store
            .record_program_status(
                "session",
                first.provider_generation,
                "surface-a",
                1,
                &records
            )
            .unwrap());
        assert!(store.session_summaries(&filter, 500).unwrap().is_empty());
        assert_eq!(store.next_quiet_waiting(1).unwrap(), None);
        records.records.push(Report {
            state: State::Blocked,
            id: Some("worker".into()),
            kind: Some(Kind::Question),
            msg: Some("Hi".into()),
            ..records.records[0].clone()
        });
        assert!(store
            .record_program_status(
                "session",
                first.provider_generation,
                "surface-a",
                2,
                &records
            )
            .unwrap());
        let second = store
            .claim_session_attachment("session", Some(&first), &process_lfid, false)
            .unwrap();
        assert_eq!(
            store.session_summaries(&filter, 500).unwrap()[0]
                .program_status
                .as_ref(),
            Some(&records)
        );
        assert!(store
            .begin_program_status("session", second.provider_generation, "surface-b")
            .unwrap());
        assert!(!store
            .record_program_status(
                "session",
                first.provider_generation,
                "surface-a",
                3,
                &records
            )
            .unwrap());
        records.records.clear();
        assert!(store
            .record_program_status(
                "session",
                second.provider_generation,
                "surface-b",
                1,
                &records
            )
            .unwrap());
        assert!(!store
            .record_program_status(
                "session",
                second.provider_generation,
                "surface-b",
                1,
                &records
            )
            .unwrap());
        assert!(store.session_summaries(&filter, 500).unwrap().is_empty());
        let third = store
            .claim_session_attachment("session", Some(&second), &process_lfid, true)
            .unwrap();
        assert!(!store
            .record_program_status(
                "session",
                second.provider_generation,
                "surface-b",
                2,
                &records
            )
            .unwrap());
        assert!(store.session_summaries(&filter, 500).unwrap().is_empty());
        assert!(store
            .session_summary("session", 500)
            .unwrap()
            .unwrap()
            .program_status
            .is_none());
        store
            .record_session_activity(
                "session",
                &third,
                &SessionActivity {
                    observed_at: 501,
                    open_tools: 0,
                    pending_input: 1,
                    yielded: false,
                },
            )
            .unwrap();
        assert_eq!(store.session_summaries(&filter, 501).unwrap().len(), 1);
        assert!(store
            .session("session")
            .unwrap()
            .unwrap()
            .completed_at
            .is_none());
    }
}
