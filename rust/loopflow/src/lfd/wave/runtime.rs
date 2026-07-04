//! The wave server's in-process state and its two reactions.
//!
//! A wave is a long-lived reactive server, not a loop. It holds one timeline
//! (`thread`) and reacts to two independent event sources:
//!
//! - **subagent progress events** — folded into [`ChatTurn`]s upstream (see
//!   [`super::subagent`]) and narrated here into the thread, with durable facts
//!   folded into [`Memory`];
//! - **user messages** — delivered over HTTP into an in-process inbox, answered
//!   TALK-ONLY from memory and current progress state.
//!
//! Neither source blocks the other: progress runs on its own supervised tasks
//! (see [`super::progress`]); chat runs on the inbox consumer below. There are
//! no files as IPC — the inbox is a channel, the thread is a `Vec`, and the
//! discovery file (see [`super::server`]) is a dumb pointer, not a transport.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};

use crate::lfd::conversations::turns::{ChatRole, ChatTurn, ChatTurnStatus};
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::supervisor::Supervisor;

/// Capacity of the live turn broadcast. SSE clients that fall this far behind
/// get a lag error and resync from `/conversation`; the thread is the source of
/// truth, so a dropped live turn is never lost.
const TURN_BROADCAST_CAPACITY: usize = 256;

/// A user message pulled from the inbox, awaiting a reply.
#[derive(Debug, Clone)]
pub struct UserMessage {
    pub text: String,
}

/// The whole live state of one running wave server.
#[derive(Debug)]
pub struct WaveRuntime {
    name: String,
    repo_root: PathBuf,
    /// The one timeline the user sees.
    thread: Mutex<Vec<ChatTurn>>,
    /// Fans finalized turns out to live SSE subscribers.
    turn_tx: broadcast::Sender<ChatTurn>,
    /// Monotonic turn id source, shared across all turn sources so ids never
    /// collide between progress narration, user messages, and replies.
    next_seq: AtomicU64,
    /// Durable shared brain.
    memory: Memory,
    /// In-process user-message inbox (a channel, not a file).
    inbox_tx: mpsc::UnboundedSender<UserMessage>,
    /// Tracks all live subagent runs.
    supervisor: Arc<Supervisor>,
}

