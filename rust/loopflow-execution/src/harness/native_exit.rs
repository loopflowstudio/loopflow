//! What an exec'd native launcher leaves its execution owner to record.
//!
//! A terminal launcher prepares the provider, then becomes it: `exec` keeps the
//! PID, PTY and process group the owner already holds, so no `lf -i` waits
//! beside the agent. Everything the launcher used to record after `wait` is
//! handed to the owner first and settled here when the provider exits.

use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::harness::Recording;
use crate::process::SessionAttachment;
use crate::session_record::CaptureHandoff;
use crate::store::StoreError;
use loopflow_storage::sqlite::SqliteStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeExit {
    pub session: String,
    /// Names the provider and its launcher Process, which completes at exit.
    pub attachment: SessionAttachment,
    pub provider: String,
    /// A client of a provider running beside it, not the provider itself.
    pub client: bool,
    pub cwd: PathBuf,
    /// The capture whose settlement the launcher's caller owned.
    pub capture: Option<CaptureHandoff>,
    /// The launcher's diagnostics log and its size before the provider ran.
    pub diagnostics: (PathBuf, u64),
}

/// How the owner reports a settled provider exit.
#[derive(Debug)]
pub struct Settled {
    pub code: u8,
    /// This attachment yielded to another place or provider.
    pub moved: bool,
    /// What a viewer reads after the provider's own exit text.
    pub lines: Vec<String>,
}

/// What a view prints for a provider that failed, however it learns of the exit.
pub(crate) fn exit_line(provider: &str, code: u8) -> String {
    format!("{provider} exited with {code}.")
}

/// Record a handed-off provider's exit as its launcher would have. Every record
/// is best-effort: the provider's own exit is what the person gets back, and a
/// store that is locked, refused or missing only loses the records, logged.
pub fn settle(exit: &NativeExit, status: ExitStatus) -> Settled {
    let owner: Option<Recording> = match open_store() {
        Ok(store) => Some(Recording {
            store,
            session: exit.session.clone(),
            attachment: exit.attachment.clone(),
        }),
        Err(error) => {
            tracing::warn!(%error, session = %exit.session, "store unavailable at exit; the provider's own result stands");
            None
        }
    };
    // A judgment the store cannot make reads as "no": the exit stands as the
    // provider reported it.
    let judged = |what: &str, judgment: Result<bool>| {
        judgment.unwrap_or_else(|error| {
            tracing::warn!(%error, session = %exit.session, "cannot read {what}");
            false
        })
    };
    if let (Some(owner), false) = (&owner, exit.client) {
        if let Err(error) = record_exit(owner) {
            tracing::warn!(%error, session = %exit.session, "cannot record the provider's exit");
        }
    }
    // A mover that stopped this provider also took its attachment.
    let lost = owner.as_ref().is_some_and(|owner| {
        judged(
            "this launch's Session attachment",
            attachment_lost(owner).map_err(Into::into),
        )
    });
    let command = crate::harness::native_session::NativeOutcome {
        status,
        interrupted: lost,
        moved: lost.then(|| owner.as_ref().and_then(departure)).flatten(),
    };
    let outcome = command.capture_outcome();
    if let Some(Recording {
        store,
        session,
        attachment,
    }) = &owner
    {
        match crate::process::finish_session_attachment(store, session, attachment, outcome) {
            Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => {
                tracing::warn!(%error, %session, "cannot record the Session attachment's end")
            }
        }
    }
    if let Some(Recording { store, .. }) = &owner {
        crate::transcript::facts::record_session(
            store,
            &exit.session,
            crate::transcript::facts::Moment::Exit,
        );
    }
    if let Some(capture) = &exit.capture {
        if let Err(error) = crate::session_record::SessionRecorder::adopt(capture.clone()).and_then(
            |mut capture| {
                capture
                    .finish(outcome)
                    .map_err(crate::session_record::record_error)
            },
        ) {
            tracing::warn!(%error, session = %exit.session, "cannot settle the Session capture");
        }
    }
    // A failure carries the provider's own code, so the person and the store
    // read it.
    let failed = command.failed();
    let code = if failed {
        crate::transport::provider_exit_code(status)
    } else {
        0
    };
    let mut settled = Settled {
        code,
        moved: command.moved.is_some(),
        lines: command.moved.into_iter().collect(),
    };
    if failed {
        settled.lines.push(exit_line(&exit.provider, code));
    }
    let (path, size) = &exit.diagnostics;
    if std::fs::metadata(path).is_ok_and(|info| info.len() > *size) {
        settled
            .lines
            .push(format!("Loopflow recorded diagnostics: {}", path.display()));
    }
    if let Some(Recording {
        store, attachment, ..
    }) = &owner
    {
        // The launcher became the provider, so its end is written here: on
        // the Session, whose exit code tells a person's quit from a death,
        // and, as a Graph's step, on its step.
        // A plain interrupt is no error: it reads as interrupted.
        let error = settled
            .lines
            .first()
            .filter(|_| !matches!(settled.code, 0 | 130));
        let recorded = store
            .record_agent_exit_code(&attachment.agent_process_id, i32::from(settled.code))
            .and_then(|()| {
                store.finish_run(
                    loopflow_storage::graph_process::RunOwner::GraphStep,
                    &attachment.launcher_lf_process_id,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                    Some(i32::from(settled.code)),
                    error.map(String::as_str),
                )
            });
        if let Err(error) = recorded {
            tracing::warn!(%error, session = %exit.session, "cannot record the launcher's exit");
        }
    }
    settled
}

