//! `session.boundary`: what a managed launch's hook reports at a turn or tool
//! boundary, for the server to record. The hook opens no store for this; it
//! reads the Session's steers and Task update from the store itself afterwards
//! ([`loopflow_execution::steer::claim`]).

use loopflow_execution::process::attachment;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::server::wire::{BoundaryMoment, SessionBoundary, SessionBoundaryAnswer};
use loopflow_execution::context_block::ContextDelivery;
use loopflow_execution::steer::NativeBoundary;
use loopflow_execution::store::SharedStore;
use loopflow_storage::sqlite::SqliteStore;

/// How long a pending Session move waits after its Stop is answered: the hook
/// prints and exits, then the provider ends its turn.
const MOVE_AFTER_STOP: Duration = Duration::from_secs(2);
/// The longest a turn end's Codex history read may take.
const CODEX_HISTORY: Duration = Duration::from_secs(10);
/// The longest a turn end's planning refresh may take.
const PLANNING_REFRESH: Duration = Duration::from_secs(8);

/// What a boundary leaves for after its answer.
#[derive(Debug)]
pub(super) enum After {
    /// A turn ended: the Session's planning, its wake and any pending move.
    TurnEnd(TurnEnd),
    /// A new provider process, or one that named a new inbox: wake the
    /// Session with what was saved while it was closed or unreachable.
    Reachable(String),
}

#[derive(Debug)]
pub(super) struct TurnEnd {
    repo: PathBuf,
    session: Option<String>,
    /// A Codex Session's app-server and the conversation whose turn ended.
    codex: Option<(PathBuf, loopflow_model::id::AgentSessionId)>,
}

/// Record the boundary; blocking.
pub(super) fn answer(
    store: &SqliteStore,
    boundary: &SessionBoundary,
) -> Result<(SessionBoundaryAnswer, Option<After>)> {
    let SessionBoundary {
        moment,
        delivery: path,
        adapter,
        callback,
        gap,
        printed: _,
        inbox,
    } = boundary;
    let mut delivery = ContextDelivery::load(path)?;
    // A checkout moved since launch: the hook renders from the new place.
    let mut changed = delivery.relocate(store)?;
    if matches!(moment, BoundaryMoment::Start | BoundaryMoment::Compact) {
        if let Some(adapter) = adapter {
            changed |= delivery.identify(store, &adapter.session)?;
        }
    }
    if changed {
        delivery.save(path)?;
    }
    let nothing = SessionBoundaryAnswer {
        steers: None,
        update: None,
        waiting: false,
    };
    let Some(adapter) = adapter else {
        anyhow::ensure!(
            *moment == BoundaryMoment::Stop,
            "Only a turn end needs no provider context"
        );
        let after = After::TurnEnd(TurnEnd {
            repo: delivery.repo,
            session: None,
            codex: None,
        });
        return Ok((nothing, Some(after)));
    };
    // OpenCode's turn end names no conversation: it records no turn, and
    // still confirms, refreshes planning and applies a pending move. A Pi
    // event names its conversation by its session file.
    let thread = match moment {
        BoundaryMoment::Event => callback["sessionFile"].as_str(),
        _ => callback["session_id"].as_str(),
    };
    let no_thread = || anyhow!("Provider callback names no native conversation");
    let event = callback["hook_event_name"]
        .as_str()
        .unwrap_or("SessionStart");
    // A reopened conversation is idle until its first turn says otherwise;
    // recorded before this boundary's own turn reading, which then wins.
    let first = store.record_first_boundary(adapter)?;
    let yielded = match (moment, event) {
        (BoundaryMoment::Stop, "Stop") => Some(true),
        (BoundaryMoment::Tool, "UserPromptSubmit") => Some(false),
        _ => None,
    };
    if let Some(yielded) = yielded {
        // Claude's Stop snapshot includes work that will wake the conversation
        // again. A yielded reply cannot settle a watch or a scheduled wakeup.
        let open_tools = callback["background_tasks"]
            .as_array()
            .zip(callback["session_crons"].as_array())
            .map(|(tasks, crons)| tasks.len() + crons.len());
        let thread = thread.ok_or_else(no_thread)?;
        store.record_native_turn(adapter, thread, yielded, open_tools)?;
    }
    if let Some(gap) = gap {
        store.record_recording_gap(&adapter.session, gap.first, gap.last)?;
    }
    let connection = attachment::session_connection(store, &adapter.session)?;
    // Where the server wakes this conversation once it is idle. A new
    // process, or one it could not reach before, may hold steers saved while
    // it was closed: the wake runs them as a turn.
    let addressed = match inbox {
        Some(inbox) => store.record_agent_inbox(adapter, inbox)?,
        None => false,
    };
    let reachable = (first || addressed).then(|| After::Reachable(adapter.session.clone()));
    if *moment == BoundaryMoment::Event {
        loopflow_execution::harness::pi::observe_native(store, adapter, callback)?;
        return Ok((nothing, reachable));
    }
    if *moment == BoundaryMoment::Stop {
        // The transcript is the only writer of usage rows; the exit read
        // catches turns it does not show yet. Pi and OpenCode have no hook name.
        loopflow_execution::transcript::facts::record_session(
            store,
            &adapter.session,
            loopflow_execution::transcript::facts::Moment::TurnEnd,
        );
        // Codex keeps its turns on its app-server; read them now that one
        // ended. A Claude Session records none.
        let codex = connection
            .zip(thread)
            .map(|((endpoint, _), thread)| (PathBuf::from(endpoint), thread.into()));
        let after = After::TurnEnd(TurnEnd {
            repo: delivery.repo,
            session: Some(adapter.session.clone()),
            codex,
        });
        return Ok((nothing, Some(after)));
    }
    // A provider start declares the conversation it now runs; every other
    // boundary must name the selected one.
    let thread = thread.ok_or_else(no_thread)?;
    let selects = match event {
        "SessionStart" => NativeBoundary::Select {
            source: callback["source"].as_str().unwrap_or("startup"),
        },
        "OpenCodeMessagesTransform" | "PiContext" => NativeBoundary::Activate { source: event },
        _ => NativeBoundary::Turn,
    };
    store.with_provider_context(adapter, thread, selects, || Ok(()))?;
    Ok((nothing, reachable))
}

