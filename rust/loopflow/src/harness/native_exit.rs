//! What an exec'd native launcher leaves its execution owner to record.
//!
//! A terminal launcher prepares the provider, then becomes it: `exec` keeps the
//! PID, PTY and process group the owner already holds, so no `lf -i` waits
//! beside the agent. Everything the launcher used to record after `wait` is
//! handed to the owner first and settled here when the provider exits.

use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::harness::agent_process::AttachmentOwner;
use crate::process::SessionAttachment;
use crate::session_record::CaptureHandoff;
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccountId, StoreError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeExit {
    pub session: String,
    /// Names the provider and its launcher Process, which completes at exit.
    pub attachment: SessionAttachment,
    pub provider: String,
    pub resuming: bool,
    pub cwd: PathBuf,
    /// The capture whose settlement the launcher's caller owned.
    pub capture: Option<CaptureHandoff>,
    /// The input whose provider session names the account home to return to.
    pub capture_key: Option<String>,
    pub account: Option<ProviderAccountId>,
    pub isolate: Option<bool>,
    /// The launcher's diagnostics log and its size before the provider ran.
    pub diagnostics: (PathBuf, u64),
}

/// How the owner reports a settled provider exit.
#[derive(Debug)]
pub(crate) struct Settled {
    pub code: u8,
    /// What a viewer reads after the provider's own exit text.
    pub lines: Vec<String>,
}

/// Record a handed-off provider's exit as its launcher would have. Every record
/// is best-effort: the provider's own exit is what the person gets back, and a
/// store that is locked, refused or missing only loses the records, logged.
pub(crate) fn settle(exit: &NativeExit, status: ExitStatus) -> Settled {
    let owner: Option<AttachmentOwner> = match open_store() {
        Ok(store) => Some((store, exit.session.clone(), exit.attachment.clone())),
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
    let interrupted = owner
        .as_ref()
        .is_some_and(|owner| judged("the provider's exit record", record_exit(owner)));
    let lost = owner.as_ref().is_some_and(|owner| {
        judged(
            "this launch's Session attachment",
            attachment_lost(owner).map_err(Into::into),
        )
    });
    if let (Some(account), Some(key), false) = (&exit.account, &exit.capture_key, lost) {
        pin_account_home(&exit.provider, account, key, exit.isolate);
    }
    let command = crate::lf::commands::util::SessionCommandOutcome {
        status,
        interrupted: interrupted || lost,
        moved: lost.then(|| owner.as_ref().and_then(departure)).flatten(),
    };
    let outcome = command.capture_outcome();
    if let Some((store, session, attachment)) = &owner {
        match crate::session_record::finish_session_attachment(store, session, attachment, outcome)
        {
            Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => {
                tracing::warn!(%error, %session, "cannot record the Session attachment's end")
            }
        }
    }
    if let Some(capture) = &exit.capture {
        if let Err(error) = crate::session_record::CaptureHandle::adopt(capture.clone())
            .and_then(|capture| capture.finish(outcome))
        {
            tracing::warn!(%error, session = %exit.session, "cannot settle the Session capture");
        }
    }
    // The command's result differs from the provider's native status. Use the
    // returning launcher's interpretation, including attachment loss.
    let result = command.result(&exit.provider, exit.resuming);
    let mut settled = Settled {
        code: crate::journal::command_exit_code(&result),
        lines: command.moved.into_iter().collect(),
    };
    if let Err(error) = result {
        settled.lines.push(error.to_string());
    }
    let (path, size) = &exit.diagnostics;
    if std::fs::metadata(path).is_ok_and(|info| info.len() > *size) {
        settled
            .lines
            .push(format!("Loopflow recorded diagnostics: {}", path.display()));
    }
    if let Some((store, _, attachment)) = &owner {
        let recorded = (|| -> Result<()> {
            let mut process = store
                .process(&attachment.provider_lf_process_id)?
                .context("handed-off launcher is not recorded")?;
            process.completed_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
            process.outcome = Some(
                match settled.code {
                    0 => "succeeded",
                    130 => "interrupted",
                    _ => "failed",
                }
                .into(),
            );
            process.exit_code = Some(i32::from(settled.code));
            process.error = settled.lines.first().filter(|_| settled.code != 0).cloned();
            store.record_process(&process)?;
            Ok(())
        })();
        if let Err(error) = recorded {
            tracing::warn!(%error, session = %exit.session, "cannot record the launcher's exit");
        }
    }
    settled
}

fn open_store() -> Result<SqliteStore> {
    Ok(SqliteStore::new(&crate::store::database_path_from_env()?)?)
}

/// Record the provider's exit under its attachment; whether an interrupt ended it.
pub(crate) fn record_exit(owner: &AttachmentOwner) -> Result<bool> {
    let (store, session, attachment) = owner;
    match store.with_session_attachment(session, attachment, || {
        store.record_agent_process_exit(session, attachment, true)
    }) {
        Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
        Err(error) => return Err(error.into()),
    }
    Ok(store
        .process(&attachment.agent_process_id)?
        .is_some_and(|process| {
            process.completed_at.is_some() && process.outcome.as_deref() == Some("interrupted")
        }))
}

/// Whether a mover ended this launch's attachment.
pub(crate) fn attachment_lost(
    (store, session, expected): &AttachmentOwner,
) -> crate::store::StoreResult<bool> {
    Ok(store.session_attachment(session)?.as_ref() != Some(expected))
}

/// Where this launch's conversation went, when a mover ended its attachment.
pub(crate) fn departure((store, session, attachment): &AttachmentOwner) -> Option<String> {
    let notice = || -> Result<Option<String>> {
        store
            .attachment_departure(session, attachment)?
            .map(|departure| crate::lf::commands::util::departure_notice(store, session, departure))
            .transpose()
    };
    notice().unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read where this conversation moved");
        None
    })
}

