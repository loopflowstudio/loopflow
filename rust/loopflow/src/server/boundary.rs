//! `session.boundary`: the store work a provider hook asks for at a turn or tool
//! boundary. The hook opens nothing; the server records the boundary and
//! answers the Session's pending steers, which count as delivered once the
//! hook's next request says it printed them. Task changes since the Session's
//! last update ride the same answer, so it hears them at its next turn.
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::context_block::ContextDelivery;
use crate::server::wire::{BoundaryMoment, SessionBoundary, SessionBoundaryAnswer, SteerBatch};
use crate::steer::{NativeBoundary, SteerAdapter, SteerContext};
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

/// How long a hook waits for the server before its turn goes on without it.
/// An answer's steers the hook has not confirmed after this long are answered
/// again.
pub(crate) const HOOK_DEADLINE: Duration = Duration::from_secs(2);
/// How long a pending Session move waits after its Stop is answered: the hook
/// prints and exits, then the provider ends its turn.
const MOVE_AFTER_STOP: Duration = Duration::from_secs(2);
/// The longest a turn end's Codex history read may take.
const CODEX_HISTORY: Duration = Duration::from_secs(10);
/// The longest a turn end's planning refresh may take.
const PLANNING_REFRESH: Duration = Duration::from_secs(8);

/// What a `stop` leaves for after its answer: the Session's planning and any
/// pending move.
#[derive(Debug)]
pub(super) struct AfterStop {
    repo: PathBuf,
    session: Option<String>,
    /// A Codex Session's app-server and the conversation whose turn ended.
    codex: Option<(PathBuf, crate::id::AgentSessionId)>,
}

/// Record the boundary and claim its steers; blocking.
pub(super) fn answer(
    store: &SqliteStore,
    boundary: &SessionBoundary,
) -> Result<(SessionBoundaryAnswer, Option<AfterStop>)> {
    let SessionBoundary {
        moment,
        delivery: path,
        adapter,
        callback,
        gap,
        printed,
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
        let after = AfterStop {
            repo: delivery.repo,
            session: None,
            codex: None,
        };
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
    let connection = store.session_connection(&adapter.session)?;
    // Where the server wakes this conversation once it is idle.
    if let Some(inbox) = inbox {
        store.record_agent_inbox(adapter, inbox)?;
    }
    if *moment == BoundaryMoment::Event {
        crate::harness::pi::observe_native(store, adapter, callback)?;
        // Pi takes a steer only while a turn runs; outside one it would be
        // stored without starting a run, so the event answers none.
        let running = callback["turn"].is_string() && callback["type"] != "agent_settled";
        if !running {
            let waiting =
                store.confirm_session_steers(adapter, printed, HOOK_DEADLINE.as_secs())?;
            return Ok((
                SessionBoundaryAnswer {
                    steers: None,
                    update: None,
                    waiting,
                },
                None,
            ));
        }
    }
    if *moment == BoundaryMoment::Stop {
        // The transcript is the only writer of usage rows; the exit read
        // catches turns it does not show yet. Pi and OpenCode have no hook name.
        crate::transcript::facts::record_session(
            store,
            &adapter.session,
            crate::transcript::facts::Moment::TurnEnd,
        );
        // A turn end delivers nothing, so what it returns to pending waits
        // for the next prompt, whose hook always asks.
        store.confirm_session_steers(adapter, printed, HOOK_DEADLINE.as_secs())?;
        // Codex keeps its turns on its app-server; read them now that one
        // ended. A Claude Session records none.
        let codex = connection
            .zip(thread)
            .map(|((endpoint, _), thread)| (PathBuf::from(endpoint), thread.into()));
        let after = AfterStop {
            repo: delivery.repo,
            session: Some(adapter.session.clone()),
            codex,
        };
        return Ok((nothing, Some(after)));
    }
    let thread = thread.ok_or_else(no_thread)?;
    let (steers, waiting) =
        claim_steers(store, &delivery, adapter, thread, callback, event, printed)?;
    let update = crate::harness::task_updates::claim(store, adapter, thread).unwrap_or_else(|error| {
        tracing::warn!(session = adapter.session, %error, "Task update unread at this boundary");
        None
    });
    Ok((
        SessionBoundaryAnswer {
            steers,
            update,
            waiting,
        },
        None,
    ))
}

/// After a Stop's answer: record a Codex Session's turns, refresh the Session's
/// planning, wake it with any Task update its turn missed, then continue it on
/// a pending move's agent.
/// Blocking; failures are logged, since no one waits for them.
pub(super) fn after_stop(after: AfterStop) {
    let ran = (|| -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let Some(store) = crate::store::open_existing_store().await? else {
                return Ok(());
            };
            let store = std::sync::Arc::new(store);
            if let (Some(session), Some((endpoint, thread))) = (&after.session, &after.codex) {
                record_codex_turns(&store.sqlite, session, thread, endpoint).await;
            }
            let repo = after.repo.to_string_lossy();
            if tokio::time::timeout(
                PLANNING_REFRESH,
                crate::ops::linear_observe::sync_repository(&store, &repo),
            )
            .await
            .is_err()
            {
                tracing::warn!(repo = %repo, "planning refresh outlasted its turn-end budget");
            }
            let Some(session) = &after.session else {
                return Ok(());
            };
            // The turn ended: an update that arrived during it goes out now.
            let (sqlite, ended) = (store.sqlite.clone(), session.clone());
            crate::journal::request::spawn_blocking(move || {
                super::wake::after_turn(&sqlite, &ended)
            })
            .await?;
            if store.sqlite.pending_session_move(session)?.is_none() {
                return Ok(());
            }
            tokio::time::sleep(MOVE_AFTER_STOP).await;
            crate::ops::human_session::moves_bulk::apply_pending_move(&store, session).await
        })
    })();
    if let Err(error) = ran {
        tracing::warn!(session = ?after.session, error = %format!("{error:#}"), "turn-end work failed");
    }
}

