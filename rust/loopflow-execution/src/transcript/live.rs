//! The transcript of a Session's current conversation.

use std::path::Path;

use loopflow_harness::transcript::Source;
use loopflow_harness::HarnessKind;
use loopflow_storage::sqlite::{CurrentConversation, SqliteStore};

/// The transcript of `conversation`, which a Session ran in `cwd`; `None`
/// when it is gone.
pub fn locate(
    store: &SqliteStore,
    conversation: &CurrentConversation,
    cwd: &Path,
) -> anyhow::Result<Option<(HarnessKind, Source)>> {
    let Some(harness) = HarnessKind::parse(&conversation.provider) else {
        return Ok(None);
    };
    Ok(
        match super::locate(store, harness, &conversation.thread, cwd)? {
            super::Located::Found { source, .. } => Some((harness, source)),
            super::Located::Gone(_) => None,
        },
    )
}
