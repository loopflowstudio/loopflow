//! Repainting a viewer from the held terminal, and what a viewer was sent.

use crate::ghostty_vt::{Error, Terminal};

/// Input modes a repaint or a program may have set in a viewer's terminal:
/// Kitty keyboard flags, modifyOtherKeys, application keys, focus and mouse reporting, bracketed
/// paste; then a visible cursor, plain style and the viewer's configured colors.
pub(super) const RESET_MODES: &[u8] =
    b"\x1b[<65535u\x1b[>4;0m\x1b[?1l\x1b>\x1b[?1004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?25h\x1b[0m\x1b]104\x1b\\\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\";

/// Origin and scroll-margin resets home the cursor. Keep its position so the
/// next shell prompt follows the command's output instead of overwriting it.
pub(super) const RESET_DRAWING: &[u8] =
    b"\x1b[s\x1b[?6l\x1b[?7h\x1b[r\x1b[u\x1b(B\x1b)B\x0f\x1b]8;;\x1b\\";

/// Scrollback rows a repaint carries at most. The whole repaint travels in one
/// transport response, so the byte bound below wins when rows are wide or styled.
const HISTORY_ROWS: usize = 2000;
const HISTORY_BYTES: usize = 768 * 1024;

/// Bytes that make a viewer's terminal show the held terminal's screen: its scrollback
/// above, then the active screen with its styles, hyperlinks, cursor, modes and
/// keyboard state. The viewer's own history is kept beneath.
pub(super) fn repaint(terminal: &Terminal) -> Result<Vec<u8>, Error> {
    let mut bytes = b"\x1b[0m\x1b[?25h".to_vec();
    if !terminal.received_output() {
        // Nothing to redraw: the held terminal's first output flows from the viewer's cursor.
        return Ok(bytes);
    }
    // The formatter emits enabled modes, not every disabled mode. Leave the old
    // screen without 1049's saved-cursor restore, which also moves a viewer
    // already on its primary screen above its existing output.
    bytes.extend(b"\x1b[?1047l\x1b[?47l");
    bytes.extend(RESET_MODES);
    bytes.extend(RESET_DRAWING);
    let retained_primary = if terminal.alternate_screen() {
        // Switching only a disposable copy preserves the held terminal's parser,
        // continuation and replies. Paint primary first so a bare screen switch
        // later reveals the held terminal's retained content and saved cursor.
        let mut primary = Terminal::decode(&terminal.encode()?)?;
        primary.write(b"\x18\x1b[?1049l\x1b[?1047l\x1b[?47l");
        Some(primary)
    } else {
        None
    };
    let primary = retained_primary.as_ref().unwrap_or(terminal);
    bytes.extend(history(primary)?);
    // Scroll the viewer's content and the held terminal's history out of the way.
    let (_, rows) = primary.size();
    for _ in 0..rows {
        bytes.extend(b"\r\n");
    }
    bytes.extend(primary.repaint_screen()?);
    // The formatter enters alternate before homing for its tabstops/content.
    // Homing here would overwrite the primary cursor that 1049 must save.
    if retained_primary.is_some() {
        bytes.extend(terminal.repaint_screen()?);
    }
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
    use super::{repaint, AlternateScreen, RESET_DRAWING, RESET_MODES};
    use crate::ghostty_vt::Terminal;

    fn viewer_showing(bytes: &[u8]) -> Terminal {
        let mut viewer = Terminal::new(80, 24).unwrap();
        viewer.write(bytes);
        viewer
    }

    #[test]
    fn drawing_cleanup_keeps_output_and_cursor_when_returning_to_the_shell() {
        for modes in [b"".as_slice(), b"\x1b[3;20r\x1b[?6h"] {
            let mut terminal = viewer_showing(b"earlier command output\r\n");
            terminal.write(modes);
            terminal.write(b"\x1b[7;21Hlast line");
            let text = terminal.text().unwrap();
            let cursor = terminal.cursor();
            terminal.write(RESET_MODES);
            terminal.write(RESET_DRAWING);
            assert_eq!(terminal.text().unwrap(), text);
            assert_eq!(terminal.cursor(), cursor);
            // Origin and margins must also be reset, not restored with the cursor.
            terminal.write(b"\x1b[Htop\x1b[24;1Hbottom");
            let lines = terminal.text().unwrap();
            assert!(
                lines.lines().next().unwrap().starts_with("top"),
                "modes={modes:?}, screen={lines:?}"
            );
            assert!(
                lines.lines().nth(23).unwrap().starts_with("bottom"),
                "modes={modes:?}, screen={lines:?}"
            );
        }
    }

    #[test]
    fn attaching_before_the_held_terminal_draws_leaves_the_viewer_terminal_alone() {
        let fresh = Terminal::new(80, 24).unwrap();
        assert_eq!(repaint(&fresh).unwrap(), b"\x1b[0m\x1b[?25h");
        let mut viewer = viewer_showing(b"shell prompt> ");
        let cursor = viewer.cursor();
        viewer.write(&repaint(&fresh).unwrap());
        assert_eq!(viewer.text().unwrap().trim(), "shell prompt>");
        assert_eq!(viewer.cursor(), cursor);
        let mut drawn = Terminal::new(80, 24).unwrap();
        drawn.write(b"Error: no stock\r\n");
        let restored = viewer_showing(&repaint(&drawn).unwrap());
        assert_eq!(restored.text().unwrap().trim(), "Error: no stock");
        assert_eq!(restored.cursor(), drawn.cursor());
    }

    #[test]
    fn attaching_to_a_blank_screen_preserves_mode_only_output() {
        let mut held = Terminal::new(80, 24).unwrap();
        held.write(b"\x1b[?2004h\x1b[?1h\x1b[>5u");
        assert!(held.text().unwrap().trim().is_empty());
        assert_eq!(held.cursor(), (0, 0));
        let mut viewer = viewer_showing(&repaint(&held).unwrap());
        let queries = b"\x1b[?2004$p\x1b[?1$p\x1b[?u";
        held.write(queries);
        viewer.write(queries);
        assert_eq!(viewer.take_replies(), held.take_replies());
    }

    #[test]
    fn redrawing_shows_the_held_screen_and_keeps_the_viewers_scrollback() {
        let mut held = Terminal::new(80, 24).unwrap();
        for line in 0..30 {
            held.write(format!("held {line}\r\n").as_bytes());
        }
        let mut viewer = viewer_showing(b"$ ls\r\nnotes.md\r\n$ lf resume\r\n");
        viewer.write(&repaint(&held).unwrap());
        assert_eq!(viewer.text().unwrap(), held.text().unwrap());
        let viewer_rows = viewer.scrollback_rows();
        let viewer_history =
            String::from_utf8_lossy(&viewer.repaint_history(viewer_rows).unwrap()).into_owned();
        assert!(
            viewer_history.starts_with("$ ls\r\nnotes.md\r\n$ lf resume"),
            "{viewer_history:?}"
        );
        // The held terminal's scrollback follows the viewer's own.
        assert!(viewer_history.contains("held 0\r\n"), "{viewer_history:?}");
        assert!(viewer_history.contains("held 5\r\n"), "{viewer_history:?}");
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
    fn reattachment_restores_default_modes_over_a_dirty_viewer() {
        for saved in [false, true] {
            let mut held = viewer_showing(b"ready\r\n> ");
            if saved {
                held = Terminal::decode(&held.encode().unwrap()).unwrap();
            }
            let mut viewer = viewer_showing(
                b"\x1b[?1049h\x1b[?25l\x1b[?2004h\x1b[?1h\x1b[?1004h\x1b[?1003h\x1b[?1006h\x1b[>5uold screen",
            );
            viewer.write(&repaint(&held).unwrap());
            let queries =
                b"\x1b[?25$p\x1b[?2004$p\x1b[?1$p\x1b[?1004$p\x1b[?1003$p\x1b[?1006$p\x1b[?u";
            held.write(queries);
            viewer.write(queries);
            assert_eq!(viewer.take_replies(), held.take_replies());
            assert!(!viewer.alternate_screen());
            assert_eq!(viewer.cursor(), held.cursor());
            assert_eq!(viewer.text().unwrap(), held.text().unwrap());
            held.write(b"typed after return");
            viewer.write(b"typed after return");
            assert_eq!(viewer.text().unwrap(), held.text().unwrap());
        }
    }

    #[test]
    fn restores_alternate_screen_colors_links_and_input_modes() {
        let mut held = Terminal::new(80, 24).unwrap();
        held.write(b"\x1b[?1049h\x1b[?2004h\x1b[?1h\x1b[>1u\x1b[34m\x1b]8;;https://loopflow.studio\x1b\\hello\x1b]8;;\x1b\\");
        let bytes = repaint(&held).unwrap();
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
        assert_eq!(restored.cursor(), held.cursor());
    }

    #[test]
    fn alternate_attachment_retains_primary_screen_history_and_modes() {
        for saved in [false, true] {
            let mut held = Terminal::new(80, 24).unwrap();
            for line in 0..30 {
                held.write(format!("primary {line}\r\n").as_bytes());
            }
            held.write(b"kept primary\x1b[?2004h\x1b[>5u\x1b[?1049h\x1b[Halternate\x1b[>1u");
            if saved {
                held = Terminal::decode(&held.encode().unwrap()).unwrap();
            }
            // Attachment must not complete or discard unfinished live input.
            held.write(b"\x1b[?1");
            let before = held.encode().unwrap();
            let mut viewer = viewer_showing(b"old shell history\r\n");
            viewer.write(&repaint(&held).unwrap());
            assert_eq!(held.encode().unwrap(), before);
            assert!(viewer.alternate_screen());
            assert_eq!(viewer.text().unwrap(), held.text().unwrap());
            assert_eq!(viewer.cursor(), held.cursor());
            viewer.write(&held.continuation().unwrap());
            held.write(b"h");
            viewer.write(b"h");

            for terminal in [&mut held, &mut viewer] {
                terminal.write(b"\x1b[?2004$p\x1b[?1$p\x1b[?u");
            }
            assert_eq!(viewer.take_replies(), held.take_replies());
            // No redraw accompanies the program's return to its primary screen.
            held.write(b"\x1b[?1049l");
            viewer.write(b"\x1b[?1049l");
            assert!(!viewer.alternate_screen());
            assert_eq!(viewer.text().unwrap(), held.text().unwrap());
            assert_eq!(viewer.cursor(), held.cursor());
            for terminal in [&mut held, &mut viewer] {
                terminal.write(b"\x1b[?2004$p\x1b[?1$p\x1b[?u");
            }
            assert_eq!(viewer.take_replies(), held.take_replies());
            let history = viewer.repaint_history(viewer.scrollback_rows()).unwrap();
            let history = String::from_utf8(history).unwrap();
            assert!(history.starts_with("old shell history\r\n"), "{history:?}");
            let held_history = held.repaint_history(held.scrollback_rows()).unwrap();
            assert!(
                history.contains(std::str::from_utf8(&held_history).unwrap()),
                "{history:?}"
            );
        }
    }

    #[test]
    fn program_colors_survive_attachment_and_snapshot_without_replacing_the_theme() {
        for saved in [false, true] {
            let mut held = Terminal::new(80, 24).unwrap();
            held.write(b"\x1b]4;1;rgb:12/34/56\x1b\\\x1b[31mP\x1b[0m\x1b[?1049h\x1b[H\x1b[31mA\x1b[32mT\x1b[0mD");
            held.write(b"\x1b[33mE");
            let explicit_default = held.foreground(3, 0);
            held.write(
                format!(
                    "\x1b]4;3;rgb:{:02x}/{:02x}/{:02x}\x1b\\",
                    explicit_default[0], explicit_default[1], explicit_default[2]
                )
                .as_bytes(),
            );
            if saved {
                held = Terminal::decode(&held.encode().unwrap()).unwrap();
            }
            let mut viewer = Terminal::new(80, 24).unwrap();
            let theme = [0x87, 0x65, 0x43];
            viewer.set_theme(theme);
            viewer.write(&repaint(&held).unwrap());
            assert_eq!(viewer.foreground(0, 0), held.foreground(0, 0));
            assert_eq!(viewer.foreground(0, 0), [0x12, 0x34, 0x56]);
            assert_eq!(viewer.foreground(1, 0), theme);
            assert_eq!(viewer.foreground(2, 0), theme);
            assert_eq!(viewer.foreground(3, 0), explicit_default);
            // Forwarded indexed output uses the restored program palette too.
            for terminal in [&mut held, &mut viewer] {
                terminal.write(b"\x1b[31mN\x1b[?1049l");
            }
            assert_eq!(viewer.foreground(0, 0), held.foreground(0, 0));
            viewer.write(RESET_MODES);
            assert_eq!(viewer.foreground(0, 0), theme);
        }
    }

    #[test]
    fn reattachment_preserves_origin_relative_cursor_and_future_output() {
        for alternate in [false, true] {
            for saved in [false, true] {
                let mut held = Terminal::new(80, 24).unwrap();
                if alternate {
                    held.write(b"\x1b[?1049h");
                }
                for row in 1..=24 {
                    held.write(format!("\x1b[{row};1Hrow {row}").as_bytes());
                }
                held.write(b"\x1b[5;20r\x1b[?69h\x1b[7;70s\x1b[?6h\x1b[3;8Hinside");
                if saved {
                    held = Terminal::decode(&held.encode().unwrap()).unwrap();
                }
                let mut viewer = viewer_showing(&repaint(&held).unwrap());
                assert_eq!(viewer.cursor(), held.cursor());
                assert_eq!(viewer.text().unwrap(), held.text().unwrap());
                held.write(b" next");
                viewer.write(b" next");
                assert_eq!(viewer.text().unwrap(), held.text().unwrap());
            }
        }
    }

    #[test]
    fn reattachment_preserves_keyboard_stack_when_program_pops_a_mode() {
        for saved in [false, true] {
            for depth in [2, 10] {
                let mut held = Terminal::new(80, 24).unwrap();
                for flags in 1..=depth {
                    held.write(format!("\x1b[>{flags}u").as_bytes());
                }
                if saved {
                    held = Terminal::decode(&held.encode().unwrap()).unwrap();
                }
                let mut viewer = viewer_showing(&repaint(&held).unwrap());
                for _ in 0..10 {
                    for terminal in [&mut held, &mut viewer] {
                        terminal.write(b"\x1b[?u");
                    }
                    assert_eq!(viewer.take_replies(), held.take_replies());
                    held.write(b"\x1b[<u");
                    viewer.write(b"\x1b[<u");
                }
            }
        }
    }

    #[test]
    fn cleanup_clears_all_nested_keyboard_modes() {
        let mut viewer = viewer_showing(b"\x1b[>5u\x1b[>1u");
        viewer.write(RESET_MODES);
        viewer.write(b"\x1b[?u");
        assert_eq!(viewer.take_replies(), b"\x1b[?0u");
    }

    #[test]
    fn reattachment_preserves_pending_wrap_after_a_wide_character() {
        let mut held = Terminal::new(80, 24).unwrap();
        held.write(b"\x1b[1;79H");
        held.write("界".as_bytes());
        let mut viewer = viewer_showing(&repaint(&held).unwrap());
        assert_eq!(viewer.text().unwrap(), held.text().unwrap());
        held.write(b"next");
        viewer.write(b"next");
        assert_eq!(viewer.text().unwrap(), held.text().unwrap());
        assert_eq!(viewer.cursor(), held.cursor());
    }
}
