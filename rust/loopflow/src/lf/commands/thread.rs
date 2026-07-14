//! The served mind's thread, rendered live — the read half of `lf chat --follow`.
//!
//! The thread is the product surface: journaled, durable, replayed. It stays
//! SSE on the listener (`GET /events`), because a thread has a past and a
//! socket that replays it is the right shape. The agent bus is the other wire
//! entirely — a table, polled ([`super::sub`]).
//!
//! Targeting and endpoint resolution are `lf chat`'s
//! ([`super::chat::resolve_target`]): the ambient rule (`LF_CHANNEL` env,
//! else `LF_WAVE_ID`, else the worktree name) with an explicit NAME override.
//!
//! The stream is followed until the process is killed: on disconnect (or a
//! wave with no live server yet) it reconnects on a backoff ladder,
//! re-resolving the endpoint each attempt — server restarts change ports.
//!
//! Output renders from the WIRE frames (never journal internals), in one of
//! three modes:
//!
//! - **conversation** (default): what the wave said, what a human must act on.
//!   Prose, actionable failures, and one consolidated line per turn for the
//!   evidence behind it (`· 4 commands, 2 files`). Tool calls, shell commands,
//!   file edits, thoughts, and turn open/close bookkeeping stay off the feed.
//! - **audit** (`--audit`): the full execution log — every item, every turn
//!   boundary. This is what the default used to be.
//! - **json** (`--json`): the raw frames as NDJSON, unchanged.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::lf::commands::chat::{resolve_target, CliContext};
use crate::lf::WaveTargetArgs;
use crate::wave::journal::ellipsize;
use crate::wave::subscription::{stream_events, Frame};

/// Reconnect backoff: floor doubling to the ceiling, reset once a connect
/// actually streams a frame (the server opens every subscription with a
/// replay, so any real connect delivers one). A dial that is merely accepted
/// and then dropped keeps climbing the ladder — a flapping server never gets
/// hammered at the floor rate.
const BACKOFF_FLOOR: Duration = Duration::from_secs(1);
const BACKOFF_CEIL: Duration = Duration::from_secs(30);

/// How the followed thread reaches the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadView {
    /// The conversation: prose, failures, one consolidated evidence line.
    Conversation,
    /// The execution log: every item and turn boundary.
    Audit,
    /// Raw wire frames, NDJSON.
    Json,
}

