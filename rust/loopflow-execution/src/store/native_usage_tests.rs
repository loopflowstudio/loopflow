//! Native usage attribution through application attachment and input replacement.
use crate::process::attachment;
use serde_json::json;
#[test]
fn native_usage_survives_attachment_and_input_replacement_without_double_counting() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let first_input =
        crate::session_record::parse_artifact_key("run_00000000000000000000000000000001").unwrap();
    let session = store.session("conversation").unwrap().unwrap();
    let first = loopflow_model::id::LfProcessId::new();
    let second = loopflow_model::id::LfProcessId::new();
    for process in [&first, &second] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
    }
    let attachment =
        attachment::claim_session_attachment(&store, &session.id, None, &first).unwrap();
    store.record_session_turn_origin(
        &"thread".into(),
        "turn",
        &store.session_turn_origin(&session.id, &attachment).unwrap(),
    );
    let counts = |input, output| {
        json!({"inputTokens":input,"outputTokens":output,
            "cachedInputTokens":2,"reasoningOutputTokens":1})
    };
    let mut replacement = session.clone();
    replacement.artifact_key = Some(crate::session_record::new_artifact_key());
    crate::store::sessions::replace_session_input(
        &store,
        session.captured,
        replacement.clone(),
        None,
    )
    .unwrap();
    attachment::claim_session_attachment(&store, &session.id, Some(&attachment), &second).unwrap();
    // The replacement AgentProcess observes the first one's surviving turn.
    store.record_turn_usage(
        &session.id,
        &"thread".into(),
        "turn",
        2,
        &json!({"total":counts(20,5),"last":counts(20,5)}),
    );
    store.record_turn_completion(
        &session.id,
        &"thread".into(),
        "turn",
        2,
        &json!({"status":"completed"}),
    );
    // Start, usage and completion are one turn, attributed at its start.
    let turns = store.session_turns(&session.id).unwrap();
    assert_eq!(turns.len(), 1);
    assert!(turns
        .iter()
        .all(|turn| turn.lf_process_id.as_deref() == Some(first.as_str())
            && turn.agent_process_id.as_deref() == Some(attachment.agent_process_id.as_str())
            && turn.usage.is_some()
            && turn.ended_at.is_some()));
    let current =
        crate::store::sessions::input_history(&store, replacement.input_key().unwrap()).unwrap();
    assert_eq!(current.usage.input_tokens, None);
    assert_eq!(current.usage.output_tokens, None);
    let old = crate::store::sessions::input_history(&store, &first_input).unwrap();
    assert_eq!(old.usage.input_tokens, Some(18));
    assert_eq!(old.usage.output_tokens, Some(5));
    assert_eq!(old.usage.cost_usd, None);
    assert_eq!(old.usage.final_streams, 0);
    assert_eq!(
        old.recorded_outcome, None,
        "native completion cannot invent the command outcome"
    );
    // A stream usage copy an earlier lf stored is not usage: only native rows count.
    for (seq, event) in [
            json!({"type":"provider_session_observed","attempt_key":"attempt","provider_session_id":"thread"}),
            json!({"type":"usage","provider":"codex","model":null,"attempt_key":"attempt","turn_key":"turn",
                "usage_stream_id":"recorder","observation_seq":1,"counter_kind":"cumulative","start_known":true,
                "final_receipt":true,"usage":{"input_tokens":18,"output_tokens":5,"cost_usd":0.2}})
        ].into_iter().enumerate() {
            let mut evidence = json!({"schema_version":1,"seq":seq,"observed_at":"2026-09-29T00:00:00Z"});
            evidence.as_object_mut().unwrap().extend(event.as_object().unwrap().clone());
            let source = format!("events.jsonl:{seq}");
            store.retain_session_observation(&session, &crate::session::SessionObservation {
                artifact_key:first_input.clone(), source:source.clone(), observed_at:1, task_id:None,wave_id:None,
                payload:json!({"input_id":first_input,"source":source,"evidence":evidence})
            }).unwrap();
        }
    let old = crate::store::sessions::input_history(&store, &first_input).unwrap();
    assert_eq!(old.usage.streams, 1);
    assert_eq!(old.usage.input_tokens, Some(18));
    assert_eq!(old.usage.output_tokens, Some(5));
    assert_eq!(old.usage.cost_usd, None);
    assert_eq!(old.usage.final_streams, 0);
    // An unobserved start cannot borrow the replacement's input or attribution.
    store.record_turn_usage(
        &session.id,
        &"thread".into(),
        "unknown",
        2,
        &json!({"total":counts(100,50),"last":counts(10,5)}),
    );
    let current =
        crate::store::sessions::input_history(&store, replacement.input_key().unwrap()).unwrap();
    assert_eq!(current.usage.input_tokens, None);
    assert_eq!(current.usage.output_tokens, None);
    assert_eq!(
        store
            .session_turns(&session.id)
            .unwrap()
            .last()
            .unwrap()
            .lf_process_id,
        None
    );
    // Even with a start, a late first usage snapshot is a lower bound, not the thread total.
    let current = store.session_attachment(&session.id).unwrap().unwrap();
    store.record_session_turn_origin(
        &"thread".into(),
        "partial",
        &store.session_turn_origin(&session.id, &current).unwrap(),
    );
    store.record_turn_usage(
        &session.id,
        &"thread".into(),
        "partial",
        2,
        &json!({"total":counts(100,50),"last":counts(10,5)}),
    );
    let partial =
        crate::store::sessions::input_history(&store, replacement.input_key().unwrap().as_str())
            .unwrap();
    assert_eq!(partial.usage.total_input_tokens, Some(10));
    assert_eq!(partial.usage.output_tokens, Some(5));
    assert!(partial.usage.gaps > 0);
}
