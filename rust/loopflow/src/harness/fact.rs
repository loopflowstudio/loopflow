//! What a provider's native stream established, decoded once per message.
//!
//! Each harness has one decoder that turns a native message into an ordered
//! `Vec<Decoded>`: `Fact` for what Loopflow records (receipts, availability,
//! account capacity, native conversation identity) and `Display` for what a
//! person watches live. Decoders hold only the cross-message correlation their
//! provider needs; `FactWriter` owns every store and account effect.

use serde_json::Value;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::id::AgentSessionId;
use crate::provider_account::RateLimitSignal;

/// One provider turn in its native conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderTurn {
    pub thread: AgentSessionId,
    pub turn: String,
}

/// What a provider's stream says about a conversation's availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Signal {
    /// The provider is producing a turn.
    Working,
    /// A new native turn began; earlier preview text no longer describes it.
    NewTurn,
    ToolStarted(String),
    ToolResolved(String),
    InputRequested(String),
    InputResolved(String),
    /// The provider handed the turn back; `true` when it succeeded.
    Yielded(bool),
    /// Assistant prose the current turn produced, shown as its preview.
    Previewed {
        turn: ProviderTurn,
        source: &'static str,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) enum ProviderFact {
    /// The provider named the native conversation this stream continues.
    SessionObserved(AgentSessionId),
    /// The provider reported that a native turn began.
    TurnStarted {
        turn: ProviderTurn,
    },
    /// The decoder correlated a native turn with the input `request` it
    /// answers. Input Loopflow sent has its origin frozen at send; input the
    /// provider's own client submitted takes the current attachment's.
    TurnAnswered {
        turn: ProviderTurn,
        request: String,
    },
    Activity(Signal),
    /// A provider that counts its own availability reported a whole reading.
    AvailabilityReading {
        open_tools: usize,
        pending_input: usize,
        yielded: bool,
        /// The turn ended; a settled turn that did not yield failed.
        settled: bool,
    },
    /// The turn's final answer: `{"text": …}` or `{"value": …}`.
    Output {
        turn: ProviderTurn,
        answer: Value,
    },
    /// The provider's own usage payload, unchanged.
    Usage {
        turn: ProviderTurn,
        receipt: Value,
    },
    /// A provider turn ended. `turn` is absent when Loopflow cannot tell which
    /// native turn it was; `detail` is the retained completion payload.
    TurnCompleted {
        turn: Option<ProviderTurn>,
        status: Lifecycle,
        detail: Value,
    },
    RateLimit(RateLimitSignal),
}

#[derive(Debug, Clone)]
pub(crate) enum Decoded {
    Fact(ProviderFact),
    Display(ConversationEvent),
}

impl Decoded {
    pub(crate) fn fact(&self) -> Option<&ProviderFact> {
        match self {
            Self::Fact(fact) => Some(fact),
            Self::Display(_) => None,
        }
    }
}

impl From<ProviderFact> for Decoded {
    fn from(fact: ProviderFact) -> Self {
        Self::Fact(fact)
    }
}

impl From<ConversationEvent> for Decoded {
    fn from(event: ConversationEvent) -> Self {
        Self::Display(event)
    }
}

/// Availability facts for a batch of signals.
pub(crate) fn activity(signals: impl IntoIterator<Item = Signal>) -> Vec<Decoded> {
    signals
        .into_iter()
        .map(|signal| ProviderFact::Activity(signal).into())
        .collect()
}

/// A recorded native trace under `harness/testdata/`, one JSON value per line.
#[cfg(test)]
pub(crate) fn trace(file: &str) -> Vec<Value> {
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

/// The facts in `decoded`, one short line each, for asserting a sequence.
#[cfg(test)]
pub(crate) fn fact_lines(decoded: &[Decoded]) -> Vec<String> {
    let turn = |turn: &ProviderTurn| format!("{}/{}", turn.thread.as_str(), turn.turn);
    decoded
        .iter()
        .filter_map(Decoded::fact)
        .map(|fact| match fact {
            ProviderFact::SessionObserved(thread) => format!("SessionObserved {}", thread.as_str()),
            ProviderFact::TurnStarted { turn: t } => format!("TurnStarted {}", turn(t)),
            ProviderFact::TurnAnswered { turn: t, request } => {
                format!("TurnAnswered {} request={request}", turn(t))
            }
            ProviderFact::Activity(Signal::Previewed { text, .. }) => {
                format!("Activity Previewed {text:?}")
            }
            ProviderFact::Activity(signal) => format!("Activity {signal:?}"),
            ProviderFact::AvailabilityReading {
                open_tools,
                pending_input,
                yielded,
                settled,
            } => format!(
                "AvailabilityReading tools={open_tools} input={pending_input} yielded={yielded} settled={settled}"
            ),
            ProviderFact::Output { turn: t, answer } => format!("Output {} {answer}", turn(t)),
            ProviderFact::Usage { turn: t, .. } => format!("Usage {}", turn(t)),
            ProviderFact::TurnCompleted { turn: t, status, .. } => format!(
                "TurnCompleted {} {status:?}",
                t.as_ref().map_or("-".to_owned(), turn)
            ),
            ProviderFact::RateLimit(_) => "RateLimit".to_owned(),
        })
        .collect()
}