/// Follow the thread until the process ends. `lf chat --follow` runs this as a
/// task so one terminal both monitors and steers.
pub(crate) async fn follow(wave: Option<&str>, view: ThreadView) -> Result<()> {
    let target = WaveTargetArgs {
        wave: wave.map(str::to_string),
        parent: false,
    };
    let mut renderer = Renderer::new(view);
    let mut backoff = BACKOFF_FLOOR;
    let mut waiting_note_shown = false;
    loop {
        // Re-resolve from scratch every attempt: a restarted server has a new
        // port, and its registry row / pointer file are the source of truth.
        let context = CliContext::detect().await;
        let resolved = resolve_target(
            &target,
            context.store.as_ref(),
            context.repo.as_deref(),
            context.env_wave_id.as_deref(),
            context.env_channel.as_deref(),
        )
        .await?;
        let Some(resolved) = resolved else {
            eprintln!("no wave here; nothing to follow");
            return Ok(());
        };
        if let Some(endpoint) = &resolved.endpoint {
            let mut saw_frame = false;
            let result = stream_events(endpoint, "", &mut |frame| {
                saw_frame = true;
                renderer.render(frame);
            })
            .await;
            if let Err(err) = result {
                tracing::debug!(error = %err, wave = resolved.name, "event stream dropped");
            }
            if saw_frame {
                // The connect really streamed: back to the floor, however
                // the stream ended (graceful close or mid-stream drop).
                backoff = BACKOFF_FLOOR;
            }
            waiting_note_shown = false;
        } else if !waiting_note_shown {
            eprintln!(
                "wave '{}' has no live listener; waiting (start one with `lf wave {}`)",
                resolved.name, resolved.name
            );
            waiting_note_shown = true;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(BACKOFF_CEIL);
    }
}

/// Per-turn render progress, so growing same-id frames print only what's new
/// and reconnect replays stay quiet for turns already shown.
#[derive(Debug, Default)]
struct TurnProgress {
    opened: bool,
    text_chars: usize,
    items: usize,
    finished: bool,
}

/// The evidence a turn produced, coalesced to counts — the CLI half of the
/// Mac's activity row. Rendered once, when the turn closes.
#[derive(Debug, Default)]
struct Evidence {
    commands: usize,
    files: usize,
    tools: usize,
}

impl Evidence {
    fn tally(items: &[ConversationItem]) -> Self {
        let mut evidence = Self::default();
        for item in items {
            match item {
                ConversationItem::Command { .. } => evidence.commands += 1,
                ConversationItem::File { .. } => evidence.files += 1,
                ConversationItem::Tool { .. } => evidence.tools += 1,
                ConversationItem::Message { .. } | ConversationItem::Thought { .. } => {}
            }
        }
        evidence
    }

    fn total(&self) -> usize {
        self.commands + self.files + self.tools
    }

    fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.commands > 0 {
            parts.push(plural(self.commands, "command"));
        }
        if self.files > 0 {
            parts.push(format!("{} edited", plural(self.files, "file")));
        }
        if self.tools > 0 {
            parts.push(plural(self.tools, "tool call"));
        }
        parts.join(", ")
    }
}

