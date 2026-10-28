#[cfg(unix)]
#[test]
fn public_open_stops_headless_group_and_resumes_saved_conversation() {
    use std::os::unix::fs::PermissionsExt;
    const CHILD: &str = "LOOPFLOW_RESUME_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        for provider in ["claude", "opencode"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", "ops::human_session::tests::public_open_stops_headless_group_and_resumes_saved_conversation", "--nocapture"]);
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("LF_") {
                    child.env_remove(key);
                }
            }
            let output = child.env(CHILD, provider).output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let provider_name = std::env::var(CHILD).unwrap();
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    let bin = home.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let lf = bin.join("lf");
    std::fs::write(&lf, "#!/bin/sh\nexit 95\n").unwrap();
    std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("LF_BIN", &lf);
    let native = bin.join(&provider_name);
    let mut script = "#!/bin/sh\nstate=$(ps -o stat= -p \"$(cat provider-pid)\" | tr -d ' ')\ncase \"$state\" in \"\"|Z*) ;; *) exit 96;; esac\nprintf '%s\\n' \"$@\" > client-args\n".to_owned();
    if provider_name == "opencode" {
        std::fs::write(
            bin.join("opencode-server.py"),
            include_str!("../../../tests/support/opencode_server.py").replace("__WAIT__", "False"),
        )
        .unwrap();
        std::fs::write(home.join("native-sessions.json"), r#"{"saved-thread":[]}"#).unwrap();
        script.push_str("exec python3 \"$(dirname \"$0\")/opencode-server.py\" \"$@\"\n");
    }
    std::fs::write(&native, script).unwrap();
    std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _path = crate::test_ambient::EnvGuard::clear(&["PATH"]);
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
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
                        harness: provider_name.clone(),
                        model: None,
                        surface: "headless".into(),
                        cwd: home.into(),
                        repo: None,
                        worktree: None,
                        skill: None,
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
                command
                    .env_clear()
                    .arg("60")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                let mut provider = crate::harness::agent_process::spawn(
                    command,
                    None,
                    &(store.sqlite.clone(), session.id.to_string(), first.clone()),
                )?;
                std::fs::write(home.join("provider-pid"), provider.id().unwrap().to_string())?;
                // Without native identity, connect must leave the running provider alone.
                let refused = super::open(&store, &session.id, super::OpenMode::Try, true).await;
                assert!(refused.unwrap_err().to_string().contains("identity is unavailable"));
                assert!(provider.try_wait()?.is_none());
                assert_eq!(store.sqlite.session_attachment(&session.id)?, Some(first.clone()));
                crate::session_record::write_provider_session(
                    &capture.artifact_dir(), &"saved-thread".into(), None,
                )?;
                let origin = store.sqlite.session_turn_origin(&session.id, &first)?;
                store.sqlite.record_session_turn_origin(&"saved-thread".into(), "pending", &origin)?;
                let result = super::open(&store, &session.id, super::OpenMode::Try, true).await;
                let alive = !crate::journal::OsProcess::group(provider.id().unwrap())?.is_empty();
                if alive {
                    provider.kill().await?;
                }
                result?;
                assert!(!alive, "native resume left the headless provider running");
                let row = store.sqlite.process(&first.agent_process_id)?.unwrap();
                assert!(row.completed_at.is_some());
                assert!(store.sqlite.with_session_attachment(&session.id, &first, || Ok(())).is_err());
                let history = store.sqlite.session_history(&session.id, 0, 0)?;
                assert!(history.iter().any(|event| event.provider_turn.as_deref() == Some("pending")
                    && event.kind == crate::session::SessionEventKind::Started));
                assert!(!history.iter().any(|event| event.provider_turn.as_deref() == Some("pending")
                    && event.kind == crate::session::SessionEventKind::Completed));
                let args = std::fs::read_to_string(home.join("client-args"))?;
                let flag = if provider_name == "claude" { "--resume" } else { "--session" };
                assert!(args.contains(&format!("{flag}\nsaved-thread\n")), "{args}");
                assert_eq!(store.sqlite.input_provider_session(session.input_key()?)?.unwrap()
                    .agent_session.as_str(), "saved-thread");
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

use super::{human_session_argv, session_is_resumable};
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
        assert_eq!(super::session_actions(case.state), case.actions);
        for action in case.actions {
            let outcome = super::require_session_action(case.state, action.kind);
            assert_eq!(
                outcome.err().map(|error| error.to_string()),
                action.unavailable_reason
            );
        }
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
        assert_eq!(session.actions, super::session_actions(session.state));
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
        assert_eq!(session.actions, super::session_actions(session.state));
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
    let argv = human_session_argv("session_123", "connect").unwrap();

    assert_eq!(
        &argv[argv.len() - 3..],
        ["session", "connect", "session_123"]
    );
    assert!(argv.contains(&format!("LF_HOME={}", home.home.path().display())));
    assert!(!argv.iter().any(|argument| argument == "tmux"));
}

