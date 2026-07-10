//! The served mind's thread, rendered live — the read half of `lf wavechat`.
//!
//! The thread is the product surface: journaled, durable, replayed. It stays
//! SSE on the listener (`GET /events`), because a thread has a past and a
//! socket that replays it is the right shape. The agent bus is the other wire
//! entirely — a table, polled ([`super::sub`]).
//!
//! Targeting and endpoint resolution are `lf chat`'s
//! ([`super::chat::resolve_target`]): the ambient rule (`LFD_CHANNEL` env,
//! else `LFD_WAVE_ID`, else the worktree name) with an explicit NAME override.
//!
//! The stream is followed until the process is killed: on disconnect (or a
//! wave with no live server yet) it reconnects on a backoff ladder,
//! re-resolving the endpoint each attempt — server restarts change ports.
//!
//! Output renders from the WIRE frames (never journal internals): human lines
//! by default (chat bylines, turn open/items/close, state transitions, memory
//! curation/adds), or raw frames as NDJSON with `--json`.

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

/// Follow the thread until the process ends. `lf wavechat` runs this as a task
/// so one terminal both monitors and steers.
pub(crate) async fn follow(wave: Option<&str>, json: bool) -> Result<()> {
    let target = WaveTargetArgs {
        wave: wave.map(str::to_string),
        parent: false,
    };
    let mut renderer = Renderer::new(json);
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
                "wave '{}' has no live server; waiting (start one with `lf serve {}`)",
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

/// Renders wire frames as output lines. Stdout is written here; the pure
/// half (`lines_for`) is what tests pin.
#[derive(Debug)]
struct Renderer {
    json: bool,
    turns: HashMap<String, TurnProgress>,
}

impl Renderer {
    fn new(json: bool) -> Self {
        Self {
            json,
            turns: HashMap::new(),
        }
    }

    fn render(&mut self, frame: Frame) {
        for line in self.lines_for(&frame) {
            println!("{line}");
        }
    }

    /// The output lines for one frame — NDJSON raw, or the human console
    /// flavor (chat bylines, turn open/items/close, state, memory
    /// curation/adds).
    fn lines_for(&mut self, frame: &Frame) -> Vec<String> {
        if self.json {
            let data: serde_json::Value = serde_json::from_str(&frame.data)
                .unwrap_or(serde_json::Value::String(frame.data.clone()));
            return vec![serde_json::json!({ "event": frame.event, "data": data }).to_string()];
        }
        match frame.event.as_str() {
            "state" => vec![format!("state {}", frame.data)],
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
                lines.push(format!(
                    "chat ← {byline}\"{}\" ({})",
                    ellipsize(&turn.text, 60),
                    turn.id
                ));
            }
            return lines;
        }

        if !progress.opened {
            progress.opened = true;
            lines.push(format!("turn {} opened", turn.id));
        }
        // New prose since the last frame of this id, one line per fragment.
        if turn.text.chars().count() > progress.text_chars {
            let fresh: String = turn.text.chars().skip(progress.text_chars).collect();
            progress.text_chars = turn.text.chars().count();
            for fragment in fresh.split('\n').filter(|f| !f.trim().is_empty()) {
                lines.push(format!("  loop: \"{}\"", ellipsize(fragment, 100)));
            }
        }
        for item in turn.items.iter().skip(progress.items) {
            if let Some(line) = item_line(item) {
                lines.push(line);
            }
        }
        progress.items = turn.items.len();

        if turn.status != Lifecycle::Running && turn.status != Lifecycle::Pending {
            progress.finished = true;
            let items = turn.items.len();
            let plural = if items == 1 { "" } else { "s" };
            lines.push(format!(
                "turn {} {} · {items} item{plural}",
                turn.id,
                turn.status.name()
            ));
        }
        lines
    }
}

/// One console line per item, Narrator flavor. Thoughts stay off the default
/// feed (they ride the journal's DEBUG level for the same reason).
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

    #[test]
    fn json_mode_emits_raw_frames_as_ndjson() {
        let mut renderer = Renderer::new(true);
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
        let mut renderer = Renderer::new(false);
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
                data: "workers report via lf radio with full detail".into()
            }),
            vec!["memory added: workers report via lf radio with full detail"]
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
        let mut renderer = Renderer::new(false);
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
            server::SubagentDoor::new(),
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
            .append_memory("workers report via lf radio with full useful detail")
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
                    && f.data == "workers report via lf radio with full useful detail"
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
