//! L2 capability: observe recent wave-chat messages and reply only if warranted.
//!
//! A direct, composable capability — no resident, no governance loop, no
//! playhead, no Run record. Given a wave and the recent conversation, it runs a
//! single chat-surface turn and returns a reply **only when one is warranted**.
//! A channel may be humans talking to each other, so most turns should produce
//! nothing: an empty turn maps to `None` and the caller posts nothing. The
//! judgment of whether to reply lives in the `wave/chat` skill, not here.
//!
//! Producing a reply is deliberately separate from posting it — output is the
//! caller's concern (print it, deliver it to Discord). That is what lets one
//! capability serve every surface (the CLI, an independent responder, the
//! resident).

use std::path::Path;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::engine::parse_agent;
use crate::engine::prompt::Surface;
use crate::harness::{default_create_harness, CreateHarness};
use crate::lf::commands::run::{prepare_wave_harness_turn, PreparedHarnessTurn};

/// Observe `conversation` (the recent channel messages) for `wave` and return a
/// reply only when one is warranted; `Ok(None)` means "stay silent" — the common
/// case, since the channel may be people talking among themselves.
///
/// `origin_repo` is the canonical repository; `resident_repo` is the worktree
/// whose `GOAL.md`/`MEMORY.md` supply the wave's identity (pass the same path as
/// `origin_repo` for a standalone reply). `agent` overrides the provider
/// (e.g. `"claude"`); `None` uses the configured default.
pub async fn reply(
    origin_repo: &Path,
    resident_repo: &Path,
    wave: &str,
    conversation: &str,
    agent: Option<String>,
    max_turns: Option<u32>,
) -> Result<Option<String>> {
    let mut prepared = prepare_wave_harness_turn(
        "wave/chat",
        conversation,
        wave,
        max_turns,
        origin_repo,
        resident_repo,
        Some(Surface::Chat),
    )?;
    if let Some(agent) = agent {
        let (harness, model) = parse_agent(&agent);
        prepared.harness = harness;
        prepared.model = model;
        prepared.config.agent = Some(agent);
    }
    let create: CreateHarness = Box::new(default_create_harness);
    reply_prepared(prepared, &create).await
}

/// Run one prepared chat turn and return its deliberate reply. Both the
/// standalone command and the resident responder use this boundary, so silence
/// has one meaning everywhere: no message is posted.
pub(crate) async fn reply_prepared(
    prepared: PreparedHarnessTurn,
    create: &CreateHarness,
) -> Result<Option<String>> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create(
        &prepared.harness,
        crate::harness::ApprovalPolicy::AutoApprove,
        event_tx,
    )?;
    harness.start(&prepared.config).await?;
    if let Err(error) = harness.send_input(&prepared.input).await {
        let _ = harness.stop().await;
        return Err(error);
    }

    let mut reply = String::new();
    let outcome = loop {
        match event_rx.recv().await {
            None => {
                break Err(anyhow!(
                    "harness event stream closed before the turn completed"
                ))
            }
            Some(ConversationEvent::TextDelta { content, .. }) => reply.push_str(&content),
            Some(ConversationEvent::TurnCompleted { status, .. }) => break Ok(status),
            Some(_) => {}
        }
    };
    let _ = harness.stop().await;
    match outcome? {
        Lifecycle::Completed => {
            let reply = reply.trim();
            Ok((!reply.is_empty()).then(|| reply.to_string()))
        }
        other => Err(anyhow!("chat turn ended {other:?} before completing")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::AgentConfig;
    use crate::harness::Harness;

    /// A harness that replays a scripted event sequence when the turn is sent.
    struct ScriptedHarness {
        events: mpsc::UnboundedSender<ConversationEvent>,
        script: Vec<ConversationEvent>,
    }

    #[async_trait::async_trait]
    impl Harness for ScriptedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }
        async fn send_input(&mut self, _content: &str) -> Result<()> {
            for event in self.script.drain(..) {
                let _ = self.events.send(event);
            }
            Ok(())
        }
        async fn interrupt(&mut self) -> Result<()> {
            Ok(())
        }
        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    fn scripted(script: Vec<ConversationEvent>) -> CreateHarness {
        Box::new(move |_name, _approval, tx| {
            Ok(Box::new(ScriptedHarness {
                events: tx,
                script: script.clone(),
            }) as Box<dyn Harness>)
        })
    }

    fn prepared() -> PreparedHarnessTurn {
        PreparedHarnessTurn {
            config: AgentConfig::default(),
            input: "recent channel messages".to_string(),
            context: crate::trace::PreparedTurnContext::from_prompts("", "recent channel messages"),
            harness: "scripted".to_string(),
            model: None,
        }
    }

    fn delta(content: &str) -> ConversationEvent {
        ConversationEvent::TextDelta {
            turn_id: "turn".to_string(),
            content: content.to_string(),
        }
    }

    fn completed(status: Lifecycle) -> ConversationEvent {
        ConversationEvent::TurnCompleted {
            turn_id: "turn".to_string(),
            status,
        }
    }

    #[tokio::test]
    async fn collects_text_deltas_into_a_reply_when_one_is_warranted() {
        let reply = reply_prepared(
            prepared(),
            &scripted(vec![
                delta("The top unstarted task is "),
                delta("LOO-258."),
                completed(Lifecycle::Completed),
            ]),
        )
        .await
        .expect("the turn completes");
        assert_eq!(reply.as_deref(), Some("The top unstarted task is LOO-258."));
    }

    #[tokio::test]
    async fn an_empty_turn_is_silence_not_a_reply() {
        // Humans talking to each other: the turn produces nothing.
        let reply = reply_prepared(prepared(), &scripted(vec![completed(Lifecycle::Completed)]))
            .await
            .expect("the turn completes");
        assert_eq!(reply, None, "an empty turn must stay silent");
    }

    #[tokio::test]
    async fn a_turn_that_does_not_complete_is_an_error() {
        let result = reply_prepared(
            prepared(),
            &scripted(vec![delta("half a thought"), completed(Lifecycle::Failed)]),
        )
        .await;
        assert!(result.is_err(), "a failed turn must not pass as a reply");
    }
}
