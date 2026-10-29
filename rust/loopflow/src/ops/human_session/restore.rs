//! `lf resume --all`: bring back every conversation a restart or crash cut off.
//!
//! A Session is interrupted when its execution owner never recorded an exit and
//! the OS shows it gone. Leaving with the provider's own exit, or detaching,
//! never qualifies. Each Session resumes through the ordinary `lf resume` path
//! in a new owner with no viewer; terminals attach when they show it.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{lock_session_process, owner_running};
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

/// Interrupted Sessions on this machine, ordered by id.
async fn interrupted(store: &SharedStore) -> Result<Vec<crate::session::LfSession>> {
    let mut sessions = Vec::new();
    for (id, owner) in store.sqlite.unexited_session_owners()? {
        if owner_running(&store.sqlite, &owner)? {
            continue;
        }
        let Some(session) = store.session(&id).await? else {
            continue;
        };
        // Only saved provider history can be reopened without replaying input.
        let history = match session.artifact_key.as_deref() {
            Some(key) => store.sqlite.input_provider_session(key)?.is_some(),
            None => false,
        };
        if history {
            sessions.push(session);
        }
    }
    Ok(sessions)
}

pub(crate) async fn resume_all(store: &SharedStore) -> Result<Vec<ResumedSession>> {
    // Concurrent runs would each start an owner for the same Session.
    let _all = lock_resume_all().await?;
    let binary = std::env::current_exe()?;
    let mut resumed = Vec::new();
    for session in interrupted(store).await? {
        resumed.push(resume(store, &session, &binary).await?);
    }
    Ok(resumed)
}

/// Serializes every caller that starts owners for Sessions it found stopped.
pub(crate) async fn lock_resume_all() -> Result<std::fs::File> {
    tokio::task::spawn_blocking(|| lock_session_process("resume-all")).await?
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

/// Start `lf resume <id>` in its own owner and wait until it holds the conversation.
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
    let owner = crate::transport::start(&argv, true, &session.cwd, &|_| {})
        .context("start its execution owner")?;
    let deadline = Instant::now() + BIND_TIMEOUT;
    loop {
        if store.sqlite.session_transport(&session.id)?.as_ref() == Some(&owner) {
            return Ok(ResumeOutcome::Resumed);
        }
        if !owner_running(&store.sqlite, &owner)? {
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

pub(crate) fn render(resumed: &[ResumedSession]) -> String {
    if resumed.is_empty() {
        return "No interrupted conversations.".into();
    }
    let mut lines = resumed.iter().map(describe).collect::<Vec<_>>();
    lines.push("Open one with: lf resume <id>".into());
    lines.join("\n")
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
