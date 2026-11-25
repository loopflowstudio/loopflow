//! Forward queued input directly to the provider, advancing only by bytes written.

use std::collections::VecDeque;
use std::io::{self, Write};

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// The delimiters already accepted from one input stream, without its content.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Paste {
    open: bool,
    matched: usize,
}

impl Paste {
    pub fn observe(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let marker = if self.open { PASTE_END } else { PASTE_START };
            if byte == marker[self.matched] {
                self.matched += 1;
                if self.matched == marker.len() {
                    self.open = !self.open;
                    self.matched = 0;
                }
            } else {
                self.matched = usize::from(byte == marker[0]);
            }
        }
    }

    /// Close only an unfinished paste, including a partially written end marker.
    pub fn finish(&mut self) -> &'static [u8] {
        if !self.open {
            return &[];
        }
        let rest = &PASTE_END[self.matched..];
        tracing::debug!("closing an interrupted terminal paste");
        *self = Self::default();
        rest
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn detaches(&self, bytes: &[u8]) -> bool {
        let mut paste = *self;
        for &byte in bytes {
            if byte == 0x1d && !paste.open {
                return true;
            }
            paste.observe(std::slice::from_ref(&byte));
        }
        false
    }

    /// A new connection continues the paste's framing, never its content.
    pub fn resume(&self) -> Vec<u8> {
        if self.open {
            [PASTE_START, &PASTE_END[..self.matched]].concat()
        } else {
            Vec::new()
        }
    }
}

/// Input from a view that does not size the terminal, without its terminal's
/// answers to the agent's queries. Every view's terminal answers them; the
/// agent hears one answer, from the view that sizes it. Keys pass unchanged.
pub(super) fn keys(mut bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
    std::iter::from_fn(move || {
        while let Some(length) = report_length(bytes) {
            bytes = &bytes[length..];
        }
        let (&key, rest) = bytes.split_first()?;
        bytes = rest;
        Some(key)
    })
}

/// The length of the terminal report `bytes` starts with: an OSC or DCS
/// string, a cursor position, status, device attributes, mode or window
/// report, or a focus event.
fn report_length(bytes: &[u8]) -> Option<usize> {
    match bytes {
        [0x1b, b']' | b'P', body @ ..] => {
            body.iter().enumerate().find_map(|(at, byte)| match byte {
                0x07 => Some(at + 3),
                0x1b if body.get(at + 1) == Some(&b'\\') => Some(at + 4),
                _ => None,
            })
        }
        [0x1b, b'[', body @ ..] => {
            let private = matches!(body.first(), Some(b'?' | b'>' | b'='));
            let mut at = usize::from(private);
            while body
                .get(at)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
            {
                at += 1;
            }
            let dollar = body.get(at) == Some(&b'$');
            at += usize::from(dollar);
            let report = match *body.get(at)? {
                b'y' => dollar,
                _ if dollar => false,
                b'R' | b'n' | b't' => true,
                b'c' => private,
                b'u' => body[0] == b'?',
                b'I' | b'O' => at == 0,
                _ => false,
            };
            report.then_some(at + 3)
        }
        _ => None,
    }
}

/// Whether `bytes` contains any real user keystroke (ignoring terminal query
/// replies, focus reports, and mouse events).
pub(super) fn has_user_keystroke(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        if let Some(length) = report_length(bytes).or_else(|| mouse_event_length(bytes)) {
            bytes = &bytes[length..];
            continue;
        }
        return true;
    }
    false
}

/// The length of a terminal mouse report `bytes` starts with: SGR (`1006`),
/// X10 (`1000`), or URXVT (`1015`).
fn mouse_event_length(bytes: &[u8]) -> Option<usize> {
    match bytes {
        // X10 / normal mouse tracking: ESC [ M <cb> <cx> <cy>
        [0x1b, b'[', b'M', _, _, _, ..] => Some(6),
        // SGR mouse tracking: ESC [ < digits ; digits ; digits (M | m)
        [0x1b, b'[', b'<', body @ ..] => {
            let mut at = 0;
            while body
                .get(at)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
            {
                at += 1;
            }
            (at > 0 && matches!(body.get(at), Some(b'M' | b'm'))).then_some(at + 4)
        }
        // URXVT mouse tracking: ESC [ digits ; digits ; digits M
        [0x1b, b'[', body @ ..] => {
            let mut at = 0;
            let mut semicolons = 0;
            while body
                .get(at)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b';')
            {
                if body[at] == b';' {
                    semicolons += 1;
                }
                at += 1;
            }
            (semicolons == 2 && body.get(at) == Some(&b'M')).then_some(at + 3)
        }
        _ => None,
    }
}

#[derive(Debug, Default)]
pub(super) struct PendingInput {
    bytes: VecDeque<u8>,
    paste: Paste,
}

