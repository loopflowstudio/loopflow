use crate::process::attachment;
struct LiveSessionFixture {
    store: SharedStore,
    database: std::path::PathBuf,
    _env: crate::test_ambient::EnvGuard,
    _home: SessionHome,
}

/// A real process standing in for a running provider, reaped even when an
/// assertion fails.
struct RunningAgent(std::process::Child);

impl Drop for RunningAgent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl LiveSessionFixture {
    async fn new() -> Self {
        let home = SessionHome::new();
        let store = home.store().await;
        let env = crate::test_ambient::EnvGuard::clear(&["CODEX_HOME", "CLAUDE_CONFIG_DIR"]);
        Self {
            store,
            database: home.home.path().join("loopflow.db"),
            _env: env,
            _home: home,
        }
    }

    fn codex_session(&self, id: &str, model: Option<&str>) -> LfSession {
        let session = self
            .store
            .sqlite
            .test_session(id, &crate::session_record::new_artifact_key());
        let thread = format!("thread-{id}");
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='codex',provider_thread=?2,model=?3 WHERE id=?1",
                rusqlite::params![&session.id, &thread, model],
            )
            .unwrap();
        session
    }

    fn run(&self, session: &LfSession) -> RunningAgent {
        let parent = crate::id::LfProcessId::new();
        rusqlite::Connection::open(&self.database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                [&parent],
            )
            .unwrap();
        let attachment =
            attachment::claim_session_attachment(&self.store.sqlite, &session.id, None, &parent)
                .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60");
        RunningAgent(
            crate::harness::agent_process::spawn_native(
                command,
                &crate::harness::Recording {
                    store: self.store.sqlite.clone(),
                    session: session.id.to_string(),
                    attachment,
                },
                crate::harness::LaunchKind::Batch,
            )
            .unwrap(),
        )
    }
}

fn listed_session(store: &crate::store::SharedStore, id: &str) -> super::SessionRecord {
    super::listed(store, id).unwrap().unwrap()
}

#[test]
fn resume_refuses_a_run_and_leaves_its_provider_running() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    crate::journal::with_runtime(home, &["lf".into(), "session".into()], || {
        runtime.block_on(async {
            let store = std::sync::Arc::new(
                crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    home.join("loopflow.db"),
                ))
                .await?,
            );
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                crate::session_record::SessionCaptureSpec {
                    harness: "claude".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: Some("implement".into()),
                    subjects: vec![],
                    graph: crate::session_record::SessionGraphMembership::Independent,
                    work: None,
                },
            ).map(crate::process::LfProcessRuntime::new)?;
            let session = store
                .sqlite
                .session_for_artifact(&capture.artifact_key())?
                .unwrap();
            let launcher = loopflow_model::id::LfProcessId::new();
            rusqlite::Connection::open(home.join("loopflow.db"))?.execute(
                "INSERT INTO processes(id,trace_id,started_at) SELECT ?1,trace_id,started_at FROM processes WHERE id=?2",
                rusqlite::params![launcher, crate::journal::current_lf_process_id().unwrap()],
            )?;
            let first = attachment::claim_session_attachment(&store.sqlite, &session.id, None, &launcher)?;
            let mut command = tokio::process::Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut provider = crate::harness::agent_process::spawn(
                command,
                None,
                &crate::harness::Recording { store: store.sqlite.clone(), session: session.id.to_string(), attachment: first.clone() },
                crate::harness::LaunchKind::Batch,
            )?;
            let opened = super::open(&store, home, &session.id, None, None, false, true, Default::default()).await;
            let forked = super::open(&store, home, &session.id, None, None, true, true, Default::default()).await;
            let running = provider.try_wait()?.is_none();
            provider.kill().await?;
            assert!(running, "resume stopped the batch's provider");
            for refused in [opened, forked] {
                let message = refused.unwrap_err().to_string();
                assert!(message.contains("is a batch (implement · running)"), "{message}");
                assert!(message.contains("`lf resume` opens this checkout's Session"), "{message}");
            }
            assert_eq!(store.sqlite.session_attachment(&session.id)?, Some(first));
            Ok::<_, anyhow::Error>(())
        })
    })
    .unwrap();
}

#[test]
fn session_states_fixture_has_only_process_or_unknown_states() {
    let states: Vec<super::SessionState> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_states.json"
    ))
    .unwrap();
    assert_eq!(
        states,
        vec![
            super::SessionState::Unknown,
            super::SessionState::Active,
            super::SessionState::Detached,
            super::SessionState::Closed
        ]
    );
    assert!(serde_json::from_str::<super::SessionState>("\"interrupted\"").is_err());
}

#[test]
fn session_preview_preserves_native_provenance() {
    let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/sessions.json"
    ))
    .unwrap();
    let preview = sessions[0].preview.as_ref().unwrap();
    assert_eq!(preview.provider_turn, "turn-fixture");
    assert_eq!(preview.observed_at, 1700000000);
    assert_eq!(
        serde_json::from_value::<Vec<super::SessionRecord>>(
            serde_json::to_value(&sessions).unwrap()
        )
        .unwrap(),
        sessions
    );
}

#[test]
fn session_metadata_wire_preserves_unknown_observation_and_occurrence() {
    let session: super::SessionRecord = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_metadata.json"
    ))
    .unwrap();
    assert_eq!(session.state, super::SessionState::Unknown);
    assert_eq!(
        session.availability,
        Some(super::SessionAvailability::Available)
    );
    assert_eq!(
        serde_json::to_value(&session).unwrap()["availability"],
        "available"
    );
    assert!(matches!(
        session.graph_membership,
        super::SessionGraphMembership::Step {
            node: None,
            iterations: None,
            occurrence: super::SessionGraphOccurrence::Unknown,
            ..
        }
    ));
    assert!(session.terminal_ids.is_empty());
    assert_eq!(
        serde_json::from_value::<super::SessionRecord>(serde_json::to_value(&session).unwrap())
            .unwrap(),
        session
    );
}

