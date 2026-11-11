//! Each live conversation's current turn: from Loopflow's hooks when they
//! recorded one, else from the end of the provider's transcript. Raw
//! conversations, open here but never launched by Loopflow, list beside the
//! Sessions with only what their transcript shows.

use std::path::Path;

use anyhow::Result;

use super::{SessionGraphMembership, SessionRecord, SessionState};
use crate::harness::HarnessKind;
use crate::session_record::SessionTitleSource;
use crate::store::sqlite::SqliteStore;
use crate::transcript::live::SessionTurn;

/// Read the current turn of each live Session among `sessions`.
pub(super) fn read(store: &SqliteStore, sessions: &mut [SessionRecord]) -> Result<()> {
    for session in sessions
        .iter_mut()
        .filter(|session| matches!(session.state, SessionState::Active | SessionState::Detached))
    {
        session.turn = current(store, session.id.as_str(), Path::new(&session.cwd))?;
    }
    Ok(())
}

fn current(store: &SqliteStore, session: &str, cwd: &Path) -> Result<Option<SessionTurn>> {
    let Some(conversation) = store.current_conversation(session)? else {
        return Ok(None);
    };
    if let Some(turn) = &conversation.turn {
        return Ok(Some(SessionTurn::from_hooks(
            turn.started_at,
            turn.settled_at,
            turn.status.as_deref(),
        )));
    }
    let Some(harness) = HarnessKind::parse(&conversation.provider) else {
        return Ok(None);
    };
    let Some(provider) = harness.provider() else {
        return Ok(None);
    };
    let id = crate::id::AgentSessionId::from(conversation.thread.as_str());
    let Some(home) = crate::transcript::conversation_home(store, provider, &id)? else {
        return Ok(None);
    };
    Ok(
        crate::transcript::live::find(harness, &home, &conversation.thread, cwd)
            .and_then(|path| crate::observe::transcript_turn(harness, &path)),
    )
}

/// Raw conversations open in `repo`, or anywhere when `None`.
pub(super) fn raw(store: &SqliteStore, repo: Option<&str>) -> Result<Vec<SessionRecord>> {
    let open = crate::observe::raw_conversations();
    let recorded = store.recorded_provider_threads(
        &open
            .iter()
            .map(|conversation| conversation.id.clone())
            .collect::<Vec<_>>(),
    )?;
    let mut records = Vec::new();
    for conversation in open {
        if recorded.contains(&conversation.id) {
            continue;
        }
        if let Some(repo) = repo {
            let here = crate::repository::CanonicalRepo::containing(&conversation.cwd)
                .ok()
                .flatten();
            if here.is_none_or(|here| here.to_string() != repo) {
                continue;
            }
        }
        let turn = crate::observe::transcript_turn(conversation.harness, &conversation.transcript);
        records.push(record(
            conversation.harness,
            &conversation.id,
            &conversation.cwd,
            turn,
        ));
    }
    Ok(records)
}

fn record(harness: HarnessKind, id: &str, cwd: &Path, turn: Option<SessionTurn>) -> SessionRecord {
    let provider = harness.as_str().to_owned();
    SessionRecord {
        transition: None,
        preview: None,
        primary_scope: None,
        availability: None,
        program_status: None,
        agent_process_id: None,
        task_primary: false,
        task_ids: Vec::new(),
        id: id.into(),
        workspace: None,
        interactive: true,
        work: None,
        wave_id: None,
        work_path: None,
        actions: Vec::new(),
        title: home_relative(cwd),
        title_source: SessionTitleSource::Generated,
        graph_membership: SessionGraphMembership::Independent,
        detail: provider.clone(),
        provider: Some(provider),
        account: None,
        model: turn.as_ref().and_then(|turn| turn.model.clone()),
        blocked: false,
        cwd: cwd.display().to_string(),
        state: SessionState::Active,
        ready_summary: None,
        open_argv: Vec::new(),
        terminal_ids: Vec::new(),
        turn,
        raw: true,
    }
}

fn home_relative(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