fn open_store() -> Result<SqliteStore> {
    Ok(crate::store::open_sqlite(
        &crate::store::database_path_from_env()?,
    )?)
}

/// Record the provider's exit under its attachment.
pub(crate) fn record_exit(owner: &Recording) -> Result<()> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    match store.with_session_attachment(session, attachment, || {
        store.record_agent_process_exit(session, attachment, true)
    }) {
        Ok(()) | Err(StoreError::InvalidAuthority(_)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Whether a mover ended this launch's attachment.
pub(crate) fn attachment_lost(
    Recording {
        store,
        session,
        attachment: expected,
    }: &Recording,
) -> crate::store::StoreResult<bool> {
    Ok(store.session_attachment(session)?.as_ref() != Some(expected))
}

/// Where this launch's conversation went, when a mover ended its attachment.
pub fn departure(
    Recording {
        store,
        session,
        attachment,
    }: &Recording,
) -> Option<String> {
    let notice = || -> Result<Option<String>> {
        store
            .attachment_departure(session, attachment)?
            .map(|departure| {
                crate::harness::native_session::departure_notice(store, session, departure)
            })
            .transpose()
    };
    notice().unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read where this conversation moved");
        None
    })
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;

    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use crate::journal::{self, TestLedgerGuard};
    use crate::process::Departure;
    use crate::session_record::{SessionCaptureSpec, SessionGraphMembership};

    use super::{settle, NativeExit};
    use loopflow_model::id::LfProcessId;
    use loopflow_storage::graph_process::ExecutionStart;

    /// Make `launcher` a Graph's step, whose end the provider's exit writes.
    fn launcher_as_step(store: &crate::store::sqlite::SqliteStore, launcher: &LfProcessId) {
        let graph = LfProcessId::new();
        let compiled = loopflow_storage::graph::CompiledGraph {
            name: "fixture".into(),
            steps: vec![],
        };
        let start = ExecutionStart::reserved(1);
        store
            .record_graph(&graph, &compiled, None, &start, None, None, None)
            .unwrap();
        store
            .record_graph_step(&graph, launcher, 0, &[vec![]], &start)
            .unwrap();
    }

    /// The step's exit code and outcome as it recorded its end.
    fn step_end(
        store: &crate::store::sqlite::SqliteStore,
        launcher: &LfProcessId,
    ) -> (Option<i32>, Option<&'static str>) {
        let (ended_at, exit_code, error) = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT ended_at,exit_code,error FROM graph_process_steps WHERE lf_process_id=?1",
                [launcher],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i32>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        (
            exit_code,
            loopflow_storage::graph_process::ExecutionResult::of(
                ended_at,
                exit_code,
                error.as_deref(),
            )
            .outcome(),
        )
    }

    #[test]
    fn settlement_without_a_store_keeps_the_providers_own_exit() {
        let ledger = TestLedgerGuard::new();
        let home = ledger.home();
        {
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                SessionCaptureSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    graph: SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .unwrap();
            let store = crate::store::open_sqlite(&home.join("loopflow.db")).unwrap();
            let session = store
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            let launcher = store.test_lf_process();
            let attachment =
                attachment::claim_session_attachment(&store, &session.id, None, &launcher).unwrap();
            let exit = NativeExit {
                session: session.id.to_string(),
                attachment,
                provider: "codex".into(),
                client: false,
                cwd: home.into(),
                capture: Some(capture.hand_off()),

                diagnostics: (home.join("absent.log"), 0),
            };
            // The store this build knows is gone by the time the provider quits.
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("ALTER TABLE agent_sessions RENAME TO agent_sessions_gone")
                .unwrap();
            let settled = settle(&exit, ExitStatus::from_raw(0));
            assert_eq!(settled.code, 0);
            assert!(settled.lines.is_empty(), "{:?}", settled.lines);
            let terminal: serde_json::Value = serde_json::from_slice(
                &std::fs::read(capture.artifact_dir().join("terminal.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(terminal["outcome"], "completed");
        }
    }

    /// A failed provider's own code reaches the person, the launcher's Graph
    /// step and the capture, whether it exited or a signal ended it.
    #[test]
    fn a_failed_provider_exit_carries_its_own_code_and_says_so() {
        for (status, code, outcome) in [
            (3 << 8, 3, "failed"),
            (7 << 8, 7, "failed"),
            (130 << 8, 130, "interrupted"),
            (libc::SIGINT, 130, "interrupted"),
            (libc::SIGKILL, 137, "failed"),
        ] {
            let ledger = TestLedgerGuard::new();
            let home = ledger.home();
            journal::with_runtime(home, &["lf".into(), "-i".into()], || {
                let capture = crate::session_record::SessionRecorder::begin_at(
                    home,
                    SessionCaptureSpec {
                        harness: "claude".into(),
                        model: None,
                        surface: "tui".into(),
                        cwd: home.into(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: vec![],
                        graph: SessionGraphMembership::Independent,
                        work: None,
                    },
                )
                .unwrap();
                let store = crate::store::open_sqlite(&home.join("loopflow.db")).unwrap();
                let session = store
                    .session_for_artifact(&capture.artifact_key())
                    .unwrap()
                    .unwrap();
                let launcher = journal::current_lf_process_id().unwrap();
                launcher_as_step(&store, &launcher);
                let attachment = attachment::claim_session_attachment(
                    &store,
                    &session.id,
                    None,
                    &launcher,
                )
                .unwrap();
                let exit = NativeExit {
                    session: session.id.to_string(),
                    attachment,
                    provider: "claude".into(),
                    client: false,
                    cwd: home.into(),
                    capture: Some(capture.hand_off()),

                    diagnostics: (home.join("absent.log"), 0),
                };
                let settled = settle(&exit, ExitStatus::from_raw(status));
                assert_eq!(settled.code, code, "{status}");
                assert_eq!(settled.lines, [format!("claude exited with {code}.")]);
                assert_eq!(
                    step_end(&store, &launcher),
                    (Some(i32::from(code)), Some(outcome)),
                    "{status}"
                );
                let captured: String = store
                    .conn
                    .lock()
                    .unwrap()
                    .query_row(
                        "SELECT json_extract(payload,'$.evidence.outcome') FROM history.session_history
                         WHERE session_id=?1 AND json_extract(payload,'$.source')='terminal.json'",
                        [session.id.as_str()],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(captured, "failed", "{status}");
                Ok::<_, anyhow::Error>(())
            })
            .unwrap();
        }
    }

    #[test]
    fn settlement_preserves_takeover_capture_and_new_attachment() {
        let ledger = TestLedgerGuard::new();
        let home = ledger.home();
        journal::with_runtime(home, &["lf".into(), "-i".into()], || {
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                SessionCaptureSpec {
                    harness: "claude".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    graph: SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .unwrap();
            let store = crate::store::open_sqlite(&home.join("loopflow.db")).unwrap();
            let session = store
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            let launcher = journal::current_lf_process_id().unwrap();
            launcher_as_step(&store, &launcher);
            let attachment =
                attachment::claim_session_attachment(&store, &session.id, None, &launcher).unwrap();
            let exit = NativeExit {
                session: session.id.to_string(),
                attachment: attachment.clone(),
                provider: "claude".into(),
                client: false,
                cwd: home.into(),
                capture: Some(capture.hand_off()),

                diagnostics: (home.join("absent.log"), 0),
            };
            attachment::finish_session_attachment(
                &store,
                &session.id,
                &attachment,
                "completed",
                Some(&Departure::Terminal {
                    to: Some("tmux %9".into()),
                }),
                || Ok(true),
            )
            .unwrap();
            let current = store.session_attachment(&session.id).unwrap();
            let next = attachment::claim_session_attachment(
                &store,
                &session.id,
                current.as_ref(),
                &store.test_lf_process(),
            )
            .unwrap();
            let before = store
                .agent_process(&next.agent_process_id)
                .unwrap()
                .unwrap();
            let settled = settle(&exit, ExitStatus::from_raw(7 << 8));
            assert_eq!(settled.code, 0);
            assert!(settled.moved);
            assert!(settled
                .lines
                .iter()
                .any(|line| line.contains("moved to tmux %9")));
            assert_eq!(
                store.session_attachment(&session.id).unwrap(),
                Some(next.clone())
            );
            let after = store
                .agent_process(&next.agent_process_id)
                .unwrap()
                .unwrap();
            assert_eq!(after.completed_at, before.completed_at);
            assert_eq!(step_end(&store, &launcher), (Some(0), Some("succeeded")));
            let terminal: serde_json::Value = serde_json::from_slice(
                &std::fs::read(capture.artifact_dir().join("terminal.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(terminal["outcome"], "completed");
            Ok::<_, anyhow::Error>(())
        })
        .unwrap();
    }
}