/// An interactive Session's summary with recorded history and unknown process evidence.
fn summary_fixture() -> crate::session::SessionSummary {
    let task = TaskId::new();
    let wave = loopflow_model::id::WaveId::new();
    crate::session::SessionSummary {
        transition: None,
        preview: None,
        has_provider_history: true,
        program_status: None,
        agent_process_id: None,
        primary_scope: None,
        agent_process_evidence: crate::store::ProcessIdentityEvidence::Unknown,
        agent_end: None,
        starts_since_turn: 0,
        last_started_at: None,
        start_failure: None,
        available: false,
        terminal_id: None,
        unviewed: false,
        held: false,
        task_terminal: false,
        in_task_checkout: false,
        launching: false,
        captured: Some(1),
        id: "metadata".into(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
        title: "Retained conversation".into(),
        title_source: crate::session::TitleSource::Human,
        completed_at: None,
        interactive: true,
        task_id: Some(task.clone()),
        wave_id: Some(wave.clone()),
        graph_lf_process_id: Some("graph".into()),
        cwd: "/unavailable".into(),
        skill: Some("review".into()),
        provider: None,
        account: None,
        model: None,
        node: Some(2),
        iterations: None,
        graph_step_latest: false,
        graph: Some(crate::session::GraphProcessSummary {
            id: "graph".into(),
            name: "retained".into(),
            state: crate::session::GraphProcessSummaryState::Current,
            task_id: Some(task.clone()),
            wave_id: Some(wave.clone()),
            updated_at: 1,
        }),
        independent: false,
        wave_name: Some("Infrastructure".into()),
        task_identifier: Some("INF-123".into()),
    }
}

/// A Session's row read at `now`, on a Machine that claims its Task and runs
/// a server.
fn row_at(summary: &crate::session::SessionSummary, now: i64) -> super::SessionRecord {
    super::summary_surface(
        summary,
        super::session_ending(summary, now, || true, || true),
    )
}

/// A Session's row long after everything it records.
fn surfaced(summary: &crate::session::SessionSummary) -> super::SessionRecord {
    row_at(summary, 1_000_000)
}

fn outlook(summary: &crate::session::SessionSummary) -> Option<super::SessionOutlook> {
    surfaced(summary).outlook
}

fn line(row: &super::SessionRecord) -> &str {
    row.status_line.as_deref().unwrap()
}

/// A Task conversation that is down says once whether it comes back, when,
/// and which command opens it otherwise.
#[test]
fn a_task_conversation_says_how_it_ended_and_what_happens_next() {
    use super::SessionOutlookKind::{ComingBack, CouldNotStart, Quit, Starting, WaitingForYou};
    use super::SessionState;
    let died = |at| {
        Some(crate::session::AgentEnd {
            at,
            exit_code: Some(137),
        })
    };
    let mut summary = summary_fixture();
    summary.primary_scope = Some("task".into());
    summary.in_task_checkout = true;
    summary.agent_process_evidence = crate::store::ProcessIdentityEvidence::Dead;
    summary.agent_end = died(1_000);

    let first = surfaced(&summary);
    assert_eq!(first.state, SessionState::Closed);
    assert!(
        line(&first).starts_with("died ") && line(&first).ends_with(" (exit 137) · starting again"),
        "{}",
        line(&first)
    );
    let ended = first.ended.unwrap();
    assert_eq!(ended.exit_code, Some(137));
    assert_eq!(first.start, None, "the pass has started nothing yet");
    let next = first.outlook.unwrap();
    assert_eq!(
        (next.kind, next.at, next.reason, next.command),
        (ComingBack, Some(ended.at), None, None),
        "the first start is immediate"
    );

    summary.starts_since_turn = 2;
    summary.last_started_at = Some(2_000);
    assert_eq!(
        outlook(&summary).unwrap().at,
        loopflow_harness::transcript::rfc3339(2_000),
        "a start counted after the death is under way"
    );
    summary.start_failure = Some("no PTY left: 511 in use (kern.tty.ptmx_max)".into());
    assert_eq!(
        outlook(&summary).unwrap().at,
        loopflow_harness::transcript::rfc3339(2_000 + crate::ops::pr_landing::repair_backoff(2)),
        "a start that failed is not under way: the next waits its backoff"
    );

    summary.agent_end = died(2_500);
    // Read before that backoff passes.
    let waiting = row_at(&summary, 2_001);
    assert!(
        line(&waiting).contains(" (exit 137) · starts again ")
            && line(&waiting)
                .ends_with(" · last start failed: no PTY left: 511 in use (kern.tty.ptmx_max)"),
        "{}",
        line(&waiting)
    );
    let next = waiting.outlook.unwrap();
    assert_eq!(
        (next.kind, next.at),
        (
            ComingBack,
            loopflow_harness::transcript::rfc3339(
                2_000 + crate::ops::pr_landing::repair_backoff(2)
            )
        ),
        "a death after that start waits its backoff"
    );
    assert_eq!(
        next.reason.as_deref(),
        Some("last start failed: no PTY left: 511 in use (kern.tty.ptmx_max)")
    );

    // This Machine's pass leaves it: nothing promises a start.
    let no_server = super::summary_surface(
        &summary,
        super::session_ending(&summary, 2_600, || true, || false),
    );
    assert!(
        line(&no_server).ends_with(
            " (exit 137) · no server runs here to start it; last start failed: no PTY left: \
             511 in use (kern.tty.ptmx_max); `lf resume --task INF-123` opens it"
        ),
        "{}",
        line(&no_server)
    );
    let left = no_server.outlook.unwrap();
    assert_eq!((left.kind, left.at), (WaitingForYou, None));
    assert_eq!(left.command.as_deref(), Some("lf resume --task INF-123"));
    summary.start_failure = None;
    let unclaimed = super::summary_surface(
        &summary,
        super::session_ending(&summary, 2_600, || false, || true),
    );
    assert_eq!(
        unclaimed.outlook.unwrap().reason.as_deref(),
        Some("its Task is not claimed on this Machine")
    );
    summary.in_task_checkout = false;
    let elsewhere = outlook(&summary).unwrap();
    assert_eq!(
        (elsewhere.kind, elsewhere.reason.as_deref()),
        (WaitingForYou, Some("its Task is not checked out here"))
    );
    summary.in_task_checkout = true;

    summary.starts_since_turn = loopflow_storage::sqlite::session_recovery::CONVERSATION_STARTS;
    let row = surfaced(&summary);
    assert!(
        line(&row).ends_with(" (exit 137) · started 5 times; `lf resume --task INF-123` opens it"),
        "{}",
        line(&row)
    );
    assert_eq!(row.start.unwrap().attempts, 5);
    let gave_up = row.outlook.unwrap();
    assert_eq!(
        (gave_up.kind, gave_up.at, gave_up.reason.as_deref()),
        (WaitingForYou, None, Some("started 5 times"))
    );
    assert_eq!(gave_up.command.as_deref(), Some("lf resume --task INF-123"));

    summary.agent_end = Some(crate::session::AgentEnd {
        at: 3_000,
        exit_code: Some(0),
    });
    let row = surfaced(&summary);
    assert!(
        line(&row).starts_with("quit ")
            && line(&row).ends_with("; `lf resume --task INF-123` opens it"),
        "{}",
        line(&row)
    );
    assert_eq!(row.start, None, "a quit conversation stays quit");
    let quit = row.outlook.unwrap();
    assert_eq!((quit.kind, quit.at), (Quit, None));
    assert_eq!(quit.command.as_deref(), Some("lf resume --task INF-123"));

    // Running again: the starts stay visible until a turn ends.
    summary.agent_end = None;
    summary.starts_since_turn = 1;
    summary.agent_process_evidence = crate::store::ProcessIdentityEvidence::Live;
    let row = surfaced(&summary);
    assert!(
        line(&row).starts_with("running (started again ") && line(&row).ends_with(", attempt 1)"),
        "{}",
        line(&row)
    );
    assert_eq!((row.ended, row.outlook), (None, None));
    assert_eq!(row.start.unwrap().attempts, 1);
    summary.starts_since_turn = 0;
    assert_eq!(line(&surfaced(&summary)), "running");

    // A start under way: its agent is attached and has no pid to check yet.
    summary.agent_process_evidence = crate::store::ProcessIdentityEvidence::Unknown;
    summary.launching = true;
    summary.starts_since_turn = 1;
    let row = surfaced(&summary);
    assert_eq!(row.state, SessionState::Unknown);
    assert_eq!(line(&row), "starting");
    assert_eq!(row.outlook.unwrap().kind, Starting);
    summary.starts_since_turn = 0;
    summary.launching = false;

    // Prepared, and no agent has run yet: the server is starting it. With no
    // agent its state is unknown, not closed.
    summary.last_started_at = None;
    let prepared = surfaced(&summary);
    assert_eq!(prepared.state, SessionState::Unknown);
    assert_eq!(line(&prepared), "starting");
    let starting = prepared.outlook.unwrap();
    assert_eq!(
        (
            starting.kind,
            starting.at,
            starting.reason,
            starting.command
        ),
        (Starting, None, None, None)
    );
    // With no server here nothing is starting it.
    let unstarted = super::summary_surface(
        &summary,
        super::session_ending(&summary, 5_000, || true, || false),
    );
    assert_eq!(
        line(&unstarted),
        "not started · no server runs here to start it; `lf resume --task INF-123` opens it"
    );
    // A recorded agent this Machine cannot check still reads unknown.
    summary.agent_process_id = Some(loopflow_model::id::AgentProcessId::new());
    let unchecked = surfaced(&summary);
    assert_eq!(
        (line(&unchecked), unchecked.outlook.as_ref()),
        ("unknown", None)
    );
    summary.agent_process_id = None;
    summary.starts_since_turn = 1;

    // Its first start failed. It never reads as a death.
    summary.last_started_at = Some(5_000);
    summary.start_failure = Some("claude is not installed on this Machine's PATH".into());
    let row = row_at(&summary, 5_001);
    assert!(
        line(&row).starts_with(
            "could not start: claude is not installed on this Machine's PATH · tries again "
        ),
        "{}",
        line(&row)
    );
    assert_eq!(row.ended, None);
    let failed = row.outlook.unwrap();
    assert_eq!(
        (failed.kind, failed.reason.as_deref(), failed.command),
        (
            CouldNotStart,
            Some("claude is not installed on this Machine's PATH"),
            None
        )
    );
    assert_eq!(
        failed.at,
        loopflow_harness::transcript::rfc3339(5_000 + crate::ops::pr_landing::repair_backoff(1))
    );
    summary.start_failure = None;

    summary.primary_scope = None;
    summary.agent_process_evidence = crate::store::ProcessIdentityEvidence::Dead;
    summary.agent_end = Some(crate::session::AgentEnd {
        at: 4_000,
        exit_code: None,
    });
    let row = surfaced(&summary);
    assert_eq!(
        line(&row).split_once(' ').map(|(died, _)| died),
        Some("died"),
        "{}",
        line(&row)
    );
    assert!(
        line(&row).ends_with(&format!(
            " (no exit recorded); `lf resume {}` opens it",
            summary.id
        )),
        "{}",
        line(&row)
    );
    assert_eq!(row.start, None);
    let other = row.outlook.unwrap();
    assert_eq!(
        (other.kind, other.command),
        (WaitingForYou, Some(format!("lf resume {}", summary.id))),
        "only a Task's conversation comes back"
    );
}

/// The recovery pass starts a conversation exactly when its Session's outlook
/// says the server does, whatever its row holds: both ask
/// `ops::recovery::server_start`.
#[test]
fn the_pass_starts_what_the_outlook_says_the_server_starts() {
    use super::SessionOutlookKind::{ComingBack, CouldNotStart, Quit, Starting, WaitingForYou};
    use rusqlite::params;
    const NOW: i64 = 1_800_000_000;
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let checkout = repo.path().to_string_lossy().into_owned();
    let task = loopflow_model::durable::TaskId::new();
    store
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: crate::repository::location::ensure_repository(&store, &checkout)
                .unwrap(),
            id: task.clone(),
            project_id: None,
            title: "Recover".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let session = store
        .test_session(
            "session_recovered",
            &crate::session_record::new_artifact_key(),
        )
        .id;
    let captured = store.session(session.as_str()).unwrap().unwrap().captured;
    crate::store::sessions::bind_session(&store, session.as_str(), captured, &task).unwrap();
    crate::process::attachment::claim_session_attachment(
        &store,
        session.as_str(),
        None,
        &store.test_lf_process(),
    )
    .unwrap();
    let execute = |sql: &str, values: &[&dyn rusqlite::ToSql]| {
        store.conn.lock().unwrap().execute(sql, values).unwrap();
    };
    execute(
        "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
        params![task.as_str(), checkout],
    );
    execute(
        "UPDATE agent_sessions SET interactive=1,cwd=?2 WHERE id=?1",
        params![session.as_str(), checkout],
    );
    execute(
        "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
        params![task.as_str(), session.as_str()],
    );
    // What the Session's outlook says, and whether the pass lists it.
    let read = || {
        let summary = super::session_projection::session_summary(&store, session.as_str(), NOW)
            .unwrap()
            .unwrap();
        let ending = super::session_ending(
            &summary,
            NOW,
            || crate::ops::recovery::claimed_here(&store, &task),
            || true,
        );
        let due = crate::ops::recovery::due_task_conversations(&store, NOW).unwrap();
        (
            ending
                .outlook
                .map(|outlook| (outlook.kind, outlook.at.is_some())),
            !due.now.is_empty() || due.next_at.is_some(),
        )
    };
    let set = |columns: &str| {
        execute(
            &format!("UPDATE agent_sessions SET {columns} WHERE id=?1"),
            params![session.as_str()],
        );
    };
    let ended = format!(
        "agent_pid=NULL,agent_os_started_at=NULL,agent_ended_at={}",
        NOW - 5
    );

    set(&format!("{ended},agent_exit_code=137"));
    assert_eq!(
        read(),
        (Some((WaitingForYou, false)), false),
        "not claimed here"
    );
    crate::store::ownership::claim_task(&store, &task).unwrap();
    assert_eq!(read(), (Some((ComingBack, true)), true), "killed");
    set(&format!("starts_since_turn=2,last_started_at={NOW}"));
    assert_eq!(
        read(),
        (Some((ComingBack, true)), true),
        "waiting its backoff"
    );
    set("starts_since_turn=5");
    assert_eq!(
        read(),
        (Some((WaitingForYou, false)), false),
        "out of starts"
    );
    set("starts_since_turn=0,last_started_at=NULL,agent_exit_code=0");
    assert_eq!(read(), (Some((Quit, false)), false), "quit");
    set(&format!("agent_exit_code=1,completed_at={NOW}"));
    assert_eq!(read(), (Some((WaitingForYou, false)), false), "completed");
    set("completed_at=NULL,cwd='/elsewhere'");
    assert_eq!(
        read(),
        (Some((WaitingForYou, false)), false),
        "not checked out here"
    );
    execute(
        "UPDATE agent_sessions SET cwd=?2 WHERE id=?1",
        params![session.as_str(), checkout],
    );

    set("agent_process_id=NULL,agent_ended_at=NULL,agent_exit_code=NULL");
    assert_eq!(
        read(),
        (Some((Starting, false)), true),
        "prepared, never started"
    );
    set(&format!(
        "starts_since_turn=1,last_started_at={NOW},start_failure='no PTY left'"
    ));
    assert_eq!(
        read(),
        (Some((CouldNotStart, true)), true),
        "its first start failed"
    );
    execute(
        "UPDATE tasks SET abandoned_at=?2 WHERE id=?1",
        params![task.as_str(), NOW],
    );
    assert_eq!(read(), (None, false), "its Task is abandoned");
}

/// A summary whose projected state is `state`: a running one has an agent
/// process, a detached one is held and unviewed.
fn summary_in(state: super::SessionState, interactive: bool) -> crate::session::SessionSummary {
    use crate::store::ProcessIdentityEvidence::*;
    let mut summary = summary_fixture();
    summary.interactive = interactive;
    summary.agent_process_evidence = match state {
        super::SessionState::Unknown => Unknown,
        super::SessionState::Active | super::SessionState::Detached => Live,
        super::SessionState::Closed => Dead,
    };
    summary.agent_process_id = matches!(
        state,
        super::SessionState::Active | super::SessionState::Detached
    )
    .then(loopflow_model::id::AgentProcessId::new);
    summary.held = state == super::SessionState::Detached;
    summary.unviewed = summary.held;
    assert_eq!(super::session_state(&summary), state);
    summary
}

#[test]
fn session_metadata_keeps_unknown_position_visible() {
    let mut summary = summary_fixture();
    let task = summary.task_id.clone().unwrap();
    let wave = summary.wave_id.clone().unwrap();
    let row = surfaced(&summary);
    assert_eq!(row.id, summary.id);
    assert_eq!(row.work, Some(loopflow_model::durable::WorkRef::Task(task)));
    assert_eq!(row.wave_id, Some(wave));
    assert_eq!(row.work_path.as_deref(), Some("Infrastructure / INF-123"));
    assert_eq!(row.state, super::SessionState::Unknown);
    assert!(matches!(
        row.graph_membership,
        super::SessionGraphMembership::Step {
            node: Some(2),
            occurrence: super::SessionGraphOccurrence::Earlier,
            ..
        }
    ));
    summary.graph_step_latest = true;
    assert!(matches!(
        surfaced(&summary).graph_membership,
        super::SessionGraphMembership::Step {
            occurrence: super::SessionGraphOccurrence::Current,
            ..
        }
    ));
    summary.graph.as_mut().unwrap().state = crate::session::GraphProcessSummaryState::Stopped;
    let row = surfaced(&summary);
    assert!(matches!(
        row.graph_membership,
        super::SessionGraphMembership::Step {
            occurrence: super::SessionGraphOccurrence::Past,
            ..
        }
    ));
    assert_eq!(
        row.state,
        super::SessionState::Unknown,
        "Graph completion is not Session/process completion"
    );
    summary.graph_lf_process_id = None;
    assert!(matches!(
        surfaced(&summary).graph_membership,
        super::SessionGraphMembership::Unknown { .. }
    ));
    summary.independent = true;
    assert_eq!(
        surfaced(&summary).graph_membership,
        super::SessionGraphMembership::Independent
    );
    let row = surfaced(&summary);
    assert_eq!(row.state, super::SessionState::Unknown);
    assert_eq!(row.availability, None);
    summary.completed_at = Some(1);
    assert_eq!(
        super::session_state(&summary),
        super::SessionState::Unknown,
        "legacy Session rows supply no process evidence"
    );
    // Session completion does not override its recorded provider's life.
    for (evidence, expected) in [
        (
            crate::store::ProcessIdentityEvidence::Live,
            super::SessionState::Active,
        ),
        (
            crate::store::ProcessIdentityEvidence::Dead,
            super::SessionState::Closed,
        ),
        (
            crate::store::ProcessIdentityEvidence::Unknown,
            super::SessionState::Unknown,
        ),
    ] {
        summary.agent_process_evidence = evidence;
        assert_eq!(super::session_state(&summary), expected);
    }
}

use crate::store::SharedStore;

use super::human_session_argv;
use crate::session::LfSession;
pub use crate::test_support::human_session::SessionHome;

use loopflow_storage::work::task::TaskId;

#[test]
fn session_action_fixtures_match_the_shared_boundary() {
    #[derive(serde::Deserialize)]
    struct Case {
        state: super::SessionState,
        action: Option<super::SessionAction>,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_actions.json"
    ))
    .unwrap();
    for case in cases {
        assert_eq!(
            super::session_action(&summary_in(case.state, true)),
            case.action
        );
    }
    let report = include_str!("../../../../../tests/fixtures/dto/session_move_report.json");
    let parsed: super::moves_bulk::MoveReport = serde_json::from_str(report).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::from_str::<serde_json::Value>(report).unwrap()
    );
    for fixture in [
        include_str!("../../../../../tests/fixtures/dto/session.json"),
        &serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../../../tests/fixtures/dto/sessions.json"
        ))
        .unwrap()[0]
            .to_string(),
    ] {
        let session: super::SessionRecord = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            session.action,
            super::session_action(&summary_in(session.state, session.interactive))
        );
        assert_eq!(session.work_path.as_deref(), Some("product / LOO-291"));
        let mut untitled = serde_json::to_value(&session).unwrap();
        untitled.as_object_mut().unwrap().remove("title_source");
        assert!(serde_json::from_value::<super::SessionRecord>(untitled).is_err());
        assert_eq!(
            serde_json::from_value::<super::SessionRecord>(serde_json::to_value(&session).unwrap())
                .unwrap(),
            session
        );
    }
}

