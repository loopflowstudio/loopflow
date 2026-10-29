//! What resume would do is a reading, never admission or a client reservation.
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
    /// A retained terminal or client holds it and hands it over; if it is
    /// gone, saved history resumes.
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
    pub actions: Vec<SessionAction>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl SessionResumeExplanation {
    pub(crate) fn unavailable(reason: impl ToString) -> Self {
        Self {
            resolution: ContextExplanation::unavailable(reason.to_string()),
            action: None,
            state: None,
            actions: Vec::new(),
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
                SessionResumeIntent::TakeOver => {
                    "Take over from the terminal or client holding it; if none remains, resume saved provider history"
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
            let holder = if action.fork {
                "fork into a new conversation; whatever holds this one keeps it"
            } else {
                "take it over; a terminal showing it hands it over, a client lf started stops, unsent text there is lost"
            };
            lines.push(format!(
                "Action: {intent}{agent}; {holder}{}",
                if action.prepare_only {
                    " (preparation only; no client takeover)"
                } else {
                    ""
                }
            ));
        }
        for action in &self.actions {
            lines.push(format!(
                "{}: {}",
                action.label,
                action.unavailable_reason.as_deref().unwrap_or(&action.help)
            ));
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

pub(crate) async fn explain_resume(
    store: &SharedStore,
    cwd: &Path,
    target: super::SessionTarget<'_>,
    fork: bool,
    prepare_only: bool,
) -> SessionResumeExplanation {
    use crate::session::PrimaryScope;
    let selected = async {
        let target = super::resolve_target(store, target).await?;
        if let super::SessionTarget::Session(id) = target {
            return Ok((None, Some(id.to_string())));
        }
        let Some(scope) = super::checkout_scope(store, cwd, target).await? else {
            if let Some(id) = super::checkout_conversation(store, cwd).await? {
                return Ok((None, Some(id)));
            }
            return Err(super::unassociated_checkout(cwd)?);
        };
        let session = match &scope {
            PrimaryScope::Task(task) => super::primary::task_conversation(store, task).await?,
            _ => store.sqlite.primary_session(&scope)?,
        };
        Ok::<_, anyhow::Error>((Some(scope), session.map(|session| session.id.to_string())))
    }
    .await;
    let (scope, selector) = match selected {
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
            None,
        )
        .await,
        action: None,
        state: None,
        actions: Vec::new(),
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
    let provider = session
        .artifact_key
        .as_deref()
        .map(|key| store.sqlite.input_provider_session(key))
        .transpose()?
        .flatten();
    // A metadata read does not acquire viewer control or probe the endpoint.
    let connection = store.sqlite.session_transport(&session.id)?;
    // Opening checks the harness before any holder: another agent never
    // takes over the running one, it starts a thread from the log.
    let handoff = provider.as_ref().and(super::handoff_agent(&session));
    // Opening refuses a Run whatever the flags, so nothing would happen.
    let refusal = super::run_refusal(&store.sqlite, &session)?;
    let intent = if !fork && !prepare_only && handoff.is_some() {
        SessionResumeIntent::NewThread
    } else if !prepare_only && connection.is_some() {
        SessionResumeIntent::TakeOver
    } else if provider.is_none() {
        SessionResumeIntent::Start
    } else {
        SessionResumeIntent::Resume
    };
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
    if connection.is_some() {
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
        if provider.is_some() {
            if let Err(error) =
                crate::lf::commands::util::require_session_task(&store.sqlite, &session)
            {
                report.impediments.push(error.to_string());
            }
        }
        report.unavailable.push(
            "Provider/account launch readiness is not inspected; no credentials are prepared"
                .into(),
        );
    }
    // Explanation and opening use the same recorded Process evidence.
    let metadata = store
        .sqlite
        .session_summary(&session.id, report.resolution.observed_at)?
        .ok_or_else(|| super::session_not_found(&session.id))?;
    let state = super::session_state(&metadata);
    report.state = Some(state);
    report.actions = super::session_actions(state);
    Ok(())
}
