//! Projection checks use the application's full capture manifest.
pub use loopflow_storage::test_fixture::record_captured_start;
use rusqlite::params;
use serde_json::json;
#[test]
fn first_provider_attempt_follows_original_input_order_after_import() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let input =
        crate::session_record::parse_artifact_key("run_00000000000000000000000000000001").unwrap();
    let session = store.session("conversation").unwrap().unwrap();
    assert_eq!(
        crate::store::sessions::input_history(&store, &input)
            .unwrap()
            .first_provider_attempt_at,
        None
    );
    let pr = loopflow_storage::work::task::TaskPrId::new();
    let manifest = json!({"schema_version":1,"artifact_key":input,
            "created_at":"1970-01-01T00:00:01Z","harness":"codex","surface":"headless",
            "cwd":"/fixture","subjects":[],"host":"fixture",
            "flow":{"kind":"step","task_pr_id":pr,"invocation_id":"retained",
                "flow":"feature","step":"review"}});
    store
        .retain_session_observation(
            &session,
            &crate::session::SessionObservation {
                artifact_key: input.clone(),
                source: "manifest.json".into(),
                observed_at: 1,
                task_id: None,
                wave_id: None,
                payload: json!({"input_id":input,"source":"manifest.json","evidence":manifest}),
            },
        )
        .unwrap();
    for seq in [1, 0] {
        let source = format!("events.jsonl:{seq}");
        let time = time::OffsetDateTime::from_unix_timestamp(10 + seq * 10).unwrap();
        let evidence = json!({"schema_version":1,"seq":seq,
                "observed_at":time.format(&time::format_description::well_known::Rfc3339).unwrap(),
                "type":"provider_attempt_started","attempt_key":format!("attempt-{seq}"),
                "provider":"codex","model":null});
        store
            .retain_session_observation(
                &session,
                &crate::session::SessionObservation {
                    artifact_key: input.clone(),
                    source: source.clone(),
                    observed_at: 100 + seq,
                    task_id: None,
                    wave_id: None,
                    payload: json!({"input_id":input,"source":source,"evidence":evidence}),
                },
            )
            .unwrap();
    }
    let snapshot = crate::store::sessions::input_history(&store, &input).unwrap();
    assert_eq!(snapshot.task_pr_id, Some(pr));
    assert_eq!(snapshot.observed_at, 1);
    assert_eq!(snapshot.first_provider_attempt_at, Some(10));
    assert_eq!(snapshot.usage.gaps, 0);
}
#[test]
fn history_limits_precede_payload_reads_and_keep_unfinished_work_and_exact_callers() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let mut template = store.test_session("seed", "run_000000000000000000000000000000ff");
    template.captured = None;
    template.caller_artifact_key = Some(template.input_key().unwrap().clone());
    let attached = loopflow_model::id::LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&attached],
        )
        .unwrap();
    for at in 1..=60 {
        let mut session = template.clone();
        session.id = format!("conversation-{at}").into();
        session.artifact_key = Some(format!("run_{at:032x}"));
        session.created_at = at;
        let session =
            crate::store::sessions::create_session(&store, session, (at == 1).then_some(&attached))
                .unwrap();
        record_captured_start(&store, &session.id, &at.to_string());
        if at != 1 {
            store.record_turn_completion(
                &session.id,
                &"thread".into(),
                &at.to_string(),
                time::OffsetDateTime::now_utc().unix_timestamp(),
                &json!({"status":"completed"}),
            );
        }
    }
    let (recent, truncated) =
        crate::store::sessions::recent_conversation_history(&store, None, None, None, 0, 5)
            .unwrap();
    assert!(truncated);
    assert_eq!(recent.len(), 5);
    assert_eq!(
        recent.iter().map(|row| row.observed_at).collect::<Vec<_>>(),
        [60, 59, 58, 1, 1]
    );
    let exact = crate::store::sessions::conversation_history(
        &store,
        None,
        None,
        None,
        Some(template.input_key().unwrap().as_str()),
        0,
        false,
    )
    .unwrap();
    assert_eq!(
        exact.len(),
        60,
        "an exact caller drill has no presentation budget"
    );
    let boundary =
        crate::store::sessions::conversation_history(&store, None, None, None, None, 59, false)
            .unwrap();
    assert_eq!(boundary.len(), 2);
    assert_eq!(boundary[1].observed_at, 59);
    assert_eq!(
        crate::store::sessions::conversation_history(&store, None, None, None, None, 59, true)
            .unwrap()
            .len(),
        59,
        "completed native turns in the window retain older captures"
    );
    // Corrupt body evidence outside the selected budget cannot break a list;
    // exact detail still reports that real error instead of hiding it.
    store
        .execute_history(
            "INSERT INTO session_history(session_id,receipt_key,observed_at,payload,captured_event)
             SELECT session_id,receipt_key||':events.jsonl:1',10,'{',seq FROM session_events
             WHERE kind='captured' AND session_id='conversation-10'",
            [],
        )
        .unwrap();
    assert_eq!(
        crate::store::sessions::recent_conversation_history(&store, None, None, None, 0, 5)
            .unwrap()
            .0,
        recent
    );
    assert!(
        crate::store::sessions::input_history(&store, "run_0000000000000000000000000000000a")
            .is_err()
    );
    // A turn left open by an attachment that has exited is over: it stops
    // holding a place in the recent list.
    store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_launcher_lf_process_id=(SELECT lf_process_id FROM history.turns WHERE session_id='conversation-1' LIMIT 1),
                    agent_started_at=1,agent_ended_at=100 WHERE id='conversation-1'",
                [],
            )
            .unwrap();
    assert_eq!(
        crate::store::sessions::recent_conversation_history(&store, None, None, None, 0, 5)
            .unwrap()
            .0
            .iter()
            .map(|row| row.observed_at)
            .collect::<Vec<_>>(),
        [60, 59, 58, 57, 1]
    );
    assert!(crate::store::sessions::conversation_history(
        &store,
        None,
        None,
        None,
        Some(template.input_key().unwrap().as_str()),
        0,
        false
    )
    .is_err());
}
#[test]
fn an_agent_end_ends_its_launchers_inputs_without_exit_events() {
    for captured in [true, false] {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let mut session = store.test_session("seed", &crate::session_record::new_artifact_key());
        session.id = "conversation".into();
        session.captured = None;
        session.artifact_key = Some(crate::session_record::new_artifact_key());
        let process = loopflow_model::id::LfProcessId::new();
        let creator = loopflow_model::id::LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process],
            )
            .unwrap();
        }
        store.conn.lock().unwrap().execute(
                "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,'fixture',1,25)", [&creator],
            ).unwrap();
        // Preparation may exit before another lf attaches and starts the turn.
        crate::store::sessions::create_session(&store, session, Some(&creator)).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE agent_sessions SET input_published=?1", [captured])
                .unwrap();
            store.execute_history(
"INSERT INTO turns(session_id,provider_thread,provider_turn,started_at,captured_event,lf_process_id)
                     SELECT id,'thread','turn',10,CASE WHEN ?1 THEN current_capture END,?2 FROM agent_sessions WHERE id='conversation'",
                    rusqlite::params![captured,process],
                ).unwrap();
        }
        let completed_since = || {
            crate::store::sessions::conversation_history(&store, None, None, None, None, 20, true)
                .unwrap()
        };
        assert!(completed_since().is_empty());
        // A turn with unknown ownership must not fall back to its ended creator.
        store
            .execute_history(
                "UPDATE turns SET lf_process_id=NULL WHERE session_id='conversation'",
                [],
            )
            .unwrap();
        assert!(completed_since().is_empty());
        store
            .execute_history(
                "UPDATE turns SET lf_process_id=?1 WHERE session_id='conversation'",
                [&process],
            )
            .unwrap();
        // An unrelated attachment's exit cannot finish this input.
        store
            .execute_history(
                "INSERT INTO session_history(session_id,receipt_key,observed_at,payload)
                 VALUES('conversation','attachment:other:exit',30,'{}')",
                [],
            )
            .unwrap();
        assert!(completed_since().is_empty());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_launcher_lf_process_id=?1,agent_started_at=1,agent_ended_at=30
                 WHERE id='conversation'",
                [&process],
            )
            .unwrap();
        let history = completed_since();
        assert_eq!(history.len(), 1, "captured={captured}");
        assert_eq!(history[0].captured.is_some(), captured);
        assert!(
            history[0].providers[0].outcome.is_none(),
            "process death is not provider success"
        );
    }
}
#[test]
fn aggregate_history_discovers_unattributed_native_receipts_without_a_capture() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let counts = json!({"inputTokens":12,"outputTokens":3,
            "cachedInputTokens":0,"reasoningOutputTokens":0});
    for (turn, at) in [("old", 10), ("recent", 100)] {
        store.record_turn_usage(
            "conversation",
            &"thread".into(),
            turn,
            at,
            &json!({"total":counts,"last":counts}),
        );
        store.record_turn_completion(
            "conversation",
            &"thread".into(),
            turn,
            at,
            &json!({"status":"completed"}),
        );
    }
    let rows =
        crate::store::sessions::conversation_history(&store, None, None, None, None, 50, true)
            .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "an older unlinked turn cannot hide or join the recent one"
    );
    let history = &rows[0];
    assert!(history.captured.is_none());
    assert!(history.artifact_key.is_none());
    assert!(history.task_id.is_none());
    assert!(history.wave_id.is_none());
    assert!(history.recorded_outcome.is_none());
    assert_eq!(history.observed_at, 100);
    assert_eq!(history.providers.len(), 1);
    let provider = &history.providers[0];
    assert!(
        matches!(&provider.reference, crate::session_record::ProviderHistoryReference::NativeTurn {
            turn, start_seq: None, completion_seq: Some(_), .. } if turn == "recent")
    );
    assert!(provider.lf_process_id.is_none());
    assert!(provider.task_id.is_none());
    assert!(provider.started_at.is_none());
    assert_eq!(provider.outcome.as_deref(), Some("completed"));
    assert_eq!(history.usage.input_tokens, Some(12));
    assert_eq!(history.usage.streams, 1);
    assert_eq!(
        history.usage.final_streams, 0,
        "a missing start keeps usage coverage partial"
    );
    assert!(crate::store::sessions::conversation_history(
        &store,
        None,
        None,
        Some("missing-task"),
        None,
        0,
        true
    )
    .unwrap()
    .is_empty());
    let (recent, truncated) =
        crate::store::sessions::recent_conversation_history(&store, None, None, None, 50, 1)
            .unwrap();
    assert_eq!(recent, vec![history.clone()]);
    assert!(!truncated);
    assert_eq!(
        crate::store::sessions::conversation_history(&store, None, None, None, None, 0, true)
            .unwrap()
            .len(),
        3,
        "both unlinked turns and the reserved capture retain independent evidence"
    );
    let wave = loopflow_model::id::WaveId::new();
    crate::work::wave::bootstrap::upsert_wave(
        &store,
        &crate::work::wave::Wave::new(wave.clone(), "proof".into(), "/repo".into()),
    )
    .unwrap();
    // Imported native history can establish Work without a capture. That
    // turn cannot lend its attribution to another unlinked receipt.
    store
        .execute_history(
            "INSERT INTO turns(session_id,provider_thread,provider_turn,wave_id,started_at)
             VALUES('conversation','thread','owned',?1,110)",
            [wave.as_str()],
        )
        .unwrap();
    let scoped = crate::store::sessions::conversation_history(
        &store,
        Some(wave.as_str()),
        None,
        None,
        None,
        0,
        true,
    )
    .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].providers.len(), 1);
    assert_eq!(scoped[0].wave_id.as_ref(), Some(&wave));
}
/// `lf history` reads inputs by this bound and stops early, so an input
/// captured long ago whose completion is the newest fact still leads.
#[test]
fn no_decoded_time_follows_its_inputs_bound() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let old = crate::session_record::new_artifact_key();
    let session = store.test_session("old", &old);
    store.test_session("new", &crate::session_record::new_artifact_key());
    // Captures are immutable and stamped now; later facts follow them.
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    store.record_turn_start("new", &"thread".into(), "turn", now + 100);
    let ended_at = time::OffsetDateTime::from_unix_timestamp(now + 200).unwrap();
    store
        .retain_session_observation(
            &session,
            &crate::session::SessionObservation {
                artifact_key: old.clone(),
                source: "terminal.json".into(),
                observed_at: ended_at.unix_timestamp(),
                task_id: None,
                wave_id: None,
                payload: json!({"input_id": old, "source": "terminal.json",
                "evidence": crate::session_record::TerminalReceipt {
                    schema_version: 1,
                    outcome: "ok".into(),
                    ended_at,
                    result_ref: None,
                }}),
            },
        )
        .unwrap();

    let mut pending = store
        .pending_conversation_history(None, None, None, 0, true)
        .unwrap();
    pending
        .inputs
        .sort_by_key(|input| std::cmp::Reverse(input.latest));
    let histories: Vec<_> = pending
        .inputs
        .iter()
        .map(|input| {
            (
                input.latest,
                crate::store::sessions::project_history_input(&store, &pending, input).unwrap(),
            )
        })
        .collect();
    assert_eq!(histories[0].1.artifact_key.as_deref(), Some(old.as_str()));
    assert_eq!(histories[0].1.recorded_at, Some(now + 200));
    for (latest, history) in histories {
        let reported = [Some(history.observed_at), history.recorded_at]
            .into_iter()
            .chain(
                history
                    .providers
                    .iter()
                    .flat_map(|provider| [provider.started_at, provider.completed_at]),
            )
            .flatten();
        for at in reported {
            assert!(at <= latest, "{at} follows its input's bound {latest}");
        }
    }
}