/// The shared fixture carries one conversation per outlook, and the key is
/// required: a client that drops it would work the outlook out for itself.
#[test]
fn outlook_fixtures_cover_every_kind_and_are_required() {
    use super::SessionOutlookKind::{ComingBack, CouldNotStart, Quit, Starting, WaitingForYou};
    let json = include_str!("../../../../../tests/fixtures/dto/session_memberships.json");
    let sessions: Vec<super::SessionRecord> = serde_json::from_str(json).unwrap();
    let kinds: Vec<_> = sessions
        .iter()
        .filter_map(|session| Some(session.outlook.as_ref()?.kind))
        .collect();
    assert_eq!(
        kinds,
        [Quit, ComingBack, WaitingForYou, CouldNotStart, Starting]
    );
    let mut without: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
    without[0].as_object_mut().unwrap().remove("outlook");
    assert!(serde_json::from_value::<super::SessionRecord>(without.remove(0)).is_err());
}

#[test]
fn graph_membership_fixtures_cover_every_projection_and_are_required() {
    let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_memberships.json"
    ))
    .unwrap();
    let mut kinds = Vec::new();
    for session in &sessions {
        assert_eq!(
            session.action,
            super::session_action(&summary_in(session.state, session.interactive))
        );
        kinds.push(match &session.graph_membership {
            super::SessionGraphMembership::Step { occurrence, .. } => match occurrence {
                super::SessionGraphOccurrence::Unknown => "unknown_occurrence",
                super::SessionGraphOccurrence::Current => "current",
                super::SessionGraphOccurrence::Earlier => "earlier",
                super::SessionGraphOccurrence::Past => "past",
            },
            super::SessionGraphMembership::Independent => "independent",
            super::SessionGraphMembership::Unknown { .. } => "unknown",
        });
        let mut value = serde_json::to_value(session).unwrap();
        assert_eq!(
            serde_json::from_value::<super::SessionRecord>(value.clone()).unwrap(),
            *session
        );
        value.as_object_mut().unwrap().remove("graph_membership");
        assert!(serde_json::from_value::<super::SessionRecord>(value).is_err());
    }
    assert_eq!(
        kinds,
        [
            "current",
            "earlier",
            "independent",
            "unknown",
            "past",
            "past",
            "current"
        ]
    );
    // An older capture keeps its Graph provenance with no node or tuple.
    assert!(matches!(
        &sessions[5].graph_membership,
        super::SessionGraphMembership::Step {
            node: None,
            iterations: None,
            ..
        }
    ));
    assert_eq!(
        sessions.last().unwrap().title_source,
        super::SessionTitleSource::Unavailable
    );
    // A remote Session's Session capture manifest is not here: no provider is claimed.
    assert_eq!(sessions.last().unwrap().provider, None);
    assert_eq!(sessions[0].provider.as_deref(), Some("codex"));
}

