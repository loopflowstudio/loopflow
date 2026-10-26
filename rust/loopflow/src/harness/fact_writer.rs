//! The one writer of what decoders establish: Session receipts, availability,
//! the native conversation selection and account capacity.
//!
//! Availability is display evidence and account effects are best effort, so
//! their failures warn. A receipt failure is returned to the driver, which
//! decides whether its stream can continue.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde_json::json;

use super::fact::{Decoded, ProviderFact, ProviderTurn, Signal};
use crate::id::AgentSessionId;
use crate::process::SessionAttachment;
use crate::provider_account::ProviderAccountRoute;
use crate::session::{SessionActivity, SessionEventKind, SessionPreview, SessionTurnOrigin};
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// Origins frozen when Loopflow sent each request, keyed by request id. The
/// sending side fills it; the reader's writer takes each one once.
pub(super) type RequestOrigins = Arc<Mutex<HashMap<String, SessionTurnOrigin>>>;

pub(super) struct FactWriter {
    store: SqliteStore,
    session: String,
    /// Absent for readers with no live attachment, such as history backfill:
    /// they retain receipts but say nothing about current availability.
    attachment: Option<SessionAttachment>,
    route: Option<ProviderAccountRoute>,
    requests: RequestOrigins,
    activity: SessionActivityTracker,
}

impl std::fmt::Debug for FactWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactWriter")
            .field("session", &self.session)
            .finish()
    }
}

impl FactWriter {
    pub(super) fn new(
        store: SqliteStore,
        session: String,
        attachment: Option<SessionAttachment>,
        route: Option<ProviderAccountRoute>,
        requests: RequestOrigins,
    ) -> Self {
        Self {
            store,
            session,
            attachment,
            route,
            requests,
            activity: SessionActivityTracker::default(),
        }
    }

    /// Freeze the origin of a request while its attachment still owns dispatch.
    pub(super) fn outbound(&self, request: &str) -> StoreResult<()> {
        let Some(attachment) = &self.attachment else {
            return Ok(());
        };
        let origin = self.store.session_turn_origin(&self.session, attachment)?;
        self.requests
            .lock()
            .expect("request origin lock poisoned")
            .insert(request.to_owned(), origin);
        Ok(())
    }

