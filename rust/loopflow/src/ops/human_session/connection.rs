//! Connection intent is a reading, never admission or a client reservation.
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ops::context::{ContextExplanation, ContextFact};
use crate::ops::WorkSelection;
use crate::store::SharedStore;

use super::{NativeSession, OpenMode, SessionAction, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionConnectIntent {
    Start,
    Resume,
    ConnectOrResume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConnectAction {
    pub intent: SessionConnectIntent,
    pub mode: OpenMode,
    pub prepare_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConnectExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<SessionConnectAction>,
    pub state: Option<SessionState>,
    pub actions: Vec<SessionAction>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl SessionConnectExplanation {
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
                SessionConnectIntent::Start => "Start this conversation",
                SessionConnectIntent::Resume => "Resume saved provider history",
                SessionConnectIntent::ConnectOrResume => {
                    "Connect to the recorded live conversation, or resume saved history"
                }
            };
            let mode = match action.mode {
                OpenMode::Refuse => "refuse an active native client",
                OpenMode::Replace => "replace Loopflow-owned clients; unsent text there is lost",
                OpenMode::Try => "ask the provider even if another client is active",
            };
            lines.push(format!(
                "Action: {intent}; {mode}{}",
                if action.prepare_only {
                    " (JSON preparation only; no client takeover)"
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

pub(crate) async fn explain_connect(
    store: &SharedStore,
    cwd: &Path,
    id: Option<&str>,
    mode: OpenMode,
    prepare_only: bool,
) -> SessionConnectExplanation {
    let selector = match super::select_connection(store, cwd, id).await {
        Ok(selector) => selector,
        Err(error) => return SessionConnectExplanation::unavailable(error),
    };
    let mut report = SessionConnectExplanation {
        resolution: crate::ops::context::explain_context(
            store,
            cwd,
            WorkSelection {
                task: None,
                wave: None,
            },
            Some(&selector),
            None,
        )
        .await,
        action: None,
        state: None,
        actions: Vec::new(),
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let ContextFact::Bound { source, .. } = &mut report.resolution.session {
        if id.is_none() {
            *source = "latest_interactive_in_checkout".into();
        }
    }
    if let Err(error) = read_connection(store, &selector, mode, prepare_only, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_connection(
    store: &SharedStore,
    selector: &str,
    mode: OpenMode,
    prepare_only: bool,
    report: &mut SessionConnectExplanation,
) -> Result<()> {
    let Some(session) = super::find_session(store, selector, true).await? else {
        report.unavailable.push(format!("Session {selector} is not recorded; provider-conversation admission is not performed by explanation"));
        return Ok(());
    };
    let native = NativeSession::of(&session)?;
    let provider = store.sqlite.input_provider_session(&session.artifact_key)?;
    // Do not probe a socket: even a connect-and-close enters the live driver's protocol.
    let connection = if native.provider == "codex" && provider.is_some() {
        store.sqlite.session_connection(&session.id)?
    } else {
        None
    };
    let intent = if provider.is_none() {
        SessionConnectIntent::Start
    } else if !prepare_only && connection.is_some() {
        SessionConnectIntent::ConnectOrResume
    } else {
        SessionConnectIntent::Resume
    };
    report.action = Some(SessionConnectAction {
        intent,
        mode,
        prepare_only,
    });
    if let Some((_, thread)) = connection {
        let matching = provider
            .as_ref()
            .is_some_and(|provider| provider.agent_session == thread);
        report.unavailable.push(if matching {
            "Recorded live endpoint is not probed; reachability and driver takeover are checked only when opening"
        } else {
            "Recorded live thread differs from saved history; a reachable endpoint would refuse connection, an absent endpoint may resume history"
        }.into());
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
        } else if session.completed_at.is_some() {
            report
                .impediments
                .push(format!("session {:?} is already complete", session.id));
        }
        report.unavailable.push(
            "Provider/account launch readiness is not inspected; no credentials are prepared"
                .into(),
        );
    }
    // Explanation needs no launch argv or UI workspace projection. Read clients
    // once so state, legal actions and refusal describe the same observation.
    let active = !native.clients()?.is_empty();
    let metadata = store
        .sqlite
        .session_summary(&session.id, report.resolution.observed_at)?
        .ok_or_else(|| super::session_not_found(&session.id))?;
    let state = super::session_state(&metadata, active);
    report.state = Some(state);
    report.actions = super::session_actions(state);
    // The live Codex connection precedes the native-client refusal in ordinary open.
    // With no socket observation, report the native fallback's refusal conditionally.
    if provider.is_some() {
        if let Err(error) = mode.require_action(active) {
            if intent == SessionConnectIntent::ConnectOrResume {
                report
                    .unavailable
                    .push(format!("If live connection is unavailable: {error}"));
            } else {
                report.impediments.push(error.to_string());
            }
        }
    }
    Ok(())
}