#[test]
fn session_wire_names_the_conversation_without_a_run_reference() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session.json"
    ))
    .unwrap();
    for kind in ["conversation", "graph"] {
        let mut value = fixture.clone();
        value["kind"] = kind.into();
        let session: super::SessionRecord = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(session.id, fixture["id"].as_str().unwrap());
        assert!(serde_json::to_value(session)
            .unwrap()
            .get("run_id")
            .is_none());
        value.as_object_mut().unwrap().remove("id");
        assert!(serde_json::from_value::<super::SessionRecord>(value).is_err());
    }
}

#[test]
fn canonical_ids_session_wire_decodes_structured_ids_and_keeps_native_bytes() {
    let fixture = include_str!("../../../../../tests/fixtures/dto/canonical_ids_session.json");
    let session: super::SessionRecord = serde_json::from_str(fixture).unwrap();
    let wire = serde_json::to_value(&session).unwrap();
    assert_eq!(
        wire["open_argv"][3], "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "argv is opaque"
    );
    let mut fields = wire.clone();
    fields.as_object_mut().unwrap().remove("open_argv");
    let wire = fields.to_string();
    for structured in [
        "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "TASK_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC",
        "DDDDDDDD-DDDD-4DDD-8DDD-DDDDDDDDDDDD",
        "HOME_EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE",
        "FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF",
    ] {
        assert!(!wire.contains(structured), "{structured}: {wire}");
        assert!(wire.contains(&structured.to_ascii_lowercase()), "{wire}");
    }
    for opaque in [
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        "Turn-A",
        "/Case/Worktree",
    ] {
        assert!(wire.contains(opaque), "{opaque}: {wire}");
    }
    let again: super::SessionRecord =
        serde_json::from_value(serde_json::to_value(&session).unwrap()).unwrap();
    assert_eq!(again, session);
}

#[tokio::test]
async fn session_work_labels_follow_stable_ancestry_without_a_roadmap() {
    let directory = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(
        crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            directory.path().join("test.db"),
        ))
        .await
        .unwrap(),
    );
    let wave = crate::work::wave::Wave::new(
        loopflow_model::id::WaveId::new(),
        "product".to_string(),
        directory.path().display().to_string(),
    );
    store.create_wave(&wave).await.unwrap();
    let session = |task_id: Option<loopflow_storage::work::task::TaskId>| LfSession {
        captured: None,
        caller_artifact_key: None,
        id: "conversation".into(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
        input_published: true,
        cwd: directory.path().into(),
        skill: None,
        provider: None,
        model: None,
        node: None,
        iterations: None,
        task_id,
        wave_id: Some(wave.id().clone()),
        graph_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: None,
        title: "Conversation".into(),
        title_source: crate::session::TitleSource::Generated,
        request: None,
        completed_at: None,
        created_at: 1,
    };
    assert_eq!(
        super::session_work_path(&store, &session(None))
            .await
            .unwrap()
            .as_deref(),
        Some("product")
    );
    let unbound = LfSession {
        wave_id: None,
        ..session(None)
    };
    assert_eq!(
        super::session_work_path(&store, &unbound).await.unwrap(),
        None
    );
    let missing = loopflow_storage::work::task::TaskId::new();
    assert_eq!(
        super::session_work_path(&store, &session(Some(missing.clone())))
            .await
            .unwrap(),
        Some(format!("product / Task {missing} (unavailable)"))
    );
}

#[test]
fn old_captured_membership_keeps_its_provenance_without_a_fabricated_node() {
    use crate::session_record::SessionGraphStep;
    // Captures from before node keys carried a leaf index and scalar visit.
    let old: SessionGraphStep = serde_json::from_value(serde_json::json!({
        "task_id": null, "task_pr_id": null, "invocation_id": "process_old",
        "flow": "review", "step": "explore", "iteration": 3, "step_index": 1
    }))
    .unwrap();
    assert_eq!((old.node, old.key, old.iterations), (None, None, None));
    assert_eq!(old.invocation_id, "process_old");
}

#[test]
fn human_sessions_open_through_the_public_session_command() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    let argv = human_session_argv("session_123").unwrap();

    assert_eq!(
        &argv[argv.len() - 3..],
        ["session", "resume", "session_123"]
    );
    assert!(argv.contains(&format!("LF_HOME={}", home.home.path().display())));
    assert!(!argv.iter().any(|argument| argument == "tmux"));
}

