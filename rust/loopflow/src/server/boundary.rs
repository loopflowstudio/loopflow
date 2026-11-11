//! `session.boundary`: the store work a provider hook asks for at a turn or tool
//! boundary. The hook opens nothing; the server records the boundary and
//! answers the Session's pending steers, which count as delivered once the
//! hook's next request says it printed them.
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
/// The longest a turn end's planning refresh may take.
const PLANNING_REFRESH: Duration = Duration::from_secs(8);

/// What a `stop` leaves for after its answer: the Session's planning and any
/// pending move.
#[derive(Debug)]
pub(super) struct AfterStop {
    repo: PathBuf,
    session: Option<String>,
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
    } = boundary;
    let mut delivery = ContextDelivery::load(path)?;
    // A checkout moved since launch: the hook renders from the new place.
    if delivery.relocate(store)? {
        delivery.save(path)?;
    }
    let nothing = SessionBoundaryAnswer {
        steers: None,
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
        };
        return Ok((nothing, Some(after)));
    };
    // OpenCode's turn end names no conversation: it records no turn, and
    // still confirms, refreshes planning and applies a pending move.
    let thread = callback["session_id"].as_str();
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
        if let Some(prompt_id) = callback["prompt_id"].as_str().filter(|id| !id.is_empty()) {
            let prompt = if yielded {
                None
            } else {
                callback["prompt"].as_str()
            };
            if yielded || prompt.is_some() {
                store.acknowledge_native_input(adapter, thread, prompt_id, prompt)?;
            }
        }
    }
    if let Some(gap) = gap {
        store.record_recording_gap(&adapter.session, gap.first, gap.last)?;
    }
    if *moment == BoundaryMoment::Stop {
        // A turn end delivers nothing, so what it returns to pending waits
        // for the next prompt, whose hook always asks.
        store.confirm_session_steers(adapter, printed, HOOK_DEADLINE.as_secs())?;
        let after = AfterStop {
            repo: delivery.repo,
            session: Some(adapter.session.clone()),
        };
        return Ok((nothing, Some(after)));
    }
    let thread = thread.ok_or_else(no_thread)?;
    let (steers, waiting) =
        claim_steers(store, &delivery, adapter, thread, callback, event, printed)?;
    Ok((SessionBoundaryAnswer { steers, waiting }, None))
}

/// After a Stop's answer: refresh the Session's planning, whose changes become
/// steers for its next boundary, then continue it on a pending move's agent.
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
        let steers =
            store.emit_session_steers(adapter, thread, &boundary, |id, steers, remaining| {
                let (text, excerpts) =
                    render_steers(delivery, &adapter.session, id, steers, remaining)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                Ok(SteerContext { text, excerpts })
            })?;
        Ok((
            steers.map(|(batch, text)| SteerBatch { batch, text }),
            waiting,
        ))
    })
}