/// Read a Codex conversation's turns from its app-server into the Session's
/// history. A failure is a recording gap the next turn end or launch fills.
async fn record_codex_turns(
    store: &SqliteStore,
    session: &str,
    thread: &crate::id::AgentSessionId,
    endpoint: &std::path::Path,
) {
    let read = tokio::time::timeout(
        CODEX_HISTORY,
        crate::harness::codex_connection::recover_history(store, session, thread, endpoint),
    )
    .await
    .map_err(|_| anyhow!("Codex history read timed out"))
    .and_then(|read| read);
    if let Err(error) = read {
        tracing::warn!(session, error = %format!("{error:#}"), "Codex turns unrecorded at turn end");
    }
}

/// Settle the batches earlier answers carried, then render the Session's next
/// batch of pending steers. Also answers whether an earlier batch still waits
/// for its hook.
fn claim_steers(
    store: &SqliteStore,
    delivery: &ContextDelivery,
    adapter: &SteerAdapter,
    thread: &str,
    callback: &serde_json::Value,
    event: &str,
    printed: &[crate::id::SteerBatchId],
) -> StoreResult<(Option<SteerBatch>, bool)> {
    let boundary = match event {
        "SessionStart" => NativeBoundary::Select {
            source: callback["source"].as_str().unwrap_or("startup"),
        },
        "OpenCodeMessagesTransform" | "PiContext" => NativeBoundary::Activate { source: event },
        _ => NativeBoundary::Turn,
    };
    store.recover_prepared_steers(&adapter.session)?;
    store.with_provider_context(adapter, thread, boundary, || {
        let waiting = store.confirm_session_steers(adapter, printed, HOOK_DEADLINE.as_secs())?;
        let invocation = callback["tool_use_id"]
            .as_str()
            .or_else(|| callback["adapter_invocation_id"].as_str())
            .or_else(|| callback["turn_id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let boundary = format!(
            "{event}:{}:{invocation}",
            callback["turn_id"].as_str().unwrap_or("")
        );
        Ok((
            emit_steers(store, &delivery.repo, adapter, thread, &boundary)?,
            waiting,
        ))
    })
}

/// The batch of pending steers a wake carries to an idle conversation,
/// rendered as a hook would print it. The waker confirms it once sent.
pub(super) fn wake_steers(
    store: &SqliteStore,
    repo: &std::path::Path,
    adapter: &SteerAdapter,
    thread: &str,
) -> Result<Option<SteerBatch>> {
    store.recover_prepared_steers(&adapter.session)?;
    let boundary = format!("wake:{}", uuid::Uuid::new_v4());
    Ok(
        store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            emit_steers(store, repo, adapter, thread, &boundary)
        })?,
    )
}