#[test]
fn membership_wire_ids_are_derived_from_their_captures() {
    use crate::graph::compiled::CompiledGraph;
    use crate::graph::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};
    fn skill(name: &str, human: bool) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            interactive: human,
            interactive_name: None,
            id: None,
            returns: None,
            sources: Vec::new(),
        })
    }
    let opening = || vec![skill("kickoff", false), skill("review-design", true)];
    let mut past = opening();
    past.push(skill("implement", false));
    let mut current = past.clone();
    current.push(skill("compress", false));
    current.push(ConcreteStep::Xor(ConcreteXor {
        router: Skill::named("route"),
        sources: Vec::new(),
        paths: std::collections::BTreeMap::from([
            (
                "fix".into(),
                ConcretePath {
                    description: "Repair".into(),
                    steps: vec![skill("patch", false), skill("implement", false)],
                },
            ),
            (
                "skip".into(),
                ConcretePath {
                    description: "Skip".into(),
                    steps: vec![],
                },
            ),
        ]),
    }));
    current.extend([
        skill("loop-or-next", false),
        skill("pr-publish", false),
        skill("demo", true),
    ]);
    let expected = std::collections::BTreeMap::from([
        (
            "00000000-0000-0000-0000-00000000f10w".to_string(),
            crate::graph::compiled::compile("task-design", &opening()),
        ),
        (
            "00000000-0000-0000-0000-0000000000f1".to_string(),
            crate::graph::compiled::compile("feature", &past),
        ),
        (
            "00000000-0000-0000-0000-0000000000f2".to_string(),
            crate::graph::compiled::compile("feature", &current),
        ),
    ]);
    let graphs: std::collections::BTreeMap<String, CompiledGraph> = serde_json::from_str(
        include_str!("../../../../../tests/fixtures/dto/session_membership_graphs.json"),
    )
    .unwrap();
    assert_eq!(graphs, expected);
    let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_memberships.json"
    ))
    .unwrap();
    for session in sessions {
        if let super::SessionGraphMembership::Step {
            graph_lf_process_id,
            step,
            node: Some(node),
            ..
        } = session.graph_membership
        {
            assert_eq!(
                graphs[&graph_lf_process_id].node_at(node).unwrap().label,
                step
            );
        }
    }
}

#[test]
fn binding_preview_requires_exact_identity_and_label() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_binding_preview.json"
    ))
    .unwrap();
    let preview: super::SessionBindingPreview = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(preview).unwrap(), value);
    for key in ["session_id", "task_id", "identifier", "title"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<super::SessionBindingPreview>(missing).is_err());
    }
}

#[test]
fn session_page_preserves_the_continuation_and_required_entries() {
    let json = include_str!("../../../../../tests/fixtures/dto/session_page.json");
    let page: super::SessionPage = serde_json::from_str(json).unwrap();
    assert_eq!(
        serde_json::to_value(page).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
    assert!(serde_json::from_str::<super::SessionPage>(r#"{"next":null}"#).is_err());
}

#[test]
fn opening_reads_as_what_resume_does_there() {
    use super::SessionResumeIntent::*;
    let _lock = crate::journal::test_env_lock();
    let _home = SessionHome::new();
    let mut summary = summary_fixture();
    summary.captured = None;
    let label = |intent, summary: &crate::session::SessionSummary| {
        super::opening(intent, summary).map(|action| action.label)
    };
    assert_eq!(label(Resume, &summary).as_deref(), Some("Open here"));
    assert_eq!(label(TakeOver, &summary).as_deref(), Some("Take over"));
    assert_eq!(
        label(Start, &summary).as_deref(),
        Some("Start conversation")
    );
    assert_eq!(label(View, &summary).as_deref(), Some("Open here"));
    assert_eq!(label(NewThread, &summary), None);
    summary.unviewed = true;
    assert_eq!(label(View, &summary).as_deref(), Some("Reconnect"));
    // A captured launch whose marker the launcher consumed.
    summary.captured = Some(1);
    assert_eq!(label(Start, &summary).as_deref(), Some("Try again"));
    let dir = super::local_capture_dir(summary.artifact_key.as_deref().unwrap()).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("prepared"), "").unwrap();
    assert_eq!(
        label(Start, &summary).as_deref(),
        Some("Start conversation")
    );
    summary.interactive = false;
    assert_eq!(label(Resume, &summary), None);
}

#[test]
fn one_decision_reads_a_sessions_holder_process_and_history() {
    use super::SessionResumeIntent::*;
    use super::SessionState::*;
    let intent = |state| super::resume_intent(&summary_in(state, true));
    assert_eq!(intent(Detached), View);
    assert_eq!(intent(Active), TakeOver);
    assert_eq!(intent(Closed), Resume);
    assert_eq!(intent(Unknown), Resume);
    let mut admitted = summary_in(Unknown, true);
    admitted.agent_process_id = Some(loopflow_model::id::AgentProcessId::new());
    assert_eq!(
        super::resume_intent(&admitted),
        TakeOver,
        "an admitted holder nothing says ended"
    );
    let mut fresh = summary_in(Closed, true);
    fresh.has_provider_history = false;
    assert_eq!(super::resume_intent(&fresh), Start);
}

#[test]
fn opening_an_admitted_holder_of_unknown_liveness_takes_it_over() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let _claude = crate::config::settings::select_invocation(
            serde_yaml_ng::from_str("agent: claude").unwrap(),
        );
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let session = store.session(&record.id).await.unwrap().unwrap();
        let session = super::append_input(&store, session).await.unwrap();
        let observation = crate::session_record::provider_session_observation(
            &session,
            session.input_key().unwrap(),
            &"claude-thread".into(),
            None,
        );
        store
            .sqlite
            .retain_session_observation(&session, &observation)
            .unwrap();
        // A launcher still running that has not recorded its provider's pid.
        let launcher = crate::id::LfProcessId::new();
        rusqlite::Connection::open(home.home.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,?2)",
                rusqlite::params![&launcher, loopflow_storage::rows::now_unix()],
            )
            .unwrap();
        let holder =
            attachment::claim_session_attachment(&store.sqlite, &session.id, None, &launcher)
                .unwrap();
        let summary = super::current_summary(&store, &session.id).unwrap();
        assert_eq!(
            summary.agent_process_evidence,
            crate::store::ProcessIdentityEvidence::Unknown
        );
        assert_eq!(
            listed_session(&store, &session.id).action.unwrap().label,
            "Take over"
        );

        let opened = super::open(
            &store,
            repo.path(),
            &session.id,
            None,
            None,
            false,
            true,
            Default::default(),
        )
        .await;
        let message = opened.unwrap_err().to_string();
        assert!(
            message.contains("resume was not started"),
            "opening resumed beside a holder it could not stop: {message}"
        );
        assert_eq!(
            store.sqlite.session_attachment(&session.id).unwrap(),
            Some(holder)
        );
    });
}