#[test]
fn native_usage_cannot_skip_a_turn_without_a_notification_for_its_baseline() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let input = crate::session_record::new_artifact_key();
    store.test_session("conversation", &input);
    let counts = |value| {
        json!({"inputTokens":value,"outputTokens":0,
            "cachedInputTokens":0,"reasoningOutputTokens":0,"cacheWriteInputTokens":0})
    };
    for (turn, total, last) in [("a", Some(20), 20), ("b", None, 30), ("c", Some(60), 10)] {
        record_captured_start(&store, "conversation", turn);
        if let Some(total) = total {
            store.record_turn_usage(
                "conversation",
                &"thread".into(),
                turn,
                2,
                &json!({"total":counts(total),"last":counts(last)}),
            );
        }
        store.record_turn_completion(
            "conversation",
            &"thread".into(),
            turn,
            3,
            &json!({"status":"completed"}),
        );
    }
    // B's usage is unknown: 20 for A and C's 10-token suffix.
    let usage = crate::store::sessions::input_history(&store, &input)
        .unwrap()
        .usage;
    assert_eq!(usage.total_input_tokens, Some(30));
    assert_eq!(usage.streams, 2);
    assert_eq!(usage.final_streams, 0);
    assert!(
        usage.gaps > 0,
        "C has only its observed suffix, not a complete baseline"
    );
}
#[test]
fn history_windows_find_inputs_by_each_start_and_end() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        // (session, capture recorded, manifest observed, terminal observed)
        for (id, captured, manifest, terminal) in [
            ("late-manifest", 10, 100, None),
            ("early-manifest", 100, 10, None),
            ("late-terminal", 10, 10, Some(100)),
        ] {
            let input = crate::session_record::new_artifact_key();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,
                     input_published,cwd,repo) VALUES(?1,?1,'human',1,1,0,'/unavailable','/repo')",
                [id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                     VALUES(?1,'captured',?2,?3,json_object('artifact_key',?2))",
                params![id, input, captured],
            )
            .unwrap();
            let captured_event = conn.last_insert_rowid();
            let manifest_payload = json!({"input_id":input,"source":"manifest.json",
                    "evidence":{"artifact_key":input,"schema_version":1}});
            let terminal_payload = json!({"input_id":input,"source":"terminal.json",
                    "evidence":{"outcome":"done","ended_at":"1970-01-01T00:01:40Z"}});
            for (file, observed, payload) in [
                ("manifest.json", Some(manifest), manifest_payload),
                ("terminal.json", terminal, terminal_payload),
            ] {
                let Some(observed) = observed else { continue };
                store.execute_history(
"INSERT INTO session_history(session_id,receipt_key,observed_at,payload,captured_event)
                         VALUES(?1,?2,?3,?4,?5)",
                        params![id, format!("{input}:{file}"), observed, payload.to_string(), captured_event],
                    )
                    .unwrap();
            }
        }
    }
    let read = |include_finished| {
        crate::store::sessions::conversation_history(
            &store,
            None,
            None,
            None,
            None,
            50,
            include_finished,
        )
        .unwrap()
        .into_iter()
        .map(|row| (row.session_id.to_string(), row.observed_at))
        .collect::<Vec<_>>()
    };
    assert_eq!(
        read(false),
        [("late-manifest".to_string(), 100)],
        "an input starts when its manifest is observed, not when it was captured"
    );
    assert_eq!(
        read(true),
        [
            ("late-manifest".to_string(), 100),
            ("late-terminal".to_string(), 10)
        ],
        "an input started earlier ends in the window when its terminal is observed there"
    );
}
#[test]
fn task_history_reads_every_row_naming_the_task() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let repository = crate::repository::location::ensure_repository(&store, "/repo").unwrap();
    let [task, other] = [0, 1].map(|_| {
        let id = loopflow_model::durable::TaskId::new();
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                repository_id: repository.clone(),
                id: id.clone(),
                project_id: None,
                title: "Drill".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        id
    });
    // Bound after its input was captured: binding assigns future work only.
    let bound = store.test_session("bound", &crate::session_record::new_artifact_key());
    crate::store::sessions::bind_session(&store, "bound", bound.captured, &task).unwrap();
    {
        let conn = store.conn.lock().unwrap();
        // (session, Task on the capture, on its manifest, on its `started` origin)
        for (id, capture, manifest, origin) in [
            ("capture", Some(&task), None, None),
            ("manifest", None, Some(&task), None),
            ("origin", None, None, Some(&task)),
            ("other", Some(&other), None, None),
        ] {
            let input = crate::session_record::new_artifact_key();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,
                     input_published,cwd,repo) VALUES(?1,?1,'human',1,1,1,'/unavailable','/repo')",
                [id],
            )
            .unwrap();
            conn.execute(
                    "INSERT INTO session_events(session_id,kind,receipt_key,task_id,observed_at,payload)
                     VALUES(?1,'captured',?2,?3,10,json_object('artifact_key',?2))",
                    params![id, input, capture.map(|task| task.as_str())],
                )
                .unwrap();
            let captured = conn.last_insert_rowid();
            conn.execute(
                "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
                params![id, captured],
            )
            .unwrap();
            if let Some(task) = manifest {
                let payload = json!({"input_id":input,"source":"manifest.json",
                        "evidence":{"artifact_key":input,"schema_version":1}});
                store.execute_history(
"INSERT INTO session_history(session_id,receipt_key,task_id,observed_at,payload,captured_event)
                         VALUES(?1,?2,?3,10,?4,?5)",
                        params![id, format!("{input}:manifest.json"), task.as_str(), payload.to_string(), captured],
                    )
                    .unwrap();
            }
            if let Some(task) = origin {
                store
                    .execute_history(
                        "INSERT INTO turns(session_id,provider_thread,provider_turn,
                         task_id,started_at,captured_event) VALUES(?1,'thread','turn',?2,10,?3)",
                        params![id, task.as_str(), captured],
                    )
                    .unwrap();
            }
        }
        conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,interactive,
                 input_published,cwd,repo) VALUES('native','native','human',1,1,0,'/unavailable','/repo')",
                [],
            )
            .unwrap();
        store
            .execute_history(
                "INSERT INTO turns(session_id,provider_thread,provider_turn,
                 task_id,started_at) VALUES('native','thread','turn',?1,10)",
                [task.as_str()],
            )
            .unwrap();
    }
    let issue = store
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT issue_identifier FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    for selector in [task.as_str(), issue.as_str()] {
        for since in [0, 5, 20] {
            let mut sessions = crate::store::sessions::conversation_history(
                &store,
                None,
                None,
                Some(selector),
                None,
                since,
                true,
            )
            .unwrap()
            .into_iter()
            .map(|row| row.session_id.to_string())
            .collect::<Vec<_>>();
            sessions.sort();
            let expected: &[&str] = if since > 10 {
                &[]
            } else {
                &["capture", "manifest", "native", "origin"]
            };
            assert_eq!(sessions, expected, "{selector} since {since}");
        }
    }
}