fn plural(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// Renders wire frames as output lines. Stdout is written here; the pure
/// half (`lines_for`) is what tests pin.
#[derive(Debug)]
struct Renderer {
    view: ThreadView,
    turns: HashMap<String, TurnProgress>,
}

impl Renderer {
    fn new(view: ThreadView) -> Self {
        Self {
            view,
            turns: HashMap::new(),
        }
    }

    fn render(&mut self, frame: Frame) {
        for line in self.lines_for(&frame) {
            println!("{line}");
        }
    }

    /// The output lines for one frame — NDJSON raw, the execution log, or the
    /// conversation.
    fn lines_for(&mut self, frame: &Frame) -> Vec<String> {
        if self.view == ThreadView::Json {
            let data: serde_json::Value = serde_json::from_str(&frame.data)
                .unwrap_or(serde_json::Value::String(frame.data.clone()));
            return vec![serde_json::json!({ "event": frame.event, "data": data }).to_string()];
        }
        match frame.event.as_str() {
            // A state transition is loop bookkeeping; the conversation shows
            // motion through the turns themselves.
            "state" if self.view == ThreadView::Audit => vec![format!("state {}", frame.data)],
            "state" => Vec::new(),
            "memory" => vec![format!("memory curated: {}", ellipsize(&frame.data, 70))],
            "memory-add" => vec![format!("memory added: {}", frame.data)],
            "turn" => {
                let Ok(turn) = serde_json::from_str::<ChatTurn>(&frame.data) else {
                    return vec![format!(
                        "(unparseable turn frame: {})",
                        ellipsize(&frame.data, 80)
                    )];
                };
                self.turn_lines(&turn)
            }
            _ => Vec::new(),
        }
    }

    fn turn_lines(&mut self, turn: &ChatTurn) -> Vec<String> {
        let audit = self.view == ThreadView::Audit;
        let progress = self.turns.entry(turn.id.clone()).or_default();
        if progress.finished {
            // Reconnect replay of a turn already rendered whole: quiet.
            return Vec::new();
        }
        let mut lines = Vec::new();

        if turn.role == ChatRole::User {
            if !progress.opened {
                progress.opened = true;
                progress.finished = true;
                let byline = turn
                    .from
                    .as_ref()
                    .map(|from| format!("[{from}] "))
                    .unwrap_or_default();
                if audit {
                    lines.push(format!(
                        "chat ← {byline}\"{}\" ({})",
                        ellipsize(&turn.text, 60),
                        turn.id
                    ));
                } else {
                    // The speaker, not the turn id: the thread exposes no
                    // runtime identifiers.
                    let who = turn.from.as_deref().unwrap_or("you");
                    lines.push(format!("{who} › {}", turn.text));
                }
            }
            return lines;
        }

        if audit && !progress.opened {
            lines.push(format!("turn {} opened", turn.id));
        }
        progress.opened = true;

        // New prose since the last frame of this id, one line per fragment.
        // The wave's speech is the content: it prints whole, never elided.
        if turn.text.chars().count() > progress.text_chars {
            let fresh: String = turn.text.chars().skip(progress.text_chars).collect();
            progress.text_chars = turn.text.chars().count();
            for fragment in fresh.split('\n').filter(|f| !f.trim().is_empty()) {
                if audit {
                    lines.push(format!("  loop: \"{}\"", ellipsize(fragment, 100)));
                } else {
                    lines.push(format!("wave › {fragment}"));
                }
            }
        }

        for item in turn.items.iter().skip(progress.items) {
            if audit {
                if let Some(line) = item_line(item) {
                    lines.push(line);
                }
            } else if let Some(line) = failure_line(item) {
                // Actionable errors are conversation, not evidence: they
                // surface as they happen, never folded into the count.
                lines.push(line);
            }
        }
        progress.items = turn.items.len();

        if turn.status != Lifecycle::Running && turn.status != Lifecycle::Pending {
            progress.finished = true;
            if audit {
                let items = turn.items.len();
                let plural = if items == 1 { "" } else { "s" };
                lines.push(format!(
                    "turn {} {} · {items} item{plural}",
                    turn.id,
                    turn.status.name()
                ));
            } else {
                let evidence = Evidence::tally(&turn.items);
                if turn.status == Lifecycle::Failed || turn.status == Lifecycle::Interrupted {
                    let detail = turn.status.name().to_string();
                    lines.push(match evidence.total() {
                        0 => format!("  · turn {detail}"),
                        _ => format!("  · turn {detail} after {}", evidence.summary()),
                    });
                } else if evidence.total() > 0 {
                    lines.push(format!("  · {}", evidence.summary()));
                }
            }
        }
        lines
    }
}

/// One console line per item, Narrator flavor. Thoughts stay off the audit
/// feed too (they ride the journal's DEBUG level for the same reason).
fn item_line(item: &ConversationItem) -> Option<String> {
    match item {
        ConversationItem::Command {
            command, status, ..
        } => Some(format!(
            "  $ {} → {}",
            ellipsize(&command.join(" "), 70),
            status.name()
        )),
        ConversationItem::Tool { name, status, .. } => {
            Some(format!("  tool {name} → {}", status.name()))
        }
        ConversationItem::File {
            changes, status, ..
        } => {
            let what = match changes.as_slice() {
                [only] => only.path.clone(),
                many => format!("{} files", many.len()),
            };
            Some(format!("  edit {what} → {}", status.name()))
        }
        // Prose fragments already rode in as turn text; thoughts are debug.
        ConversationItem::Message { .. } | ConversationItem::Thought { .. } => None,
    }
}

/// The line for an item a human has to act on. A nonzero exit is a failure
/// even when the harness marked the command `completed` — the command ran,
/// and it said no.
fn failure_line(item: &ConversationItem) -> Option<String> {
    match item {
        ConversationItem::Command {
            command,
            status,
            exit_code,
            ..
        } => {
            let failed =
                matches!(exit_code, Some(code) if *code != 0) || *status == Lifecycle::Failed;
            failed.then(|| {
                let detail = match exit_code {
                    Some(code) if *code != 0 => format!("exit {code}"),
                    _ => status.name().to_string(),
                };
                format!("  ✗ $ {} → {detail}", ellipsize(&command.join(" "), 70))
            })
        }
        ConversationItem::Tool { name, status, .. } => {
            (*status == Lifecycle::Failed).then(|| format!("  ✗ tool {name} → failed"))
        }
        ConversationItem::File {
            changes, status, ..
        } => (*status == Lifecycle::Failed).then(|| {
            let what = match changes.as_slice() {
                [only] => only.path.clone(),
                many => format!("{} files", many.len()),
            };
            format!("  ✗ edit {what} → failed")
        }),
        ConversationItem::Message { .. } | ConversationItem::Thought { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(raw: &str) -> Vec<Frame> {
        use crate::wave::subscription::SseFrameParser;

        let mut parser = SseFrameParser::default();
        let mut out = Vec::new();
        for byte in raw.bytes() {
            if let Some(frame) = parser.push(byte) {
                out.push(frame);
            }
        }
        out
    }

    fn turn_json(id: &str, role: &str, text: &str, status: &str, items: &str) -> String {
        format!(
            "{{\"id\":\"{id}\",\"role\":\"{role}\",\"text\":\"{text}\",\"status\":\"{status}\",\
             \"items\":{items},\"created_at\":\"2026-07-04T00:00:00Z\",\"from\":null}}"
        )
    }

    #[test]
    fn sse_parser_splits_frames_on_blank_lines() {
        let out = frames("event: state\ndata: idle\n\nevent: turn\ndata: {\"id\":1}\n\n");
        assert_eq!(
            out,
            vec![
                Frame {
                    event: "state".into(),
                    data: "idle".into()
                },
                Frame {
                    event: "turn".into(),
                    data: "{\"id\":1}".into()
                },
            ]
        );
    }

    #[test]
    fn sse_parser_handles_crlf_comments_and_multiline_data() {
        let out = frames(": ping\r\n\r\nevent: memory-add\r\ndata: first\r\ndata: second\r\n\r\n");
        assert_eq!(
            out,
            vec![Frame {
                event: "memory-add".into(),
                data: "first\nsecond".into()
            }]
        );
        // An unterminated frame stays pending.
        assert!(frames("event: turn\ndata: {\"id\":1}\n").is_empty());
    }

    /// The failure case this task exists to fix, in one transcript: a
    /// long-running `task` flow that clarifies, builds, hits a red test,
    /// recovers, and reports. The default view must read as what the wave SAID
    /// and what needs a human — the twenty-odd tool calls and shell commands
    /// underneath it are evidence, and evidence gets one line.
    #[test]
    fn a_long_running_task_reads_as_conversation_not_a_build_log() {
        let cmd = |id: &str, argv: &str, exit: i64| {
            format!(
                "{{\"type\":\"command\",\"id\":\"{id}\",\"command\":[\"sh\",\"-c\",\"{argv}\"],\
                 \"cwd\":\"/repo\",\"status\":\"completed\",\"output\":\"…\",\"exit_code\":{exit},\
                 \"duration_ms\":900}}"
            )
        };
        let tool = |id: &str, name: &str| {
            format!(
                "{{\"type\":\"tool\",\"id\":\"{id}\",\"name\":\"{name}\",\"status\":\"completed\",\
                 \"input\":null,\"output\":\"ok\"}}"
            )
        };
        let edit = |id: &str, path: &str| {
            format!(
                "{{\"type\":\"file\",\"id\":\"{id}\",\"changes\":[{{\"path\":\"{path}\",\
                 \"kind\":\"modified\",\"diff\":null}}],\"status\":\"completed\"}}"
            )
        };
        let think = |id: &str| {
            format!("{{\"type\":\"thought\",\"id\":\"{id}\",\"text\":\"weighing the options\"}}")
        };

        let mut renderer = Renderer::new(ThreadView::Conversation);
        let mut out = Vec::new();
        let mut feed = |renderer: &mut Renderer, event: &str, data: String| {
            out.extend(renderer.lines_for(&Frame {
                event: event.into(),
                data,
            }));
        };

        // The human asks for the work.
        feed(
            &mut renderer,
            "turn",
            turn_json(
                "turn-1",
                "user",
                "make wave chat human-first",
                "completed",
                "[]",
            ),
        );
        feed(&mut renderer, "state", "turning".into());

        // task_clarify: reads the repo, writes a design note, says what it found.
        let clarify_items = format!(
            "[{},{},{},{},{}]",
            think("h0"),
            tool("t0", "Read"),
            tool("t1", "Grep"),
            cmd("c0", "git log --oneline -3", 0),
            edit("f0", "scratch/w2-129.md")
        );
        feed(
            &mut renderer,
            "turn",
            turn_json("turn-2", "assistant", "", "running", "[]"),
        );
        feed(
            &mut renderer,
            "turn",
            turn_json(
                "turn-2",
                "assistant",
                "The transcript renders every tool call as a card. I'll collapse them behind an audit toggle.",
                "completed",
                &clarify_items,
            ),
        );

        // task_pursue: builds, and the test run goes red.
        let pursue_items = format!(
            "[{},{},{},{},{},{}]",
            edit("f1", "swift/Loopflow/Models/WaveChatTranscript.swift"),
            edit("f2", "swift/LoopflowMac/Views/MessageRow.swift"),
            cmd("c1", "cargo build", 0),
            cmd("c2", "swift test", 1),
            tool("t2", "Edit"),
            cmd("c3", "swift test", 0)
        );
        feed(
            &mut renderer,
            "turn",
            turn_json(
                "turn-3",
                "assistant",
                "Projection landed. One test caught a stale signature; fixed and green.",
                "completed",
                &pursue_items,
            ),
        );

        assert_eq!(
            out,
            vec![
                "you › make wave chat human-first",
                "wave › The transcript renders every tool call as a card. I'll collapse them behind an audit toggle.",
                "  · 1 command, 1 file edited, 2 tool calls",
                "wave › Projection landed. One test caught a stale signature; fixed and green.",
                // The red test surfaces as it happens — an actionable error is
                // conversation, never folded into the count below it.
                "  ✗ $ sh -c swift test → exit 1",
                "  · 3 commands, 2 files edited, 1 tool call",
            ],
            "the conversation is prose, the failure, and one evidence line per turn"
        );

        // Audit over the same turns is the execution log, in full.
        let mut audit = Renderer::new(ThreadView::Audit);
        let audit_lines = audit.lines_for(&Frame {
            event: "turn".into(),
            data: turn_json(
                "turn-3",
                "assistant",
                "Projection landed. One test caught a stale signature; fixed and green.",
                "completed",
                &pursue_items,
            ),
        });
        assert!(audit_lines.contains(&"turn turn-3 opened".to_string()));
        assert!(audit_lines.contains(&"  $ sh -c cargo build → completed".to_string()));
        assert!(audit_lines.contains(&"  tool Edit → completed".to_string()));
        assert!(audit_lines
            .contains(&"  edit swift/LoopflowMac/Views/MessageRow.swift → completed".to_string()));
        assert!(audit_lines.contains(&"turn turn-3 completed · 6 items".to_string()));
    }

    #[test]
    fn json_mode_emits_raw_frames_as_ndjson() {
        let mut renderer = Renderer::new(ThreadView::Json);
        let lines = renderer.lines_for(&Frame {
            event: "state".into(),
            data: "turning".into(),
        });
        assert_eq!(lines, vec!["{\"data\":\"turning\",\"event\":\"state\"}"]);

        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: turn_json("turn-3", "assistant", "hi", "completed", "[]"),
        });
        let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid NDJSON");
        assert_eq!(parsed["event"], "turn");
        assert_eq!(parsed["data"]["id"], "turn-3", "turn data stays JSON");
    }

    #[test]
    fn human_mode_renders_chat_state_and_memory_lines() {
        let mut renderer = Renderer::new(ThreadView::Audit);
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "state".into(),
                data: "turning".into()
            }),
            vec!["state turning"]
        );
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "memory-add".into(),
                data: "workers report via lf radio pub with full detail".into()
            }),
            vec!["memory added: workers report via lf radio pub with full detail"]
        );
        let user = turn_json("turn-1", "user", "how goes it?", "completed", "[]");
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn".into(),
                data: user.clone()
            }),
            vec!["chat ← \"how goes it?\" (turn-1)"]
        );
        // Replay of the same user turn (reconnect) prints nothing.
        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: user
            })
            .is_empty());
    }

    #[test]
    fn human_mode_prints_only_the_growth_of_a_repeating_turn_id() {
        let mut renderer = Renderer::new(ThreadView::Audit);
        let running =
            |text: &str, items: &str| turn_json("turn-2", "assistant", text, "running", items);
        let tool = "[{\"type\":\"tool\",\"id\":\"t0\",\"name\":\"Bash\",\"status\":\"completed\",\
              \"input\":null,\"output\":null}]";

        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("", "[]"),
        });
        assert_eq!(lines, vec!["turn turn-2 opened"]);

        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("thinking", "[]"),
        });
        assert_eq!(lines, vec!["  loop: \"thinking\""]);

        // Same text re-sent with a new item: only the item prints.
        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("thinking", tool),
        });
        assert_eq!(lines, vec!["  tool Bash → completed"]);

        // The terminal frame closes the turn; replays of it are quiet.
        let terminal = turn_json("turn-2", "assistant", "thinking", "completed", tool);
        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: terminal.clone(),
        });
        assert_eq!(lines, vec!["turn turn-2 completed · 1 item"]);
        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: terminal
            })
            .is_empty());
    }

    /// End to end against a live server: subscribe, watch replay + live
    /// frames arrive as parsed SSE frames.
    #[tokio::test]
    async fn stream_events_delivers_replay_then_live_frames() {
        use crate::wave::runtime::WaveRuntime;
        use crate::wave::server;
        use std::sync::{Arc, Mutex};

        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            server::ResidentDoor::new("test-token"),
            None,
            None,
            server::ShutdownDoor::new(),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        runtime
            .deliver(crate::wave::journal::MessageOp::Message, "replayed".into())
            .expect("user turn");
        runtime
            .append_memory("workers report via lf radio pub with full useful detail")
            .unwrap();

        let seen: Arc<Mutex<Vec<Frame>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let endpoint = addr.to_string();
        let task = tokio::spawn(async move {
            let mut on_frame = |frame: Frame| sink.lock().unwrap().push(frame);
            let _ = stream_events(&endpoint, "", &mut on_frame).await;
        });

        // Wait for the replay to land, then publish a live fact.
        for _ in 0..200 {
            if seen.lock().unwrap().iter().any(|f| f.event == "turn") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        runtime
            .append_memory("a fact published after subscribe")
            .unwrap();
        for _ in 0..200 {
            if seen
                .lock()
                .unwrap()
                .iter()
                .any(|f| f.event == "memory-add" && f.data.contains("after subscribe"))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        task.abort();

        let frames = seen.lock().unwrap().clone();
        assert_eq!(frames[0].event, "state", "replay opens with the state");
        assert!(
            frames
                .iter()
                .any(|f| f.event == "turn" && f.data.contains("replayed")),
            "replayed turn arrives: {frames:?}"
        );
        assert!(
            frames.iter().any(|f| {
                f.event == "memory-add"
                    && f.data == "workers report via lf radio pub with full useful detail"
            }),
            "replayed memory-add frame arrives with the full fact: {frames:?}"
        );
        assert!(
            frames
                .iter()
                .any(|f| f.event == "memory-add" && f.data == "a fact published after subscribe"),
            "live memory-add frame arrives: {frames:?}"
        );
    }
}