fn excerpt(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn render_steers(
    delivery: &ContextDelivery,
    session: &str,
    batch: &str,
    steers: &[crate::steer::Steer],
    remaining: usize,
) -> Result<(String, Vec<crate::id::SteerId>)> {
    let mut excerpts = Vec::new();
    let mut text = format!("<lf:steers batch=\"{batch}\">\n");
    for steer in steers {
        let path = crate::prompt::write_prompt_log(
            &delivery.repo,
            &serde_json::to_string_pretty(steer)?,
            "steer",
            None,
        )?;
        let path = path.strip_prefix(&delivery.repo).unwrap_or(&path);
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
                "native",
                NativeBoundary::Activate { source: "startup" },
                || Ok(()),
            )
            .unwrap();
        let repo = home.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let delivery = home.path().join("delivery.json");
        ContextDelivery {
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
        }
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
        };
        let (answered, after) = answer(&store, &stop(BoundaryMoment::Stop)).unwrap();
        assert_eq!(answered.steers, None);
        let after = after.unwrap();
        assert!(after.repo.ends_with("repo") && after.session.is_none());
        assert!(answer(&store, &stop(BoundaryMoment::Tool)).is_err());
    }

    static TRACED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    /// `sqlite_stat1` as `ANALYZE` measured it on a Machine with 14,698
    /// Sessions, 10,417 steer batches and 235,713 Session events (November
    /// 11), for every table a boundary reads except `steers`, which the test
    /// analyzes itself so its owner indexes are measured as built.
    const MEASURED: &str = "INSERT INTO sqlite_stat1(tbl,idx,stat) VALUES
         ('agent_sessions','agent_sessions_agent_process','14672 1'),
         ('agent_sessions','agent_sessions_primary_repository','9 1'),
         ('agent_sessions','agent_sessions_primary_wave','14 1'),
         ('agent_sessions','agent_sessions_transport','9512 4'),
         ('agent_sessions','conversation_task_inventory','14698 32 3 1'),
         ('agent_sessions','open_sessions','13948 1 1'),
         ('agent_sessions','session_inventory','14698 7349 21 3 1'),
         ('agent_sessions','session_repo_inventory','14698 669 475 20 3 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_1','14698 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_2','14698 1'),
         ('processes','agent_process_attachment','15452 85'),
         ('processes','agent_process_session','15452 1'),
         ('processes','processes_parent','160090 7 2 1'),
         ('processes','processes_receipt_locator','160090 1'),
         ('processes','processes_recent','160090 2 1'),
         ('processes','processes_trace','160090 35 2 1'),
         ('processes','processes_unfinished','741 2 1'),
         ('processes','sqlite_autoindex_processes_1','160090 1'),
         ('projects','idx_projects_wave','63 3 1'),
         ('projects','idx_projects_wave_updated','63 3 2'),
         ('projects','sqlite_autoindex_projects_1','63 1'),
         ('projects','sqlite_autoindex_projects_2','63 2'),
         ('session_activity','sqlite_autoindex_session_activity_1','12922 1'),
         ('session_events','session_capture_key','14860 1'),
         ('session_events','session_event_input','18150 2 2'),
         ('session_events','session_event_receipt','235713 17 9 8 3 1'),
         ('session_events','session_events_captured_event','166889 12'),
         ('session_events','session_events_history','235713 17 1'),
         ('session_events','session_events_process','54078 4 4'),
         ('session_events','session_events_turn','235713 17 9 8 1'),
         ('session_events','session_input_membership','14840 1 1 1'),
         ('session_events','session_observation_receipt','148667 11 1'),
         ('session_events','session_process_membership','54078 4 4'),
         ('session_events','session_turn_events','72186 6 6 4 1 1'),
         ('session_steers','sqlite_autoindex_session_steers_1','41523 6 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_1','11834 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_2','11834 1'),
         ('steer_batches','sqlite_autoindex_steer_batches_1','10417 1'),
         ('steer_batches','steer_batches_boundary','10396 2 1'),
         ('steer_batches','steer_batches_session','10417 2 2 1'),
         ('steer_deliveries','sqlite_autoindex_steer_deliveries_1','40864 6 1 1'),
         ('steer_deliveries','steer_deliveries_batch','40864 4'),
         ('steer_deliveries','steer_deliveries_steer','40864 27'),
         ('tasks','idx_tasks_issue_identifier','1096 1'),
         ('tasks','idx_tasks_project','1096 21 2'),
         ('tasks','idx_tasks_repository','1096 137 19 2'),
         ('tasks','idx_tasks_updated','1096 2'),
         ('tasks','idx_tasks_worktree','1096 2'),
         ('tasks','sqlite_autoindex_tasks_1','1096 1'),
         ('tasks','sqlite_autoindex_tasks_2','1096 1'),
         ('tasks','tasks_branch','1096 2'),
         ('tasks','tasks_ship_process','127 1'),
         ('waves','idx_waves_active_locator','61 2 2 1'),
         ('waves','idx_waves_parent','61 61'),
         ('waves','sqlite_autoindex_waves_1','63 1'),
         ('waves','waves_project_activation_process','15 1');
         ANALYZE sqlite_schema";

    /// Every statement a boundary runs looks its rows up by index: a scan of
    /// a table that grows with every steer, turn or Task slows each turn of
    /// every Session on the Machine. Machines run unanalyzed today; an
    /// analyzed store must plan the same way.
    #[test]
    fn boundary_statements_never_scan_tables_that_grow_with_use() {
        const GROWING: [&str; 6] = [
            "steers",
            "steer_batches",
            "steer_deliveries",
            "session_steers",
            "session_events",
            "tasks",
        ];
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
        store.conn.lock().unwrap().trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_PROFILE,
            Some(|event| {
                if let rusqlite::trace::TraceEvent::Profile(statement, _) = event {
                    TRACED.lock().unwrap().extend(statement.expanded_sql());
                }
            }),
        );
        let at =
            |moment, callback: serde_json::Value| boundary(moment, &adapter, &delivery, callback);
        // A resumed conversation selects again and receives the steer.
        let printed = answer(
            &store,
            &at(
                BoundaryMoment::Tool,
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
        let conn = store.conn.lock().unwrap();
        conn.trace_v2(rusqlite::trace::TraceEventCodes::empty(), None);
        let traced = std::mem::take(&mut *TRACED.lock().unwrap());
        let scans = |stats: &str| {
            let mut scans = Vec::new();
            for sql in &traced {
                // Plans name a table by its alias: `steers p` scans as `SCAN p`.
                let words: Vec<&str> = sql
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|word| !word.is_empty())
                    .collect();
                let mut growing: Vec<&str> = GROWING.to_vec();
                for (at, word) in words.iter().enumerate() {
                    if GROWING.contains(word) {
                        let alias = match words.get(at + 1) {
                            Some(&"AS") => words.get(at + 2),
                            alias => alias,
                        };
                        growing.extend(alias);
                    }
                }
                let mut plan = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
                let details = plan
                    .query_map([], |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap();
                for detail in details {
                    let table = detail
                        .strip_prefix("SCAN ")
                        .and_then(|rest| rest.split_whitespace().next());
                    if table.is_some_and(|table| growing.contains(&table)) {
                        scans.push(format!("{stats}: {detail}\n  in {sql}"));
                    }
                }
            }
            scans
        };
        let mut found = scans("unanalyzed");
        conn.execute_batch("ANALYZE steers").unwrap();
        conn.execute_batch(MEASURED).unwrap();
        found.extend(scans("analyzed"));
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
