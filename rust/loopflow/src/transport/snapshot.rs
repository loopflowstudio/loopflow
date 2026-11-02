//! The last screen a Session's owner had, kept after it ends.
//!
//! An ending owner writes `<Machine home>/screens/<session>.ghostsnp`: one
//! header line naming the Ghostty revision, then Ghostty's own snapshot stream
//! (`GHOSTSNP`, format v1), which carries both screens, their scrollback, styles,
//! hyperlinks, modes and unfinished input. The format promises no compatibility
//! across revisions, so a snapshot from another revision reads as absent.
//! `lf resume` paints it before the provider is back.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::ghostty_vt::{self, Terminal};
use crate::id::LfProcessId;

use super::output::Output;

const HEADER: &str = "lf-terminal-snapshot/1 ghostty=";

fn path(session: &str) -> PathBuf {
    crate::store::lf_home_dir().join("screens").join(format!(
        "{}.ghostsnp",
        crate::id::canonical_session(session)
    ))
}

/// Write the owner's terminal for whoever reopens its Session before the
/// provider is back. Pipe owners and ownerless commands leave nothing.
pub(super) fn leave(owner: &LfProcessId, output: &Output) {
    let Some(terminal) = output.terminal() else {
        return;
    };
    let written = || -> Result<Option<PathBuf>> {
        let Some(session) = crate::journal::process_ledger()?.transport_session(owner)? else {
            return Ok(None);
        };
        write(&session, terminal).map(Some)
    };
    match written() {
        Ok(Some(path)) => {
            tracing::info!(%owner, path = %path.display(), "Terminal snapshot written")
        }
        Ok(None) => tracing::debug!(%owner, "No Session to leave a terminal snapshot for"),
        Err(error) => tracing::warn!(%error, %owner, "Terminal snapshot not written"),
    }
}

fn write(session: &str, terminal: &Terminal) -> Result<PathBuf> {
    let path = path(session);
    let mut bytes = format!("{HEADER}{}\n", ghostty_vt::REVISION).into_bytes();
    bytes.extend(terminal.encode().context("Encode the terminal snapshot")?);
    std::fs::create_dir_all(path.parent().expect("snapshot path has a parent"))?;
    // Written whole: a reader never meets a half-written stream.
    let draft = path.with_extension("ghostsnp.draft");
    std::fs::write(&draft, bytes).with_context(|| format!("Write {}", draft.display()))?;
    std::fs::rename(&draft, &path).with_context(|| format!("Write {}", path.display()))?;
    Ok(path)
}

/// Bytes that show the Session's last screen in the terminal running this
/// command, if its owner left a snapshot this lf can decode.
pub(crate) fn last_screen(session: &str) -> Option<Vec<u8>> {
    let path = path(session);
    let bytes = std::fs::read(&path).ok()?;
    let painted =
        Terminal::decode(stream(&bytes)?).and_then(|terminal| super::terminal::repaint(&terminal));
    match painted {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "Terminal snapshot does not paint");
            None
        }
    }
}

/// The Ghostty stream behind a header this lf's library wrote.
fn stream(bytes: &[u8]) -> Option<&[u8]> {
    let newline = bytes.iter().position(|b| *b == b'\n')?;
    let header = std::str::from_utf8(&bytes[..newline]).ok()?;
    let revision = header.strip_prefix(HEADER)?;
    (revision == ghostty_vt::REVISION).then_some(&bytes[newline + 1..])
}

#[cfg(test)]
mod tests {
    use super::stream;
    use crate::ghostty_vt;

    #[test]
    fn only_this_revisions_snapshots_read() {
        let ours = format!(
            "lf-terminal-snapshot/1 ghostty={}\nGHOSTSNP",
            ghostty_vt::REVISION
        );
        assert_eq!(stream(ours.as_bytes()), Some(b"GHOSTSNP".as_slice()));
        assert_eq!(
            stream(b"lf-terminal-snapshot/1 ghostty=0000000\nGHOSTSNP"),
            None
        );
        assert_eq!(stream(b"GHOSTSNP"), None);
    }
}
