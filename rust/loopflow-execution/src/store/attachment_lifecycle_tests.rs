use crate::process::attachment;
use crate::store::StoreError;
use loopflow_model::id::LfProcessId;
use loopflow_storage::session::SessionActivity;

use loopflow_storage::test_fixture::process as insert_process;

#[test]
fn ended_agent_socket_cannot_be_adopted() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session(
        "session",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let attachment =
        attachment::claim_session_attachment(&store, "session", None, &store.test_lf_process())
            .unwrap();
    let path = home.path().join("answering.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    store
        .record_session_connection(
            "session",
            &attachment,
            path.to_str().unwrap(),
            &"native".into(),
        )
        .unwrap();
    // Missing identity is unknown, never permission for a second launch.
    assert!(attachment::session_connection(&store, "session").is_err());
    store
        .record_agent_process_identity(
            "session",
            &attachment,
            std::process::id(),
            crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    assert!(attachment::session_connection(&store, "session")
        .unwrap()
        .is_some());
    store
        .record_agent_process_exit("session", &attachment, true)
        .unwrap();
    assert!(std::os::unix::net::UnixStream::connect(&path).is_ok());
    assert!(attachment::session_connection(&store, "session")
        .unwrap()
        .is_none());
}

#[test]
fn losing_input_claim_rolls_back_capture_and_provider_reservation() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let session = store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let first = insert_process(&store, 1, 1);
    let second = insert_process(&store, 2, 2);
    let attachment =
        attachment::claim_session_attachment(&store, &session.id, None, &first).unwrap();
    let mut next = session.clone();
    next.artifact_key = Some(loopflow_storage::session_record::new_artifact_key());
    next.input_published = false;
    let before = store.session_history(&session.id, 0, 0).unwrap();
    assert!(
        attachment::claim_session_input(&store, next.clone(), None, &second, || panic!(
            "reserved process needs no close"
        ))
        .is_err()
    );
    assert_eq!(store.session(&session.id).unwrap().unwrap(), session);
    assert_eq!(
        store.session_attachment(&session.id).unwrap(),
        Some(attachment.clone())
    );
    assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
    assert!(store
        .session_for_artifact(next.input_key().unwrap())
        .unwrap()
        .is_none());

    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET completed_at=10 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
    // A completion concurrent with preparation invalidates that snapshot.
    assert!(attachment::claim_session_input(
        &store,
        next.clone(),
        Some(&attachment),
        &second,
        || { panic!("reserved process needs no close") }
    )
    .is_err());
    assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
    assert_eq!(
        store.session_attachment(&session.id).unwrap(),
        Some(attachment.clone())
    );
    next.completed_at = Some(10);
    let birth = crate::process::os_process::process_started_at(std::process::id())
        .unwrap()
        .unwrap();
    store.test_set_holder(&session.id, std::process::id(), birth);
    // Completion does not permit taking input from a still-attached owner.
    assert!(attachment::claim_session_input(
        &store,
        next.clone(),
        Some(&attachment),
        &second,
        || { panic!("live owner must be refused before closing its provider") }
    )
    .is_err());
    assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
    assert_eq!(
        store.session(&session.id).unwrap().unwrap().completed_at,
        Some(10)
    );
    // The holder exits: this pid no longer runs the process that started then.
    store.test_set_holder(&session.id, std::process::id(), birth - 3600);
    let (admitted, claimed) =
        attachment::claim_session_input(&store, next.clone(), Some(&attachment), &second, || {
            panic!("reserved process needs no close")
        })
        .unwrap();
    assert_eq!(admitted.artifact_key, next.artifact_key);
    assert_eq!(admitted.completed_at, Some(10));
    assert_ne!(admitted.captured, session.captured);
    assert_eq!(claimed.lf_process_id, Some(second.clone()));
    let history = store.session_history(&session.id, 0, 0).unwrap();
    let reserved = store
        .agent_process(&claimed.agent_process_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        reserved.agent_session_id.as_deref(),
        Some(session.id.as_str())
    );
    assert_eq!(reserved.parent_lf_process_id.as_ref(), Some(&second));
    assert!(reserved.completed_at.is_none());
    next.artifact_key = Some(loopflow_storage::session_record::new_artifact_key());
    assert!(
        attachment::claim_session_input(&store, next, Some(&claimed), &first, || panic!(
            "reserved process needs no close"
        ))
        .is_err()
    );
    assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), history);
}