/// Run what a boundary left, after its answer. Blocking; failures are logged,
/// since no one waits for them.
pub(super) fn after(after: After) {
    let session = match &after {
        After::TurnEnd(ended) => ended.session.clone(),
        After::Reachable(session) => Some(session.clone()),
    };
    let ran = loopflow_execution::runtime::block_on(async {
        let Some(store) = loopflow_execution::store::open_existing_store().await? else {
            return Ok(());
        };
        match after {
            After::TurnEnd(ended) => turn_end(std::sync::Arc::new(store), ended).await,
            After::Reachable(session) => wake(&store.sqlite, session).await,
        }
    });
    if let Err(error) = ran {
        tracing::warn!(?session, error = %format!("{error:#}"), "boundary follow-up failed");
    }
}

/// Record a Codex Session's turns, refresh the Session's planning, wake it
/// with any Task update its turn missed, then continue it on a pending move's
/// agent.
async fn turn_end(store: SharedStore, ended: TurnEnd) -> Result<()> {
    if let (Some(session), Some((endpoint, thread))) = (&ended.session, &ended.codex) {
        record_codex_turns(&store.sqlite, session, thread, endpoint).await;
    }
    let repo = ended.repo.to_string_lossy();
    if tokio::time::timeout(
        PLANNING_REFRESH,
        loopflow_execution::ops::tracker_events::sync_repository(&store, &repo),
    )
    .await
    .is_err()
    {
        tracing::warn!(repo = %repo, "planning refresh outlasted its turn-end budget");
    }
    let Some(session) = ended.session else {
        return Ok(());
    };
    // The turn ended: an update that arrived during it goes out now.
    wake(&store.sqlite, session.clone()).await?;
    if store.sqlite.pending_session_move(&session)?.is_none() {
        return Ok(());
    }
    tokio::time::sleep(MOVE_AFTER_STOP).await;
    loopflow_execution::ops::human_session::moves_bulk::apply_pending_move(&store, &session).await
}