#[test]
fn resuming_an_ended_conversation_launches_its_provider_as_the_session() {
    use std::os::unix::fs::PermissionsExt;
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    let bin = tempfile::tempdir().unwrap();
    let received = bin.path().join("received");
    let claude = bin.path().join("claude");
    std::fs::write(
        &claude,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nprintf '%s ' \"$@\" > '{}'\n",
            received.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The thread's transcript is still here, so it resumes rather than starting fresh.
    let _claude_home = crate::test_ambient::EnvGuard::clear(&["CLAUDE_CONFIG_DIR"]);
    let claude_home = tempfile::tempdir().unwrap();
    let project = claude_home.path().join("projects").join("-home");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("claude-thread.jsonl"), "{}\n").unwrap();
    std::env::set_var("CLAUDE_CONFIG_DIR", claude_home.path());
    // The launch records the CLI it would hand children; never the Machine's.
    std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::set_var(
        "PATH",
        std::env::join_paths(
            std::iter::once(bin.path().to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .unwrap(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let opened = crate::journal::with_runtime(home, &["lf".into(), "resume".into()], || {
        runtime.block_on(async {
            let store = std::sync::Arc::new(
                crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    home.join("loopflow.db"),
                ))
                .await
                .unwrap(),
            );
            // A bare conversation: no Task, Wave or repository holds it.
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                crate::session_record::SessionCaptureSpec {
                    harness: "claude".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    graph: crate::session_record::SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .unwrap();
            let session = store
                .sqlite
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            let observation = crate::session_record::provider_session_observation(
                &session,
                session.input_key().unwrap(),
                &"claude-thread".into(),
                None,
            );
            store
                .sqlite
                .retain_session_observation(&session, &observation)
                .unwrap();
            assert_eq!(
                super::invocation_intent(
                    &session,
                    &super::current_summary(&store, &session.id).unwrap(),
                    false,
                    false,
                ),
                super::SessionResumeIntent::Resume
            );
            super::open(
                &store,
                home,
                &session.id,
                None,
                None,
                false,
                true,
                Default::default(),
            )
            .await
            .map(|_| ())
        })
    });
    std::env::set_var("PATH", path);
    opened.unwrap();
    let argv = std::fs::read_to_string(&received).unwrap();
    assert!(argv.contains("claude-thread"), "{argv}");
}

#[test]
fn preparing_a_consumed_first_launch_offers_try_again_and_changes_nothing() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let before = store.session(&record.id).await.unwrap().unwrap();
        let first = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        assert_eq!(
            first.open_argv,
            super::human_session_argv(&record.id).unwrap()
        );
        for refreshed in [
            super::surface(&store, &before).await.unwrap(),
            listed_session(&store, &record.id),
            super::list(&store, &crate::session::SessionFilter::default())
                .await
                .unwrap()
                .into_iter()
                .find(|row| row.id == record.id)
                .unwrap(),
        ] {
            assert_eq!(refreshed.action, first.action);
            assert_eq!(refreshed.action.unwrap().label, "Start conversation");
        }
        let unchanged = store.session(&record.id).await.unwrap().unwrap();
        assert_eq!(unchanged.captured, before.captured);
        assert_eq!(unchanged.input_published, before.input_published);

        let session = super::append_input(&store, unchanged).await.unwrap();
        let (dir, _) = crate::session_record::resolve_manifest(
            &crate::store::lf_home_dir(),
            session.input_key().unwrap(),
        )
        .unwrap();
        // The launcher consumes this marker before starting the provider; the
        // next opening gives the request another attempt.
        std::fs::remove_file(dir.join("prepared")).unwrap();
        let listed = listed_session(&store, &record.id);
        assert_eq!(listed.action.as_ref().unwrap().label, "Try again");
        // `--explain` reads its Action and the opening from one decision.
        let explained = super::explain_resume(
            &store,
            std::path::Path::new("/"),
            super::SessionTarget::Session(&record.id),
            false,
            false,
        )
        .await;
        assert_eq!(
            explained.action.unwrap().intent,
            super::SessionResumeIntent::Start
        );
        assert_eq!(explained.opening, listed.action);
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        assert_eq!(prepared.action, listed.action);
        let after = store.session(&record.id).await.unwrap().unwrap();
        assert_eq!(after.captured, session.captured);
        assert_eq!(after.artifact_key, session.artifact_key);
        assert!(store
            .sqlite
            .session_attachment(&record.id)
            .unwrap()
            .is_none());
    });
}

#[test]
fn recovery_refresh_follows_current_holder_and_history() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        assert_eq!(prepared.action.unwrap().label, "Start conversation");

        // A launcher that may still be running; an ended one would make its
        // unidentified agent dead instead of unknown.
        let launcher = store.sqlite.test_lf_process();
        rusqlite::Connection::open(crate::store::default_db_path())
            .unwrap()
            .execute(
                "UPDATE processes SET started_at=strftime('%s','now') WHERE id=?1",
                [&launcher],
            )
            .unwrap();
        let run = attachment::claim_session_attachment(&store.sqlite, &record.id, None, &launcher)
            .unwrap();
        store
            .sqlite
            .record_agent_process_launch(
                &record.id,
                &run,
                &std::process::Command::new("fixture-provider"),
                true,
            )
            .unwrap();
        // An admitted run with unknown OS evidence: opening takes it over,
        // and preparing leaves it attached.
        let refreshed = listed_session(&store, &record.id);
        assert_eq!(refreshed.action.as_ref().unwrap().label, "Take over");
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        assert_eq!(prepared.action, refreshed.action);
        assert_eq!(
            store.sqlite.session_attachment(&record.id).unwrap(),
            Some(run.clone())
        );

        store
            .sqlite
            .record_session_agent_session(&record.id, &run, &"saved-thread".into())
            .unwrap();
        store
            .sqlite
            .record_agent_process_exit(&record.id, &run, true)
            .unwrap();
        let refreshed = listed_session(&store, &record.id);
        assert_eq!(refreshed.action.unwrap().label, "Start conversation");
    });
}

#[test]
fn recovery_refresh_recognizes_imported_history_before_attachment() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let session = store.session(&record.id).await.unwrap().unwrap();
        let session = super::append_input(&store, session).await.unwrap();
        let observation = crate::session_record::provider_session_observation(
            &session,
            session.input_key().unwrap(),
            &"imported-history".into(),
            None,
        );
        store
            .sqlite
            .retain_session_observation(&session, &observation)
            .unwrap();
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        let refreshed = listed_session(&store, &record.id);
        assert_eq!(prepared.action, refreshed.action);
        assert_eq!(refreshed.action.unwrap().label, "Open here");
        assert!(store
            .sqlite
            .session_attachment(&record.id)
            .unwrap()
            .is_none());
        // A new captured input cannot borrow the older input's resume reference.
        let next = super::append_input(&store, session).await.unwrap();
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &next.id,
            None,
            None,
            false,
            false,
            Default::default(),
        )
        .await
        .unwrap()
        .prepared()
        .unwrap();
        let refreshed = listed_session(&store, &next.id);
        assert_eq!(prepared.action, refreshed.action);
        assert_eq!(refreshed.action.unwrap().label, "Start conversation");
    });
}

#[test]
fn a_session_switched_to_another_provider_resumes_on_it() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = {
            let _claude = crate::config::settings::select_invocation(
                serde_yaml_ng::from_str("agent: claude").unwrap(),
            );
            super::primary::ensure(&store, repo.path(), None)
                .await
                .unwrap()
        };
        let session = store.session(&record.id).await.unwrap().unwrap();
        assert_eq!(session.provider.as_deref(), Some("claude"));

        // `lf -a codex resume`: the input it appends runs on Codex.
        let switched = {
            let _codex = crate::config::settings::select_invocation(
                serde_yaml_ng::from_str("agent: codex").unwrap(),
            );
            super::append_input(&store, session).await.unwrap()
        };
        assert_eq!(switched.provider.as_deref(), Some("codex"));
        let (_, manifest) = crate::session_record::resolve_manifest(
            &crate::store::lf_home_dir(),
            switched.input_key().unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.harness, "codex");
        let observation = crate::session_record::provider_session_observation(
            &switched,
            switched.input_key().unwrap(),
            &"codex-thread".into(),
            None,
        );
        store
            .sqlite
            .retain_session_observation(&switched, &observation)
            .unwrap();

        // A plain resume stays on Codex, in Codex's conversation.
        let resumed = store.session(&record.id).await.unwrap().unwrap();
        assert_eq!(resumed.provider.as_deref(), Some("codex"));
        let config =
            crate::agent::resolve_session_config(&resumed, serde_yaml_ng::Value::Null, None)
                .unwrap();
        assert_eq!(config.agent.to_string(), "codex");
        assert_eq!(config.resume_token, Some("codex-thread".into()));
    });
}

#[test]
fn opening_any_primary_without_a_request_only_reads() {
    let opening = super::first_turn(None);
    assert!(opening.contains("change no Work"));
    assert!(opening.contains("launch no Graph"));
    assert!(opening.contains("claim or start no Task"));
    assert!(opening.contains("Steers delivered with this turn are recorded direction"));
    assert_eq!(
        super::first_turn(Some("land it")),
        "land it",
        "a person's stored request is the first turn"
    );
    for skill in [
        loopflow_definitions::builtins::get_builtin_skill("task-session").unwrap(),
        loopflow_definitions::builtins::get_builtin_skill("repo-session").unwrap(),
        loopflow_definitions::builtins::get_builtin_skill("wave-session").unwrap(),
    ] {
        let skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(skill.contains("A first turn without a request"));
        assert!(skill.contains("with no Work changed"));
        assert!(skill.contains("report them, and act when asked here"));
        assert!(!skill.contains("without waiting for a greeting"));
    }
}

