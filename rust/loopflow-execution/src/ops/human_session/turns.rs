//! Each live conversation's current turn: the last turn in its provider's
//! transcript. Raw conversations, provider processes open here that Loopflow
//! is not holding, list beside the Sessions with what their transcript shows.

use crate::store::session_projection;
use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::{SessionAvailability, SessionGraphMembership, SessionRecord, SessionState};
use crate::session_record::SessionTitleSource;
use loopflow_harness::transcript::live::SessionTurn;
use loopflow_harness::HarnessKind;
use loopflow_model::durable::TaskId;
use loopflow_storage::sqlite::SqliteStore;

/// Read the current turn of each live Session among `sessions`. A Session
/// whose agent reports no status is available once its turn hands back.
pub fn read(store: &SqliteStore, sessions: &mut [SessionRecord]) -> Result<()> {
    for session in sessions
        .iter_mut()
        .filter(|session| matches!(session.state, SessionState::Active | SessionState::Detached))
    {
        current(store, session)?;
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
fn current(store: &SqliteStore, session: &mut SessionRecord) -> Result<()> {
    session.turn = None;
    let Some(conversation) = store.current_conversation(session.id.as_str())? else {
        return Ok(());
    };
    let Some((harness, source)) =
        crate::transcript::live::locate(store, &conversation, Path::new(&session.cwd))?
    else {
        return Ok(());
    };
    let Some(reading) = crate::observe::transcript_current(harness, &source) else {
        return Ok(());
    };
    if harness == HarnessKind::Codex {
        let current = reading.turn.id.as_deref();
        if session
            .preview
            .as_ref()
            .is_none_or(|preview| Some(preview.provider_turn.as_str()) != current)
        {
            session.preview = reading.response.and_then(|(text, observed_at)| {
                Some(crate::session::SessionPreview {
                    provider_thread: conversation.thread.clone(),
                    provider_turn: reading.turn.id.clone()?,
                    observed_at,
                    source: "codex_transcript".to_owned(),
                    text,
                })
            });
        }
    }
    session.turn = Some(close(
        store,
        session.id.as_str(),
        &conversation.thread,
        reading.turn,
    )?);
    Ok(())
}

/// End a turn its transcript leaves open where Loopflow recorded the end of
/// the turn with the same id. A prompt Loopflow folded into an earlier turn
/// has no such record and reads as running until the transcript moves.
fn close(
    store: &SqliteStore,
    session: &str,
    thread: &loopflow_model::id::AgentSessionId,
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
/// Session's title; one its Session wrote last is that Session's, not a raw
/// process's.
pub fn raw(store: &SqliteStore, repo: Option<&str>) -> Result<Vec<SessionRecord>> {
    let open = crate::observe::raw_conversations();
    let recorded = store.sessions_of_provider_threads(
        &open
            .iter()
            .flat_map(|directory| directory.ids())
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut summaries = HashMap::new();
    for (thread, session) in &recorded {
        if let Some(summary) = session_projection::session_summary(store, session, now)? {
            summaries.insert(thread.as_str(), summary);
        }
    }
    let running =
        |session: &crate::session::SessionSummary| super::is_running(super::session_state(session));
    // Written no later than its Session ended; one that stopped with no end
    // recorded, a failed launch, is taken for the Session's too.
    let launched = |thread: &str, written: i64| {
        summaries.get(thread).is_some_and(|session| {
            running(session) || session.completed_at.is_none_or(|ended| written <= ended)
        })
    };
    let mut records = Vec::new();
    for conversation in open
        .iter()
        .flat_map(|directory| directory.conversations(launched))
    {
        let mut title = home_relative(&conversation.cwd);
        let mut title_source = SessionTitleSource::Generated;
        if let Some(session) = summaries.get(conversation.id.as_str()) {
            match session {
                // Its Session lists it.
                session if running(session) => continue,
                session if !session.title.is_empty() => {
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
            let here = crate::repository::containing(&conversation.cwd)
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
        action: None,
        title,
        title_source,
        graph_membership: SessionGraphMembership::Independent,
        detail: provider.clone(),
        provider: Some(provider),
        account: None,
        model: turn.as_ref().and_then(|turn| turn.model.clone()),
        cwd: cwd.display().to_string(),
        state: SessionState::Active,
        open_argv: Vec::new(),
        terminal_ids: Vec::new(),
        turn,
        raw: true,
        ended: None,
        start: None,
        outlook: None,
        status_line: None,
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
    use loopflow_harness::transcript::live::TurnState;

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
    fn an_unavailable_transcript_does_not_retain_a_previous_turn() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let mut sessions = [raw(turn(TurnState::Running))];

        read(&store, &mut sessions).unwrap();

        assert_eq!(sessions[0].turn, None);
        assert_eq!(sessions[0].availability, None);
    }

    #[test]
    fn a_recorded_end_closes_only_the_open_turn_with_its_id() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let thread = loopflow_model::id::AgentSessionId::from("thread");
        let detail = serde_json::json!({"status":"interrupted"});
        store
            .record_answered_turn_end(
                session.captured.unwrap(),
                &thread,
                "u1",
                1_794_328_071,
                &detail,
            )
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
}