/// Render and record the Session's next batch of pending steers at `boundary`.
fn emit_steers(
    store: &SqliteStore,
    repo: &std::path::Path,
    adapter: &SteerAdapter,
    thread: &str,
    boundary: &str,
) -> StoreResult<Option<SteerBatch>> {
    let steers =
        store.emit_session_steers(adapter, thread, boundary, |id, steers, remaining| {
            let (text, excerpts) = render_steers(repo, &adapter.session, id, steers, remaining)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            Ok(SteerContext { text, excerpts })
        })?;
    Ok(steers.map(|(batch, text)| SteerBatch { batch, text }))
}

fn excerpt(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn render_steers(
    repo: &std::path::Path,
    session: &str,
    batch: &str,
    steers: &[crate::steer::Steer],
    remaining: usize,
) -> Result<(String, Vec<crate::id::SteerId>)> {
    let mut excerpts = Vec::new();
    let mut text = format!("<lf:steers batch=\"{batch}\">\n");
    for steer in steers {
        let path = crate::prompt::write_prompt_log(
            repo,
            &serde_json::to_string_pretty(steer)?,
            "steer",
            None,
        )?;
        let path = path.strip_prefix(repo).unwrap_or(&path);
        let mut item = format!(
            "Steer {}. Source {} revision {}; author {}. Complete source: {}\n",
            steer.id,
            serde_json::to_string(excerpt(&steer.source, 96))?,
            serde_json::to_string(excerpt(&steer.revision, 64))?,
            serde_json::to_string(excerpt(&steer.author.to_string(), 96))?,
            serde_json::to_string(path)?
        );
        let mut body = excerpt(&steer.text, 400);
        while !body.is_empty()
            && item.len() + crate::prompt::render_reference(body).len() + 32 > 1024
        {
            body = excerpt(body, body.len() - 1);
        }
        item.push_str(&crate::prompt::render_reference(body));
        if body.len() != steer.text.len() {
            excerpts.push(steer.id.clone());
            item.push_str(" [excerpt; full source above]");
        }
        item.push('\n');
        anyhow::ensure!(
            item.len() <= 1024,
            "Steer attribution and source pointer exceed the item budget"
        );
        text.push_str(&item);
    }
    text.push_str(&format!(
        "{remaining} pending. Inspect: lf session steers {session} --json\n</lf:steers>\n"
    ));
    Ok((text, excerpts))
}

#[cfg(test)]
mod tests {
    use super::answer;
    use crate::context_block::ContextDelivery;
    use crate::server::wire::{BoundaryGap, BoundaryMoment, SessionBoundary};
    use crate::steer::{NativeBoundary, SteerAdapter};
    use crate::store::sqlite::SqliteStore;

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
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        let process = store.test_lf_process();
        let claim = store
            .claim_session_attachment("session", None, &process, false)
            .unwrap();
        store
            .record_agent_process_identity("session", &claim, std::process::id(), 1)
            .unwrap();
        let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET agent_interactive=1 WHERE id=?1",
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
    fn start_and_compact_refresh_session_and_task_identity_without_steers() {
        let (_home, store, adapter, delivery) = fixture();
        let render = |moment, source| {
            let request = boundary(
                moment,
                &adapter,
                &delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":source}),
            );
            assert!(answer(&store, &request).unwrap().0.steers.is_none());
            ContextDelivery::load(&delivery).unwrap()
        };
        let start = render(BoundaryMoment::Start, "startup");
        let identity = start.identity.unwrap();
        assert_eq!(identity.session, "session");
        assert_eq!(identity.task, None);

        let task = store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/repo").unwrap(),
                due_date: None,
                id: crate::durable::TaskId::new(),
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
            .block_with_prefix(crate::context_block::ContextMoment::Compact, "")
            .unwrap()
            .text;
        assert!(
            text.starts_with(&crate::prompt::render_reference(&format!(
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

    #[test]
    fn a_steer_is_delivered_once_the_hook_says_it_printed_it() {
        let (_home, store, adapter, delivery) = fixture();
        let tool = |id: &str, printed: &[crate::id::SteerBatchId]| {
            let mut tool = boundary(
                BoundaryMoment::Tool,
                &adapter,
                &delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":id}),
            );
            tool.printed = printed.to_vec();
            tool
        };
        let evidence = || {
            store.session_steers("session", 0, 8).unwrap().items[0]
                .evidence
                .clone()
        };
        let (answered, after) = answer(&store, &tool("quiet", &[])).unwrap();
        assert_eq!((answered.steers, answered.waiting), (None, false));
        assert!(after.is_none());
        store
            .save_session_steer("session", "Fixture author", "KEEP_THE_API_SMALL")
            .unwrap();
        let (answered, _) = answer(&store, &tool("first", &[])).unwrap();
        let steers = answered.steers.unwrap();
        assert!(steers.text.contains("KEEP_THE_API_SMALL"));
        assert_eq!(evidence(), "uncertain");
        // A hook running beside the one still printing gets nothing new and
        // is told to ask again.
        let (answered, _) = answer(&store, &tool("beside", &[])).unwrap();
        assert_eq!((answered.steers, answered.waiting), (None, true));
        let (answered, _) = answer(&store, &tool("second", &[steers.batch])).unwrap();
        assert_eq!((answered.steers, answered.waiting), (None, false));
        assert_eq!(evidence(), "emitted");
    }

    /// A Pi event is recorded, and answers steers only while a turn runs:
    /// outside one, Pi would store the steer without starting a run.
    #[test]
    fn a_pi_event_answers_steers_only_during_a_turn() {
        let (_home, store, adapter, delivery) = fixture_on("/pi/session.jsonl");
        store
            .save_session_steer("session", "Fixture author", "KEEP_THE_API_SMALL")
            .unwrap();
        let event = |callback: serde_json::Value| {
            let mut callback = callback;
            callback["sessionFile"] = "/pi/session.jsonl".into();
            answer(
                &store,
                &boundary(BoundaryMoment::Event, &adapter, &delivery, callback),
            )
            .unwrap()
        };
        let (idle, after) =
            event(serde_json::json!({"type":"ui_prompt_end","openTools":0,"pendingInput":0}));
        assert_eq!(
            (idle.steers, idle.waiting, after.is_none()),
            (None, false, true)
        );
        let (running, _) = event(serde_json::json!({"type":"tool_execution_start",
            "toolCallId":"tool","turn":"turn","openTools":1,"pendingInput":0}));
        assert!(running.steers.unwrap().text.contains("KEEP_THE_API_SMALL"));
        let (settled, _) = event(serde_json::json!({"type":"agent_settled","turn":"turn",
            "aborted":false,"events":[],"openTools":0,"pendingInput":0}));
        assert_eq!((settled.steers, settled.waiting), (None, true));
    }

    /// A checkout that moved since launch: the server points the delivery
    /// file at where the store now places the repository, and the hook
    /// renders the saved skill from there.
    #[test]
    fn a_moved_checkouts_delivery_follows_the_repository() {
        let (home, store, adapter, delivery) = fixture();
        let old = home.path().join("repo");
        let id = store.ensure_repository(old.to_str().unwrap()).unwrap();
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

    /// Comments on the Session's Task reach its next prompt once, with no
    /// watcher between: direction as a steer, anything else as a Task update.
    #[test]
    fn a_task_comment_rides_the_next_boundary_answer_once() {
        let (_home, store, adapter, delivery) = fixture();
        let task = crate::durable::TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/updates").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Updates".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='session'",
                [],
            )
            .unwrap();
        let captured = store.session("session").unwrap().unwrap().captured;
        store.bind_session("session", captured, &task).unwrap();
        let prompt = |id: &str| {
            boundary(
                BoundaryMoment::Tool,
                &adapter,
                &delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit",
                    "prompt_id":id,"prompt":"go"}),
            )
        };
        // The first boundary is the baseline: history before it is not news.
        assert_eq!(answer(&store, &prompt("p-1")).unwrap().0.update, None);
        for (id, body) in [
            ("c-1", "use the staging bucket"),
            ("c-2", "PR: https://example.test/pr/1"),
        ] {
            store
                .append_task_comment(
                    &task,
                    &crate::work::task::comment::TaskComment {
                        id: id.into(),
                        body: body.into(),
                        author: crate::work::task::comment::TaskCommentAuthor::Person {
                            name: Some("jack".into()),
                        },
                        created_at: Some("2026-11-11T09:00:00Z".into()),
                    },
                )
                .unwrap();
        }
        let answered = answer(&store, &prompt("p-2")).unwrap().0;
        assert!(answered
            .steers
            .unwrap()
            .text
            .contains("use the staging bucket"));
        let update = answered.update.unwrap();
        assert!(update.contains("New comment from jack: PR: https://example.test/pr/1"));
        assert!(!update.contains("staging bucket"), "direction arrives once");
        assert_eq!(answer(&store, &prompt("p-3")).unwrap().0.update, None);
    }

    #[test]
    fn a_steer_whose_hook_gave_up_is_answered_again() {
        let (_home, store, adapter, delivery) = fixture();
        let tool = |id: &str| {
            boundary(
                BoundaryMoment::Tool,
                &adapter,
                &delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":id}),
            )
        };
        store
            .save_session_steer("session", "Fixture author", "KEEP_THE_API_SMALL")
            .unwrap();
        let late = answer(&store, &tool("late")).unwrap().0.steers.unwrap();
        // The hook stopped waiting before this answer; its deadline has passed.
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE steer_batches SET created_at=created_at-10", [])
            .unwrap();
        let again = answer(&store, &tool("next")).unwrap().0.steers.unwrap();
        assert!(again.text.contains("KEEP_THE_API_SMALL"));
        assert_ne!(again.batch, late.batch);
        // The first hook printed after all: the steer arrived twice, and its
        // late confirmation changes nothing.
        let mut both = tool("after");
        both.printed = vec![late.batch, again.batch];
        let (answered, _) = answer(&store, &both).unwrap();
        assert_eq!((answered.steers, answered.waiting), (None, false));
        let page = store.session_steers("session", 0, 8).unwrap();
        assert_eq!(
            (page.items[0].evidence.as_str(), page.pending),
            ("emitted", 0)
        );
    }

    #[test]
    fn a_foreign_conversation_is_refused_and_its_steer_waits() {
        let (_home, store, adapter, delivery) = fixture();
        store
            .save_session_steer("session", "Fixture author", "NOT_FOR_A_CHILD")
            .unwrap();
        let foreign = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"child","hook_event_name":"PostToolUse","tool_use_id":"t"}),
        );
        assert!(answer(&store, &foreign).is_err());
        assert_eq!(store.session_steers("session", 0, 8).unwrap().pending, 1);
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
        assert_eq!(after.unwrap().session.as_deref(), Some("session"));
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
                "SELECT count(*) FROM session_events WHERE session_id='session'
                 AND json_extract(payload,'$.source')='store-gap'
                 AND json_extract(payload,'$.first_miss')=1794358000",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(gaps, 1);
    }

    /// A Stop whose snapshot still names a background watch or a scheduled
    /// wakeup keeps the turn holding its owner, and so does a Stop that names
    /// neither list; only an empty snapshot settles it.
    #[test]
    fn a_turn_end_settles_only_once_no_watch_or_wakeup_remains() {
        use crate::store::sqlite::sessions::OwnerRetention::LiveTurn;
        let (_home, store, adapter, delivery) = fixture();
        let owner = store
            .session_attachment("session")
            .unwrap()
            .unwrap()
            .lf_process_id
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1,transport_lf_process_id=?1",
                [&owner],
            )
            .unwrap();
        let hook = |moment, callback: serde_json::Value| {
            answer(&store, &boundary(moment, &adapter, &delivery, callback)).unwrap();
            store.owner_retention(&owner).unwrap()
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
            .session_history("session", 0, 0)
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == crate::session::SessionEventKind::Usage)
            .collect::<Vec<_>>();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].provider_turn.as_deref(), Some("u1"));
        assert_eq!(usage[0].payload["usage"]["total_input_tokens"], 100);
        assert_eq!(usage[0].payload["usage"]["output_tokens"], 5);
        assert_eq!(usage[0].payload["usage"]["cost_usd"], 0.25);
    }

    /// OpenCode reports a managed turn end with no callback body.
    #[test]
    fn a_turn_end_naming_no_conversation_still_confirms_and_leaves_work_for_after() {
        let (_home, store, adapter, delivery) = fixture();
        store
            .save_session_steer("session", "Fixture author", "PRINTED_BEFORE")
            .unwrap();
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t"}),
        );
        let printed = answer(&store, &tool).unwrap().0.steers.unwrap().batch;
        let mut stop = boundary(
            BoundaryMoment::Stop,
            &adapter,
            &delivery,
            serde_json::json!({}),
        );
        stop.printed = vec![printed];
        let (answered, after) = answer(&store, &stop).unwrap();
        assert_eq!(answered.steers, None);
        assert_eq!(after.unwrap().session.as_deref(), Some("session"));
        assert_eq!(
            store.session_steers("session", 0, 8).unwrap().items[0].evidence,
            "emitted"
        );
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
        let after = after.unwrap();
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
                &crate::session_record::new_artifact_key(),
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
        let traced = crate::store::query_plans::statements(&store, || {
            let at = |moment, callback: serde_json::Value| {
                boundary(moment, &adapter, &delivery, callback)
            };
            // A resumed conversation selects again and receives the steer.
            let printed = answer(
            &store,
            &at(
                BoundaryMoment::Start,
                serde_json::json!({"session_id":"native","hook_event_name":"SessionStart","source":"resume"}),
            ),
        )
        .unwrap()
        .0
        .steers
        .unwrap()
        .batch;
            let mut prompt = at(
                BoundaryMoment::Tool,
                serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit",
                "prompt_id":"p-1","prompt":"go"}),
            );
            prompt.printed = vec![printed];
            answer(&store, &prompt).unwrap();
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
        });
        let found = crate::store::query_plans::growing_scans(&store, &traced);
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    #[test]
    fn a_boundary_with_nothing_to_settle_does_not_wait_for_another_sessions_write() {
        let (home, store, adapter, delivery) = fixture();
        store
            .conn
            .lock()
            .unwrap()
            .busy_timeout(std::time::Duration::from_millis(50))
            .unwrap();
        let writer = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t1"}),
        );
        let (answered, _) = answer(&store, &tool).unwrap();
        assert_eq!(answered.steers, None);
        assert!(!answered.waiting);
        writer.execute_batch("ROLLBACK").unwrap();
        // A steer to hand over needs the write lock again.
        store
            .save_session_steer("session", "Fixture author", "NEXT")
            .unwrap();
        let tool = boundary(
            BoundaryMoment::Tool,
            &adapter,
            &delivery,
            serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":"t2"}),
        );
        assert!(answer(&store, &tool).unwrap().0.steers.is_some());
    }
}