    /// Write every fact in one decoded message, in order, including the
    /// account effects only a launch with an account route has.
    pub(super) async fn write(&mut self, decoded: &[Decoded]) -> Result<()> {
        super::dispatch::off_reactor(|| self.record(decoded))?;
        let Some(route) = &self.route else {
            return Ok(());
        };
        for fact in decoded.iter().filter_map(Decoded::fact) {
            match fact {
                ProviderFact::SessionObserved(thread) => {
                    if let Err(error) = route.pin_session(thread).await {
                        tracing::warn!(%error, session = self.session, "failed to pin provider session account");
                    }
                }
                ProviderFact::RateLimit(signal) => {
                    if let Err(error) = route.record_rate_limit(signal).await {
                        tracing::warn!(%error, session = self.session, "failed to record provider rate limit");
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Write the Session's own evidence from one decoded message: receipts,
    /// availability and the native conversation selection.
    pub(super) fn record(&mut self, decoded: &[Decoded]) -> StoreResult<()> {
        let mut signals = Vec::new();
        for fact in decoded.iter().filter_map(Decoded::fact) {
            match fact {
                ProviderFact::Activity(signal) => signals.push(signal.clone()),
                ProviderFact::SessionObserved(thread) => self.select_session(thread),
                fact => self.record_fact(fact)?,
            }
        }
        if !signals.is_empty() {
            self.record_activity(signals);
        }
        Ok(())
    }

    fn select_session(&self, thread: &AgentSessionId) {
        let Some(attachment) = &self.attachment else {
            return;
        };
        if let Err(error) =
            self.store
                .record_session_agent_session(&self.session, attachment, thread)
        {
            tracing::warn!(%error, session = self.session, "could not select native conversation");
        }
    }

    fn record_fact(&mut self, fact: &ProviderFact) -> StoreResult<()> {
        let event = |turn: &ProviderTurn, kind, payload| {
            self.store
                .record_session_event(&self.session, &turn.thread, &turn.turn, kind, payload)
        };
        match fact {
            ProviderFact::TurnStarted { turn } => {
                event(turn, SessionEventKind::Started, &json!({}))?;
            }
            ProviderFact::TurnAnswered { turn, request } => {
                // Input the provider's own client submitted has no frozen
                // origin; it belongs to the attachment that saw it start.
                let frozen = self
                    .requests
                    .lock()
                    .expect("request origin lock poisoned")
                    .remove(request);
                let origin = match (frozen, &self.attachment) {
                    (Some(origin), _) => Some(origin),
                    (None, Some(attachment)) => {
                        Some(self.store.session_turn_origin(&self.session, attachment)?)
                    }
                    (None, None) => None,
                };
                if let Some(origin) = origin {
                    self.store
                        .record_session_turn_origin(&turn.thread, &turn.turn, &origin)?;
                }
            }
            ProviderFact::Output { turn, answer } => {
                event(turn, SessionEventKind::Output, answer)?;
            }
            ProviderFact::Usage { turn, receipt } => {
                event(turn, SessionEventKind::Usage, receipt)?;
            }
            ProviderFact::TurnCompleted {
                turn: Some(turn),
                detail,
                ..
            } => {
                event(turn, SessionEventKind::Completed, detail)?;
            }
            &ProviderFact::AvailabilityReading {
                open_tools,
                pending_input,
                yielded,
                settled,
            } => {
                let Some(attachment) = &self.attachment else {
                    return Ok(());
                };
                let reading = SessionActivity {
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    open_tools,
                    pending_input,
                    yielded,
                    preview: None,
                };
                self.store
                    .record_session_activity(&self.session, attachment, &reading)?;
                report(&reading, settled);
            }
            _ => {}
        }
        Ok(())
    }

    fn record_activity(&mut self, signals: Vec<Signal>) {
        let Some(attachment) = &self.attachment else {
            return;
        };
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.activity
            .record_at(&self.store, &self.session, attachment, signals, now);
    }
}

/// Tell the hosting transport what a saved reading means; `failed` marks a
/// turn that ended without yielding.
fn report(reading: &SessionActivity, failed: bool) {
    let state = if reading.pending_input > 0 {
        crate::program_status::State::Blocked
    } else if reading.yielded {
        crate::program_status::State::Idle
    } else if failed {
        crate::program_status::State::Error
    } else {
        crate::program_status::State::Working
    };
    crate::transport::report(state, false, None);
}

/// Unchanged readings are saved this often, so quiet time is measured from a
/// stream that was recently alive.
const SAVE_INTERVAL: i64 = 5;

/// Each attached LfProcess that owns a stream keeps one tracker and saves its
/// reading; the Available rule itself is read from that row by the Session
/// inventory.
#[derive(Debug, Default)]
struct SessionActivityTracker {
    open_tools: BTreeSet<String>,
    pending_input: BTreeSet<String>,
    yielded: bool,
    failed: bool,
    saved: Option<SessionActivity>,
    preview: Option<SessionPreview>,
}

impl SessionActivityTracker {
    /// Apply one stream message's signals; false when it carried none.
    fn apply(&mut self, signals: Vec<Signal>, now: i64) -> bool {
        let any = !signals.is_empty();
        for signal in signals {
            match signal {
                Signal::Working => {
                    self.yielded = false;
                    self.failed = false;
                }
                Signal::NewTurn => {
                    self.yielded = false;
                    self.failed = false;
                    self.preview = None;
                }
                Signal::ToolStarted(id) => {
                    self.yielded = false;
                    self.failed = false;
                    self.open_tools.insert(id);
                }
                Signal::ToolResolved(id) => {
                    self.open_tools.remove(&id);
                }
                Signal::InputRequested(id) => {
                    self.pending_input.insert(id);
                }
                Signal::InputResolved(id) => {
                    self.pending_input.remove(&id);
                }
                Signal::Yielded(ok) => {
                    self.open_tools.clear();
                    self.failed |= !ok;
                    self.yielded = !self.failed;
                }
                Signal::Previewed { turn, source, text } => {
                    self.yielded = false;
                    self.failed = false;
                    self.preview = Some(SessionPreview {
                        provider_thread: turn.thread,
                        provider_turn: turn.turn,
                        observed_at: now,
                        source: source.into(),
                        text: text.chars().take(2048).collect(),
                    });
                }
            }
        }
        any
    }

    /// The reading to save, when it changed or the saved one has aged.
    fn reading(&mut self, now: i64) -> Option<SessionActivity> {
        let reading = SessionActivity {
            preview: self.preview.clone(),
            observed_at: now,
            open_tools: self.open_tools.len(),
            pending_input: self.pending_input.len(),
            yielded: self.yielded,
        };
        let unchanged = self.saved.as_ref().is_some_and(|saved| {
            now - saved.observed_at < SAVE_INTERVAL
                && SessionActivity {
                    observed_at: now,
                    ..saved.clone()
                } == reading
        });
        if unchanged {
            return None;
        }
        self.saved = Some(reading.clone());
        Some(reading)
    }

    /// Activity is display evidence: a failed save never fails the turn.
    fn record_at(
        &mut self,
        store: &SqliteStore,
        session: &str,
        attachment: &SessionAttachment,
        signals: Vec<Signal>,
        now: i64,
    ) {
        if !self.apply(signals, now) {
            return;
        }
        if let Some(reading) = self.reading(now) {
            report(&reading, false);
            if let Err(error) = store.record_session_activity(session, attachment, &reading) {
                tracing::warn!(%error, session, "failed to save Session activity");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{FactWriter, SessionActivityTracker};
    use crate::harness::claude_decode::ClaudeDecoder;
    use crate::harness::codex_decode::{turn_request, CodexDecoder};
    use crate::harness::fact::{trace, Decoded, ProviderFact, Signal};
    use crate::harness::opencode_decode::OpenCodeDecoder;
    use crate::id::LfProcessId;
    use crate::process::SessionAttachment;
    use crate::session::SessionEventKind;
    use crate::store::sqlite::SqliteStore;

    const START: i64 = 1_000;
    const QUIET: i64 = crate::session::AVAILABILITY_QUIET_SECONDS;

    /// A store with one Session, `conversation`, attached by a fresh Process.
    fn attached(interactive: bool) -> (tempfile::TempDir, SqliteStore, SessionAttachment) {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("facts.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let conn = rusqlite::Connection::open(&path).unwrap();
        let process = LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&process],
        )
        .unwrap();
        conn.execute("UPDATE agent_sessions SET interactive=?1", [interactive])
            .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, false)
            .unwrap();
        store
            .record_agent_process_identity(
                "conversation",
                &attachment,
                std::process::id(),
                crate::journal::process_started_at(std::process::id())
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        (home, store, attachment)
    }

    fn writer(store: &SqliteStore, attachment: &SessionAttachment) -> FactWriter {
        FactWriter::new(
            store.clone(),
            "conversation".into(),
            Some(attachment.clone()),
            None,
            Default::default(),
        )
    }

    fn signals(decoded: Vec<Decoded>) -> Vec<Signal> {
        decoded
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Fact(ProviderFact::Activity(signal)) => Some(signal),
                _ => None,
            })
            .collect()
    }

    /// A conversation whose attachment saves what a recorded stream says, one
    /// message a second from `START`.
    struct Driven {
        _home: tempfile::TempDir,
        store: SqliteStore,
        attachment: SessionAttachment,
        activity: SessionActivityTracker,
        now: i64,
    }

    impl Driven {
        fn new(interactive: bool) -> Self {
            let (home, store, attachment) = attached(interactive);
            Self {
                _home: home,
                store,
                attachment,
                activity: SessionActivityTracker::default(),
                now: START,
            }
        }

        fn hear(&mut self, decoded: Vec<Decoded>) {
            self.now += 1;
            self.activity.record_at(
                &self.store,
                "conversation",
                &self.attachment,
                signals(decoded),
                self.now,
            );
        }

        /// Whether the inventory calls it Available `quiet` seconds after the
        /// last message.
        fn available_after(&self, quiet: i64) -> bool {
            self.store
                .session_summary("conversation", self.now + quiet)
                .unwrap()
                .unwrap()
                .available
        }
    }

    fn claude(decoder: &mut ClaudeDecoder, line: &Value) -> Vec<Decoded> {
        decoder.decode(&line.to_string(), "turn").unwrap()
    }

    #[test]
    fn a_claude_turn_waits_once_its_tools_are_answered_and_it_hands_back() {
        let lines = trace("claude_multi_tool.ndjson");
        let mut decoder = ClaudeDecoder::default();
        let mut interactive = Driven::new(true);
        assert!(!interactive.available_after(QUIET), "nothing heard yet");
        for line in &lines[..lines.len() - 1] {
            interactive.hear(claude(&mut decoder, line));
            let tool_open = line["type"] == "content_block_start";
            assert!(!interactive.available_after(0));
            assert_eq!(interactive.available_after(QUIET), !tool_open);
            assert!(!interactive.available_after(QUIET - 1));
        }
        interactive.hear(claude(&mut decoder, lines.last().unwrap()));
        assert!(
            interactive.available_after(0),
            "a successful result hands back"
        );
        interactive.hear(claude(
            &mut decoder,
            &json!({"type":"user","message":{"content":"next"}}),
        ));
        assert!(!interactive.available_after(0), "new activity clears it");

        let mut decoder = ClaudeDecoder::default();
        let mut headless = Driven::new(false);
        for line in &lines {
            headless.hear(claude(&mut decoder, line));
        }
        assert!(
            !headless.available_after(0),
            "a Run's finished turn asks nothing"
        );
        assert!(headless.available_after(QUIET));
    }

    #[test]
    fn a_claude_tool_left_open_never_reads_as_available() {
        let mut decoder = ClaudeDecoder::default();
        let mut session = Driven::new(true);
        for line in trace("claude_crash_mid_tool.ndjson") {
            session.hear(claude(&mut decoder, &line));
        }
        assert!(!session.available_after(QUIET * 100));
    }

    #[test]
    fn a_failed_claude_result_waits_only_after_quiet() {
        let mut session = Driven::new(true);
        session.hear(claude(
            &mut ClaudeDecoder::default(),
            &json!({"type":"result","subtype":"error_during_execution","is_error":true}),
        ));
        assert!(!session.available_after(QUIET - 1));
        assert!(session.available_after(QUIET));
    }

    #[test]
    fn a_codex_turn_waits_when_it_completes_or_asks() {
        let mut decoder = CodexDecoder::default();
        let mut session = Driven::new(true);
        for rpc in trace("codex_normal_turn.jsonl") {
            let completed = rpc["method"] == "turn/completed";
            session.hear(decoder.decode(&rpc, None));
            assert_eq!(session.available_after(0), completed, "{}", rpc["method"]);
        }
        let thread = Some("thread".into());
        let mut server = |session: &mut Driven, rpc: Value| {
            session.hear(decoder.decode(&rpc, thread.as_ref()));
        };
        let command = json!({"type":"commandExecution","id":"call_1"});
        server(
            &mut session,
            json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"next"}}}),
        );
        server(
            &mut session,
            json!({"method":"item/started","params":{"threadId":"thread","turnId":"next","item":command}}),
        );
        assert!(
            !session.available_after(QUIET * 100),
            "a command is running"
        );
        server(
            &mut session,
            json!({"id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread","itemId":"call_1"}}),
        );
        assert!(session.available_after(0), "an approval is a question");
        session.hear(decoder.outbound(&json!({"id":7,"result":{"decision":"accept"}})));
        let mut server = |session: &mut Driven, rpc: Value| {
            session.hear(decoder.decode(&rpc, thread.as_ref()));
        };
        server(
            &mut session,
            json!({"method":"item/commandExecution/outputDelta","params":{"threadId":"thread","turnId":"next"}}),
        );
        assert!(
            !session.available_after(QUIET * 100),
            "answered, still running"
        );
        server(
            &mut session,
            json!({"method":"item/completed","params":{"threadId":"thread","turnId":"next","item":command}}),
        );
        assert!(!session.available_after(QUIET - 1));
        assert!(session.available_after(QUIET));
        server(
            &mut session,
            json!({"method":"turn/completed","params":{"threadId":"thread","turn":{"id":"next","status":"failed"}}}),
        );
        assert!(
            !session.available_after(0),
            "a failed turn is not a hand-back"
        );
    }

