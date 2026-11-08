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
            let capture = crate::session_record::CaptureHandle::begin_at(
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
            )?;
            let session = store
                .sqlite
                .session_for_artifact(&capture.artifact_key())?
                .unwrap();
            let launcher = crate::id::LfProcessId::new();
            rusqlite::Connection::open(home.join("loopflow.db"))?.execute(
                "INSERT INTO processes(id,trace_id,started_at) SELECT ?1,trace_id,started_at FROM processes WHERE id=?2",
                rusqlite::params![launcher, crate::journal::current_lf_process_id().unwrap()],
            )?;
            let first = store.sqlite.claim_session_attachment(&session.id, None, &launcher, true)?;
            let mut command = tokio::process::Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut provider = crate::harness::agent_process::spawn(
                command,
                None,
                &(store.sqlite.clone(), session.id.to_string(), first.clone()),
            )?;
            let opened = super::open(&store, home, &session.id, false, true, false).await;
            let forked = super::open(&store, home, &session.id, true, true, false).await;
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
    let wave = crate::id::WaveId::new();
    let mut summary = crate::session::SessionSummary {
        preview: None,
        has_provider_history: true,
        program_status: None,
        agent_process_id: None,
        primary_scope: None,
        agent_process_evidence: crate::journal::ProcessIdentityEvidence::Unknown,
        available: false,
        terminal_id: None,
        unviewed: false,
        task_terminal: false,
        task_primary: false,
        task_ids: vec![task.clone()],
        captured: Some(1),
        id: "metadata".into(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
        title: "Retained conversation".into(),
        title_source: crate::session::TitleSource::Human,
        ready_summary: None,
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
    assert_eq!(row.work, Some(crate::durable::WorkRef::Task(task)));
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
    summary.ready_summary = Some("Exact retained feedback".into());
    let row = super::summary_surface(&summary);
    assert_eq!(row.state, super::SessionState::Unknown);
    assert_eq!(row.ready_summary, summary.ready_summary);
    assert_eq!(row.availability, None);
    summary.ready_summary = None;
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

use std::ffi::OsString;

use super::human_session_argv;
use crate::lf::commands::write::SessionOpenIntent;
use crate::session::LfSession;
use crate::store::{open_ephemeral_store, SharedStore, StorageConfig};
use crate::work::task::TaskId;

pub(super) struct SessionHome {
    home: tempfile::TempDir,
    previous: Vec<(&'static str, Option<OsString>)>,
    _ambient: crate::test_ambient::EnvGuard,
}

impl SessionHome {
    pub(super) fn new() -> Self {
        let ambient = crate::test_ambient::EnvGuard::new();
        let home = tempfile::tempdir().unwrap();
        let previous = ["LF_HOME", "LF_BIN"]
            .into_iter()
            .map(|name| {
                let value = std::env::var_os(name);
                std::env::remove_var(name);
                (name, value)
            })
            .collect();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            home,
            previous,
            _ambient: ambient,
        }
    }

    pub(super) async fn store(&self) -> SharedStore {
        std::sync::Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(self.home.path().join("loopflow.db")))
                .await
                .unwrap(),
        )
    }
}

impl Drop for SessionHome {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

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
        crate::id::WaveId::new(),
        "product".to_string(),
        directory.path().display().to_string(),
    );
    store.create_wave(&wave).await.unwrap();
    let session = |task_id: Option<crate::work::task::TaskId>| LfSession {
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
        ready_summary: None,
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
    let missing = crate::work::task::TaskId::new();
    assert_eq!(
        super::session_work_path(&store, &session(Some(missing.clone())))
            .await
            .unwrap(),
        Some(format!("Task {missing} (unavailable)"))
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
            CompiledGraph::new("task-design", &opening()),
        ),
        (
            "00000000-0000-0000-0000-0000000000f1".to_string(),
            CompiledGraph::new("feature", &past),
        ),
        (
            "00000000-0000-0000-0000-0000000000f2".to_string(),
            CompiledGraph::new("feature", &current),
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
        let listed = super::listed(&store, &record.id).unwrap().unwrap();
        assert_eq!(retry(&listed).label, "Try again");
        assert!(retry(&listed).unavailable_reason.is_none());
        let refused = super::open_without_replacing(
            &store,
            &record.id,
            SessionOpenIntent::Start,
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
            super::listed(&store, &record.id).unwrap().unwrap(),
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
        )
        .await
        .unwrap();
        assert!(!prepared
            .actions
            .iter()
            .any(|a| a.kind == super::SessionActionKind::MoveHere));

        let launcher = store.sqlite.test_lf_process();
        let holder = store
            .sqlite
            .claim_session_attachment(&record.id, None, &launcher, true)
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
        let refreshed = super::listed(&store, &record.id).unwrap().unwrap();
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
        let refreshed = super::listed(&store, &record.id).unwrap().unwrap();
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
        for source in ["provider-session:imported", "events.jsonl:1"] {
            let repo = loopflow_test_support::TestRepo::new();
            let (record, _) = super::primary::ensure(&store, repo.path(), None)
                .await
                .unwrap();
            let session = store.session(&record.id).await.unwrap().unwrap();
            let session = super::append_input(&store, session).await.unwrap();
            let mut observation = crate::session_record::provider_session_observation(
                &session,
                session.input_key().unwrap(),
                &"imported-history".into(),
                None,
            );
            observation.source = source.into();
            observation.payload["source"] = source.into();
            if source.starts_with("events") {
                observation.payload["evidence"] = serde_json::json!({
                    "schema_version": 1, "seq": 1, "observed_at": "2026-11-04T00:00:00Z",
                    "type": "provider_session_observed", "attempt_key": "imported",
                    "provider_session_id": "imported-history"
                });
            }
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
            )
            .await
            .unwrap();
            let refreshed = super::listed(&store, &record.id).unwrap().unwrap();
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
            )
            .await
            .unwrap();
            let refreshed = super::listed(&store, &next.id).unwrap().unwrap();
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
        }
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
        include_str!("../../builtins/task/skill/task_session.md"),
        include_str!("../../builtins/ops/skill/repo_session.md"),
        include_str!("../../builtins/wave/skill/wave_session.md"),
    ] {
        let skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(skill.contains("request is a reading pass"));
        assert!(skill.contains("Steers delivered with that turn are recorded direction"));
        assert!(!skill.contains("without waiting for a greeting"));
    }
}
