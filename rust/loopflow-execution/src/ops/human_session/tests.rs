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
        let attachment = attachment::claim_session_attachment(
            &self.store.sqlite,
            &session.id,
            None,
            &parent,
            true,
        )
        .unwrap();
        let mut command = std::process::Command::new("/bin/sleep");
        command.env_clear().arg("60");
        RunningAgent(
            crate::harness::agent_process::spawn_native(
                command,
                &(
                    self.store.sqlite.clone(),
                    session.id.to_string(),
                    attachment,
                ),
                crate::session::Holder::Caller,
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
    crate::journal::with_process(|| {
        crate::journal::admit_process(home, &["lf".into(), "session".into()]);
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
            let first = attachment::claim_session_attachment(&store.sqlite, &session.id, None, &launcher, true)?;
            let mut command = tokio::process::Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut provider = crate::harness::agent_process::spawn(
                command,
                None,
                &(store.sqlite.clone(), session.id.to_string(), first.clone()),
                crate::session::Holder::Caller,
            )?;
            let opened = super::open(&store, home, &session.id, false, true, false, false).await;
            let forked = super::open(&store, home, &session.id, true, true, false, false).await;
            let running = provider.try_wait()?.is_none();
            provider.kill().await?;
            assert!(running, "resume stopped the Run's provider");
            for refused in [opened, forked] {
                let message = refused.unwrap_err().to_string();
                assert!(message.contains("is a Run (implement · running)"), "{message}");
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

#[test]
fn session_metadata_keeps_unknown_position_visible() {
    let task = TaskId::new();
    let wave = loopflow_model::id::WaveId::new();
    let mut summary = crate::session::SessionSummary {
        transition: None,
        preview: None,
        has_provider_history: true,
        program_status: None,
        agent_process_id: None,
        primary_scope: None,
        agent_process_evidence: crate::journal::ProcessIdentityEvidence::Unknown,
        available: false,
        terminal_id: None,
        unviewed: false,
        held: false,
        task_terminal: false,
        task_primary: false,
        task_ids: vec![task.clone()],
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
    };
    let row = super::summary_surface(&summary);
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
        super::summary_surface(&summary).graph_membership,
        super::SessionGraphMembership::Step {
            occurrence: super::SessionGraphOccurrence::Current,
            ..
        }
    ));
    summary.graph.as_mut().unwrap().state = crate::session::GraphProcessSummaryState::Stopped;
    let row = super::summary_surface(&summary);
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
        super::summary_surface(&summary).graph_membership,
        super::SessionGraphMembership::Unknown { .. }
    ));
    summary.independent = true;
    assert_eq!(
        super::summary_surface(&summary).graph_membership,
        super::SessionGraphMembership::Independent
    );
    let row = super::summary_surface(&summary);
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
            crate::journal::ProcessIdentityEvidence::Live,
            super::SessionState::Active,
        ),
        (
            crate::journal::ProcessIdentityEvidence::Dead,
            super::SessionState::Closed,
        ),
        (
            crate::journal::ProcessIdentityEvidence::Unknown,
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
use crate::session::SessionOpenIntent;
pub use crate::test_support::human_session::SessionHome;

use loopflow_storage::work::task::TaskId;

#[test]
fn session_action_fixtures_match_the_shared_boundary() {
    #[derive(serde::Deserialize)]
    struct Case {
        state: super::SessionState,
        actions: Vec<super::SessionAction>,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_actions.json"
    ))
    .unwrap();
    for case in cases {
        assert_eq!(
            super::session_actions(
                case.state,
                true,
                true,
                case.state == super::SessionState::Active,
                false
            ),
            case.actions
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
            session.actions,
            super::session_actions(
                session.state,
                session.interactive,
                true,
                session.state == super::SessionState::Active,
                false
            )
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

#[test]
fn graph_membership_fixtures_cover_every_projection_and_are_required() {
    let sessions: Vec<super::SessionRecord> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/dto/session_memberships.json"
    ))
    .unwrap();
    let mut kinds = Vec::new();
    for session in &sessions {
        assert_eq!(
            session.actions,
            super::session_actions(
                session.state,
                session.interactive,
                true,
                session.state == super::SessionState::Active,
                false
            )
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
        paths: std::collections::HashMap::from([
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
fn only_a_consumed_first_launch_offers_try_again() {
    use super::{SessionActionKind::*, SessionState::*};
    let kinds = |state, history, occupied, consumed| -> Vec<_> {
        super::session_actions(state, true, history, occupied, consumed)
            .into_iter()
            .map(|action| (action.kind, action.label))
            .collect()
    };
    assert_eq!(
        kinds(Closed, false, false, true),
        [
            (MoveHere, "Try again".to_string()),
            (Reveal, "Show conversation".to_string())
        ]
    );
    assert_eq!(kinds(Closed, true, false, true)[0].0, Open);
    assert_eq!(kinds(Closed, false, false, false)[0].0, Open);
    let held = kinds(Active, false, true, true);
    assert_eq!(held[0].0, Open);
    assert_eq!(held[1], (MoveHere, "Take back".to_string()));
    assert!(super::session_actions(Closed, false, false, false, true).is_empty());
}

#[test]
fn automatic_start_prepares_new_history_but_never_replays_consumed_input() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let first = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Start,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(first.open_argv.ends_with(&["--start-if-free".into()]));
        assert!(first.actions.iter().any(|action| {
            action.kind == super::SessionActionKind::Open && action.unavailable_reason.is_none()
        }));

        let session = store.session(&record.id).await.unwrap().unwrap();
        let session = super::append_input(&store, session).await.unwrap();
        let (dir, _) = crate::session_record::resolve_manifest(
            &crate::store::lf_home_dir(),
            session.input_key().unwrap(),
        )
        .unwrap();
        // The launcher consumes this marker before starting the provider. A
        // crash at that boundary must not replay the captured request on load.
        std::fs::remove_file(dir.join("prepared")).unwrap();
        let error = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Start,
            true,
            false,
            false,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("retry it explicitly"));
        let retry = |record: &super::SessionRecord| {
            let action = record
                .actions
                .iter()
                .find(|action| action.kind == super::SessionActionKind::MoveHere)
                .unwrap()
                .clone();
            assert!(!record
                .actions
                .iter()
                .any(|action| action.kind == super::SessionActionKind::Open));
            action
        };
        let listed = listed_session(&store, &record.id);
        assert_eq!(retry(&listed).label, "Try again");
        assert!(retry(&listed).unavailable_reason.is_none());
        let refused = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Start,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert_eq!(retry(&refused), retry(&listed));
        // The click prepares the launch that `open_waiting` gives a new attempt.
        let prepared = super::open(
            &store,
            std::path::Path::new("/"),
            &record.id,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!prepared.open_argv.contains(&"--start-if-free".into()));
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
fn reveal_requires_history_and_never_publishes_a_first_turn() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = home.store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let (record, _) = super::primary::ensure(&store, repo.path(), None)
            .await
            .unwrap();
        let before = store.session(&record.id).await.unwrap().unwrap();
        let prepared = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Reveal,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(prepared.open_argv.ends_with(&["--reveal-existing".into()]));
        let action = prepared
            .actions
            .iter()
            .find(|action| action.kind == super::SessionActionKind::Reveal)
            .unwrap();
        assert!(action
            .unavailable_reason
            .as_ref()
            .unwrap()
            .contains("No saved provider history"));
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
            assert_eq!(refreshed.actions, prepared.actions);
            let start = refreshed
                .actions
                .iter()
                .find(|action| action.kind == super::SessionActionKind::Open)
                .unwrap();
            assert_eq!(start.label, "Start conversation");
            assert!(start.unavailable_reason.is_none());
        }
        let error = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Reveal,
            true,
            false,
            false,
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("start this conversation explicitly"));
        let after = store.session(&record.id).await.unwrap().unwrap();
        assert_eq!(after.artifact_key, before.artifact_key);
        assert_eq!(after.captured, before.captured);
        assert_eq!(after.input_published, before.input_published);
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
        let prepared = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Reveal,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert!(!prepared
            .actions
            .iter()
            .any(|a| a.kind == super::SessionActionKind::MoveHere));

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
        let holder =
            attachment::claim_session_attachment(&store.sqlite, &record.id, None, &launcher, true)
                .unwrap();
        store
            .sqlite
            .record_agent_process_launch(
                &record.id,
                &holder,
                &std::process::Command::new("fixture-provider"),
                true,
            )
            .unwrap();
        // An admitted holder with unknown OS evidence cannot be replaced by a reveal.
        let refreshed = listed_session(&store, &record.id);
        let takeover = refreshed
            .actions
            .iter()
            .find(|a| a.kind == super::SessionActionKind::MoveHere)
            .unwrap();
        assert_eq!(takeover.label, "Take back");
        let reveal = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Reveal,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        assert_eq!(reveal.actions, refreshed.actions);
        assert_eq!(
            store.sqlite.session_attachment(&record.id).unwrap(),
            Some(holder.clone())
        );

        store
            .sqlite
            .record_session_agent_session(&record.id, &holder, &"saved-thread".into())
            .unwrap();
        store
            .sqlite
            .record_agent_process_exit(&record.id, &holder, true)
            .unwrap();
        let refreshed = listed_session(&store, &record.id);
        assert!(!refreshed
            .actions
            .iter()
            .any(|a| a.kind == super::SessionActionKind::MoveHere));
        assert!(refreshed
            .actions
            .iter()
            .find(|a| a.kind == super::SessionActionKind::Reveal)
            .unwrap()
            .unavailable_reason
            .is_some());
        assert_eq!(
            refreshed
                .actions
                .iter()
                .find(|a| a.kind == super::SessionActionKind::Open)
                .unwrap()
                .label,
            "Start conversation"
        );
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
        let prepared = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Reveal,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let refreshed = listed_session(&store, &record.id);
        assert_eq!(prepared.actions, refreshed.actions);
        assert_eq!(
            refreshed
                .actions
                .iter()
                .find(|a| a.kind == super::SessionActionKind::Open)
                .unwrap()
                .label,
            "Open here"
        );
        assert!(refreshed
            .actions
            .iter()
            .find(|a| a.kind == super::SessionActionKind::Reveal)
            .unwrap()
            .unavailable_reason
            .is_none());
        assert!(store
            .sqlite
            .session_attachment(&record.id)
            .unwrap()
            .is_none());
        // A new captured input cannot borrow the older input's resume reference.
        let next = super::append_input(&store, session).await.unwrap();
        let prepared = super::open_without_replacing(
            &store,
            &next.id,
            SessionOpenIntent::Reveal,
            false,
            false,
            false,
        )
        .await
        .unwrap();
        let refreshed = listed_session(&store, &next.id);
        assert_eq!(prepared.actions, refreshed.actions);
        assert_eq!(
            refreshed
                .actions
                .iter()
                .find(|a| a.kind == super::SessionActionKind::Open)
                .unwrap()
                .label,
            "Start conversation"
        );
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
            crate::agent::resolve_session_config(&resumed, serde_yaml_ng::Value::Null, None, false)
                .unwrap();
        assert_eq!(config.agent().unwrap(), "codex");
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
        let stopped = store
            .sqlite
            .creation_receipt("interrupted")
            .unwrap()
            .unwrap();
        assert_eq!(
            stopped.disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Inspect
        );
        assert_eq!(stopped.explanation, "Conversation change unconfirmed");
        // A crash while stopping is uncertain, even if no provider is live now.
        let refused = super::new_thread(&store, &record.id, None, false, false, "interrupted")
            .await
            .unwrap_err();
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
        assert_eq!(
            store
                .sqlite
                .creation_receipt("interrupted")
                .unwrap()
                .unwrap()
                .milestone,
            "admitted"
        );
        assert_eq!(
            store
                .sqlite
                .creation_receipt("interrupted")
                .unwrap()
                .unwrap()
                .disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Continue
        );
        // Simulate app/server death after the atomic append, before publication.
        let recovered = super::new_thread(&store, &record.id, None, false, false, "interrupted")
            .await
            .unwrap();
        assert_eq!(recovered.captured, next.captured);
        assert_eq!(recovered.artifact_key.as_deref(), Some(artifact.as_str()));
        assert!(recovered.input_published);
        let result = store
            .sqlite
            .creation_receipt("interrupted")
            .unwrap()
            .unwrap()
            .result;
        assert!(result.is_some());
        assert_eq!(
            store
                .sqlite
                .creation_receipt("interrupted")
                .unwrap()
                .unwrap()
                .disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Complete
        );
        let mut newer = recovered.clone();
        newer.request = Some("A later independent input".into());
        let newer = super::append_input(&store, newer).await.unwrap();
        super::new_thread(&store, &record.id, None, false, false, "interrupted")
            .await
            .unwrap();
        assert_eq!(
            store.session(&record.id).await.unwrap().unwrap().captured,
            newer.captured
        );
        assert_eq!(
            store
                .sqlite
                .creation_receipt("interrupted")
                .unwrap()
                .unwrap()
                .result,
            result
        );

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
            store
                .sqlite
                .creation_receipt("superseded")
                .unwrap()
                .unwrap()
                .disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Continue
        );
        let newest = super::append_input(&store, pending).await.unwrap();
        let refused = super::new_thread(&store, &record.id, None, false, false, "superseded")
            .await
            .unwrap_err();
        assert_eq!(
            refused.downcast_ref::<crate::engine::Error>().unwrap().code,
            crate::engine::ErrorCode::OutcomeUnknown
        );
        let receipt = store
            .sqlite
            .creation_receipt("superseded")
            .unwrap()
            .unwrap();
        assert_eq!(
            receipt.disposition,
            loopflow_storage::sqlite::creation::RecoveryDisposition::Inspect
        );
        assert_eq!(
            receipt.explanation,
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
        let recovered = super::new_thread(&store, &record.id, None, false, false, "published")
            .await
            .unwrap();
        assert_eq!(recovered.captured, published.captured);
        assert!(recovered.input_published);
        assert_eq!(
            store
                .sqlite
                .creation_receipt("published")
                .unwrap()
                .unwrap()
                .disposition,
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
        let first = super::new_thread(&store, &record.id, None, false, false, "first")
            .await
            .unwrap();
        let retry = super::new_thread(&store, &record.id, None, false, false, "first")
            .await
            .unwrap();
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
/// attaches to it: there is no other terminal's input to take back.
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
    let offered: Vec<_> = row
        .actions
        .iter()
        .map(|action| (action.label.as_str(), action.unavailable_reason.as_deref()))
        .collect();
    assert_eq!(offered, [("Reconnect", None), ("Show conversation", None)]);
    assert!(row.actions[0]
        .help
        .ends_with(&format!("lf resume {}", session.id)));
    // Desktop reopens and refreshes a pane with reveal; it needs the action
    // that carries whether revealing can attach.
    let prepared = super::open_without_replacing(
        &logins.store,
        &session.id,
        SessionOpenIntent::Reveal,
        false,
        false,
        false,
    )
    .await
    .unwrap();
    assert!(prepared
        .actions
        .iter()
        .any(|action| action.kind == super::SessionActionKind::Reveal));
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
                    "repository-in-task",
                    "task-primary",
                    "wave-primary"
                ]
            );
            let filter = crate::session::SessionFilter {
                after: Some("outside".into()),
                limit: 1,
                ..filter
            };
            assert_eq!(fixture.ids(filter).await, ["repository-in-task"]);
        })
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