    #[test]
    fn an_opencode_question_waits_until_it_is_answered() {
        let mut decoder = OpenCodeDecoder::new("ses_trace".into(), None);
        let mut session = Driven::new(true);
        let mut seen = Vec::new();
        for event in trace("opencode_question.jsonl") {
            session.hear(decoder.event(&event));
            seen.push((
                event["type"].as_str().unwrap().to_owned(),
                session.available_after(0),
                session.available_after(QUIET),
            ));
        }
        let at = |kind: &str| {
            seen.iter()
                .rev()
                .find(|(seen, _, _)| seen == kind)
                .map(|(_, now, quiet)| (*now, *quiet))
                .unwrap()
        };
        // A permission the attached LfProcess answers itself is no question, and the tool
        // it guards is still open.
        assert_eq!(at("permission.asked"), (false, false));
        assert_eq!(at("form.created"), (true, true));
        assert_eq!(at("form.replied"), (false, false), "its tool is open");
        assert_eq!(at("session.execution.succeeded"), (true, true));
        // Another conversation going idle on the shared stream neither answers
        // the question nor hands this turn back.
        let other = seen
            .iter()
            .position(|(kind, _, _)| kind == "session.execution.succeeded");
        assert_eq!(seen[other.unwrap() + 1].0, "form.replied");
    }

