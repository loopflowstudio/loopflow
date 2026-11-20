//! What resume would do is a reading, never admission or a client reservation.
use crate::store::session_projection;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ops::context::{ContextExplanation, ContextFact};
use crate::ops::WorkSelection;
use crate::store::SharedStore;

use super::{SessionAction, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionResumeIntent {
    /// Nothing to resume: the conversation starts.
    Start,
    /// Saved provider history resumes.
    Resume,
    /// Its held terminal gains a view here; if it ended, saved history resumes.
    View,
    /// A client lf can't share runs it; that client ends and it continues here.
    TakeOver,
    /// Another harness starts a new thread seeded from the conversation log.
    NewThread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResumeAction {
    pub intent: SessionResumeIntent,
    /// The selected agent, else the conversation's recorded provider.
    pub agent: Option<String>,
    /// A new conversation continues from this one's log; its holder keeps it.
    pub fork: bool,
    pub prepare_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResumeExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<SessionResumeAction>,
    pub state: Option<SessionState>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub opening: Option<SessionAction>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl SessionResumeExplanation {
    pub fn unavailable(reason: impl ToString) -> Self {
        Self {
            resolution: ContextExplanation::unavailable(reason.to_string()),
            action: None,
            state: None,
            opening: None,
            impediments: Vec::new(),
            unavailable: vec![reason.to_string()],
        }
    }

    pub fn render(&self) -> String {
        let mut lines = vec![self.resolution.render()];
        if let Some(action) = &self.action {
            let intent = match action.intent {
                SessionResumeIntent::Start => "Start this conversation",
                SessionResumeIntent::Resume => "Resume saved provider history",
                SessionResumeIntent::View => {
                    "View its running terminal here; if it ended, resume saved provider history"
                }
                SessionResumeIntent::TakeOver => {
                    "Take over from the client running it; that client ends, unsent text there is lost"
                }
                SessionResumeIntent::NewThread => {
                    "Start a new thread from this conversation's log"
                }
            };
            let agent = action
                .agent
                .as_deref()
                .map(|agent| format!(" on {agent}"))
                .unwrap_or_default();
            let fork = if action.fork {
                "; fork into a new conversation, whatever holds this one keeps it"
            } else {
                ""
            };
            lines.push(format!(
                "Action: {intent}{agent}{fork}{}",
                if action.prepare_only {
                    " (preparation only; no client takeover)"
                } else {
                    ""
                }
            ));
        }
        if let Some(action) = &self.opening {
            lines.push(format!("{}: {}", action.label, action.help));
        }
        lines.extend(
            self.impediments
                .iter()
                .map(|reason| format!("Impediment: {reason}")),
        );
        lines.extend(
            self.unavailable
                .iter()
                .map(|reason| format!("Unavailable: {reason}")),
        );
        lines.push(
            "Nothing was executed. Opening re-reads identity and client evidence before acting."
                .into(),
        );
        lines.join("\n")
    }
}

pub async fn explain_resume(
    store: &SharedStore,
    cwd: &Path,
    target: super::SessionTarget<'_>,
    fork: bool,
    prepare_only: bool,
) -> SessionResumeExplanation {
    use crate::session::PrimaryScope;
    let (scope, selector) = match super::existing_connection(store, cwd, target).await {
        Ok(selected) => selected,
        Err(error) => return SessionResumeExplanation::unavailable(error),
    };
    let task = match &scope {
        Some(PrimaryScope::Task(task)) => Some(task.as_str()),
        _ => None,
    };
    let wave = match &scope {
        Some(PrimaryScope::Wave(wave)) => Some(wave.as_str()),
        _ => None,
    };
    let mut report = SessionResumeExplanation {
        resolution: crate::ops::context::explain_context(
            store,
            cwd,
            WorkSelection { task, wave },
            selector.as_deref(),
        )
        .await,
        action: None,
        state: None,
        opening: None,
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Some(PrimaryScope::Repository(_)) = &scope {
        report.resolution.task = ContextFact::Unbound;
        report.resolution.wave = ContextFact::Unbound;
    }
    if selector.is_none() {
        if let Some(scope @ (PrimaryScope::Repository(_) | PrimaryScope::Wave(_))) = &scope {
            let path = match &report.resolution.repository_path {
                ContextFact::Bound { value, .. } => Path::new(value),
                _ => cwd,
            };
            match super::primary::scope_worktree(path, scope) {
                Ok(plan) => {
                    report.resolution.checkout = ContextFact::Bound {
                        value: plan.worktree_path.to_string_lossy().into_owned(),
                        source: "persistent_checkout".into(),
                    }
                }
                Err(error) => report.unavailable.push(error.to_string()),
            }
        }
    }
    if let ContextFact::Bound { source, .. } = &mut report.resolution.session {
        if !matches!(target, super::SessionTarget::Session(_)) {
            *source = "contextual_session".into();
        }
    }
    if let Some(selector) = selector {
        if let Err(error) = read_resume(store, &selector, fork, prepare_only, &mut report).await {
            report.unavailable.push(error.to_string());
        }
    } else {
        report.action = Some(SessionResumeAction {
            intent: SessionResumeIntent::Start,
            agent: crate::config::agent_override(),
            fork: false,
            prepare_only,
        });
        report.opening = Some(SessionAction {
            label: "Start conversation".into(),
            help: "No existing conversation; opening starts one".into(),
        });
    }
    report
}

async fn read_resume(
    store: &SharedStore,
    selector: &str,
    fork: bool,
    prepare_only: bool,
    report: &mut SessionResumeExplanation,
) -> Result<()> {
    let Some(session) = super::find_session(store, selector, true).await? else {
        report.unavailable.push(format!("Session {selector} is not recorded; provider-conversation admission is not performed by explanation"));
        return Ok(());
    };
    // Opening refuses a Run whatever the flags, so nothing would happen.
    let refusal = super::run_refusal(&store.sqlite, &session)?;
    // Explanation and opening use the same recorded Process evidence; a
    // metadata read does not acquire viewer control or probe the endpoint.
    let metadata = session_projection::session_summary(
        &store.sqlite,
        &session.id,
        report.resolution.observed_at,
    )?
    .ok_or_else(|| super::session_not_found(&session.id))?;
    let intent = super::invocation_intent(&session, &metadata, fork, prepare_only);
    match refusal {
        Some(refusal) => report.impediments.push(refusal),
        None => {
            report.action = Some(SessionResumeAction {
                intent,
                agent: crate::config::agent_override().or_else(|| session.provider.clone()),
                fork,
                prepare_only,
            })
        }
    }
    if metadata.held {
        report.unavailable.push("Retained execution endpoint is not probed; an unavailable owner leaves continuity unknown".into());
    }
    if !prepare_only {
        if !session.cwd.is_dir() {
            if store.sqlite.primary_scope(&session.id)?.is_some() {
                report.unavailable.push("Persistent workspace may need preparation; explanation does not recreate or move it".into());
            } else {
                report.impediments.push(format!(
                    "Session checkout {} is unavailable",
                    session.cwd.display()
                ));
            }
        }
        if metadata.has_provider_history {
            if let Err(error) =
                crate::harness::native_session::require_session_task(&store.sqlite, &session)
            {
                report.impediments.push(error.to_string());
            }
        }
        report.unavailable.push(
            "Provider/account launch readiness is not inspected; no credentials are prepared"
                .into(),
        );
    }
    report.state = Some(super::session_state(&metadata));
    report.opening = super::session_action(&metadata);
    Ok(())
}