#[test]
fn foreground_session_can_wait_for_first_use_before_native_history() {
    let _lock = crate::journal::test_env_lock();
    let home = SessionHome::new();
    let _store =
        crate::store::sqlite::SqliteStore::new(&home.home.path().join("loopflow.db")).unwrap();
    let mut command = tokio::process::Command::new("/bin/sleep");
    command.arg("60").stdin(std::process::Stdio::null());
    let input = crate::session_record::new_artifact_key();
    // Advance explicitly: a real child's I/O can prevent the paused clock
    // from auto-advancing. Dropping the future reaps it through kill_on_drop.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(async {
            let launch = super::spawn_session_process(&mut command, &input);
            tokio::pin!(launch);
            assert!(futures_util::poll!(&mut launch).is_pending());
            tokio::time::advance(std::time::Duration::from_secs(31)).await;
            tokio::time::resume();
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), &mut launch).await;
            assert!(
                result.is_err(),
                "foreground launch ended before first-use input: {result:?}"
            );
        });
}

#[test]
fn initial_session_publication_requires_history_and_a_live_agent() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::CaptureHandle::begin_at(
        home.path(),
        crate::session_record::SessionCaptureSpec {
            harness: "sleep".into(),
            model: None,
            surface: "tui".into(),
            cwd: home.path().into(),
            repo: None,
            worktree: None,
            skill: Some("explore".into()),
            subjects: Vec::new(),
            graph: crate::session_record::SessionGraphMembership::Independent,
            work: None,
        },
    )
    .unwrap();
    let dir = capture.artifact_dir();
    let store = crate::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    assert!(!session_is_resumable(&store, &dir).unwrap());
    crate::session_record::write_provider_session(&dir, &"provider-session".into(), None).unwrap();
    assert!(!session_is_resumable(&store, &dir).unwrap());
    let mut client = std::process::Command::new("/bin/sleep")
        .arg("60")
        .spawn()
        .unwrap();
    let session = store
        .session_for_artifact(&capture.artifact_key())
        .unwrap()
        .unwrap();
    let attachment = store
        .claim_session_attachment(&session.id, None, &store.test_lf_process(), true)
        .unwrap();
    let publication = store.record_agent_process_identity(
        &session.id,
        &attachment,
        client.id(),
        crate::journal::process_started_at(client.id())
            .unwrap()
            .unwrap(),
    );
    let resumable = publication
        .map_err(anyhow::Error::from)
        .and_then(|()| session_is_resumable(&store, &dir));
    client.kill().unwrap();
    client.wait().unwrap();
    assert!(resumable.unwrap());
}

#[test]
fn membership_wire_ids_are_derived_from_their_captures() {
    use crate::graph::compiled::CompiledGraph;
    use crate::graph::{ConcretePath, ConcreteSkill, ConcreteStep, ConcreteXor, Skill};
    fn skill(name: &str, human: bool) -> ConcreteStep {
        ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(name),
            interactive: human,
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