    #[test]
    fn a_reading_from_an_attachment_that_let_go_says_nothing() {
        let mut session = Driven::new(true);
        session.hear(claude(
            &mut ClaudeDecoder::default(),
            &json!({"type":"result","subtype":"success"}),
        ));
        assert!(session.available_after(0));
        session
            .store
            .release_session_attachment("conversation", &session.attachment)
            .unwrap();
        assert!(!session.available_after(QUIET));
    }

    #[test]
    fn availability_is_chosen_before_the_page_is_cut() {
        let mut session = Driven::new(true);
        // Two working conversations sort ahead of the available one.
        session
            .store
            .test_session("a", "run_0000000000000000000000000000000a");
        session
            .store
            .test_session("b", "run_0000000000000000000000000000000b");
        session.hear(claude(
            &mut ClaudeDecoder::default(),
            &json!({"type":"result","subtype":"success"}),
        ));
        let page = |available: bool| {
            let filter = crate::session::SessionFilter {
                available,
                limit: 1,
                after: Some(String::new()),
                ..Default::default()
            };
            let rows = session.store.session_summaries(&filter, session.now);
            rows.unwrap().remove(0).id
        };
        assert_eq!(page(false), "a");
        assert_eq!(page(true), "conversation");
    }

    #[test]
    fn an_unchanged_reading_is_saved_only_as_it_ages() {
        let mut activity = SessionActivityTracker::default();
        assert!(activity.apply(vec![Signal::Working], 10));
        assert!(activity.reading(10).is_some());
        activity.apply(vec![Signal::Working], 12);
        assert!(activity.reading(12).is_none());
        activity.apply(vec![Signal::ToolStarted("call".into())], 12);
        assert_eq!(activity.reading(12).unwrap().open_tools, 1);
        activity.apply(vec![Signal::Working], 17);
        assert_eq!(activity.reading(17).unwrap().observed_at, 17);
    }

