//! Library-backed terminal snapshots and replies while no controlling viewer exists.

#[derive(Default)]
pub(super) struct Queries {
    pub detached: bool,
    pub replies: Vec<u8>,
}

impl vt100::Callbacks for Queries {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        prefix: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        command: char,
    ) {
        if !self.detached {
            return;
        }
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (prefix, first, command) {
            (None, 6, 'n') => {
                let (row, col) = screen.cursor_position();
                self.replies
                    .extend(format!("\x1b[{};{}R", row + 1, col + 1).bytes());
            }
            (None, 5, 'n') => self.replies.extend(b"\x1b[0n"),
            (None, 0, 'c') => self.replies.extend(b"\x1b[?1;2c"),
            _ => {}
        }
    }
}

/// Bytes that paint the owner's visible screen on an attaching viewer.
pub(super) fn snapshot(screen: &vt100::Screen) -> Vec<u8> {
    let mut bytes = b"\x1b[0m\x1b[?25h".to_vec();
    let mut history = screen.clone();
    history.set_scrollback(usize::MAX);
    if screen.alternate_screen() {
        bytes.extend(b"\x1b[?1049h");
    } else if untouched(screen, &history) {
        // Nothing to redraw: the viewer's terminal keeps its own history and
        // the owner's output flows from the cursor as any command's would.
        return bytes;
    } else {
        // Only the owner's visible screen is redrawn. The viewer's screen
        // scrolls into its own scrollback first, so nothing it showed is
        // erased: a screen's height of newlines blanks it from any cursor row.
        bytes.extend(blank(screen));
    }
    bytes.extend(screen.state_formatted());
    bytes
}

/// Bytes that paint the owner's last screen on a fresh terminal after the
/// owner is gone: its scrollback, oldest row first, then the visible screen.
/// The alternate screen keeps no scrollback.
pub(super) fn last_screen(screen: &vt100::Screen) -> Vec<u8> {
    let mut bytes = b"\x1b[0m\x1b[?25h".to_vec();
    if screen.alternate_screen() {
        bytes.extend(b"\x1b[?1049h");
    } else {
        bytes.extend(scrollback(screen));
        bytes.extend(blank(screen));
    }
    bytes.extend(screen.state_formatted());
    bytes
}

/// The owner's scrollback as terminal lines, oldest row first.
fn scrollback(screen: &vt100::Screen) -> Vec<u8> {
    let (rows, cols) = screen.size();
    let mut history = screen.clone();
    history.set_scrollback(usize::MAX);
    let mut bytes = Vec::new();
    // Scrolled back `offset` rows, the view starts `offset` rows above the
    // screen; a screen's height at a time.
    let mut offset = history.scrollback();
    while offset > 0 {
        history.set_scrollback(offset);
        let take = offset.min(usize::from(rows));
        for row in history.rows_formatted(0, cols).take(take) {
            bytes.extend(row);
            bytes.extend(b"\r\n");
        }
        offset -= take;
    }
    bytes
}

/// A screen's height of newlines: whatever the terminal shows scrolls into
/// its scrollback before the redraw clears the screen.
fn blank(screen: &vt100::Screen) -> Vec<u8> {
    b"\r\n".repeat(usize::from(screen.size().0))
}

/// A screen the owner has not drawn on: no history, nothing visible, cursor home.
fn untouched(screen: &vt100::Screen, history: &vt100::Screen) -> bool {
    history.scrollback() == 0
        && screen.cursor_position() == (0, 0)
        && screen.contents().trim().is_empty()
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

/// Reports may be inserted only between complete terminal sequences and UTF-8
/// characters. The parser decides boundaries; raw provider bytes stay unchanged.
pub(super) struct Boundary {
    parser: vte::Parser,
    state: Complete,
    pending: Vec<u8>,
    overflow: bool,
}
struct Complete {
    ready: bool,
    was_ready: bool,
}
impl vte::Perform for Complete {
    fn print(&mut self, _: char) {
        self.ready = true;
    }
    fn execute(&mut self, _: u8) {
        self.ready = self.was_ready;
    }
    fn csi_dispatch(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {
        self.ready = true;
    }
    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {
        self.ready = true;
    }
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {
        self.ready = true;
    }
    fn unhook(&mut self) {
        self.ready = true;
    }
}
impl Boundary {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            pending: Vec::new(),
            overflow: false,
            state: Complete {
                ready: true,
                was_ready: true,
            },
        }
    }
    pub fn advance(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.pending.len() < 65536 {
                self.pending.push(*byte);
            } else {
                self.overflow = true;
            }
            self.state.was_ready = self.state.ready;
            self.state.ready = false;
            self.parser.advance(&mut self.state, &[*byte]);
            if *byte == 0x1b {
                self.state.ready = false;
            }
            if self.state.ready {
                self.pending.clear();
                self.overflow = false;
            }
        }
    }
    pub fn pending(&self) -> Option<&[u8]> {
        (!self.overflow).then_some(self.pending.as_slice())
    }
    pub fn ready(&self) -> bool {
        self.state.ready
    }
}

