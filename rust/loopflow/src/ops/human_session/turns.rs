//! Each live conversation's current turn: the last turn in its provider's
//! transcript. Raw conversations, provider processes open here that Loopflow
//! is not holding, list beside the Sessions with what their transcript shows.

use std::path::Path;

use anyhow::Result;

use super::{SessionAvailability, SessionGraphMembership, SessionRecord, SessionState};
use crate::durable::TaskId;
use crate::harness::HarnessKind;
use crate::session_record::SessionTitleSource;
use crate::store::sqlite::SqliteStore;
use crate::transcript::live::SessionTurn;

/// Read the current turn of each live Session among `sessions`. A Session
/// whose agent reports no status is available once its turn hands back.
pub(super) fn read(store: &SqliteStore, sessions: &mut [SessionRecord]) -> Result<()> {
    for session in sessions
        .iter_mut()
        .filter(|session| matches!(session.state, SessionState::Active | SessionState::Detached))
    {
        session.turn = current(store, session.id.as_str(), Path::new(&session.cwd))?;
        if session.availability.is_none()
            && handed_back(&session.turn)
            && !store.session_reports_status(session.id.as_str())?
        {
            session.availability = Some(SessionAvailability::Available);
        }
    }
    Ok(())
}

fn handed_back(turn: &Option<SessionTurn>) -> bool {
    turn.as_ref().is_some_and(SessionTurn::handed_back)
}

/// The transcript's last turn; ended where Loopflow recorded the end of the
/// same turn when the transcript shows none.
fn current(store: &SqliteStore, session: &str, cwd: &Path) -> Result<Option<SessionTurn>> {
    let Some(conversation) = store.current_conversation(session)? else {
        return Ok(None);
    };
    crate::transcript::live::locate(store, &conversation, cwd)?
        .and_then(|(harness, source)| crate::observe::transcript_turn(harness, &source))
        .map(|turn| close(store, session, &conversation.thread, turn))
        .transpose()
}

/// End a turn its transcript leaves open where Loopflow recorded the end of
/// the turn with the same id. A prompt Loopflow folded into an earlier turn
/// has no such record and reads as running until the transcript moves.
fn close(
    store: &SqliteStore,
    session: &str,
    thread: &crate::id::AgentSessionId,
    mut turn: SessionTurn,
) -> Result<SessionTurn> {
    if let (false, Some(id)) = (turn.handed_back(), &turn.id) {
        if let Some((at, status)) = store.recorded_turn_end(session, thread, id)? {
            turn.close(at, status.as_deref());
        }
    }
    Ok(turn)
}

/// Raw conversations open in `repo`, or anywhere when `None`. One a Session
/// recorded, resumed outside Loopflow after that Session ended, keeps the
/// Session's title.
pub(super) fn raw(store: &SqliteStore, repo: Option<&str>) -> Result<Vec<SessionRecord>> {
    let open = crate::observe::raw_conversations();
    let recorded = store.sessions_of_provider_threads(
        &open
            .iter()
            .map(|conversation| conversation.id.clone())
            .collect::<Vec<_>>(),
    )?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut records = Vec::new();
    for conversation in open {
        let mut title = home_relative(&conversation.cwd);
        let mut title_source = SessionTitleSource::Generated;
        if let Some(session) = recorded.get(&conversation.id) {
            match store.session_summary(session, now)? {
                // Its Session lists it.
                Some(session) if super::is_running(super::session_state(&session)) => continue,
                Some(session) if !session.title.is_empty() => {
                    title = session.title.clone();
                    title_source = match session.title_source {
                        crate::session::TitleSource::Human => SessionTitleSource::Human,
                        crate::session::TitleSource::Generated => SessionTitleSource::Generated,
                    };
                }
                _ => {}
            }
        }
        if let Some(repo) = repo {
            let here = crate::repository::CanonicalRepo::containing(&conversation.cwd)
                .ok()
                .flatten();
            if here.is_none_or(|here| here.to_string() != repo) {
                continue;
            }
        }
        let turn = crate::observe::transcript_turn(conversation.harness, &conversation.source);
        // A checkout two Tasks claim lists under neither.
        let task = crate::ops::task::counted_task(store, &conversation.cwd).unwrap_or(None);
        records.push(record(
            conversation.harness,
            &conversation.id,
            &conversation.cwd,
            (title, title_source),
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
    (title, title_source): (String, SessionTitleSource),
    turn: Option<SessionTurn>,
    task: Option<TaskId>,
) -> SessionRecord {
    let provider = harness.as_str().to_owned();
    SessionRecord {
        transition: None,
        preview: None,
        primary_scope: None,
        availability: handed_back(&turn).then_some(SessionAvailability::Available),
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
        title,
        title_source,
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

    fn turn(state: TurnState) -> Option<SessionTurn> {
        Some(SessionTurn {
            id: None,
            state,
            started_at: None,
            ended_at: None,
            model: None,
            usage: None,
        })
    }

    fn raw(turn: Option<SessionTurn>) -> SessionRecord {
        let title = ("~/work".to_owned(), SessionTitleSource::Generated);
        record(
            HarnessKind::Claude,
            "provider-id",
            Path::new("/work"),
            title,
            turn,
            None,
        )
    }

    #[test]
    fn a_raw_conversation_is_available_once_it_hands_back() {
        assert_eq!(raw(turn(TurnState::Running)).availability, None);
        for state in [TurnState::Ended, TurnState::Interrupted] {
            assert_eq!(
                raw(turn(state)).availability,
                Some(SessionAvailability::Available)
            );
        }
    }

    #[test]
    fn a_recorded_end_closes_only_the_open_turn_with_its_id() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let thread = crate::id::AgentSessionId::from("thread");
        let detail = serde_json::json!({"status":"interrupted"});
        store
            .record_turn_completion("conversation", &thread, "u1", 1_794_328_071, &detail)
            .unwrap();
        let open = |id: &str| {
            let mut turn = turn(TurnState::Running).unwrap();
            turn.id = Some(id.into());
            close(&store, "conversation", &thread, turn).unwrap()
        };
        let closed = open("u1");
        assert_eq!(closed.state, TurnState::Interrupted);
        assert_eq!(closed.ended_at.as_deref(), Some("2026-11-10T16:27:51Z"));
        assert_eq!(open("u2").state, TurnState::Running);

        // What the transcript says ended stays as the transcript says.
        let mut ended = turn(TurnState::Ended).unwrap();
        ended.id = Some("u1".into());
        let kept = close(&store, "conversation", &thread, ended.clone()).unwrap();
        assert_eq!(kept, ended);
    }

    #[test]
    fn the_card_for_a_raw_conversation_carries_its_turn() {
        use crate::lf::commands::screens::{SessionCard, SessionLiveness};
        let card = SessionCard::from(raw(turn(TurnState::Running)));
        assert!(card.session.raw);
        assert_eq!(card.session.id.as_str(), "provider-id");
        assert_eq!(card.session.turn.unwrap().state, TurnState::Running);
        assert_eq!(card.liveness, SessionLiveness::Live);
        assert_eq!(card.control_holder, None);
        assert_eq!(card.last_line, None);
    }
}
