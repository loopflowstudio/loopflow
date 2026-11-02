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
use crate::id::LfProcessId;
use crate::process::SessionAttachment;
use crate::session_record::CaptureHandoff;
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccountId, StoreError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NativeExit {
    pub session: String,
    pub attachment: SessionAttachment,
    /// The launcher's own Process, completed when the provider exits.
    pub lf_process_id: LfProcessId,
    pub provider: String,
    pub cwd: PathBuf,
    /// The capture whose settlement the launcher's caller owned.
    pub capture: Option<CaptureHandoff>,
    /// The input whose provider session names the account home to return to.
    pub capture_key: Option<String>,
    pub account: Option<ProviderAccountId>,
    /// The launcher's diagnostics log and its size before the provider ran.
    pub diagnostics: (PathBuf, u64),
}

/// How the owner reports a settled provider exit.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    pub code: u8,
    /// What a viewer reads after the provider's own exit text.
    pub lines: Vec<String>,
}

/// Record a handed-off provider's exit as its launcher would have.
pub(crate) fn settle(exit: &NativeExit, status: ExitStatus) -> Result<Settled> {
    use std::os::unix::process::ExitStatusExt;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let owner: AttachmentOwner = (store, exit.session.clone(), exit.attachment.clone());
    let (store, session, attachment) = &owner;
    let code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)) as u8;
    // No launcher held an interrupt handler for this provider; its own 130 is one.
    let interrupted = record_exit(&owner)? || code == 130;
    let lost = attachment_lost(&owner).unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read this launch's Session attachment");
        false
    });
    if let (Some(account), Some(key), false) = (&exit.account, &exit.capture_key, lost) {
        pin_account_home(&exit.provider, account, key);
    }
    let outcome = if status.success() || interrupted {
        "completed"
    } else {
        "failed"
    };
    match crate::session_record::finish_session_attachment(store, session, attachment, outcome) {
        Ok(()) | Err(StoreError::InvalidAuthority(_)) => {}
        Err(error) => return Err(error.into()),
    }
    if let Some(capture) = &exit.capture {
        crate::session_record::CaptureHandle::adopt(capture.clone())?.finish(outcome)?;
    }
    let mut settled = Settled::default();
    if lost {
        settled.lines.extend(departure(&owner));
    } else if !status.success() && !interrupted {
        settled.lines.push(format!(
            "{} exited with {status}. If another client still owns this session, close it and run `lf resume` again.",
            exit.provider
        ));
        settled.code = code;
    } else if code == 130 {
        settled.code = code;
    }
    let (path, size) = &exit.diagnostics;
    if std::fs::metadata(path).is_ok_and(|info| info.len() > *size) {
        settled
            .lines
            .push(format!("Loopflow recorded diagnostics: {}", path.display()));
    }
    let mut process = store
        .process(&exit.lf_process_id)?
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
    Ok(settled)
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
fn pin_account_home(provider: &str, account: &ProviderAccountId, capture_key: &str) {
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