impl PendingInput {
    /// False leaves the queue unchanged when it cannot take more input.
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        if self.bytes.len() + bytes.len() > super::LIMIT {
            return false;
        }
        self.bytes.extend(bytes);
        self.paste.observe(bytes);
        true
    }

    pub fn in_paste(&self) -> bool {
        self.paste.is_open()
    }

    pub fn finish_paste(&mut self) {
        self.bytes.extend(self.paste.finish());
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn forward(&mut self, input: &mut impl Write) {
        if self.is_empty() {
            return;
        }
        match input.write(self.bytes.as_slices().0) {
            Ok(n) => {
                self.bytes.drain(..n);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => self.bytes.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{has_user_keystroke, keys, Paste, PendingInput};

    #[test]
    fn mouse_focus_and_query_reports_are_not_user_keystrokes() {
        for ignored in [
            b"\x1b[I".as_slice(),
            b"\x1b[O",
            b"\x1b[12;40R",
            b"\x1b[?62;22c",
            b"\x1b]11;rgb:1e1e/1e1e/2e2e\x07",
            b"\x1b[<64;10;20M",
            b"\x1b[<0;10;20m",
            b"\x1b[M !#",
            b"\x1b[32;10;20M",
            b"\x1b[I\x1b[<65;1;2M\x1b[O",
        ] {
            assert!(!has_user_keystroke(ignored), "{ignored:?}");
        }
        for typed in [
            b"a".as_slice(),
            b"\r",
            b"\x7f",
            b"\x1b",
            b"\x1b[A",
            b"\x1b[<64;10;20Mhello",
            b"\x1b[200~pasted\x1b[201~",
        ] {
            assert!(has_user_keystroke(typed), "{typed:?}");
        }
    }

    #[test]
    fn a_second_views_terminal_answers_never_reach_the_agent_but_its_keys_do() {
        let answers: &[&[u8]] = &[
            b"\x1b[12;40R",
            b"\x1b[0n",
            b"\x1b[?62;22c",
            b"\x1b[>1;10;0c",
            b"\x1b[?2026;2$y",
            b"\x1b[8;40;120t",
            b"\x1b[?1u",
            b"\x1b]11;rgb:1e1e/1e1e/2e2e\x07",
            b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\",
            b"\x1bP>|ghostty 1.2\x1b\\",
            b"\x1b[I",
        ];
        for answer in answers {
            let typed = [b"a".as_slice(), answer, b"b"].concat();
            assert!(keys(&typed).eq(*b"ab"), "{answer:?}");
        }
        let sequences: &[&[u8]] = &[
            b"\x1b[A",
            b"\x1b[1;5D",
            b"\x1b[3~",
            b"\x1b[97;5u",
            b"\x1b[Z",
            b"\x1bOP",
            b"\x1b[200~pasted\x1b[201~",
            b"plain\r",
        ];
        for sequence in sequences {
            assert!(keys(sequence).eq(sequence.iter().copied()), "{sequence:?}");
        }
    }

    fn drain(pending: &mut PendingInput, output: &mut Vec<u8>) {
        while !pending.is_empty() {
            pending.forward(output);
        }
    }

    #[test]
    fn complete_pastes_and_ordinary_keys_pass_through_unchanged() {
        for bytes in [
            b"keys\x1b[D\r\x1b".as_slice(),
            b"\x1b[200~one\ntwo\x1b[201~\r",
        ] {
            for split in 0..=bytes.len() {
                let mut pending = PendingInput::default();
                let mut output = Vec::new();
                assert!(pending.push(&bytes[..split]));
                drain(&mut pending, &mut output);
                assert!(pending.push(&bytes[split..]));
                drain(&mut pending, &mut output);
                pending.finish_paste();
                drain(&mut pending, &mut output);
                assert_eq!(output, bytes);
            }
        }
    }

    #[test]
    fn interrupted_pastes_close_once_without_return_or_replayed_content() {
        let paste = b"\x1b[200~one\ntwo\x1b[201~";
        for cut in 6..paste.len() {
            let mut pending = PendingInput::default();
            let mut output = Vec::new();
            for byte in &paste[..cut] {
                assert!(pending.push(std::slice::from_ref(byte)));
                drain(&mut pending, &mut output);
            }
            pending.finish_paste();
            pending.finish_paste();
            assert!(pending.push(b"next"));
            drain(&mut pending, &mut output);
            let body_end = cut.min(13);
            let expected = [&paste[..body_end], b"\x1b[201~", b"next"].concat();
            assert_eq!(output, expected, "interrupted at {cut}");
        }
    }

    #[test]
    fn partial_writes_keep_the_paste_end_after_accepted_content() {
        struct ByteWriter(Vec<u8>);
        impl Write for ByteWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.push(bytes[0]);
                Ok(1)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut pending = PendingInput::default();
        let mut output = ByteWriter(Vec::new());
        pending.push(b"\x1b[200~hello");
        pending.forward(&mut output);
        pending.finish_paste();
        pending.push(b"next");
        while !pending.is_empty() {
            pending.forward(&mut output);
        }
        assert_eq!(output.0, b"\x1b[200~hello\x1b[201~next");
    }

    #[test]
    fn reconnecting_restores_only_framing_before_the_unsent_paste_tail() {
        for split in 6..19 {
            let bytes = b"\x1b[200~one\ntwo\x1b[201~";
            let mut sent = Paste::default();
            sent.observe(&bytes[..split]);
            let mut replacement = Paste::default();
            replacement.observe(&sent.resume());
            replacement.observe(&bytes[split..]);
            assert!(replacement.finish().is_empty(), "split at {split}");
            assert!(!sent.resume().contains(&b'\n'));
        }
    }

    #[test]
    fn pasted_detach_keys_reach_the_provider() {
        let mut paste = Paste::default();
        assert!(!paste.detaches(b"\x1b[200~a\x1db\x1b[201~"));
        assert!(paste.detaches(b"\x1b[200~a\x1db\x1b[201~\x1d"));
        paste.observe(b"\x1b[200~");
        assert!(!paste.detaches(b"\x1d"));
        paste.observe(b"\x1b[201~");
        assert!(paste.detaches(b"\x1d"));
    }
}
