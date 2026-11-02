//! Repainting a viewer from the owner's terminal, and what a viewer was sent.

use crate::ghostty_vt::{Error, Terminal};

/// Scrollback rows a repaint carries at most. The whole repaint travels in one
/// transport response, so the byte bound below wins when rows are wide or styled.
const HISTORY_ROWS: usize = 2000;
const HISTORY_BYTES: usize = 768 * 1024;

/// Bytes that make a viewer's terminal show the owner's screen: its scrollback
/// above, then the active screen with its styles, hyperlinks, cursor, modes and
/// keyboard state. The viewer's own history is kept beneath.
pub(super) fn repaint(terminal: &Terminal) -> Result<Vec<u8>, Error> {
    let mut bytes = b"\x1b[0m\x1b[?25h".to_vec();
    // On the alternate screen the formatter re-enters it, which starts blank.
    if !terminal.alternate_screen() {
        if untouched(terminal) {
            // Nothing to redraw: the viewer's terminal keeps its own history and
            // the owner's output flows from the cursor as any command's would.
            return Ok(bytes);
        }
        bytes.extend(history(terminal)?);
        // Scroll everything the viewer showed, history included, into its own
        // scrollback so the owner's screen starts at the top.
        let (_, rows) = terminal.size();
        for _ in 0..rows {
            bytes.extend(b"\r\n");
        }
    }
    bytes.extend(b"\x1b[H");
    bytes.extend(terminal.repaint_screen()?);
    Ok(bytes)
}

fn history(terminal: &Terminal) -> Result<Vec<u8>, Error> {
    let mut rows = terminal.scrollback_rows().min(HISTORY_ROWS);
    loop {
        let bytes = terminal.repaint_history(rows)?;
        if bytes.len() <= HISTORY_BYTES || rows <= 1 {
            return Ok(bytes);
        }
        rows /= 2;
    }
}

/// A screen the owner has not drawn on: no history, nothing visible, cursor home.
fn untouched(terminal: &Terminal) -> bool {
    terminal.scrollback_rows() == 0
        && terminal.cursor() == (0, 0)
        && terminal.text().is_ok_and(|text| text.trim().is_empty())
}

/// Whether output written to a viewer's terminal left it on the alternate
/// screen. Leaving it is restored on exit only when it is still entered: in
/// xterm-family terminals 1049-reset also restores the cursor saved on entry,
/// which moves the cursor above whatever was printed since.
#[derive(Debug, Default)]
pub(super) struct AlternateScreen {
    pub entered: bool,
    tail: Vec<u8>,
}

impl AlternateScreen {
    pub fn observe(&mut self, bytes: &[u8]) {
        let mut window = std::mem::take(&mut self.tail);
        window.extend_from_slice(bytes);
        for (i, _) in window.iter().enumerate().filter(|(_, b)| **b == 0x1b) {
            let rest = &window[i..];
            for mode in [b"\x1b[?1049".as_slice(), b"\x1b[?1047", b"\x1b[?47"] {
                match rest.strip_prefix(mode).and_then(|r| r.first()) {
                    Some(b'h') => self.entered = true,
                    Some(b'l') => self.entered = false,
                    _ => {}
                }
            }
        }
        // Keep a sequence split across writes.
        let start = window.len().saturating_sub(7);
        self.tail = window[start..].to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::{repaint, AlternateScreen};
    use crate::ghostty_vt::Terminal;

    fn viewer_showing(bytes: &[u8]) -> Terminal {
        let mut viewer = Terminal::new(80, 24).unwrap();
        viewer.write(bytes);
        viewer
    }

    #[test]
    fn attaching_before_the_owner_draws_leaves_the_viewer_terminal_alone() {
        let fresh = Terminal::new(80, 24).unwrap();
        assert_eq!(repaint(&fresh).unwrap(), b"\x1b[0m\x1b[?25h");
        let mut drawn = Terminal::new(80, 24).unwrap();
        drawn.write(b"Error: no stock\r\n");
        let restored = viewer_showing(&repaint(&drawn).unwrap());
        assert_eq!(restored.text().unwrap().trim(), "Error: no stock");
        assert_eq!(restored.cursor(), drawn.cursor());
    }

    #[test]
    fn redrawing_shows_the_owner_screen_and_keeps_the_viewers_scrollback() {
        let mut owner = Terminal::new(80, 24).unwrap();
        for line in 0..30 {
            owner.write(format!("owner {line}\r\n").as_bytes());
        }
        let mut viewer = viewer_showing(b"$ ls\r\nnotes.md\r\n$ lf resume\r\n");
        viewer.write(&repaint(&owner).unwrap());
        assert_eq!(viewer.text().unwrap(), owner.text().unwrap());
        let viewer_rows = viewer.scrollback_rows();
        let viewer_history =
            String::from_utf8_lossy(&viewer.repaint_history(viewer_rows).unwrap()).into_owned();
        assert!(
            viewer_history.starts_with("$ ls\r\nnotes.md\r\n$ lf resume"),
            "{viewer_history:?}"
        );
        // The owner's scrollback follows the viewer's own.
        assert!(viewer_history.contains("owner 0\r\n"), "{viewer_history:?}");
        assert!(viewer_history.contains("owner 5\r\n"), "{viewer_history:?}");
    }

    #[test]
    fn alternate_screen_follows_the_last_mode_change_across_writes() {
        let mut screen = AlternateScreen::default();
        screen.observe(b"frame\x1b[?10");
        screen.observe(b"49hmore");
        assert!(screen.entered);
        screen.observe(b"\x1b[?1049lbye\x1b[?1049h\x1b[?1049l");
        assert!(!screen.entered);
    }

    #[test]
    fn restores_alternate_screen_colors_links_and_input_modes() {
        let mut owner = Terminal::new(80, 24).unwrap();
        owner.write(b"\x1b[?1049h\x1b[?2004h\x1b[?1h\x1b[>1u\x1b[34m\x1b]8;;https://loopflow.studio\x1b\\hello\x1b]8;;\x1b\\");
        let bytes = repaint(&owner).unwrap();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(text.contains("\x1b[?2004h"), "{text:?}");
        assert!(text.contains("\x1b[?1h"), "{text:?}");
        assert!(text.contains("\x1b[=1;1u"), "{text:?}");
        assert!(
            text.contains("\x1b]8;;https://loopflow.studio\x1b\\hello"),
            "{text:?}"
        );
        assert!(text.contains("\x1b[38;5;4m"), "{text:?}");
        let restored = viewer_showing(&bytes);
        assert!(restored.alternate_screen());
        assert_eq!(restored.text().unwrap().trim_end(), "hello");
        assert_eq!(restored.cursor(), owner.cursor());
    }
}