/// Record which account home the provider session lives in, so reopening
/// returns there.
fn pin_account_home(
    provider: &str,
    account: &ProviderAccountId,
    capture_key: &str,
    isolate: Option<bool>,
) {
    let pinned = (|| -> Result<()> {
        let Ok(provider) = provider.parse::<crate::provider_auth::Provider>() else {
            return Ok(());
        };
        let dir = crate::session_record::capture_dir(capture_key)?;
        let Some(session) = crate::session_record::read_provider_session(&dir)? else {
            return Ok(());
        };
        let route = crate::provider_account::resolve_provider_account_exact_blocking(
            provider,
            Some(session.agent_session.clone()),
            Some(account.clone()),
            None,
            isolate,
        )?;
        if let Some(route) = route {
            route.pin_session_blocking(Some(session.agent_session))?;
        }
        Ok(())
    })();
    if let Err(error) = pinned {
        tracing::warn!(%error, "failed to record the provider session's account home");
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use crate::journal::{self, TestLedgerGuard};
    use crate::process::Departure;
    use crate::session_record::{CaptureHandle, SessionCaptureSpec, SessionGraphMembership};
    use crate::store::sqlite::SqliteStore;

    use super::{settle, NativeExit};

    #[test]
    fn settlement_without_a_store_keeps_the_providers_own_exit() {
        let ledger = TestLedgerGuard::new();
        let home = ledger.home();
        {
            let capture = CaptureHandle::begin_at(
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
            let store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
            let session = store
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            let launcher = store.test_lf_process();
            let attachment = store
                .claim_session_attachment(&session.id, None, &launcher, true)
                .unwrap();
            let exit = NativeExit {
                session: session.id.to_string(),
                attachment,
                provider: "codex".into(),
                resuming: false,
                cwd: home.into(),
                capture: Some(capture.hand_off()),
                capture_key: Some(capture.artifact_key()),
                account: None,
                isolate: None,
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

    #[test]
    fn settlement_preserves_takeover_capture_and_new_attachment() {
        let ledger = TestLedgerGuard::new();
        let home = ledger.home();
        journal::with_process(|| {
            journal::admit_process(home, &["lf".into(), "-i".into()]);
            let capture = CaptureHandle::begin_at(
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
            let store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
            let session = store
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            let launcher = journal::current_lf_process_id().unwrap();
            let attachment = store
                .claim_session_attachment(&session.id, None, &launcher, true)
                .unwrap();
            let exit = NativeExit {
                session: session.id.to_string(),
                attachment: attachment.clone(),
                provider: "claude".into(),
                resuming: true,
                cwd: home.into(),
                capture: Some(capture.hand_off()),
                capture_key: Some(capture.artifact_key()),
                account: None,
                isolate: None,
                diagnostics: (home.join("absent.log"), 0),
            };
            store
                .finish_session_attachment(
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
            let next = store
                .claim_session_attachment(
                    &session.id,
                    current.as_ref(),
                    &store.test_lf_process(),
                    true,
                )
                .unwrap();
            let before = store.process(&next.agent_process_id).unwrap().unwrap();
            let settled = settle(&exit, ExitStatus::from_raw(7 << 8));
            assert_eq!(settled.code, 0);
            assert!(settled
                .lines
                .iter()
                .any(|line| line.contains("moved to tmux %9")));
            assert_eq!(
                store.session_attachment(&session.id).unwrap(),
                Some(next.clone())
            );
            let after = store.process(&next.agent_process_id).unwrap().unwrap();
            assert_eq!(after.completed_at, before.completed_at);
            assert_eq!(after.outcome, before.outcome);
            let launcher = store.process(&launcher).unwrap().unwrap();
            assert_eq!(launcher.exit_code, Some(0));
            assert_eq!(launcher.outcome.as_deref(), Some("succeeded"));
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
