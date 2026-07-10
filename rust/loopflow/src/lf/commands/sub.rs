//! `lf sub` — tune in to the agent bus.
//!
//! The bus is a table in the shared store, so subscribing is a forward poll
//! from an id cursor: you hear what is said while you listen, from where you
//! tuned in. Nothing is delivered, nothing is replayed, and a frame published
//! to a channel nobody was listening on is gone. No HTTP is in the path — the
//! served mind need not exist.
//!
//! Scope is a prefix: `lf sub goals` hears the whole family (the wave's own
//! channel plus every hand's `goals.<run>`), `lf sub goals.148e` hears exactly
//! that channel and its descendants. Bare, it hears the ambient channel — the
//! family at the wave home, the work line's own channel inside its worktree.
//!
//! The mind's own ear is [`crate::wave::bus::BusListener`], the same poll with
//! a durable cursor — at-least-once across a crash, and nothing replayed on a
//! clean restart. The mind's THREAD — durable, replayed, human — is the other
//! wire: [`super::thread`], still SSE on the listener, and what `lf chat --follow`
//! follows. `lf sub` never touches it.

use anyhow::Result;

use crate::lf::commands::chat::CliContext;
use crate::lf::commands::radio::ambient_channel;
use crate::lfdb::BusMessage;
use crate::wave::bus::POLL_CADENCE;
use crate::wave::channel::matches_prefix;

pub fn run(channel: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        tokio::select! {
            result = follow(channel, json) => result,
            // Clean exit on Ctrl-C: the subscription just ends.
            _ = tokio::signal::ctrl_c() => Ok(()),
        }
    })
}

/// Poll the bus until the process is killed. Resolution failure is the
/// publish-to-no-subscriber case mirrored: exit 0 with one stderr note, so
/// `lf sub` is safe in every prompt unconditionally.
async fn follow(channel: Option<&str>, json: bool) -> Result<()> {
    let context = CliContext::detect().await;
    let Some(store) = context.store.clone() else {
        eprintln!("no registry store here; nothing to tune in to");
        return Ok(());
    };
    let prefix = match channel {
        Some(channel) => channel.to_string(),
        None => match ambient_channel(&context, &store).await {
            Some(channel) => channel,
            None => {
                eprintln!("no wave here; nothing to tune in to");
                return Ok(());
            }
        },
    };

    // Tune in at the head: a topic has no past.
    let mut cursor = store.bus_head().await?;
    loop {
        tokio::time::sleep(POLL_CADENCE).await;
        for message in store.read_bus_after(cursor).await? {
            cursor = message.id;
            if matches_prefix(&message.channel, &prefix) {
                println!("{}", line_for(&message, json));
            }
        }
    }
}

/// One heard frame: the raw row as NDJSON, or `[channel] byline: text`.
fn line_for(message: &BusMessage, json: bool) -> String {
    if json {
        return serde_json::json!({
            "id": message.id,
            "channel": message.channel,
            "byline": message.byline,
            "text": message.text,
            "at": message.at,
        })
        .to_string();
    }
    format!("[{}] {}: {}", message.channel, message.byline, message.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(channel: &str, byline: &str, text: &str) -> BusMessage {
        BusMessage {
            id: 7,
            channel: channel.into(),
            byline: byline.into(),
            text: text.into(),
            at: 1_780_000_000,
        }
    }

    /// A subscription is a prefix, so a wave hears its hands and a hand hears
    /// only itself. `goalsmith` is not in `goals`.
    #[test]
    fn a_subscription_is_a_prefix_over_the_dot_tree() {
        assert!(matches_prefix("goals", "goals"));
        assert!(matches_prefix("goals.148e", "goals"));
        assert!(!matches_prefix("goals", "goals.148e"));
        assert!(!matches_prefix("goalsmith", "goals"));
    }

    /// The human line shows both halves of the record: who claimed to speak,
    /// and where the frame actually arrived.
    #[test]
    fn a_heard_frame_shows_byline_beside_channel() {
        assert_eq!(
            line_for(&message("goals.148e", "ci", "all green"), false),
            "[goals.148e] ci: all green"
        );
        let json: serde_json::Value =
            serde_json::from_str(&line_for(&message("goals.148e", "ci", "all green"), true))
                .expect("valid NDJSON");
        assert_eq!(json["channel"], "goals.148e");
        assert_eq!(json["byline"], "ci");
        assert_eq!(json["text"], "all green");
    }
}
