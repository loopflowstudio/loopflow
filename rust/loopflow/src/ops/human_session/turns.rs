//! Each live conversation's current turn: the end of the provider's
//! transcript, combined with the turn Loopflow recorded from the provider's
//! live stream when there is one. Raw
//! conversations, open here but never launched by Loopflow, list beside the
//! Sessions with only what their transcript shows.

use std::path::Path;

use anyhow::Result;

use super::{SessionAvailability, SessionGraphMembership, SessionRecord, SessionState};
use crate::durable::TaskId;
use crate::harness::HarnessKind;
use crate::session_record::SessionTitleSource;
use crate::store::sqlite::SqliteStore;
use crate::transcript::live::{SessionTurn, TurnSource};

/// Read the current turn of each live Session among `sessions`. One whose
/// turn comes from its transcript has no status report to say it waits on a
/// person, so the transcript says: the agent handed back, or asked a question.
pub(super) fn read(store: &SqliteStore, sessions: &mut [SessionRecord]) -> Result<()> {
    for session in sessions
        .iter_mut()
        .filter(|session| matches!(session.state, SessionState::Active | SessionState::Detached))
    {
        session.turn = current(store, session.id.as_str(), Path::new(&session.cwd))?;
        if waits(&session.turn) {
            session.availability = Some(SessionAvailability::Available);
        }
    }
    Ok(())
}

fn waits(turn: &Option<SessionTurn>) -> bool {
    turn.as_ref()
        .is_some_and(|turn| turn.source == TurnSource::Transcript && turn.waits())
}

fn current(store: &SqliteStore, session: &str, cwd: &Path) -> Result<Option<SessionTurn>> {
    let Some(conversation) = store.current_conversation(session)? else {
        return Ok(None);
    };
    let run = crate::transcript::live::locate(store, &conversation, cwd)?
        .and_then(|(harness, path)| crate::observe::transcript_run(harness, &path));
    Ok(match &conversation.turn {
        Some(stream) => Some(crate::transcript::live::combine(
            stream,
            run,
            crate::store::rows::now_unix(),
        )),
        None => run.map(|run| run.turn),
    })
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
        let turn = crate::observe::transcript_run(conversation.harness, &conversation.transcript)
            .map(|run| run.turn);
        // A checkout two Tasks claim lists under neither.
        let task = crate::ops::task::counted_task(store, &conversation.cwd).unwrap_or(None);
        records.push(record(
            conversation.harness,
            &conversation.id,
            &conversation.cwd,
            turn,
            task,
        ));
    }
    Ok(records)
}

fn record(
    harness: HarnessKind,
    id: &str,
    cwd: &Path,
    turn: Option<SessionTurn>,
    task: Option<TaskId>,
) -> SessionRecord {
    let provider = harness.as_str().to_owned();
    SessionRecord {
        transition: None,
        preview: None,
        primary_scope: None,
        availability: waits(&turn).then_some(SessionAvailability::Available),
        program_status: None,
        agent_process_id: None,
        task_primary: false,
        task_ids: task.into_iter().collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::live::TurnState;

    fn turn(state: TurnState, source: TurnSource) -> Option<SessionTurn> {
        Some(SessionTurn {
            state,
            source,
            started_at: None,
            ended_at: None,
            model: None,
            usage: None,
        })
    }

    #[test]
    fn a_raw_conversation_waits_on_you_once_it_hands_back_or_asks() {
        let availability = |state| {
            let turn = turn(state, TurnSource::Transcript);
            record(HarnessKind::Claude, "id", Path::new("/work"), turn, None).availability
        };
        assert_eq!(availability(TurnState::Running), None);
        for state in [TurnState::Asking, TurnState::Ended, TurnState::Interrupted] {
            assert_eq!(availability(state), Some(SessionAvailability::Available));
        }
        // A turn lf recorded leaves that to the agent's own status reports.
        assert!(!waits(&turn(TurnState::Ended, TurnSource::Stream)));
    }

    #[test]
    fn the_card_for_a_raw_conversation_carries_its_turn() {
        use crate::lf::commands::screens::{SessionCard, SessionLiveness};
        let turn = turn(TurnState::Running, TurnSource::Transcript);
        let raw = record(
            HarnessKind::Claude,
            "provider-id",
            Path::new("/work"),
            turn,
            None,
        );
        let card = SessionCard::from(raw);
        assert!(card.session.raw);
        assert_eq!(card.session.id.as_str(), "provider-id");
        assert_eq!(card.session.turn.unwrap().state, TurnState::Running);
        assert_eq!(card.liveness, SessionLiveness::Live);
        assert_eq!(card.control_holder, None);
        assert_eq!(card.last_line, None);
    }
}