#[cfg(test)]
mod tests {
    use super::{snapshot, AlternateScreen, Boundary, Queries};

    #[test]
    fn attaching_before_the_owner_draws_leaves_the_viewer_terminal_alone() {
        let fresh = vt100::Parser::new(24, 80, 2000);
        assert_eq!(snapshot(fresh.screen()), b"\x1b[0m\x1b[?25h");
        let mut drawn = vt100::Parser::new(24, 80, 2000);
        drawn.process(b"Error: no stock\r\n");
        let bytes = snapshot(drawn.screen());
        let mut restored = vt100::Parser::new(24, 80, 2000);
        restored.process(&bytes);
        assert_eq!(restored.screen().contents().trim(), "Error: no stock");
    }

    #[test]
    fn redrawing_shows_the_owner_screen_and_keeps_the_viewers_scrollback() {
        let mut owner = vt100::Parser::new(24, 80, 2000);
        for line in 0..30 {
            owner.process(format!("owner {line}\r\n").as_bytes());
        }
        let mut viewer = vt100::Parser::new(24, 80, 2000);
        viewer.process(b"$ ls\r\nnotes.md\r\n$ lf resume\r\n");
        viewer.process(&snapshot(owner.screen()));
        assert_eq!(viewer.screen().contents(), owner.screen().contents());
        viewer.screen_mut().set_scrollback(usize::MAX);
        let scrollback = viewer.screen().contents();
        assert!(
            scrollback.starts_with("$ ls\nnotes.md\n$ lf resume"),
            "{scrollback}"
        );
        assert!(!scrollback.contains("owner 0\n"), "{scrollback}");
    }

    #[test]
    fn the_last_screen_replays_scrollback_and_the_grid_on_a_fresh_terminal() {
        let mut owner = vt100::Parser::new(4, 20, 2000);
        for line in 0..10 {
            owner.process(format!("line {line}\r\n").as_bytes());
        }
        owner.process(b"\x1b[1mprompt>\x1b[0m ");
        let mut fresh = vt100::Parser::new(4, 20, 2000);
        fresh.process(&super::last_screen(owner.screen()));
        assert_eq!(fresh.screen().contents(), owner.screen().contents());
        assert_eq!(fresh.screen().cursor_position(), (3, 8));
        assert!(fresh.screen().cell(3, 0).unwrap().bold());
        fresh.screen_mut().set_scrollback(usize::MAX);
        let scrollback = fresh.screen().contents();
        assert!(
            scrollback.starts_with("line 0\nline 1\nline 2\nline 3"),
            "{scrollback}"
        );
        let mut untouched = vt100::Parser::new(4, 20, 2000);
        untouched.process(&super::last_screen(
            vt100::Parser::new(4, 20, 2000).screen(),
        ));
        assert_eq!(untouched.screen().contents().trim(), "");
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
    fn restores_grid_and_input_modes_without_replaying_terminal_queries() {
        let mut parser = vt100::Parser::new_with_callbacks(
            24,
            80,
            2000,
            Queries {
                detached: true,
                ..Default::default()
            },
        );
        parser.process(b"\x1b[?1049h\x1b[?2004h\x1b[?1hhello\x1b[6n");
        assert_eq!(parser.callbacks().replies, b"\x1b[1;6R");
        let mut restored = vt100::Parser::new(24, 80, 2000);
        restored.process(&snapshot(parser.screen()));
        assert_eq!(restored.screen().contents(), "hello");
        assert!(restored.screen().alternate_screen());
        assert_eq!(
            restored.screen().input_mode_formatted(),
            parser.screen().input_mode_formatted()
        );
    }

    #[test]
    fn reconnect_retains_an_incomplete_escape_prefix() {
        let mut boundary = Boundary::new();
        boundary.advance(b"text\x1b[?20");
        assert_eq!(boundary.pending(), Some(b"\x1b[?20".as_slice()));
        let mut restored = vt100::Parser::new(24, 80, 0);
        restored.process(boundary.pending().unwrap());
        restored.process(b"04h");
        assert!(restored.screen().bracketed_paste());
        boundary.advance(b"04h");
        assert_eq!(boundary.pending(), Some(b"".as_slice()));
    }

    #[test]
    fn status_never_splits_an_osc_csi_or_utf8_character() {
        let mut boundary = Boundary::new();
        for frame in [
            b"\x1b]7501;state=working\x1b\\".as_slice(),
            b"\x1b[?2004h",
            "\u{1f980}".as_bytes(),
        ] {
            for byte in &frame[..frame.len() - 1] {
                boundary.advance(&[*byte]);
                assert!(!boundary.ready());
            }
            boundary.advance(&frame[frame.len() - 1..]);
            assert!(boundary.ready());
        }
    }
}