/// Wake `session` if it is idle and has news.
async fn wake(store: &SqliteStore, session: String) -> Result<()> {
    let store = store.clone();
    loopflow_execution::journal::request::spawn_blocking(move || {
        super::wake::after_turn(&store, &session)
    })
    .await?;
    Ok(())
}

/// Read a Codex conversation's turns from its app-server into the Session's
/// history. A failure is a recording gap the next turn end or launch fills.
async fn record_codex_turns(
    store: &SqliteStore,
    session: &str,
    thread: &loopflow_model::id::AgentSessionId,
    endpoint: &std::path::Path,
) {
    let read = tokio::time::timeout(
        CODEX_HISTORY,
        loopflow_execution::harness::codex_connection::recover_history(
            store, session, thread, endpoint,
        ),
    )
    .await
    .map_err(|_| anyhow!("Codex history read timed out"))
    .and_then(|read| read);
    if let Err(error) = read {
        tracing::warn!(session, error = %format!("{error:#}"), "Codex turns unrecorded at turn end");
    }
}

#[cfg(test)]
mod tests {
    use loopflow_execution::process::attachment;

    use super::{answer, After, TurnEnd};
    use crate::server::wire::{BoundaryGap, BoundaryMoment, SessionBoundary};
    use loopflow_execution::context_block::ContextDelivery;
    use loopflow_execution::steer::{NativeBoundary, SteerAdapter};
    use loopflow_storage::sqlite::SqliteStore;

    /// The turn end a boundary left.
    fn ended(after: Option<After>) -> TurnEnd {
        match after {
            Some(After::TurnEnd(ended)) => ended,
            other => panic!("no turn end: {other:?}"),
        }
    }

    /// A native Session whose provider selected conversation `native`, and a
    /// delivery file for its checkout.
    fn fixture() -> (
        tempfile::TempDir,
        SqliteStore,
        SteerAdapter,
        std::path::PathBuf,
    ) {
        fixture_on("native")
    }

