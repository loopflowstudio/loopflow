//! One Session's conversation: each provider conversation it used, read from
//! the provider when the transcript is there, and the facts Loopflow kept when
//! it is not.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::{locate, read, ExpiryPolicy, Gap, Located, Source, Turn};
use crate::harness::HarnessKind;
use crate::id::AgentSessionId;
use crate::store::sqlite::SqliteStore;
use crate::store::ProviderAccountId;

/// What the store kept of one provider conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadFacts {
    pub thread: AgentSessionId,
    pub harness: Option<String>,
    pub account: Option<ProviderAccountId>,
    pub turns: Vec<TurnFacts>,
}

/// One native turn's receipts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnFacts {
    pub turn: String,
    pub started_at: Option<i64>,
    pub settled_at: Option<i64>,
    pub status: Option<String>,
    pub final_answer: Option<String>,
}

impl ThreadFacts {
    fn last_at(&self) -> Option<i64> {
        self.turns
            .iter()
            .flat_map(|turn| [turn.started_at, turn.settled_at])
            .flatten()
            .max()
    }
}

/// `lf history show SESSION --transcript --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTranscript {
    pub session_id: String,
    pub conversations: Vec<Conversation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub harness: String,
    pub agent_session: String,
    pub account: Option<ProviderAccountId>,
    pub retention: Retention,
    /// Read from the provider; empty when only facts remain.
    pub turns: Vec<Turn>,
    pub facts: Vec<TurnFacts>,
}

/// Where a conversation's text is, and how long it stays there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Retention {
    pub state: RetentionState,
    /// The transcript file, when it is one.
    pub path: Option<String>,
    pub policy: ExpiryPolicy,
    /// Unix seconds after which the provider may remove it, or did.
    pub after: Option<i64>,
    /// Why there is no text: a gap, or a transcript this lf could not read.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionState {
    /// The provider still has the transcript.
    Provider,
    /// Only the facts Loopflow recorded remain.
    Facts,
    /// Presence is not checked without running the provider.
    Unknown,
}

/// One provider conversation a Session used, and where its transcript is.
pub(crate) struct Thread {
    pub facts: ThreadFacts,
    pub located: Result<(HarnessKind, Located)>,
}

/// Every provider conversation `session` used, oldest first.
pub(crate) fn threads(store: &SqliteStore, session: &str) -> Result<Vec<Thread>> {
    let cwd = store
        .session(session)?
        .map(|session| session.cwd)
        .unwrap_or_default();
    Ok(store
        .session_turn_facts(session)?
        .into_iter()
        .map(|facts| {
            let located = locate_thread(store, &facts, &cwd);
            Thread { facts, located }
        })
        .collect())
}

fn locate_thread(
    store: &SqliteStore,
    facts: &ThreadFacts,
    cwd: &Path,
) -> Result<(HarnessKind, Located)> {
    let name = facts.harness.as_deref().unwrap_or_default();
    let harness = HarnessKind::parse(name).ok_or_else(|| anyhow!("no reader for {name:?}"))?;
    let located = locate(store, harness, &facts.thread, facts.account.as_ref(), cwd)?;
    Ok((harness, located))
}

/// `session`'s conversations, read from their providers.
pub fn session_transcript(
    store: &SqliteStore,
    session: &str,
    now: i64,
) -> Result<SessionTranscript> {
    Ok(SessionTranscript {
        session_id: session.to_owned(),
        conversations: threads(store, session)?
            .into_iter()
            .map(|thread| conversation(thread, now))
            .collect(),
    })
}

fn conversation(Thread { facts, located }: Thread, now: i64) -> Conversation {
    let mut conversation = Conversation {
        harness: facts.harness.clone().unwrap_or_default(),
        agent_session: facts.thread.as_str().to_owned(),
        account: facts.account.clone(),
        retention: Retention {
            state: RetentionState::Facts,
            path: None,
            policy: ExpiryPolicy::Kept,
            after: None,
            note: None,
        },
        turns: Vec::new(),
        facts: facts.turns.clone(),
    };
    let (harness, located) = match located {
        Ok(located) => located,
        Err(error) => {
            conversation.retention.note = Some(format!("{error:#}"));
            return conversation;
        }
    };
    let retention = &mut conversation.retention;
    match located {
        Located::Found { source, home } => {
            retention.state = RetentionState::Provider;
            retention.policy = ExpiryPolicy::read(harness, home.as_deref());
            let mut written = false;
            if let Source::File(path) = &source {
                retention.path = Some(path.display().to_string());
                retention.after = super::modified(path).and_then(|at| retention.policy.after(at));
                written = std::fs::metadata(path).is_ok_and(|meta| meta.len() > 0);
            }
            match read(harness, &source) {
                Ok(turns) if turns.is_empty() && written => {
                    retention.note = Some("unreadable by this lf (format changed?)".into());
                }
                Ok(turns) => conversation.turns = turns,
                Err(error) => retention.note = Some(format!("{error:#}")),
            }
        }
        Located::Gone(gap) => {
            let home = match &gap {
                Gap::FileMissing { home } => Some(home.as_path()),
                Gap::AccountHomeRemoved => None,
            };
            retention.policy = ExpiryPolicy::read(harness, home);
            retention.after = facts
                .last_at()
                .and_then(|last| retention.policy.after(last));
            retention.note = Some(gone(harness, &gap, retention.after, now));
        }
    }
    conversation
}

/// Why a transcript is gone, claiming expiry only when the policy explains it.
fn gone(harness: HarnessKind, gap: &Gap, after: Option<i64>, now: i64) -> String {
    match gap {
        Gap::AccountHomeRemoved => "account home removed".into(),
        Gap::FileMissing { .. } => match after {
            Some(after) if after < now => format!(
                "gone at {}; removed after {}",
                harness.as_str(),
                super::date(after)
            ),
            _ => format!("missing at {}", harness.as_str()),
        },
    }
}