#[test]
fn a_graph_run_names_the_session_whose_agent_started_it_else_the_person() {
    use crate::process::Starter;
    use loopflow_storage::graph_process::ExecutionStart;
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&dir.path().join("store.db")).unwrap();
    store.test_session(
        "session-a",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let compiled = loopflow_storage::graph::CompiledGraph {
        name: "proof".into(),
        steps: vec![],
    };
    for (cwd, session) in [("/by-agent", Some("session-a")), ("/by-person", None)] {
        let start = ExecutionStart {
            caller_session: session.map(Into::into),
            ..ExecutionStart::reserved(1)
        };
        store
            .record_graph(
                &LfProcessId::new(),
                &compiled,
                None,
                &start,
                None,
                Some(cwd),
                None,
            )
            .unwrap();
    }
    let starter = |cwd: &str| {
        let graph = store
            .graphs_at(std::path::Path::new(cwd))
            .unwrap()
            .remove(0);
        store
            .starter(graph.caller_session.as_ref(), Some("Jack Heart"))
            .unwrap()
    };
    assert_eq!(
        starter("/by-agent"),
        Starter::Session {
            id: "session-a".into(),
            name: Some(store.session("session-a").unwrap().unwrap().title)
                .filter(|title| !title.is_empty()),
            requester: Some("Jack Heart".into()),
        }
    );
    assert_eq!(
        starter("/by-person"),
        Starter::Person {
            name: Some("Jack Heart".into())
        }
    );
}