    /// Session receipts other than capture bookkeeping.
    fn receipts(store: &SqliteStore) -> Vec<crate::session::SessionEvent> {
        store
            .session_history("conversation", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| {
                !matches!(
                    event.kind,
                    SessionEventKind::Captured | SessionEventKind::Observed
                )
            })
            .collect()
    }

    #[test]
    fn a_claude_result_retains_exact_output_without_borrowing_another_request() {
        let (_home, store, attachment) = attached(false);
        let mut writer = writer(&store, &attachment);
        let mut decoder = ClaudeDecoder::default();
        let hear = |decoder: &mut ClaudeDecoder, writer: &mut FactWriter, line: Value| {
            writer
                .record(&decoder.decode(&line.to_string(), "runner").unwrap())
                .unwrap();
        };
        hear(
            &mut decoder,
            &mut writer,
            json!({"type":"user","uuid":"old","session_id":"thread"}),
        );
        hear(
            &mut decoder,
            &mut writer,
            json!({"type":"result","uuid":"old-result","session_id":"thread","subtype":"success","result":"old"}),
        );
        assert!(receipts(&store).is_empty());

        writer.outbound("request").unwrap();
        decoder.outbound("request");
        let original = store.session("conversation").unwrap().unwrap();
        let mut next = original.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        let next = store
            .replace_session_input(original.captured, next)
            .unwrap();
        hear(
            &mut decoder,
            &mut writer,
            json!({"type":"user","uuid":"request","session_id":"thread"}),
        );
        let value = json!({"decision":"advance","summary":"native proof"});
        hear(
            &mut decoder,
            &mut writer,
            json!({"type":"result","uuid":"result","session_id":"thread","subtype":"success","structured_output":value}),
        );
        let events = receipts(&store);
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|e| e.provider_turn.as_deref() == Some("request")));
        assert_eq!(events[1].kind, SessionEventKind::Output);
        assert_eq!(events[1].payload["value"], value);
        assert_eq!(
            events[2].payload,
            json!({"status":"completed","result_id":"result","errors":null,"subtype":"success"})
        );
        assert!(store
            .input_final_answer(next.input_key().unwrap())
            .unwrap()
            .is_none());
        let answer = store
            .input_final_answer(
                &crate::session_record::parse_artifact_key("run_00000000000000000000000000000001")
                    .unwrap(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&answer).unwrap(), value);
    }

    /// A Codex client message: freeze a turn request's origin, then decode it.
    fn send(decoder: &mut CodexDecoder, writer: &mut FactWriter, rpc: Value) {
        if let Some(request) = turn_request(&rpc) {
            writer.outbound(&request).unwrap();
        }
        writer.record(&decoder.outbound(&rpc)).unwrap();
    }

    fn hear(decoder: &mut CodexDecoder, writer: &mut FactWriter, rpc: Value) {
        writer
            .record(&decoder.decode(&rpc, Some(&"thread".into())))
            .unwrap();
    }

    #[test]
    fn a_codex_preview_belongs_to_the_current_turn_thread_and_attachment() {
        let (home, store, attachment) = attached(true);
        let sql = rusqlite::Connection::open(home.path().join("facts.db")).unwrap();
        let mut writer = writer(&store, &attachment);
        let mut decoder = CodexDecoder::default();
        let thread = "thread".into();
        store
            .record_session_agent_session("conversation", &attachment, &thread)
            .unwrap();
        let preview = || -> Option<crate::session::SessionPreview> {
            let value: Option<String> = sql
                .query_row(
                    "SELECT preview FROM session_activity WHERE session_id='conversation'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            value.map(|value| serde_json::from_str(&value).unwrap())
        };
        for turn in ["first", "second"] {
            send(
                &mut decoder,
                &mut writer,
                json!({"id":turn,"method":"turn/start"}),
            );
            assert_eq!(preview(), None);
            // Old output cannot return while the next turn is still starting.
            hear(
                &mut decoder,
                &mut writer,
                json!({"method":"item/completed","params":{"threadId":"thread","turnId":"first","item":{"type":"agentMessage","text":"late"}}}),
            );
            assert_eq!(preview(), None);
            hear(
                &mut decoder,
                &mut writer,
                json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":turn}}}),
            );
            assert_eq!(preview(), None);
            hear(
                &mut decoder,
                &mut writer,
                json!({"method":"item/completed","params":{"threadId":"thread","turnId":turn,"item":{"type":"agentMessage","phase":"commentary","text":turn}}}),
            );
            let reading = preview().unwrap();
            assert_eq!(reading.provider_turn, turn);
            assert_eq!(reading.text, turn);
            assert_eq!(
                store
                    .session_summary("conversation", 1000)
                    .unwrap()
                    .unwrap()
                    .preview,
                Some(reading)
            );
        }
        let current = preview();
        for (provider_thread, turn) in [("thread", "first"), ("other", "second")] {
            hear(
                &mut decoder,
                &mut writer,
                json!({"method":"item/completed","params":{"threadId":provider_thread,"turnId":turn,"item":{"type":"agentMessage","text":"late"}}}),
            );
            assert_eq!(preview(), current);
        }
        store
            .record_session_agent_session("conversation", &attachment, &"replacement".into())
            .unwrap();
        assert_eq!(
            store
                .session_summary("conversation", 1000)
                .unwrap()
                .unwrap()
                .preview,
            None
        );
    }

    #[test]
    fn a_delayed_codex_start_keeps_request_capture_across_a_b_a_takeover() {
        for reply_first in [false, true] {
            let (home, store, first) = attached(false);
            let sql = rusqlite::Connection::open(home.path().join("facts.db")).unwrap();
            let original = store.session("conversation").unwrap().unwrap();
            let a = first.lf_process_id.clone().unwrap();
            let b = LfProcessId::new();
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&b],
            )
            .unwrap();
            let origins = super::RequestOrigins::default();
            let writer_for = |attachment: &SessionAttachment| {
                FactWriter::new(
                    store.clone(),
                    "conversation".into(),
                    Some(attachment.clone()),
                    None,
                    origins.clone(),
                )
            };
            let mut decoder = CodexDecoder::default();
            send(
                &mut decoder,
                &mut writer_for(&first),
                json!({"id":1,"method":"turn/start"}),
            );
            let middle = store
                .claim_session_attachment("conversation", Some(&first), &b, false)
                .unwrap();
            let mut next = original.clone();
            next.artifact_key = Some(crate::session_record::new_artifact_key());
            let next = store
                .replace_session_input(original.captured, next)
                .unwrap();
            let current = store
                .claim_session_attachment("conversation", Some(&middle), &a, false)
                .unwrap();
            assert_ne!(first.token, current.token);
            assert!(store.session_turn_origin("conversation", &first).is_err());
            // Both requests predate the start notification. A second reply for
            // the same turn must not replace the first correlated origin.
            send(
                &mut decoder,
                &mut writer_for(&current),
                json!({"id":2,"method":"turn/start"}),
            );
            let reply = json!({"id":1,"result":{"turn":{"id":"late"}}});
            let start = json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"late"}}});
            let messages = if reply_first {
                [reply.clone(), start.clone()]
            } else {
                [start.clone(), reply.clone()]
            };
            for message in messages {
                hear(&mut decoder, &mut writer_for(&first), message);
                assert_eq!(sql.query_row("SELECT count(*) FROM session_events WHERE kind='started' AND captured_event=?1", [next.captured], |row| row.get::<_,i64>(0)).unwrap(),0);
            }
            let before = store.session_history("conversation", 0, 0).unwrap();
            for message in [json!({"id":2,"result":{"turn":{"id":"late"}}}), start] {
                hear(&mut decoder, &mut writer_for(&current), message);
            }
            assert_eq!(store.session_history("conversation", 0, 0).unwrap(), before);
            hear(
                &mut decoder,
                &mut writer_for(&first),
                json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":"late","tokenUsage":{"total":{"inputTokens":12},"last":{"inputTokens":12}}}}),
            );
            hear(
                &mut decoder,
                &mut writer_for(&first),
                json!({"method":"turn/completed","params":{"threadId":"thread","turn":{"id":"late","status":"completed"}}}),
            );
            let retained = store
                .session_history("conversation", 0, 0)
                .unwrap()
                .into_iter()
                .filter(|event| event.provider_turn.as_deref() == Some("late"))
                .collect::<Vec<_>>();
            assert_eq!(retained.len(), 3);
            assert!(retained
                .iter()
                .all(|event| event.lf_process_id.as_deref() == Some(a.as_str())));
            assert_eq!(
                store
                    .input_history(original.input_key().unwrap())
                    .unwrap()
                    .usage
                    .total_input_tokens,
                Some(12)
            );
            assert_eq!(
                store
                    .input_history(next.input_key().unwrap())
                    .unwrap()
                    .usage
                    .total_input_tokens,
                None
            );
            assert_eq!(
                store.session_attachment("conversation").unwrap(),
                Some(current)
            );
        }
    }

    #[test]
    fn codex_resumed_or_broadcast_turns_do_not_claim_this_clients_origin() {
        let (_home, store, attachment) = attached(false);
        for (turn, reply_first) in [("notification-first", false), ("reply-first", true)] {
            let mut writer = writer(&store, &attachment);
            let mut decoder = CodexDecoder::default();
            send(
                &mut decoder,
                &mut writer,
                json!({"id":1,"method":"turn/start"}),
            );
            let reply = json!({"id":1,"result":{"turn":{"id":turn}}});
            let started =
                json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":turn}}});
            let messages = if reply_first {
                [reply, started]
            } else {
                [started, reply]
            };
            for message in messages {
                hear(&mut decoder, &mut writer, message);
            }
            let before = store.session_history("conversation", 0, 0).unwrap();
            assert_eq!(
                before.last().unwrap().lf_process_id,
                attachment.lf_process_id.as_ref().map(ToString::to_string)
            );
            // A reconnect discovers the existing turn; another input receives
            // that same turn rather than a fresh native start notification.
            let mut current = CodexDecoder::default();
            hear(
                &mut current,
                &mut writer,
                json!({"result":{"thread":{"id":"thread","turns":[{"id":turn,"status":"inProgress"}]}}}),
            );
            send(
                &mut current,
                &mut writer,
                json!({"id":2,"method":"turn/start"}),
            );
            hear(
                &mut current,
                &mut writer,
                json!({"id":2,"result":{"turn":{"id":turn}}}),
            );
            assert_eq!(store.session_history("conversation", 0, 0).unwrap(), before);
        }
        let mut writer = writer(&store, &attachment);
        let mut current = CodexDecoder::default();
        // Even a broadcast before this client's request does not establish
        // that this client started the turn.
        hear(
            &mut current,
            &mut writer,
            json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"unknown"}}}),
        );
        send(
            &mut current,
            &mut writer,
            json!({"id":3,"method":"turn/start"}),
        );
        hear(
            &mut current,
            &mut writer,
            json!({"id":3,"result":{"turn":{"id":"unknown"}}}),
        );
        let events = store.session_history("conversation", 0, 0).unwrap();
        let unknown = events
            .iter()
            .find(|event| event.provider_turn.as_deref() == Some("unknown"))
            .unwrap();
        assert_eq!(unknown.kind, SessionEventKind::Started);
        assert_eq!(unknown.lf_process_id, None);
        assert_eq!(unknown.agent_process_id, None);
    }

    #[test]
    fn opencode_steers_share_execution_but_keep_the_initiating_capture_after_handoff() {
        let (home, store, attachment) = attached(false);
        let input = store
            .session("conversation")
            .unwrap()
            .unwrap()
            .input_key()
            .unwrap()
            .clone();
        let sql = rusqlite::Connection::open(home.path().join("facts.db")).unwrap();
        let mut writer = writer(&store, &attachment);
        let mut decoder = OpenCodeDecoder::new("native".into(), None);
        let request = decoder.request(false);
        writer.outbound(&request).unwrap();
        let steer = decoder.request(true);
        writer.outbound(&steer).unwrap();
        let previous = store.session("conversation").unwrap().unwrap();
        let mut next = previous.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        store
            .replace_session_input(previous.captured, next.clone())
            .unwrap();
        let replacement = LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [&replacement],
        )
        .unwrap();
        store
            .claim_session_attachment("conversation", Some(&attachment), &replacement, true)
            .unwrap();
        let messages = vec![
            json!({"type":"user","id":request}),
            json!({"type":"assistant","id":"step1","time":{"completed":2},"content":[{"type":"tool","id":"tool","name":"shell","state":{"status":"completed"}}],"tokens":{"input":20,"output":5}}),
            json!({"type":"user","id":steer}),
            json!({"type":"assistant","id":"step2","time":{"completed":4},"content":[{"type":"text","text":"Result"}],"tokens":{"input":10,"output":5}}),
            json!({"type":"idle","id":"idle","outcome":"succeeded","time":{"created":5}}),
        ];
        for _ in 0..2 {
            writer.record(&decoder.messages(&messages).decoded).unwrap();
        }
        let completion: String = sql
            .query_row(
                "SELECT payload FROM session_events WHERE kind='completed' AND provider_turn=?1",
                [&request],
                |row| row.get(0),
            )
            .unwrap();
        let completion: Value = serde_json::from_str(&completion).unwrap();
        assert_eq!(completion["status"], "completed");
        assert!(completion["error"].is_null());
        let usage = store.input_history(input.as_str()).unwrap().usage;
        assert_eq!(usage.input_tokens, Some(30));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(
            store
                .input_history(next.input_key().unwrap().as_str())
                .unwrap()
                .usage
                .input_tokens,
            None
        );
    }
}
