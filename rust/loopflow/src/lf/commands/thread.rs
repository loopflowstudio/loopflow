//! The served mind's thread, rendered live — the read half of `lf chat --follow`.
//!
//! The thread is the product surface: journaled, durable, replayed. It stays
//! SSE on the listener (`GET /events`), because a thread has a past and a
//! socket that replays it is the right shape.
//!
//! Targeting and endpoint resolution are `lf chat`'s
//! ([`super::chat::resolve_target`]): `LF_WAVE_ID` with an explicit NAME override.
//!
//! The stream is followed until the process is killed: on disconnect (or a
//! wave with no live server yet) it reconnects on a backoff ladder,
//! re-resolving the endpoint each attempt — server restarts change ports.
//!
//! Output renders from the WIRE frames (never journal internals) as the
//! conversation: what the human and Wave said. Decisions and delivery reports
//! arrive as speech; tool calls, shell commands, file edits, thoughts, states,
//! and turn boundaries stay out of chat.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::time::Duration;

use anyhow::Result;

use crate::chat::turns::{
    ChatRole, ChatTurn, ChildActivityKind, ChildActivitySubject, ChildControlActivity, TurnDelta,
};
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

/// Follow the thread until the process ends. `lf chat --follow` runs this as a
/// task so one terminal both monitors and steers.
pub(crate) async fn follow(wave: Option<&str>) -> Result<()> {
    let target = WaveTargetArgs {
        wave: wave.map(str::to_string),
        parent: false,
    };
    let mut renderer = Renderer::new();
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
                // A `resync` means the live turn stream lagged; drop the
                // connection so the reconnect below replays a fresh whole-turn
                // snapshot the reconstruction resumes from.
                let resync = frame.event == "resync";
                renderer.render(frame);
                if resync {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
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
    items_shown: usize,
    finished: bool,
}

/// Renders wire frames as output lines. Stdout is written here; the pure
/// half (`lines_for`) is what tests pin.
#[derive(Debug)]
struct Renderer {
    turns: HashMap<String, TurnProgress>,
    /// Open turns reconstructed from `turn`/`turn-delta` frames, keyed by id.
    /// A whole `turn` frame (re)baselines an entry; each `turn-delta` absorbs
    /// one increment into it through the same rule the listener folds with, so
    /// the reconstruction matches the served turn without the whole turn
    /// crossing the wire per token.
    open: HashMap<String, ChatTurn>,
}

impl Renderer {
    fn new() -> Self {
        Self {
            turns: HashMap::new(),
            open: HashMap::new(),
        }
    }

    fn render(&mut self, frame: Frame) {
        for line in self.lines_for(&frame) {
            println!("{line}");
        }
    }

    /// The conversational output lines for one wire frame.
    fn lines_for(&mut self, frame: &Frame) -> Vec<String> {
        match frame.event.as_str() {
            // A state transition is loop bookkeeping; the conversation shows
            // motion through the turns themselves.
            "state" => Vec::new(),
            "turn" => {
                let Ok(turn) = serde_json::from_str::<ChatTurn>(&frame.data) else {
                    return vec![format!(
                        "(unparseable turn frame: {})",
                        ellipsize(&frame.data, 80)
                    )];
                };
                // Re-baseline the reconstruction: the whole turn is authoritative.
                self.open.insert(turn.id.clone(), turn.clone());
                self.turn_lines(&turn)
            }
            "turn-delta" => {
                let Ok(delta) = serde_json::from_str::<TurnDelta>(&frame.data) else {
                    return vec![format!(
                        "(unparseable turn-delta frame: {})",
                        ellipsize(&frame.data, 80)
                    )];
                };
                // Grow the reconstructed open turn, then render as if a whole
                // turn arrived. No open turn for this id means we missed its
                // opening (a gap the server heals with `resync`); nothing to show.
                let Some(turn) = self.open.get_mut(&delta.turn_id) else {
                    return Vec::new();
                };
                turn.absorb_item(delta.item);
                let turn = turn.clone();
                self.turn_lines(&turn)
            }
            // The live turn stream lagged; our open-turn reconstructions may
            // have a gap. Drop them so the reconnect's whole-turn replay
            // rebuilds cleanly. Per-turn print progress stays, so nothing already
            // shown reprints.
            "resync" => {
                self.open.clear();
                Vec::new()
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

        if let Some(activity) = &turn.activity {
            if !progress.opened {
                progress.opened = true;
                progress.finished = true;
                if let Some(line) = child_activity_line(activity) {
                    lines.push(line);
                }
            }
            return lines;
        }

        if turn.role == ChatRole::User {
            if !progress.opened {
                progress.opened = true;
                progress.finished = true;
                lines.push(format!("you › {}", turn.text));
            }
            return lines;
        }

        progress.opened = true;

        // Operational narration the provider tagged `commentary` rides as a
        // discrete item, not folded into `text` (see `ChatTurn::absorb_item`),
        // so the GUI can curate it. The live follow has no disclosure to fold
        // behind — this is the raw conversation — so it prints the narration as
        // the wave speaking, exactly as it read when it lived in `text`. Every
        // other item (commands, edits, tools) stays execution evidence, unshown.
        if turn.items.len() > progress.items_shown {
            for item in &turn.items[progress.items_shown..] {
                if let ConversationItem::Message { text, phase, .. } = item {
                    if phase.as_deref() == Some("commentary") {
                        for fragment in text.split('\n').filter(|f| !f.trim().is_empty()) {
                            lines.push(format!("wave › {fragment}"));
                        }
                    }
                }
            }
            progress.items_shown = turn.items.len();
        }

        // New prose since the last frame of this id, one line per fragment.
        // The wave's speech is the content: it prints whole, never elided.
        if turn.text.chars().count() > progress.text_chars {
            let fresh: String = turn.text.chars().skip(progress.text_chars).collect();
            progress.text_chars = turn.text.chars().count();
            for fragment in fresh.split('\n').filter(|f| !f.trim().is_empty()) {
                lines.push(format!("wave › {fragment}"));
            }
        }

        if turn.status != Lifecycle::Running && turn.status != Lifecycle::Pending {
            progress.finished = true;
            if turn.status == Lifecycle::Failed {
                lines.push("wave › Turn failed.".into());
            } else if turn.status == Lifecycle::Interrupted {
                lines.push("wave › Turn interrupted.".into());
            }
        }
        lines
    }
}

fn child_activity_line(activity: &ChildControlActivity) -> Option<String> {
    match activity.kind {
        ChildActivityKind::StateChanged => return None,
        ChildActivityKind::PrOpened | ChildActivityKind::Completed | ChildActivityKind::Failed => {}
    }
    let subject = match activity.subject {
        ChildActivitySubject::Project => "project",
        ChildActivitySubject::Task => "task",
    };
    let message = if activity.summary.is_empty() {
        activity.title.clone()
    } else {
        format!("{} — {}", activity.title, activity.summary)
    };
    Some(format!("{subject} {} › {message}", activity.subject_id))
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

    fn activity_turn_json(id: &str, kind: &str, title: &str, summary: &str) -> String {
        format!(
            "{{\"id\":\"{id}\",\"role\":\"user\",\"text\":\"\",\"status\":\"completed\",\
             \"items\":[],\"created_at\":\"2026-07-04T00:00:00Z\",\"from\":\"task\",\
             \"activity\":{{\"id\":\"activity-{id}\",\"subject\":\"task\",\"subject_id\":\"W2-132\",\
             \"work_id\":\"ts_1\",\"kind\":\"{kind}\",\"title\":\"{title}\",\"summary\":\"{summary}\",\
             \"directive_version\":null,\"command_id\":null,\"effect\":null,\"source\":null,\
             \"decision_id\":null,\"options\":[]}}}}"
        )
    }

    fn turn_delta_json(turn_id: &str, item: &str) -> String {
        format!("{{\"turn_id\":\"{turn_id}\",\"item\":{item}}}")
    }

    fn stream_message_item(id: &str, text: &str) -> String {
        format!("{{\"type\":\"message\",\"id\":\"{id}\",\"text\":\"{text}\",\"phase\":\"stream\"}}")
    }

    fn commentary_message_item(id: &str, text: &str) -> String {
        format!(
            "{{\"type\":\"message\",\"id\":\"{id}\",\"text\":\"{text}\",\"phase\":\"commentary\"}}"
        )
    }

    /// The GUI curates `commentary` narration behind a disclosure; the live
    /// follow has no such surface, so it prints the narration as the wave
    /// speaking — the same reading it had before the fold kept it out of `text`.
    /// Execution items stay hidden.
    #[test]
    fn commentary_items_render_as_the_wave_speaking() {
        let mut renderer = Renderer::new();

        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: turn_json("turn-10", "assistant", "", "running", "[]"),
            })
            .is_empty());

        // A commentary increment reads as conversation, once.
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn-delta".into(),
                data: turn_delta_json(
                    "turn-10",
                    &commentary_message_item("m-0", "Auditing the plan first."),
                ),
            }),
            vec!["wave › Auditing the plan first."]
        );

        // The conclusion streams as prose after it.
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn-delta".into(),
                data: turn_delta_json("turn-10", &stream_message_item("text-0", "Done.")),
            }),
            vec!["wave › Done."]
        );

        // A re-baselining whole-turn frame reprints nothing already shown.
        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: turn_json(
                    "turn-10",
                    "assistant",
                    "Done.",
                    "completed",
                    &format!(
                        "[{}]",
                        commentary_message_item("m-0", "Auditing the plan first.")
                    ),
                ),
            })
            .is_empty());
    }

    /// The wire the server actually sends now: a whole `turn` frame opens the
    /// turn, then `turn-delta` increments grow it — and the reader renders the
    /// wave's speech from the deltas without a whole turn per token. A `resync`
    /// drops the reconstruction so the reconnect's whole-turn replay resumes it.
    #[test]
    fn turn_delta_frames_render_growth_and_resync_drops_reconstruction() {
        let mut renderer = Renderer::new();

        // The turn opens as a whole (empty, running) frame — no prose yet.
        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: turn_json("turn-1", "assistant", "", "running", "[]"),
            })
            .is_empty());

        // Prose increments render as the wave speaking; nothing else on the wire.
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn-delta".into(),
                data: turn_delta_json(
                    "turn-1",
                    &stream_message_item("text-0", "I fixed the parser.")
                ),
            }),
            vec!["wave › I fixed the parser."]
        );

        // A tool increment stays out of the conversation, exactly like a whole turn.
        assert!(renderer
            .lines_for(&Frame {
                event: "turn-delta".into(),
                data: turn_delta_json(
                    "turn-1",
                    "{\"type\":\"tool\",\"id\":\"t-1\",\"name\":\"Bash\",\"status\":\"completed\",\"input\":null,\"output\":null}",
                ),
            })
            .is_empty());

        // A resync drops the reconstruction: a further delta is quiet until the
        // reconnect replays the whole turn.
        assert!(renderer
            .lines_for(&Frame {
                event: "resync".into(),
                data: "reconnect".into(),
            })
            .is_empty());
        assert!(
            renderer
                .lines_for(&Frame {
                    event: "turn-delta".into(),
                    data: turn_delta_json("turn-1", &stream_message_item("text-1", "more")),
                })
                .is_empty(),
            "no reconstruction survives a resync; the whole-turn replay rebuilds it"
        );
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
        let out = frames(": ping\r\n\r\nevent: note\r\ndata: first\r\ndata: second\r\n\r\n");
        assert_eq!(
            out,
            vec![Frame {
                event: "note".into(),
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
    /// underneath it never become chat.
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

        let mut renderer = Renderer::new();
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
                "The transcript renders every tool call as a card. I'll keep them out of the conversation.",
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
                "wave › The transcript renders every tool call as a card. I'll keep them out of the conversation.",
                "wave › Projection landed. One test caught a stale signature; fixed and green.",
            ],
            "the conversation is prose; backend evidence remains in the journal"
        );
    }

    #[test]
    fn conversation_renders_chat_without_backend_state() {
        let mut renderer = Renderer::new();
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "state".into(),
                data: "turning".into()
            }),
            Vec::<String>::new()
        );
        let user = turn_json("turn-1", "user", "how goes it?", "completed", "[]");
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn".into(),
                data: user.clone()
            }),
            vec!["you › how goes it?"]
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
    fn conversation_shows_child_outcomes_but_not_lifecycle_churn() {
        let mut renderer = Renderer::new();
        let state = activity_turn_json(
            "turn-1",
            "state_changed",
            "Task is running",
            "provider turn is active",
        );
        assert!(renderer
            .lines_for(&Frame {
                event: "turn".into(),
                data: state,
            })
            .is_empty());

        let opened = activity_turn_json(
            "turn-2",
            "pr_opened",
            "Opened PR #877",
            "https://github.com/loopflowstudio/loopflow/pull/877",
        );
        assert_eq!(
            renderer.lines_for(&Frame {
                event: "turn".into(),
                data: opened,
            }),
            vec!["task W2-132 › Opened PR #877 — https://github.com/loopflowstudio/loopflow/pull/877"]
        );
    }

    #[test]
    fn conversation_prints_only_the_growth_of_a_repeating_turn_id() {
        let mut renderer = Renderer::new();
        let running =
            |text: &str, items: &str| turn_json("turn-2", "assistant", text, "running", items);
        let tool = "[{\"type\":\"tool\",\"id\":\"t0\",\"name\":\"Bash\",\"status\":\"completed\",\
              \"input\":null,\"output\":null}]";

        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("", "[]"),
        });
        assert!(lines.is_empty());

        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("thinking", "[]"),
        });
        assert_eq!(lines, vec!["wave › thinking"]);

        // Same text re-sent with a successful item: backend machinery stays
        // quiet.
        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: running("thinking", tool),
        });
        assert!(lines.is_empty());

        // The terminal frame closes the turn; replays of it are quiet.
        let terminal = turn_json("turn-2", "assistant", "thinking", "completed", tool);
        let lines = renderer.lines_for(&Frame {
            event: "turn".into(),
            data: terminal.clone(),
        });
        assert!(lines.is_empty());
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

        for i in 0..20 {
            runtime
                .deliver(crate::wave::journal::MessageOp::Message, format!("old {i}"))
                .expect("older user turn");
        }
        runtime
            .deliver(crate::wave::journal::MessageOp::Message, "replayed".into())
            .expect("user turn");
        let seen: Arc<Mutex<Vec<Frame>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let endpoint = addr.to_string();
        let task = tokio::spawn(async move {
            let mut on_frame = |frame: Frame| {
                sink.lock().unwrap().push(frame);
                ControlFlow::Continue(())
            };
            let _ = stream_events(&endpoint, "", &mut on_frame).await;
        });

        // Wait for the replay to land, then publish a live turn.
        for _ in 0..200 {
            if seen
                .lock()
                .unwrap()
                .iter()
                .filter(|f| f.event == "turn")
                .count()
                == 12
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        runtime
            .deliver(
                crate::wave::journal::MessageOp::Message,
                "live after subscribe".into(),
            )
            .expect("live user turn");
        for _ in 0..200 {
            if seen
                .lock()
                .unwrap()
                .iter()
                .any(|f| f.event == "turn" && f.data.contains("live after subscribe"))
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
            frames
                .iter()
                .all(|f| f.event != "turn" || !f.data.contains("old 0")),
            "the default bounded replay omits older turns: {frames:?}"
        );
        assert_eq!(
            frames.iter().filter(|f| f.event == "turn").count(),
            13,
            "human subscriptions replay 12 turns, then stream the live turn"
        );
        assert!(
            frames
                .iter()
                .any(|f| f.event == "turn" && f.data.contains("live after subscribe")),
            "live turn arrives: {frames:?}"
        );
    }
}