#[test]
fn attachment_close_keeps_transfer_fenced_without_blocking_store_writes() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    store.test_session(
        "unrelated",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let parent = insert_process(&store, 1, 1);
    let attachment =
        attachment::claim_session_attachment(&store, "conversation", None, &parent).unwrap();
    let history = store.session_history("conversation", 0, 0).unwrap();
    let lock_path = std::fs::read_dir(path.with_extension("session-locks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let lock = std::fs::File::open(lock_path).unwrap();

    let close = || -> crate::store::StoreResult<()> {
        // Fail immediately on the old nested-lock shape instead of hanging.
        drop(store.conn.try_lock().expect("close must release SQLite"));
        assert_eq!(
            fs2::FileExt::try_lock_exclusive(&lock).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        crate::store::sessions::rename_session(
            &store,
            "unrelated",
            "Saved during close",
            loopflow_storage::session::TitleSource::Human,
        )?;
        assert_eq!(
            store.session_attachment("conversation")?,
            Some(attachment.clone())
        );
        Ok(())
    };
    assert!(attachment::finish_session_attachment(
        &store,
        "conversation",
        &attachment,
        "completed",
        None,
        || {
            close()?;
            Err(StoreError::InvalidData("provider close failed".into()))
        }
    )
    .is_err());
    assert_eq!(
        store.session_history("conversation", 0, 0).unwrap(),
        history
    );
    assert_eq!(
        store.session_attachment("conversation").unwrap(),
        Some(attachment.clone())
    );
    assert!(store
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());

    attachment::finish_session_attachment(
        &store,
        "conversation",
        &attachment,
        "completed",
        None,
        || {
            close()?;
            Ok(true)
        },
    )
    .unwrap();
    assert!(store
        .session_attachment("conversation")
        .unwrap()
        .unwrap()
        .lf_process_id
        .is_none());
    assert!(store
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
    assert_eq!(
        store.session("unrelated").unwrap().unwrap().title,
        "Saved during close"
    );
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
}

#[test]
fn invocation_retry_retains_history_and_refuses_failed_close_or_stale_authority() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let parent = insert_process(&store, 1, 1);
    let first =
        attachment::claim_session_attachment(&store, "conversation", None, &parent).unwrap();
    store
        .record_agent_process_launch(
            "conversation",
            &first,
            &std::process::Command::new("fixture"),
            false,
        )
        .unwrap();
    store
        .record_session_connection(
            "conversation",
            &first,
            "/fixture.sock",
            &"original-thread".into(),
        )
        .unwrap();
    let original = store
        .agent_process(&first.agent_process_id)
        .unwrap()
        .unwrap();
    for closed in [
        Ok(false),
        Err(StoreError::InvalidAuthority("close failed".into())),
    ] {
        assert!(attachment::replace_session_agent_process(
            &store,
            "conversation",
            &first,
            None,
            || closed
        )
        .is_err());
        assert_eq!(
            store.agent_process(&first.agent_process_id).unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            store.session_thread("conversation").unwrap(),
            Some("original-thread".into())
        );
        assert_eq!(
            store.session_attachment("conversation").unwrap(),
            Some(first.clone())
        );
    }
    let next =
        attachment::replace_session_agent_process(&store, "conversation", &first, None, || {
            Ok(true)
        })
        .unwrap();
    assert_ne!(first.agent_process_id, next.agent_process_id);
    assert_eq!(first.lf_process_id, next.lf_process_id);
    assert!(store
        .agent_process(&first.agent_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
    assert_eq!(store.session_thread("conversation").unwrap(), None);
    assert!(attachment::replace_session_agent_process(
        &store,
        "conversation",
        &first,
        None,
        || panic!("stale retry must never close the replacement")
    )
    .is_err());
}

#[test]
fn respawn_requires_positive_exit_and_preserves_uncertain_attempts() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let parent = insert_process(&store, 1, 1);
    let first =
        attachment::claim_session_attachment(&store, "conversation", None, &parent).unwrap();
    assert_eq!(
        attachment::prepare_session_agent_process(&store, "conversation", &first).unwrap(),
        first
    );
    store
        .record_agent_process_launch(
            "conversation",
            &first,
            &std::process::Command::new("fixture"),
            false,
        )
        .unwrap();
    assert!(attachment::prepare_session_agent_process(&store, "conversation", &first).is_err());
    assert_eq!(
        store.session_attachment("conversation").unwrap(),
        Some(first.clone())
    );
    store
        .record_agent_process_exit("conversation", &first, false)
        .unwrap();
    let failed = store
        .agent_process(&first.agent_process_id)
        .unwrap()
        .unwrap();
    let next = attachment::prepare_session_agent_process(&store, "conversation", &first).unwrap();
    assert_ne!(next.agent_process_id, first.agent_process_id);
    assert_eq!(next.lf_process_id, first.lf_process_id);
    assert_eq!(
        store.agent_process(&first.agent_process_id).unwrap(),
        Some(failed)
    );
    assert!(attachment::prepare_session_agent_process(&store, "conversation", &first).is_err());
    assert!(store
        .with_session_attachment("conversation", &first, || Ok(()))
        .is_err());
    assert_eq!(
        store.session_attachment("conversation").unwrap(),
        Some(next)
    );
}

#[test]
fn returning_to_the_same_lf_process_does_not_revive_its_previous_attachment() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let a = LfProcessId::new();
    let b = LfProcessId::new();
    for id in [&a, &b] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [id],
            )
            .unwrap();
    }
    let first = attachment::claim_session_attachment(&store, "conversation", None, &a).unwrap();
    let second =
        attachment::claim_session_attachment(&store, "conversation", Some(&first), &b).unwrap();
    let third =
        attachment::claim_session_attachment(&store, "conversation", Some(&second), &a).unwrap();
    assert_eq!(first.lf_process_id, third.lf_process_id);
    assert_eq!(first.launcher_lf_process_id, third.launcher_lf_process_id);
    assert_ne!(first.agent_process_id, third.agent_process_id);
    assert_ne!(first.token, third.token);
    assert!(store
        .record_session_agent_session("conversation", &first, &"stale-history".into())
        .is_err());
    store
        .record_agent_process_identity(
            "conversation",
            &third,
            std::process::id(),
            crate::process::os_process::process_started_at(std::process::id())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    store
        .record_session_connection("conversation", &third, "endpoint", &"native-history".into())
        .unwrap();
    assert_eq!(
        attachment::session_connection(&store, "conversation").unwrap(),
        Some(("endpoint".into(), "native-history".into()))
    );

    let current = SessionActivity {
        preview: None,
        observed_at: 100,
        open_tools: 1,
        pending_input: 0,
        yielded: false,
    };
    store
        .record_session_activity("conversation", &third, &current)
        .unwrap();
    for stale in [&first, &second] {
        // Queued sends and approval replies share this exact dispatch boundary.
        assert!(matches!(
            store.with_session_attachment::<()>("conversation", stale, || panic!(
                "stale native write"
            )),
            Err(StoreError::InvalidAuthority(_))
        ));
        assert!(matches!(
            attachment::finish_session_attachment(
                &store,
                "conversation",
                stale,
                "interrupted",
                None,
                || panic!("stale stop")
            ),
            Err(StoreError::InvalidAuthority(_))
        ));
        assert!(matches!(
            store.release_session_attachment("conversation", stale),
            Err(StoreError::InvalidAuthority(_))
        ));
        store
            .record_session_activity(
                "conversation",
                stale,
                &SessionActivity {
                    preview: None,
                    observed_at: 200,
                    pending_input: 1,
                    ..current.clone()
                },
            )
            .unwrap();
    }
    let reading: (i64, i64) = store.conn.lock().unwrap().query_row(
        "SELECT observed_at,pending_input FROM session_activity WHERE session_id='conversation'", [], |row| Ok((row.get(0)?,row.get(1)?)),
    ).unwrap();
    assert_eq!(reading, (100, 0));
    store
        .with_session_attachment("conversation", &third, || Ok(()))
        .unwrap();
    store
        .record_agent_process_exit("conversation", &third, true)
        .unwrap();
    let released = store
        .release_session_attachment("conversation", &third)
        .unwrap();
    let fourth =
        attachment::claim_session_attachment(&store, "conversation", Some(&released), &a).unwrap();
    assert_ne!(third.token, fourth.token);
    assert!(matches!(
        store.with_session_attachment::<()>("conversation", &third, || panic!(
            "released native write"
        )),
        Err(StoreError::InvalidAuthority(_))
    ));
}
