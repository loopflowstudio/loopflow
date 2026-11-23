use crate::process::attachment;

use loopflow_storage::session::{SessionActivity, SessionFilter};

use loopflow_model::id::LfProcessId;
use loopflow_model::program_status::{Kind, Records, Report, State};

#[test]
fn dead_agent_never_waits_for_approval_quiet_or_blocked_report() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for reading in ["approval", "quiet", "blocked"] {
        for record_exit in [false, true] {
            let home = tempfile::tempdir().unwrap();
            let store =
                crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
            store.test_session(
                "session",
                &loopflow_storage::session_record::new_artifact_key(),
            );
            let attachment = attachment::claim_session_attachment(
                &store,
                "session",
                None,
                &store.test_lf_process(),
            )
            .unwrap();
            let mut child = Child(
                std::process::Command::new("/bin/sleep")
                    .env_clear()
                    .arg("60")
                    .spawn()
                    .unwrap(),
            );
            store
                .record_agent_process_identity(
                    "session",
                    &attachment,
                    child.0.id(),
                    crate::process::os_process::process_started_at(child.0.id())
                        .unwrap()
                        .unwrap(),
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
                        pending_input: usize::from(reading == "approval"),
                        yielded: false,
                    },
                )
                .unwrap();
            if reading == "blocked" {
                store
                    .begin_program_status("session", Some(&attachment.agent_process_id), "terminal")
                    .unwrap();
                let records = Records {
                    seen: true,
                    records: vec![Report {
                        state: State::Blocked,
                        id: None,
                        kind: None,
                        progress: None,
                        app: None,
                        title: None,
                        msg: Some("approval".into()),
                    }],
                };
                store
                    .record_program_status(
                        "session",
                        Some(&attachment.agent_process_id),
                        "terminal",
                        1,
                        &records,
                    )
                    .unwrap();
            }
            let filter = SessionFilter {
                available: true,
                ..SessionFilter::default()
            };
            assert_eq!(
                crate::store::session_projection::session_summaries(&store, &filter, 500)
                    .unwrap()
                    .len(),
                1
            );
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            if record_exit {
                store
                    .record_agent_process_exit("session", &attachment, true)
                    .unwrap();
            }
            assert!(
                crate::store::session_projection::sessions(&store, &filter)
                    .unwrap()
                    .is_empty(),
                "{reading}"
            );
            assert!(
                crate::store::session_projection::session_summaries(&store, &filter, 500)
                    .unwrap()
                    .is_empty(),
                "{reading}"
            );
            let summary = crate::store::session_projection::session_summary(&store, "session", 500)
                .unwrap()
                .unwrap();
            assert!(!summary.available);
            assert!(summary.program_status.is_none());
            assert_eq!(
                summary.agent_process_evidence,
                crate::store::ProcessIdentityEvidence::Dead
            );
            assert_eq!(
                store.session_attachment("session").unwrap(),
                Some(attachment),
                "death needs no token rotation"
            );
        }
    }
}

#[test]
fn program_status_overrides_inference_before_paging_and_ends_with_its_agent() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("session", "run_00000000000000000000000000000001");
    let lf_process_id = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&lf_process_id],
        )
        .unwrap();
    let first =
        attachment::claim_session_attachment(&store, "session", None, &lf_process_id).unwrap();
    store
        .record_agent_process_identity(
            "session",
            &first,
            std::process::id(),
            crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    store
        .record_session_activity(
            "session",
            &first,
            &SessionActivity {
                preview: None,
                observed_at: 1,
                open_tools: 0,
                pending_input: 0,
                yielded: false,
            },
        )
        .unwrap();
    let filter = SessionFilter {
        available: true,
        limit: 1,
        headless: true,
        ..SessionFilter::default()
    };
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 500)
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .begin_program_status("session", Some(&first.agent_process_id), "surface-a")
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
            Some(&first.agent_process_id),
            "surface-a",
            1,
            &records
        )
        .unwrap());
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 500)
            .unwrap()
            .is_empty()
    );
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
            Some(&first.agent_process_id),
            "surface-a",
            2,
            &records
        )
        .unwrap());
    assert!(store
        .begin_program_status("session", Some(&first.agent_process_id), "surface-b")
        .unwrap());
    assert!(!store
        .record_program_status(
            "session",
            Some(&first.agent_process_id),
            "surface-a",
            3,
            &records
        )
        .unwrap());
    records.records.clear();
    assert!(store
        .record_program_status(
            "session",
            Some(&first.agent_process_id),
            "surface-b",
            1,
            &records
        )
        .unwrap());
    assert!(!store
        .record_program_status(
            "session",
            Some(&first.agent_process_id),
            "surface-b",
            1,
            &records
        )
        .unwrap());
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 500)
            .unwrap()
            .is_empty()
    );
    // A Session runs one agent: the replaced one has exited.
    store
        .record_agent_process_exit("session", &first, true)
        .unwrap();
    let third =
        attachment::claim_session_attachment(&store, "session", Some(&first), &lf_process_id)
            .unwrap();
    assert!(!store
        .record_program_status(
            "session",
            Some(&first.agent_process_id),
            "surface-b",
            2,
            &records
        )
        .unwrap());
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 500)
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::store::session_projection::session_summary(&store, "session", 500)
            .unwrap()
            .unwrap()
            .program_status
            .is_none()
    );
    store
        .record_session_activity(
            "session",
            &third,
            &SessionActivity {
                preview: None,
                observed_at: 501,
                open_tools: 0,
                pending_input: 1,
                yielded: false,
            },
        )
        .unwrap();
    assert!(
        crate::store::session_projection::session_summaries(&store, &filter, 501)
            .unwrap()
            .is_empty(),
        "unknown replacement is not live"
    );
    store
        .record_agent_process_identity(
            "session",
            &third,
            std::process::id(),
            crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        crate::store::session_projection::session_summaries(&store, &filter, 501)
            .unwrap()
            .len(),
        1
    );
    assert!(store
        .session("session")
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
}
