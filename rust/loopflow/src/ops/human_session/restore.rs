//! Reopen stopped Sessions during an installed-build rollover.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::store::SharedStore;

/// How long one owner may take to bind before it is reported as still starting.
const BIND_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumedSession {
    pub id: String,
    pub title: String,
    pub outcome: ResumeOutcome,
    /// Its provider was mid-turn when it stopped; that turn is not replayed.
    pub turn_cut_off: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResumeOutcome {
    Resumed,
    /// The owner started but has not bound the conversation yet.
    Starting,
    Failed {
        reason: String,
    },
}

/// Reopen one stopped Session in a new owner running `binary`.
pub(crate) async fn resume(
    store: &SharedStore,
    session: &crate::session::LfSession,
    binary: &Path,
) -> Result<ResumedSession> {
    let turn_cut_off = store.sqlite.session_turn_was_running(&session.id)?;
    let outcome = match start(store, session, binary).await {
        Ok(outcome) => outcome,
        Err(error) => ResumeOutcome::Failed {
            reason: format!("{error:#}"),
        },
    };
    Ok(ResumedSession {
        id: session.id.as_str().to_owned(),
        title: session.title.clone(),
        outcome,
        turn_cut_off,
    })
}

/// Start `lf resume <id>` in a held terminal and wait until it holds the conversation.
async fn start(
    store: &SharedStore,
    session: &crate::session::LfSession,
    binary: &Path,
) -> Result<ResumeOutcome> {
    anyhow::ensure!(
        session.cwd.is_dir(),
        "its checkout {} no longer exists",
        session.cwd.display()
    );
    let argv = [
        binary.to_string_lossy().into_owned(),
        "session".into(),
        "resume".into(),
        session.id.as_str().to_owned(),
    ];
    let terminal = crate::transport::start_held(&argv, &session.cwd, Some(&session.id), &|_| {})
        .context("start its terminal")?;
    let host = crate::transport::Host::Terminal(terminal.clone());
    let deadline = Instant::now() + BIND_TIMEOUT;
    loop {
        if store.sqlite.session_transport(&session.id)?.as_ref() == Some(&host) {
            return Ok(ResumeOutcome::Resumed);
        }
        if !store.sqlite.terminal_running(&terminal)? {
            return Ok(ResumeOutcome::Failed {
                reason: format!(
                    "it stopped while resuming; `lf resume {}` shows why",
                    session.id
                ),
            });
        }
        if Instant::now() >= deadline {
            return Ok(ResumeOutcome::Starting);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// One line saying how `session` came back.
pub(crate) fn describe(session: &ResumedSession) -> String {
    let name = if session.title.is_empty() {
        session.id.clone()
    } else {
        format!("“{}” ({})", session.title, session.id)
    };
    let mut line = match &session.outcome {
        ResumeOutcome::Resumed => format!("Resumed {name}"),
        ResumeOutcome::Starting => format!("Resuming {name}; still starting"),
        ResumeOutcome::Failed { reason } => format!("Could not resume {name}: {reason}"),
    };
    if session.turn_cut_off {
        line.push_str(" — its last turn was cut off; ask again");
    }
    line
}