impl WaveRuntime {
    /// Build the runtime and return it alongside the inbox receiver the chat
    /// consumer drains.
    pub fn new(
        name: String,
        repo_root: PathBuf,
    ) -> (Arc<Self>, mpsc::UnboundedReceiver<UserMessage>) {
        let (turn_tx, _) = broadcast::channel(TURN_BROADCAST_CAPACITY);
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        let memory = Memory::for_wave(&repo_root, &name);
        let runtime = Arc::new(Self {
            name,
            repo_root,
            thread: Mutex::new(Vec::new()),
            turn_tx,
            next_seq: AtomicU64::new(1),
            memory,
            inbox_tx,
            supervisor: Supervisor::new(),
        });
        (runtime, inbox_rx)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn repo_root(&self) -> &std::path::Path {
        &self.repo_root
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn supervisor(&self) -> &Arc<Supervisor> {
        &self.supervisor
    }

    /// Append a turn to the thread and broadcast it live. Assigns the global
    /// turn id so every source shares one monotonic sequence. Returns the
    /// stored turn (with its assigned id).
    pub fn append_turn(&self, mut turn: ChatTurn) -> ChatTurn {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        turn.id = format!("turn-{seq}");
        {
            let mut thread = self.thread.lock().expect("thread lock poisoned");
            thread.push(turn.clone());
        }
        // A send error just means no live subscribers — the thread still has it.
        let _ = self.turn_tx.send(turn.clone());
        turn
    }

    /// Snapshot of the whole thread, for replay-on-connect and `/conversation`.
    pub fn thread_snapshot(&self) -> Vec<ChatTurn> {
        self.thread.lock().expect("thread lock poisoned").clone()
    }

    /// Subscribe to live turns. Subscribe *before* snapshotting to avoid a gap;
    /// callers dedupe by id against the snapshot.
    pub fn subscribe(&self) -> broadcast::Receiver<ChatTurn> {
        self.turn_tx.subscribe()
    }

    /// Reaction 1 (progress): narrate a finalized subagent turn into the thread
    /// and fold a durable fact into memory. Called for every progress turn.
    pub fn narrate_progress(&self, turn: ChatTurn) -> ChatTurn {
        let stored = self.append_turn(turn);
        if !stored.text.trim().is_empty() {
            self.memory.fold_fact(&stored.text);
        }
        stored
    }

    /// Deliver a user message: append the user turn immediately, then hand the
    /// text to the inbox for the chat consumer to reply to. Returns the stored
    /// user turn so the HTTP handler can echo it back.
    pub fn deliver_user_message(&self, text: String) -> ChatTurn {
        let turn = self.append_turn(ChatTurn::user(String::new(), text.clone()));
        // Unbounded inbox: delivering a message never blocks progress.
        let _ = self.inbox_tx.send(UserMessage { text });
        turn
    }

    /// Reaction 2 (chat): compose a TALK-ONLY reply from memory + current
    /// progress state and append it. Chat observes; it does not steer progress
    /// (no memory writes, no spawning, no reprioritizing).
    pub fn reply_to(&self, message: &UserMessage) -> ChatTurn {
        let reply = compose_reply(message, &self.thread_snapshot(), &self.memory);
        self.append_turn(ChatTurn {
            id: String::new(),
            role: ChatRole::Assistant,
            text: reply,
            status: ChatTurnStatus::Completed,
            items: Vec::new(),
            created_at: now_rfc3339(),
        })
    }
}

/// Drain the inbox, replying to each user message. One reply per message,
/// TALK-ONLY. Runs until the inbox closes (server shutdown).
pub async fn run_chat_consumer(
    runtime: Arc<WaveRuntime>,
    mut inbox_rx: mpsc::UnboundedReceiver<UserMessage>,
) {
    while let Some(message) = inbox_rx.recv().await {
        runtime.reply_to(&message);
    }
}

/// Build a talk-only reply purely by observing state: what progress has
/// narrated so far, and the head of memory. Deterministic — this is where a
/// chat LLM would slot in later, but even now it must only read, never steer.
fn compose_reply(message: &UserMessage, thread: &[ChatTurn], memory: &Memory) -> String {
    let progress: Vec<&ChatTurn> = thread
        .iter()
        .filter(|t| t.role == ChatRole::Assistant && !t.text.trim().is_empty())
        .collect();
    let latest = progress
        .last()
        .map(|t| snippet(&t.text, 200))
        .unwrap_or_else(|| "no progress narrated yet".to_string());

    let mut reply = format!(
        "You asked: \"{}\".\n\nProgress so far: {} update(s). Latest: {}.",
        snippet(&message.text, 200),
        progress.len(),
        latest,
    );

    let mem = memory.head(6);
    if !mem.trim().is_empty() {
        reply.push_str("\n\nFrom memory:\n");
        reply.push_str(&mem);
    }
    reply
}

fn snippet(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let truncated: String = flat.chars().take(max).collect();
    format!("{truncated}…")
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::conversations::turns::ChatTurnStatus;

    fn progress_turn(text: &str) -> ChatTurn {
        ChatTurn {
            id: String::new(),
            role: ChatRole::Assistant,
            text: text.to_string(),
            status: ChatTurnStatus::Completed,
            items: Vec::new(),
            created_at: now_rfc3339(),
        }
    }

    #[test]
    fn append_turn_assigns_monotonic_ids() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
        let a = rt.append_turn(progress_turn("one"));
        let b = rt.append_turn(progress_turn("two"));
        assert_eq!(a.id, "turn-1");
        assert_eq!(b.id, "turn-2");
        assert_eq!(rt.thread_snapshot().len(), 2);
    }

    #[test]
    fn narrate_progress_folds_text_into_memory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
        rt.narrate_progress(progress_turn("landed the parser"));
        assert!(rt.memory().read().contains("landed the parser"));
    }

    #[test]
    fn deliver_user_message_appends_user_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, mut rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
        let turn = rt.deliver_user_message("how goes it?".into());
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.text, "how goes it?");
        // The message landed in the inbox for the consumer.
        assert_eq!(rx.try_recv().expect("inbox message").text, "how goes it?");
    }

    #[test]
    fn reply_reflects_progress_and_memory_without_steering() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
        rt.narrate_progress(progress_turn("wired the reactive server"));
        let mem_before = rt.memory().read();

        let reply = rt.reply_to(&UserMessage {
            text: "status?".into(),
        });

        assert_eq!(reply.role, ChatRole::Assistant);
        assert!(reply.text.contains("1 update"));
        assert!(reply.text.contains("wired the reactive server"));
        // Talk-only: replying didn't write memory.
        assert_eq!(rt.memory().read(), mem_before);
    }

    #[tokio::test]
    async fn chat_consumer_replies_to_each_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
        let consumer = tokio::spawn(run_chat_consumer(rt.clone(), rx));

        rt.deliver_user_message("first".into());
        rt.deliver_user_message("second".into());

        // Wait for both replies to land: 2 user turns + 2 replies = 4.
        for _ in 0..50 {
            if rt.thread_snapshot().len() >= 4 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 4, "each message gets exactly one reply");
        let users = thread.iter().filter(|t| t.role == ChatRole::User).count();
        let replies = thread
            .iter()
            .filter(|t| t.role == ChatRole::Assistant)
            .count();
        assert_eq!(users, 2, "both user messages appear as turns");
        assert_eq!(replies, 2, "each message gets exactly one reply");

        consumer.abort();
    }
}