#[test]
fn creation_retry_reconciles_publication_but_never_repeats_a_stop_or_append() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let retry = |request| {
            super::new_thread(&store, &record.id, None, false, Default::default(), request)
        };
        let receipt = |request: &str| store.sqlite.creation_receipt(request).unwrap().unwrap();
        let original = store.session(&record.id).await.unwrap().unwrap();
        let identity = crate::engine::creation::identity(
            "session.new",
            "interrupted",
            &serde_json::json!({"id":record.id,"agent":null}),
            false,
        )
        .unwrap();
        let artifact = crate::session_record::new_artifact_key();
        store
            .sqlite
            .begin_thread_creation(&identity, &record.id, &artifact)
            .unwrap();
        let stopped = receipt("interrupted");
        assert_eq!(
            stopped.disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Inspect
        );
        assert_eq!(stopped.explanation, "Conversation change unconfirmed");
        // A crash while stopping is uncertain, even if no provider is live now.
        let refused = retry("interrupted").await.unwrap_err();
        assert_eq!(
            refused.downcast_ref::<crate::engine::Error>().unwrap().code,
            crate::engine::ErrorCode::OutcomeUnknown
        );
        assert_eq!(
            store.session(&record.id).await.unwrap().unwrap().captured,
            original.captured
        );

        let mut next = original.clone();
        next.artifact_key = Some(artifact.clone());
        next.input_published = false;
        let next = store
            .replace_session_input(original.captured, next, Some("interrupted".into()))
            .await
            .unwrap();
        assert_eq!(receipt("interrupted").milestone, "admitted");
        assert_eq!(
            receipt("interrupted").disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Continue
        );
        // Simulate app/server death after the atomic append, before publication.
        let recovered = retry("interrupted").await.unwrap().0;
        assert_eq!(recovered.captured, next.captured);
        assert_eq!(recovered.artifact_key.as_deref(), Some(artifact.as_str()));
        assert!(recovered.input_published);
        let result = receipt("interrupted").result;
        assert!(result.is_some());
        assert_eq!(
            receipt("interrupted").disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Complete
        );
        let mut newer = recovered.clone();
        newer.request = Some("A later independent input".into());
        let newer = super::append_input(&store, newer).await.unwrap();
        retry("interrupted").await.unwrap();
        assert_eq!(
            store.session(&record.id).await.unwrap().unwrap().captured,
            newer.captured
        );
        assert_eq!(receipt("interrupted").result, result);

        let identity = crate::engine::creation::identity(
            "session.new",
            "superseded",
            &serde_json::json!({"id":record.id,"agent":null}),
            false,
        )
        .unwrap();
        let artifact = crate::session_record::new_artifact_key();
        store
            .sqlite
            .begin_thread_creation(&identity, &record.id, &artifact)
            .unwrap();
        let mut pending = newer.clone();
        pending.artifact_key = Some(artifact);
        pending.input_published = false;
        let pending = store
            .replace_session_input(newer.captured, pending, Some("superseded".into()))
            .await
            .unwrap();
        // A lookup can permit continuation, then newer input wins before retry.
        assert_eq!(
            receipt("superseded").disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Continue
        );
        let newest = super::append_input(&store, pending).await.unwrap();
        let refused = retry("superseded").await.unwrap_err();
        assert_eq!(
            refused.downcast_ref::<crate::engine::Error>().unwrap().code,
            crate::engine::ErrorCode::OutcomeUnknown
        );
        let superseded = receipt("superseded");
        assert_eq!(
            superseded.disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Inspect
        );
        assert_eq!(
            superseded.explanation,
            "Newer conversation input replaced this change"
        );
        assert_eq!(
            store.session(&record.id).await.unwrap().unwrap().captured,
            newest.captured
        );

        let identity = crate::engine::creation::identity(
            "session.new",
            "published",
            &serde_json::json!({"id":record.id,"agent":null}),
            false,
        )
        .unwrap();
        let artifact = crate::session_record::new_artifact_key();
        store
            .sqlite
            .begin_thread_creation(&identity, &record.id, &artifact)
            .unwrap();
        let mut pending = newest.clone();
        pending.artifact_key = Some(artifact);
        pending.input_published = false;
        let pending = store
            .replace_session_input(newest.captured, pending, Some("published".into()))
            .await
            .unwrap();
        let published = super::publish_prepared_input(
            &store.sqlite,
            &pending,
            crate::session_record::SessionGraphMembership::Independent,
        )
        .unwrap();
        // Publication succeeded but the result was not recorded before exit.
        let recovered = retry("published").await.unwrap().0;
        assert_eq!(recovered.captured, published.captured);
        assert!(recovered.input_published);
        assert_eq!(
            receipt("published").disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Complete
        );
    });
}

#[test]
fn new_thread_before_a_first_provider_keeps_the_original_prompt_and_retries_once() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let first = super::new_thread(&store, &record.id, None, false, Default::default(), "first")
            .await
            .unwrap()
            .0;
        let retry = super::new_thread(&store, &record.id, None, false, Default::default(), "first")
            .await
            .unwrap()
            .0;
        assert_eq!(first.captured, retry.captured);
        assert_eq!(first.request, retry.request);
        assert!(store
            .sqlite
            .provider_transition(&record.id)
            .unwrap()
            .is_none());
    });
}

/// A held Task conversation nobody views is the Task's, and any terminal
/// attaches to it as one more view.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_held_task_conversation_without_a_view_offers_attach() {
    let _lock = crate::journal::test_env_lock();
    let logins = LiveSessionFixture::new().await;
    let session = logins.codex_session("held", None);
    let _agent = logins.run(&session);
    let task = TaskId::new();
    let created = logins
        .store
        .sqlite
        .create_task(&loopflow_storage::work::task::NewTask {
            parent_task_id: None,
            repository_id: crate::repository::location::ensure_repository(
                &logins.store.sqlite,
                "/fixture",
            )
            .unwrap(),
            id: task.clone(),
            project_id: None,
            title: "Held".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let terminal = loopflow_model::id::TerminalId::new();
    rusqlite::Connection::open(&logins.database)
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO held_terminals(id,holder_build,created_at,viewer_hold,viewer_hold_at,viewed)
                 VALUES('{terminal}','fixture',1,'absent',1,0);
             UPDATE agent_sessions SET task_id='{task}',held_terminal_id='{terminal}' WHERE id='held';"
        ))
        .unwrap();

    let rows = super::list(&logins.store, &crate::session::SessionFilter::default())
        .await
        .unwrap();
    let row = rows.iter().find(|row| row.id == session.id).unwrap();
    assert_eq!(row.state, super::SessionState::Detached);
    assert_eq!(row.work_path.as_deref(), Some(created.identifier.as_str()));
    let action = row.action.as_ref().unwrap();
    assert_eq!(action.label, "Reconnect");
    assert!(action.help.ends_with(&format!("lf resume {}", session.id)));
}

struct ListFilters {
    home: SessionHome,
    store: SharedStore,
    wave: loopflow_model::id::WaveId,
    repo: loopflow_test_support::TestRepo,
}