    fn fixture_on(
        thread: &str,
    ) -> (
        tempfile::TempDir,
        SqliteStore,
        SteerAdapter,
        std::path::PathBuf,
    ) {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        store.test_session(
            "session",
            &loopflow_execution::session_record::new_artifact_key(),
        );
        let process = store.test_lf_process();
        let claim =
            attachment::claim_session_attachment(&store, "session", None, &process).unwrap();
        store
            .record_agent_process_identity("session", &claim, std::process::id(), 1)
            .unwrap();
        let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_interactive=1 WHERE agent_process_id=?1",
                [&adapter.agent_process_id],
            )
            .unwrap();
        store
            .with_provider_context(
                &adapter,
                thread,
                NativeBoundary::Activate { source: "startup" },
                || Ok(()),
            )
            .unwrap();
        let repo = home.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let delivery = home.path().join("delivery.json");
        ContextDelivery {
            identity: None,
            repository: None,
            repo,
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
            home: home.path().to_owned(),
        }
        .save(&delivery)
        .unwrap();
        (home, store, adapter, delivery)
    }

    fn boundary(
        moment: BoundaryMoment,
        adapter: &SteerAdapter,
        delivery: &std::path::Path,
        callback: serde_json::Value,
    ) -> SessionBoundary {
        SessionBoundary {
            moment,
            delivery: delivery.to_owned(),
            adapter: Some(adapter.clone()),
            callback,
            gap: None,
            printed: Vec::new(),
            inbox: None,
        }
    }

    #[test]
    fn start_and_compact_refresh_session_and_task_identity() {
        let (_home, store, adapter, delivery) = fixture();
        let render = |moment, source| {
            let request = boundary(
                moment,
                &adapter,
                &delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":source}),
            );
            answer(&store, &request).unwrap();
            ContextDelivery::load(&delivery).unwrap()
        };
        let start = render(BoundaryMoment::Start, "startup");
        let identity = start.identity.unwrap();
        assert_eq!(identity.session, "session");
        assert_eq!(identity.task, None);

        let task = store
            .create_task(&loopflow_storage::work::task::NewTask {
                parent_task_id: None,
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    &store, "/repo",
                )
                .unwrap(),
                due_date: None,
                id: loopflow_model::durable::TaskId::new(),
                project_id: None,
                title: "Keep \"$native\" <text>".into(),
                description: String::new(),
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET task_id=?1 WHERE id='session'",
                [task.id.as_str()],
            )
            .unwrap();
        let compact = render(BoundaryMoment::Compact, "compact");
        assert_eq!(
            compact.identity.as_ref().unwrap().task,
            Some((task.identifier.clone(), task.title.clone()))
        );
        let text = compact
            .block_with_prefix(
                loopflow_execution::context_block::ContextMoment::Compact,
                "",
            )
            .unwrap()
            .text;
        assert!(
            text.starts_with(&loopflow_execution::prompt::render_reference(&format!(
                "This Session: session\nThis Session's Task: {} {}\n",
                task.identifier,
                serde_json::to_string(&task.title).unwrap(),
            ))),
            "{text}"
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET issue_title='Renamed Task' WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        assert_eq!(
            render(BoundaryMoment::Compact, "compact")
                .identity
                .unwrap()
                .task,
            Some((task.identifier, "Renamed Task".into()))
        );
    }

    /// A checkout that moved since launch: the server points the delivery
    /// file at where the store now places the repository, and the hook
    /// renders the saved skill from there.
    #[test]
    fn a_moved_checkouts_delivery_follows_the_repository() {
        let (home, store, adapter, delivery) = fixture();
        let old = home.path().join("repo");
        let id = loopflow_execution::repository::location::ensure_repository(
            &store,
            old.to_str().unwrap(),
        )
        .unwrap();
        let mut saved = ContextDelivery::load(&delivery).unwrap();
        saved.repository = Some((id.clone(), old.clone()));
        saved.skill_file = Some(old.join("active.md"));
        saved.save(&delivery).unwrap();
        let moved = home.path().join("moved");
        std::fs::rename(&old, &moved).unwrap();
        store
            .relocate_repository(&id, moved.to_str().unwrap())
            .unwrap();
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"moved"}),
        );
        answer(&store, &tool).unwrap();
        let followed = ContextDelivery::load(&delivery).unwrap();
        assert_eq!(followed.repo, moved);
        assert_eq!(followed.skill_file, Some(moved.join("active.md")));
    }

    #[test]
    fn a_foreign_conversation_is_refused() {
        let (_home, store, adapter, delivery) = fixture();
        let foreign = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"child","hook_event_name":"PostToolUse","tool_use_id":"t"}),
        );
        assert!(answer(&store, &foreign).is_err());
    }

    /// The provider that ran this Session before was stopped mid-turn.
    fn stopped_mid_turn(store: &SqliteStore, adapter: &SteerAdapter, delivery: &std::path::Path) {
        let prompt = boundary(
            BoundaryMoment::Tool,
            adapter,
            delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit"}),
        );
        answer(store, &prompt).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE session_activity SET agent_process_id='earlier'", [])
            .unwrap();
    }

    fn activity(store: &SqliteStore) -> (bool, String) {
        store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT yielded,agent_process_id FROM session_activity WHERE session_id='session'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    #[test]
    fn a_reopened_conversation_is_idle_at_its_first_boundary_and_woken_once() {
        let (_home, store, adapter, delivery) = fixture();
        stopped_mid_turn(&store, &adapter, &delivery);
        // A reopened Codex or Claude conversation starts before anyone types,
        // naming no inbox.
        let start = boundary(
            BoundaryMoment::Start,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"resume"}),
        );
        let (_, after) = answer(&store, &start).unwrap();
        assert!(
            matches!(&after, Some(After::Reachable(session)) if session == "session"),
            "{after:?}"
        );
        assert_eq!(
            activity(&store),
            (true, adapter.agent_process_id.to_string())
        );
        // Its next boundary is not its first.
        assert!(answer(&store, &start).unwrap().1.is_none());
        // An inbox named late is new: the wake runs again, and claims nothing twice.
        let mut listening = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"OpenCodeMessagesTransform"}),
        );
        listening.inbox = Some(loopflow_execution::steer::Inbox::Socket(
            "/tmp/lf-wake-fixture/s".into(),
        ));
        assert!(matches!(
            answer(&store, &listening).unwrap().1,
            Some(After::Reachable(_))
        ));
        // The same socket again is nothing new.
        assert!(answer(&store, &listening).unwrap().1.is_none());
        assert_eq!(
            store
                .idle_conversation_candidates(Some("session"))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn a_first_boundary_that_is_a_prompt_leaves_the_conversation_working() {
        let (_home, store, adapter, delivery) = fixture();
        stopped_mid_turn(&store, &adapter, &delivery);
        let mut prompt = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit"}),
        );
        prompt.inbox = Some(loopflow_execution::steer::Inbox::Socket(
            "/tmp/lf-wake-fixture/s".into(),
        ));
        answer(&store, &prompt).unwrap();
        assert_eq!(
            activity(&store),
            (false, adapter.agent_process_id.to_string())
        );
        assert!(store
            .idle_conversation_candidates(Some("session"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_reopened_work_session_is_idle_after_its_terminal_status_moves_first() {
        let (_home, store, adapter, delivery) = fixture();
        // The provider before it failed its turn, which reports no turn end.
        stopped_mid_turn(&store, &adapter, &delivery);
        // The held terminal's status stream reaches the new provider first.
        assert!(store
            .begin_program_status("session", Some(&adapter.agent_process_id), "held:terminal")
            .unwrap());
        let start = boundary(
            BoundaryMoment::Start,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"resume"}),
        );
        assert!(answer(&store, &start).unwrap().1.is_some());
        assert_eq!(
            activity(&store),
            (true, adapter.agent_process_id.to_string())
        );
    }

    #[test]
    fn a_turn_end_records_the_yield_and_the_hooks_gap_then_leaves_work_for_after() {
        let (_home, store, adapter, delivery) = fixture();
        store
            .save_session_steer("session", "Fixture author", "NEXT_TURN")
            .unwrap();
        let prompt = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit",
                "prompt_id":"p-1","prompt":"go"}),
        );
        answer(&store, &prompt).unwrap();
        let mut stop = boundary(
            BoundaryMoment::Stop,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"Stop","prompt_id":"p-1"}),
        );
        stop.gap = Some(BoundaryGap {
            first: 1_794_358_000,
            last: 1_794_358_020,
        });
        let (answered, after) = answer(&store, &stop).unwrap();
        // A turn end delivers nothing; the steer waits for the next boundary.
        assert_eq!(answered.steers, None);
        assert_eq!(ended(after).session.as_deref(), Some("session"));
        let conn = store.conn.lock().unwrap();
        let yielded: bool = conn
            .query_row(
                "SELECT yielded FROM session_activity WHERE session_id='session'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(yielded);
        let gaps: i64 = conn
            .query_row(
                "SELECT count(*) FROM history.session_history WHERE session_id='session'
                 AND json_extract(payload,'$.source')='store-gap'
                 AND json_extract(payload,'$.first_miss')=1794358000",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(gaps, 1);
    }

    /// A Stop whose snapshot still names a background watch or a scheduled
    /// wakeup keeps the turn holding its terminal, and so does a Stop that names
    /// neither list; only an empty snapshot settles it.
    #[test]
    fn a_turn_end_settles_only_once_no_watch_or_wakeup_remains() {
        use loopflow_storage::sqlite::sessions::TerminalRetention::LiveTurn;
        let (_home, store, adapter, delivery) = fixture();
        let terminal = loopflow_model::id::TerminalId::new();
        store.create_terminal(&terminal, "build").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1,held_terminal_id=?1",
                [&terminal],
            )
            .unwrap();
        let hook = |moment, callback: serde_json::Value| {
            answer(&store, &boundary(moment, &adapter, &delivery, callback)).unwrap();
            store.terminal_retention(&terminal).unwrap()
        };
        let stop = |tasks: serde_json::Value, crons: serde_json::Value| {
            let mut callback = serde_json::json!({"session_id":"native","hook_event_name":"Stop"});
            if !tasks.is_null() {
                callback["background_tasks"] = tasks;
                callback["session_crons"] = crons;
            }
            hook(BoundaryMoment::Stop, callback)
        };
        let prompt =
            serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit"});
        assert_eq!(hook(BoundaryMoment::Tool, prompt.clone()), Some(LiveTurn));
        assert_eq!(stop(serde_json::Value::Null, serde_json::Value::Null), None);
        // A second prompt starts a turn with nothing new to deliver.
        assert_eq!(hook(BoundaryMoment::Tool, prompt), Some(LiveTurn));
        let watch = serde_json::json!([{"id":"watch","type":"shell","status":"running"}]);
        assert_eq!(stop(watch, serde_json::json!([])), Some(LiveTurn));
        assert_eq!(
            stop(serde_json::Value::Null, serde_json::Value::Null),
            Some(LiveTurn),
            "a Stop without the lists cannot settle a watch"
        );
        let cron = serde_json::json!([{"id":"poll","schedule":"* * * * *","recurring":true}]);
        assert_eq!(stop(serde_json::json!([]), cron), Some(LiveTurn));
        assert_eq!(stop(serde_json::json!([]), serde_json::json!([])), None);
    }

    #[test]
    fn a_turn_end_records_transcript_usage_once_without_a_hook_name() {
        let (home, store, adapter, delivery) = fixture();
        let path = home.path().join("pi.jsonl");
        let entries = [
            serde_json::json!({"type":"session","version":3,"id":"s","cwd":"/work"}),
            serde_json::json!({"type":"message","id":"u1","parentId":"s","timestamp":"2026-11-10T16:25:00Z",
                "message":{"role":"user","content":"List files"}}),
            serde_json::json!({"type":"message","id":"a1","parentId":"u1","timestamp":"2026-11-10T16:25:09Z",
                "message":{"role":"assistant","model":"gemini-4-argon","stopReason":"stop",
                    "usage":{"input":10,"output":5,"cacheRead":90,"cacheWrite":0,"cost":{"total":0.25}},
                    "content":[{"type":"text","text":"Two files."}]}}),
        ];
        std::fs::write(
            &path,
            entries
                .iter()
                .map(|row| format!("{row}\n"))
                .collect::<String>(),
        )
        .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='pi',provider_thread=?1 WHERE id='session'",
                [path.to_str().unwrap()],
            )
            .unwrap();
        let stop = boundary(
            BoundaryMoment::Stop,
            &adapter,
            &delivery,
            serde_json::json!({}),
        );
        answer(&store, &stop).unwrap();
        answer(&store, &stop).unwrap();
        let usage = store
            .session_turns("session")
            .unwrap()
            .into_iter()
            .filter_map(|turn| Some((turn.turn, turn.usage?.usage)))
            .collect::<Vec<_>>();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].0, "u1");
        assert_eq!(usage[0].1.total_input_tokens, Some(100));
        assert_eq!(usage[0].1.output_tokens, Some(5));
        assert_eq!(usage[0].1.cost_usd, Some(0.25));
    }

    /// OpenCode reports a managed turn end with no callback body.
    #[test]
    fn a_turn_end_naming_no_conversation_leaves_work_for_after() {
        let (_home, store, adapter, delivery) = fixture();
        let stop = boundary(
            BoundaryMoment::Stop,
            &adapter,
            &delivery,
            serde_json::json!({}),
        );
        let (_, after) = answer(&store, &stop).unwrap();
        assert_eq!(ended(after).session.as_deref(), Some("session"));
        let bare_tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({}),
        );
        assert!(answer(&store, &bare_tool).is_err());
    }

    #[test]
    fn a_conversation_lf_did_not_start_still_refreshes_planning_at_its_turn_end() {
        let (_home, store, _adapter, delivery) = fixture();
        let stop = |moment| SessionBoundary {
            moment,
            delivery: delivery.clone(),
            adapter: None,
            callback: serde_json::json!({"session_id":"codex-thread","hook_event_name":"Stop"}),
            gap: None,
            printed: Vec::new(),
            inbox: None,
        };
        let (answered, after) = answer(&store, &stop(BoundaryMoment::Stop)).unwrap();
        assert_eq!(answered.steers, None);
        let after = ended(after);
        assert!(after.repo.ends_with("repo") && after.session.is_none());
        assert!(answer(&store, &stop(BoundaryMoment::Tool)).is_err());
    }
    /// Every statement a boundary runs looks its rows up by index: a scan of
    /// a table that grows with every steer, turn or Task slows each turn of
    /// every Session on the Machine.
    #[test]
    fn boundary_statements_never_scan_tables_that_grow_with_use() {
        let (_home, store, adapter, delivery) = fixture();
        // A Machine's worth of steers for other Sessions. Each steer has one
        // owner, so its other owner columns are NULL.
        for other in 0..20 {
            store.test_session(
                &format!("other-{other}"),
                &loopflow_execution::session_record::new_artifact_key(),
            );
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "INSERT INTO steers(id,session_id,source,revision,author,text,created_at)
                 WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<2999)
                 SELECT lower(hex(randomblob(16))),'other-'||(i%20),'fixture:'||i,'1',
                     '{\"kind\":\"named\",\"id\":\"Fixture author\"}','OTHER',unixepoch() FROM n",
            )
            .unwrap();
        store
            .save_session_steer("session", "Fixture author", "TRACED")
            .unwrap();
        let traced = loopflow_storage::query_plans::assert_no_empty_writes(&store, || {
            loopflow_storage::query_plans::statements(&store, || {
                let at = |moment, callback: serde_json::Value| {
                    boundary(moment, &adapter, &delivery, callback)
                };
                // What a managed launch's hooks do at each boundary: report it,
                // then claim the Session's steers themselves.
                answer(
                &store,
                &at(
                    BoundaryMoment::Start,
                    serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"resume"}),
                ),
            )
            .unwrap();
                let claimed = loopflow_execution::steer::claim(
                    &store,
                    &ContextDelivery::load(&delivery).unwrap().repo,
                    &adapter,
                    "native",
                    NativeBoundary::Select { source: "resume" },
                )
                .unwrap()
                .unwrap();
                loopflow_execution::steer::delivered(&store, "session", &claimed);
                answer(
                &store,
                &at(
                    BoundaryMoment::Tool,
                    serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit",
                    "prompt_id":"p-1","prompt":"go"}),
                ),
            )
            .unwrap();
                let quiet = NativeBoundary::Activate {
                    source: "PostToolUse",
                };
                assert!(loopflow_execution::steer::claim(
                    &store, &delivery, &adapter, "native", quiet
                )
                .unwrap()
                .is_none());
                answer(
            &store,
            &at(
                BoundaryMoment::Compact,
                serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"compact"}),
            ),
        )
        .unwrap();
                answer(
            &store,
            &at(
                BoundaryMoment::Tool,
                serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t"}),
            ),
        )
        .unwrap();
                answer(
            &store,
            &at(
                BoundaryMoment::Stop,
                serde_json::json!({"session_id":"native","hook_event_name":"Stop","prompt_id":"p-1"}),
            ),
        )
        .unwrap();
            })
        });
        let found = loopflow_storage::query_plans::growing_scans(&store, &traced);
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    /// Another connection holding the Machine's write lock; `store` gives up
    /// waiting for it after 50ms.
    fn hold_write_lock(home: &tempfile::TempDir, store: &SqliteStore) -> rusqlite::Connection {
        store
            .conn
            .lock()
            .unwrap()
            .busy_timeout(std::time::Duration::from_millis(50))
            .unwrap();
        let writer = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        writer
    }

    #[test]
    fn a_boundary_with_nothing_to_settle_does_not_wait_for_another_sessions_write() {
        let (home, store, adapter, delivery) = fixture();
        // A process's first boundary writes; this one is not its first.
        assert!(store.record_first_boundary(&adapter).unwrap());
        let writer = hold_write_lock(&home, &store);
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t1"}),
        );
        answer(&store, &tool).unwrap();
        // Nor does a hook's steer check with nothing to claim.
        let quiet = NativeBoundary::Activate {
            source: "PostToolUse",
        };
        assert!(
            loopflow_execution::steer::claim(&store, &delivery, &adapter, "native", quiet)
                .unwrap()
                .is_none()
        );
        writer.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn a_workers_boundaries_do_not_wait_for_another_sessions_write() {
        let (home, store, adapter, delivery) = fixture();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id=?1",
                [&adapter.session],
            )
            .unwrap();
        let writer = hold_write_lock(&home, &store);
        // A worker reports through the stream lf owns: no boundary is its
        // first, so none writes its reading.
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t1"}),
        );
        assert!(answer(&store, &tool).unwrap().1.is_none());
        writer.execute_batch("ROLLBACK").unwrap();
    }
}
