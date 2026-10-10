//! What a provider's own stream says about a conversation waiting on a person.
//! Each attached LfProcess that owns a stream keeps one tracker and saves its reading; the
//! Waiting rule itself is read from that row by the Session inventory.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::process::SessionAttachment;
use crate::session::SessionActivity;
use crate::store::sqlite::SqliteStore;

/// Unchanged readings are saved this often, so quiet time is measured from a
/// stream that was recently alive.
const SAVE_INTERVAL: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Signal {
    /// The provider is producing a turn.
    Working,
    ToolStarted(String),
    ToolResolved(String),
    InputRequested(String),
    InputResolved(String),
    /// The provider handed the turn back; `true` when it succeeded.
    Yielded(bool),
}

#[derive(Debug, Default)]
pub(super) struct Attention {
    open_tools: BTreeSet<String>,
    pending_input: BTreeSet<String>,
    yielded: bool,
    failed: bool,
    saved: Option<SessionActivity>,
}

impl Attention {
    /// Apply one stream message's signals; false when it carried none.
    pub(super) fn apply(&mut self, signals: Vec<Signal>) -> bool {
        let any = !signals.is_empty();
        for signal in signals {
            match signal {
                Signal::Working => {
                    self.yielded = false;
                    self.failed = false;
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
            }
        }
        any
    }

    /// The reading to save, when it changed or the saved one has aged.
    pub(super) fn reading(&mut self, now: i64) -> Option<SessionActivity> {
        let reading = SessionActivity {
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

    /// Attention is display evidence: a failed save never fails the turn.
    pub(super) fn record(
        &mut self,
        store: &SqliteStore,
        session: &str,
        attachment: &SessionAttachment,
        signals: Vec<Signal>,
    ) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.record_at(store, session, attachment, signals, now);
    }

    fn record_at(
        &mut self,
        store: &SqliteStore,
        session: &str,
        attachment: &SessionAttachment,
        signals: Vec<Signal>,
        now: i64,
    ) {
        if !self.apply(signals) {
            return;
        }
        if let Some(reading) = self.reading(now) {
            if let Err(error) = store.record_session_activity(session, attachment, &reading) {
                tracing::warn!(%error, session, "failed to save Session activity");
            }
        }
    }
}

/// One line of Claude's stream-json output.
pub(super) fn claude(line: &Value) -> Vec<Signal> {
    let event = if line["type"] == "stream_event" {
        &line["event"]
    } else {
        line
    };
    let blocks = |kind: &str, id: &str| -> Vec<String> {
        event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == kind)
            .filter_map(|block| block[id].as_str().map(str::to_owned))
            .collect()
    };
    match event["type"].as_str() {
        Some("result") => vec![Signal::Yielded(
            event["is_error"] != true
                && (event["subtype"].is_null() || event["subtype"] == "success"),
        )],
        Some("content_block_start") if event["content_block"]["type"] == "tool_use" => event
            ["content_block"]["id"]
            .as_str()
            .map(|id| Signal::ToolStarted(id.to_owned()))
            .into_iter()
            .collect(),
        Some("assistant") => {
            let tools = blocks("tool_use", "id");
            if tools.is_empty() {
                vec![Signal::Working]
            } else {
                tools.into_iter().map(Signal::ToolStarted).collect()
            }
        }
        Some("user") => {
            let results = blocks("tool_result", "tool_use_id");
            if results.is_empty() {
                vec![Signal::Working]
            } else {
                results.into_iter().map(Signal::ToolResolved).collect()
            }
        }
        Some("message_start" | "content_block_start" | "content_block_delta") => {
            vec![Signal::Working]
        }
        _ => Vec::new(),
    }
}

/// One Codex app-server message. A server request that asks a person something
/// stays pending until the client answers it.
pub(super) fn codex(rpc: &Value, from_client: bool) -> Vec<Signal> {
    let method = rpc["method"].as_str();
    if from_client {
        return match (method, rpc.get("id")) {
            (None, Some(id)) => vec![Signal::InputResolved(id.to_string())],
            _ => Vec::new(),
        };
    }
    let Some(method) = method else {
        return Vec::new();
    };
    let params = &rpc["params"];
    if let Some(id) = rpc.get("id") {
        let asks = method.ends_with("requestApproval")
            || method.ends_with("requestUserInput")
            || method.contains("elicitation");
        return if asks {
            vec![Signal::InputRequested(id.to_string())]
        } else {
            Vec::new()
        };
    }
    let item = &params["item"];
    let conversational = matches!(
        item["type"].as_str(),
        Some("userMessage" | "agentMessage" | "reasoning" | "plan")
    );
    match method {
        "turn/started" => vec![Signal::Working],
        "turn/completed" => vec![Signal::Yielded(params["turn"]["status"] == "completed")],
        "serverRequest/resolved" => vec![Signal::InputResolved(params["requestId"].to_string())],
        "item/started" if conversational => vec![Signal::Working],
        "item/started" | "item/completed" => match item["id"].as_str() {
            Some(id) if method == "item/started" => vec![Signal::ToolStarted(id.to_owned())],
            Some(id) => vec![Signal::ToolResolved(id.to_owned())],
            None => Vec::new(),
        },
        _ if method.starts_with("item/") && method.to_ascii_lowercase().ends_with("delta") => {
            vec![Signal::Working]
        }
        _ => Vec::new(),
    }
}

/// One OpenCode server event for conversation `thread`; the server reports
/// every conversation on one stream. A permission request is answered by its
/// attached LfProcess and is not a question for a person.
pub(super) fn opencode(event: &Value, thread: &str) -> Vec<Signal> {
    let properties = &event["properties"];
    let id = |value: &Value| value.as_str().map(str::to_owned);
    let owner = properties["sessionID"]
        .as_str()
        .or(properties["part"]["sessionID"].as_str());
    if owner != Some(thread) {
        return Vec::new();
    }
    match event["type"].as_str() {
        Some("message.part.updated") => {
            let part = &properties["part"];
            if part["type"] != "tool" {
                return vec![Signal::Working];
            }
            let Some(call) = id(&part["callID"]).or_else(|| id(&part["id"])) else {
                return Vec::new();
            };
            match part["state"]["status"].as_str() {
                Some("pending" | "running") => vec![Signal::ToolStarted(call)],
                Some("completed" | "error") => vec![Signal::ToolResolved(call)],
                _ => Vec::new(),
            }
        }
        Some("message.part.delta") => vec![Signal::Working],
        Some("question.asked") => id(&properties["id"])
            .map(Signal::InputRequested)
            .into_iter()
            .collect(),
        Some("question.replied" | "question.rejected") => id(&properties["requestID"])
            .map(Signal::InputResolved)
            .into_iter()
            .collect(),
        Some("session.error") => vec![Signal::Yielded(false)],
        Some("session.idle") => vec![Signal::Yielded(true)],
        Some("session.status") => match properties["status"]["type"].as_str() {
            Some("idle") => vec![Signal::Yielded(true)],
            Some("busy") => vec![Signal::Working],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{claude, codex, opencode, Attention, Signal};
    use crate::id::LfProcessId;
    use crate::process::SessionAttachment;
    use crate::store::sqlite::SqliteStore;

    /// A conversation whose attachment saves what a recorded stream says, one
    /// message a second from `START`.
    struct Driven {
        _home: tempfile::TempDir,
        store: SqliteStore,
        attachment: SessionAttachment,
        attention: Attention,
        now: i64,
    }

    const START: i64 = 1_000;
    const QUIET: i64 = crate::session::WAITING_QUIET_SECONDS;

    #[test]
    fn automated_auth_and_permission_requests_do_not_wait_for_input() {
        assert!(codex(
            &json!({"id": 7, "method": "account/chatgptAuthTokens/refresh", "params": {}}),
            false
        )
        .is_empty());
        assert!(opencode(
            &json!({"type": "permission.asked", "properties": {"sessionID": "thread", "id": "permission"}}),
            "thread"
        )
        .is_empty());
    }

    impl Driven {
        fn new(interactive: bool) -> Self {
            let home = tempfile::tempdir().unwrap();
            let path = home.path().join("attention.db");
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
            Self {
                _home: home,
                store,
                attachment,
                attention: Attention::default(),
                now: START,
            }
        }

        fn hear(&mut self, signals: Vec<Signal>) {
            self.now += 1;
            self.attention.record_at(
                &self.store,
                "conversation",
                &self.attachment,
                signals,
                self.now,
            );
        }

        /// Whether the inventory calls it Waiting `quiet` seconds after the
        /// last message.
        fn waiting_after(&self, quiet: i64) -> bool {
            self.store
                .session_summary("conversation", self.now + quiet)
                .unwrap()
                .unwrap()
                .waiting
        }
    }

    fn trace(file: &str) -> Vec<Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/harness/testdata")
            .join(file);
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn a_claude_turn_waits_once_its_tools_are_answered_and_it_hands_back() {
        let lines = trace("claude_multi_tool.ndjson");
        let mut interactive = Driven::new(true);
        assert!(!interactive.waiting_after(QUIET), "nothing heard yet");
        for line in &lines[..lines.len() - 1] {
            interactive.hear(claude(line));
            let tool_open = line["type"] == "content_block_start";
            assert!(!interactive.waiting_after(0));
            assert_eq!(interactive.waiting_after(QUIET), !tool_open);
            assert!(!interactive.waiting_after(QUIET - 1));
        }
        interactive.hear(claude(lines.last().unwrap()));
        assert!(
            interactive.waiting_after(0),
            "a successful result hands back"
        );
        interactive.hear(claude(&json!({"type":"user","message":{"content":"next"}})));
        assert!(!interactive.waiting_after(0), "new activity clears it");

        let mut headless = Driven::new(false);
        for line in &lines {
            headless.hear(claude(line));
        }
        assert!(
            !headless.waiting_after(0),
            "a Run's finished turn asks nothing"
        );
        assert!(headless.waiting_after(QUIET));
    }

    #[test]
    fn a_claude_tool_left_open_never_reads_as_waiting() {
        let mut session = Driven::new(true);
        for line in trace("claude_crash_mid_tool.ndjson") {
            session.hear(claude(&line));
        }
        assert!(!session.waiting_after(QUIET * 100));
    }

    #[test]
    fn a_failed_claude_result_waits_only_after_quiet() {
        let mut session = Driven::new(true);
        session.hear(claude(
            &json!({"type":"result","subtype":"error_during_execution","is_error":true}),
        ));
        assert!(!session.waiting_after(QUIET - 1));
        assert!(session.waiting_after(QUIET));
    }

    #[test]
    fn a_codex_turn_waits_when_it_completes_or_asks() {
        let mut session = Driven::new(true);
        for rpc in trace("codex_normal_turn.jsonl") {
            let completed = rpc["method"] == "turn/completed";
            session.hear(codex(&rpc, false));
            assert_eq!(session.waiting_after(0), completed, "{}", rpc["method"]);
        }
        let command = json!({"type":"commandExecution","id":"call_1"});
        session.hear(codex(&json!({"method":"turn/started","params":{}}), false));
        session.hear(codex(
            &json!({"method":"item/started","params":{"item":command}}),
            false,
        ));
        assert!(!session.waiting_after(QUIET * 100), "a command is running");
        session.hear(codex(
            &json!({"id":7,"method":"item/commandExecution/requestApproval","params":{"itemId":"call_1"}}),
            false,
        ));
        assert!(session.waiting_after(0), "an approval is a question");
        session
            .attention
            .apply(codex(&json!({"id":7,"result":{"decision":"accept"}}), true));
        session.hear(codex(
            &json!({"method":"item/commandExecution/outputDelta","params":{}}),
            false,
        ));
        assert!(
            !session.waiting_after(QUIET * 100),
            "answered, still running"
        );
        session.hear(codex(
            &json!({"method":"item/completed","params":{"item":command}}),
            false,
        ));
        assert!(!session.waiting_after(QUIET - 1));
        assert!(session.waiting_after(QUIET));
        session.hear(codex(
            &json!({"method":"turn/completed","params":{"turn":{"status":"failed"}}}),
            false,
        ));
        assert!(
            !session.waiting_after(0),
            "a failed turn is not a hand-back"
        );
    }

    #[test]
    fn an_opencode_question_waits_until_it_is_answered() {
        let mut session = Driven::new(true);
        let mut seen = Vec::new();
        for event in trace("opencode_question.jsonl") {
            session.hear(opencode(&event, "ses_trace"));
            seen.push((
                event["type"].as_str().unwrap().to_owned(),
                session.waiting_after(0),
                session.waiting_after(QUIET),
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
        assert_eq!(at("question.asked"), (true, true));
        assert_eq!(at("question.replied"), (false, false), "its tool is open");
        assert_eq!(at("session.idle"), (true, true));
        // Another conversation going idle on the shared stream neither answers
        // the question nor hands this turn back.
        let other = seen.iter().position(|(kind, _, _)| kind == "session.idle");
        assert_eq!(seen[other.unwrap() + 1].0, "question.replied");
    }

    #[test]
    fn a_reading_from_an_attachment_that_let_go_says_nothing() {
        let mut session = Driven::new(true);
        session.hear(claude(&json!({"type":"result","subtype":"success"})));
        assert!(session.waiting_after(0));
        session
            .store
            .release_session_attachment("conversation", &session.attachment)
            .unwrap();
        assert!(!session.waiting_after(QUIET));
    }

    #[test]
    fn waiting_is_chosen_before_the_page_is_cut() {
        let mut session = Driven::new(true);
        // Two working conversations sort ahead of the waiting one.
        session
            .store
            .test_session("a", "run_0000000000000000000000000000000a");
        session
            .store
            .test_session("b", "run_0000000000000000000000000000000b");
        session.hear(claude(&json!({"type":"result","subtype":"success"})));
        let page = |waiting: bool| {
            let filter = crate::session::SessionFilter {
                waiting,
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
        let mut attention = Attention::default();
        assert!(attention.apply(vec![Signal::Working]));
        assert!(attention.reading(10).is_some());
        attention.apply(vec![Signal::Working]);
        assert!(attention.reading(12).is_none());
        attention.apply(vec![Signal::ToolStarted("call".into())]);
        assert_eq!(attention.reading(12).unwrap().open_tools, 1);
        attention.apply(vec![Signal::Working]);
        assert_eq!(attention.reading(17).unwrap().observed_at, 17);
    }
}