impl ListFilters {
    async fn new() -> Self {
        let home = SessionHome::new();
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let wave = loopflow_model::id::WaveId::new();
        let project = loopflow_storage::work::project::ProjectId::new();
        let task = loopflow_model::durable::TaskId::new();
        let db = rusqlite::Connection::open(home.home.path().join("loopflow.db")).unwrap();
        db.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'craft',?2,1)",
            rusqlite::params![wave, repo.path().to_str().unwrap()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000000000000000000000001',?1,1)",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
            rusqlite::params![project.as_str(), wave],
        )
        .unwrap();
        db.execute("INSERT INTO tasks(id,repository_id,project_id,issue_identifier,created_at) VALUES(?1,'repo_00000000000000000000000000000001',?2,'TEST-1',1)", rusqlite::params![task.as_str(), project.as_str()]).unwrap();
        db.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
            rusqlite::params![task.as_str(), repo.path().to_str().unwrap()],
        )
        .unwrap();
        for id in [
            "outside",
            "wave-primary",
            "ad-hoc",
            "task-primary",
            "former",
            "bare",
            "repository-primary",
            "repository-in-task",
        ] {
            store
                .sqlite
                .test_session(id, &crate::session_record::new_artifact_key());
            db.execute("UPDATE agent_sessions SET title=id WHERE id=?1", [id])
                .unwrap();
        }
        db.execute("UPDATE agent_sessions SET wave_id=?1 WHERE id IN ('wave-primary','ad-hoc','task-primary','former')", [&wave]).unwrap();
        db.execute(
            "UPDATE agent_sessions SET primary_scope='wave' WHERE id='wave-primary'",
            [],
        )
        .unwrap();
        db.execute("UPDATE agent_sessions SET primary_scope='repository', repo=id WHERE id IN ('repository-primary','repository-in-task')", []).unwrap();
        db.execute(
            "UPDATE agent_sessions SET task_id=?1 WHERE id IN ('task-primary','former')",
            [task.as_str()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO task_primary_sessions(task_id,session_id) VALUES(?1,'task-primary')",
            [task.as_str()],
        )
        .unwrap();
        db.execute(
            "UPDATE agent_sessions SET cwd=?1 WHERE id IN ('bare','repository-in-task')",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
        db.execute(
            "UPDATE agent_sessions SET task_id=?1,wave_id=?2 WHERE id='bare'",
            rusqlite::params![task.as_str(), wave],
        )
        .unwrap();
        Self {
            home,
            store,
            wave,
            repo,
        }
    }

    async fn ids(&self, filter: crate::session::SessionFilter) -> Vec<String> {
        let mut ids = super::list(&self.store, &filter)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.id.to_string())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

#[test]
fn list_wave_includes_attribution_and_raw_checkout_conversations_before_paging() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(ListFilters::new());
    let filter = crate::session::SessionFilter {
        wave: Some(fixture.wave.clone()),
        ..Default::default()
    };
    let raw = crate::observe::Cache::with_raw_conversations(vec![
        crate::transcript::raw::RawDirectory::fixture(
            fixture.repo.path().to_path_buf(),
            "raw-in-task",
        ),
        crate::transcript::raw::RawDirectory::fixture(
            fixture.home.home.path().to_path_buf(),
            "raw-outside",
        ),
    ]);
    raw.capture(|| {
        runtime.block_on(async {
            assert_eq!(
                fixture.ids(filter.clone()).await,
                [
                    "ad-hoc",
                    "bare",
                    "former",
                    "raw-in-task",
                    "task-primary",
                    "wave-primary"
                ]
            );
            let filter = crate::session::SessionFilter {
                after: Some("outside".into()),
                limit: 1,
                ..filter
            };
            assert_eq!(fixture.ids(filter).await, ["task-primary"]);
        })
    });
}

#[test]
fn list_task_keeps_each_session_with_its_task_after_a_move_and_path_reuse() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let home = SessionHome::new();
        let store = home.store().await;
        let reused = loopflow_test_support::TestRepo::new();
        let path = reused.path().to_str().unwrap();
        let db = rusqlite::Connection::open(home.home.path().join("loopflow.db")).unwrap();
        db.execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000000000000000000000001','/moved',1)",
            [],
        )
        .unwrap();
        // Task A's checkout moved with its repository; Task C's clone took the
        // old path, where A's Session still says it ran.
        let (a, c) = (
            loopflow_model::durable::TaskId::new(),
            loopflow_model::durable::TaskId::new(),
        );
        for (task, identifier, worktree) in [(&a, "TEST-1", "/moved/task"), (&c, "TEST-2", path)] {
            db.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES(?1,'repo_00000000000000000000000000000001',?2,1)", [task.as_str(), identifier]).unwrap();
            db.execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                [task.as_str(), worktree],
            )
            .unwrap();
        }
        for (id, task) in [("first", &a), ("second", &c)] {
            store
                .sqlite
                .test_session(id, &crate::session_record::new_artifact_key());
            db.execute(
                "UPDATE agent_sessions SET task_id=?1,cwd=?2 WHERE id=?3",
                [task.as_str(), path, id],
            )
            .unwrap();
        }
        let listed = |task: &loopflow_model::durable::TaskId| {
            let filter = crate::session::SessionFilter {
                task: Some(task.to_string()),
                ..Default::default()
            };
            let store = store.clone();
            async move {
                super::list(&store, &filter)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|row| (row.id.to_string(), row.task_ids))
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(listed(&a).await, [("first".to_owned(), vec![a.clone()])]);
        assert_eq!(listed(&c).await, [("second".to_owned(), vec![c.clone()])]);
    });
}

#[test]
fn list_orphan_means_no_task_and_no_wave_including_raw_conversations() {
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(ListFilters::new());
    let raw = crate::observe::Cache::with_raw_conversations(vec![
        crate::transcript::raw::RawDirectory::fixture(
            fixture.repo.path().to_path_buf(),
            "raw-in-task",
        ),
        crate::transcript::raw::RawDirectory::fixture(
            fixture.home.home.path().to_path_buf(),
            "raw-outside",
        ),
    ]);
    raw.capture(|| {
        runtime.block_on(async {
            let filter = crate::session::SessionFilter {
                orphan: true,
                ..Default::default()
            };
            assert_eq!(
                fixture.ids(filter.clone()).await,
                [
                    "outside",
                    "raw-outside",
                    "repository-in-task",
                    "repository-primary"
                ]
            );
            assert_eq!(
                fixture
                    .ids(crate::session::SessionFilter {
                        raw: false,
                        ..filter
                    })
                    .await,
                ["outside", "repository-in-task", "repository-primary"]
            );
        })
    });
}

#[test]
fn list_primary_selects_current_scopes_before_paging() {
    use crate::session::{SessionFilter, SessionPrimary};
    let _lock = crate::journal::test_env_lock();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(ListFilters::new());
    runtime.block_on(async {
        for (primary, expected) in [
            (
                SessionPrimary::All,
                vec![
                    "repository-in-task",
                    "repository-primary",
                    "task-primary",
                    "wave-primary",
                ],
            ),
            (SessionPrimary::Task, vec!["task-primary"]),
            (SessionPrimary::Wave, vec!["wave-primary"]),
            (
                SessionPrimary::Repository,
                vec!["repository-in-task", "repository-primary"],
            ),
        ] {
            let filter = SessionFilter {
                primary: Some(primary),
                ..Default::default()
            };
            assert_eq!(fixture.ids(filter.clone()).await, expected);
            assert_eq!(
                fixture.ids(SessionFilter { limit: 1, ..filter }).await,
                expected[..1]
            );
        }
        assert_eq!(
            fixture
                .ids(SessionFilter {
                    primary: Some(SessionPrimary::Wave),
                    wave: Some(fixture.wave.clone()),
                    limit: 1,
                    ..Default::default()
                })
                .await,
            ["wave-primary"]
        );
    });
}

/// `lf machine forward --session` after a refresh that left Codex without a
/// usable login: the restart is refused before the running provider stops.
#[test]
fn a_restart_without_a_usable_login_leaves_the_running_provider_alone() {
    let _lock = crate::journal::test_env_lock();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = LiveSessionFixture::new().await;
        let bin = tempfile::tempdir().unwrap();
        let codex = bin.path().join("codex");
        std::fs::write(&codex, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let _path = crate::test_ambient::EnvGuard::clear(&["PATH", "CODEX_ACCESS_TOKEN"]);
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.path().display()));

        let session = fixture.codex_session("forward-refresh", None);
        let mut provider = fixture.run(&session);
        let session = fixture.store.sqlite.session(&session.id).unwrap().unwrap();
        let refused = super::restart_provider(&fixture.store.sqlite, &session)
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("was not restarted: codex has no usable login"),
            "{refused}"
        );
        assert!(
            provider.0.try_wait().unwrap().is_none(),
            "the running provider was stopped"
        );
    });
}

/// A repository Session or chat a steer could not start says why, and which
/// command opens it; nothing else starts it again.
#[test]
fn a_stopped_session_that_is_not_a_tasks_says_why_its_start_failed() {
    let mut summary = summary_fixture();
    summary.agent_process_id = Some(loopflow_model::id::AgentProcessId::new());
    summary.agent_process_evidence = crate::store::ProcessIdentityEvidence::Dead;
    summary.agent_end = Some(crate::session::AgentEnd {
        at: 4_000,
        exit_code: Some(1),
    });
    let login = "codex has no usable login on this Machine; run `codex login`";
    summary.start_failure = Some(format!(
        "{}{login}",
        crate::ops::recovery::WAITING_FOR_HARNESS
    ));
    let waiting = surfaced(&summary);
    assert_eq!(
        line(&waiting),
        format!("could not start: {login}; `lf resume metadata` opens it")
    );
    let outlook = waiting.outlook.unwrap();
    assert_eq!(
        (outlook.kind, outlook.reason.as_deref()),
        (super::SessionOutlookKind::CouldNotStart, Some(login))
    );

    // A held terminal that did not start.
    summary.start_failure = Some("no PTY left".into());
    let failed = surfaced(&summary);
    assert!(
        line(&failed)
            .ends_with(" (exit 1) · last start failed: no PTY left; `lf resume metadata` opens it"),
        "{}",
        line(&failed)
    );
}
