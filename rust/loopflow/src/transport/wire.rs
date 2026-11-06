//! What crosses a viewer connection: length-prefixed frames, JSON requests and
//! replies, and events riding inside the terminal stream as nonce-tagged APC
//! sequences. The stream a viewer counts and the owner spools is the bytes of
//! its snapshot, out and err frames, in order.

use std::io;

use serde::{Deserialize, Serialize};

/// The largest stream frame the owner emits.
pub(super) const CHUNK: usize = 64 * 1024;
/// The largest payload either side accepts.
pub(super) const MAX_PAYLOAD: usize = 4 * 1024 * 1024;
/// Events may span stream frames, up to the transport's payload limit.
const MAX_EVENT: usize = MAX_PAYLOAD;
const HEADER: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Tag {
    /// JSON answering one request.
    Reply = 0x01,
    /// The owner's terminal snapshot, in whatever format the owner has.
    Snapshot = 0x02,
    /// Stream 1: PTY output, or stdout for a piped command.
    Out = 0x03,
    /// Stream 2: stderr for a piped command.
    Err = 0x04,
    /// JSON carrying an `id`.
    Request = 0x11,
    /// Keystrokes from the controlling connection.
    Input = 0x12,
}

impl Tag {
    /// The tag carrying stream `stream`: 1 is out, 2 is err.
    pub(super) fn stream(stream: u8) -> Self {
        if stream == 2 {
            Self::Err
        } else {
            Self::Out
        }
    }

    fn parse(byte: u8) -> Option<Self> {
        Some(match byte {
            0x01 => Self::Reply,
            0x02 => Self::Snapshot,
            0x03 => Self::Out,
            0x04 => Self::Err,
            0x11 => Self::Request,
            0x12 => Self::Input,
            _ => return None,
        })
    }
}

/// Append `payload` as one frame.
pub(super) fn encode(tag: Tag, payload: &[u8], into: &mut impl Extend<u8>) {
    into.extend([tag as u8]);
    into.extend((payload.len() as u32).to_be_bytes());
    into.extend(payload.iter().copied());
}

pub(super) fn frame(tag: Tag, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER + payload.len());
    encode(tag, payload, &mut bytes);
    bytes
}

/// Reassemble frames from a byte stream.
#[derive(Debug, Default)]
pub(super) struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn extend(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn next(&mut self) -> io::Result<Option<(Tag, Vec<u8>)>> {
        if self.buffer.len() < HEADER {
            return Ok(None);
        }
        let tag = Tag::parse(self.buffer[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unknown transport frame"))?;
        let length = u32::from_be_bytes(
            self.buffer[1..HEADER]
                .try_into()
                .expect("header holds four length bytes"),
        ) as usize;
        if length > MAX_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "transport frame too large",
            ));
        }
        if self.buffer.len() < HEADER + length {
            return Ok(None);
        }
        let payload = self.buffer[HEADER..HEADER + length].to_vec();
        self.buffer.drain(..HEADER + length);
        Ok(Some((tag, payload)))
    }
}

// ---------------------------------------------------------------------------
// Requests and replies

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Own {
    /// Become the size and color owner now.
    Take,
    /// Queue behind the owner; granted when it leaves.
    Request,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Incoming {
    pub id: u64,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Request {
    Attach {
        control: bool,
        replace: bool,
        own: Option<Own>,
        rows: u16,
        cols: u16,
        /// How this viewer is named to others who lose ownership to it.
        name: Option<String>,
        /// What a controlling viewer this attachment replaces prints as it leaves.
        moved: Option<String>,
    },
    /// Continue a dropped viewer's stream, no snapshot.
    Resume(Resume),
    Resize {
        rows: u16,
        cols: u16,
    },
    EndInput,
    Own {
        mode: Own,
    },
    /// Remember this viewer's colors; relay them while it owns appearance.
    Theme {
        theme: serde_json::Value,
    },
    /// The viewer is closing on purpose; keep nothing for it.
    Leave,
    DetachViewers {
        key: String,
    },
    /// An OSC 7501 program status for `program`, relayed to every viewer.
    Status {
        key: String,
        program: String,
        state: crate::program_status::State,
        progress: Option<u8>,
    },
    Directory {
        key: String,
        cwd: std::path::PathBuf,
    },
    /// The direct child's native client began. `exit` replaces the client's own
    /// exit text; a detaching viewer prints `detach`.
    NativeStarted {
        key: String,
        exit: String,
        detach: String,
    },
    /// The native client exited; acknowledged once its exit line is delivered.
    NativeEnded {
        key: String,
    },
    /// What a controlling viewer can type, shown when it attaches and while the
    /// registering connection stays open.
    InputHint {
        key: String,
        hint: String,
    },
    /// Whether some viewer holds input control, asked before opening another.
    /// On an attached connection this also refreshes the viewer's liveness.
    Controlled,
    /// Transfer the direct child's exit recording to its owner before exec.
    ExitRecord {
        key: String,
        pid: u32,
        record: Box<crate::harness::native_exit::NativeExit>,
    },
    /// Cancel that transfer when exec fails.
    DropExitRecord {
        key: String,
    },
}

/// A dropped viewer's connection asking to continue from byte `received`.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Resume {
    pub reconnect: String,
    pub received: u64,
    pub control: bool,
    pub own: Option<Own>,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Outgoing {
    /// The request this answers.
    pub answers: u64,
    /// The last event queued on this viewer's stream when the reply was computed.
    pub seq: u64,
    #[serde(flatten)]
    pub reply: Reply,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Reply {
    Attached {
        viewer: String,
        reconnect: String,
        nonce: String,
        /// The input claim, when this connection may type.
        token: Option<String>,
        terminal: bool,
        grace_ms: u64,
        owner: Option<String>,
    },
    Busy,
    /// The spool no longer holds the requested byte; attach afresh.
    ReconnectRequired,
    Controlled {
        held: bool,
    },
    NativeEnded,
    Accepted,
}

// ---------------------------------------------------------------------------
// In-band events

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Event {
    Resize {
        rows: u16,
        cols: u16,
    },
    Theme {
        theme: serde_json::Value,
    },
    /// Who sizes and colors the terminal now; `held` is whether the recipient does.
    Owner {
        viewer: Option<String>,
        name: Option<String>,
        held: bool,
    },
    /// Input control changed hands. A recipient with `held: false` restores
    /// its terminal, prints `notice` and leaves: another viewer took control,
    /// or the owner is ending.
    Control {
        held: bool,
        notice: Option<String>,
    },
    ChildExited {
        code: u8,
    },
    Detached,
    Hint {
        text: String,
    },
    /// How a viewer that detaches can return.
    Return {
        detach: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Sequenced {
    seq: u64,
    #[serde(flatten)]
    event: Event,
}

fn prefix(nonce: &str) -> Vec<u8> {
    format!("\x1b_lf1;{nonce};").into_bytes()
}

/// `event` as the APC sequence a viewer holding `nonce` recognizes.
pub(super) fn event(nonce: &str, seq: u64, event: Event) -> Vec<u8> {
    let mut bytes = prefix(nonce);
    bytes.extend(serde_json::to_vec(&Sequenced { seq, event }).expect("events serialize"));
    bytes.extend(b"\x1b\\");
    bytes
}

/// One stretch of a scanned stream: program bytes or one of our events.
#[derive(Debug, PartialEq)]
pub(super) enum Piece {
    Bytes(Vec<u8>),
    Event(u64, Event),
}

/// Separate a viewer's events from program output, across frame boundaries.
#[derive(Debug)]
pub(super) struct Scanner {
    prefix: Vec<u8>,
    /// Bytes that may begin or continue one of our events.
    held: Vec<u8>,
}

impl Scanner {
    pub fn new(nonce: &str) -> Self {
        Self {
            prefix: prefix(nonce),
            held: Vec::new(),
        }
    }

    pub fn scan(&mut self, bytes: &[u8]) -> Vec<Piece> {
        let mut data = std::mem::take(&mut self.held);
        data.extend_from_slice(bytes);
        let mut pieces = Vec::new();
        let mut text = Vec::new();
        let mut i = 0;
        while i < data.len() {
            if data[i] != 0x1b {
                text.push(data[i]);
                i += 1;
                continue;
            }
            let rest = &data[i..];
            let common = rest.len().min(self.prefix.len());
            if rest[..common] != self.prefix[..common] {
                text.push(0x1b);
                i += 1;
                continue;
            }
            if rest.len() < self.prefix.len() {
                self.held = rest.to_vec();
                break;
            }
            let body = &rest[self.prefix.len()..];
            match body.windows(2).position(|w| w == b"\x1b\\") {
                Some(end) if self.prefix.len() + end + 2 <= MAX_EVENT => {
                    if let Ok(Sequenced { seq, event }) = serde_json::from_slice(&body[..end]) {
                        if !text.is_empty() {
                            pieces.push(Piece::Bytes(std::mem::take(&mut text)));
                        }
                        pieces.push(Piece::Event(seq, event));
                    } else {
                        text.extend_from_slice(&rest[..self.prefix.len() + end + 2]);
                    }
                    i += self.prefix.len() + end + 2;
                }
                None if rest.len() <= MAX_EVENT => {
                    self.held = rest.to_vec();
                    break;
                }
                _ => {
                    text.push(0x1b);
                    i += 1;
                }
            }
        }
        if !text.is_empty() {
            pieces.push(Piece::Bytes(text));
        }
        pieces
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{
        encode, event, frame, Decoder, Event, Incoming, Piece, Request, Scanner, Tag, MAX_PAYLOAD,
    };

    #[test]
    fn a_maximum_frame_can_share_a_read_with_the_next_frame() {
        let payload = vec![b'x'; MAX_PAYLOAD];
        let mut bytes = frame(Tag::Input, &payload);
        bytes.extend(frame(Tag::Input, b"next"));
        let mut decoder = Decoder::default();
        let mut decoded = Vec::new();
        for chunk in bytes.chunks(8192) {
            decoder.extend(chunk);
            while let Some(frame) = decoder.next().unwrap() {
                decoded.push(frame);
            }
        }
        assert_eq!(
            decoded,
            vec![(Tag::Input, payload), (Tag::Input, b"next".to_vec())]
        );
    }

    #[test]
    fn an_oversized_frame_is_rejected_before_receiving_its_payload() {
        let mut decoder = Decoder::default();
        decoder.extend(&[Tag::Input as u8]);
        decoder.extend(&((MAX_PAYLOAD + 1) as u32).to_be_bytes());
        assert_eq!(
            decoder.next().unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn frames_round_trip_across_split_reads() {
        let mut wire = VecDeque::new();
        encode(Tag::Out, b"hello", &mut wire);
        encode(Tag::Err, b"", &mut wire);
        let wire: Vec<u8> = wire.into();
        let mut decoder = Decoder::default();
        decoder.extend(&wire[..3]);
        assert!(decoder.next().unwrap().is_none());
        decoder.extend(&wire[3..]);
        assert_eq!(decoder.next().unwrap(), Some((Tag::Out, b"hello".to_vec())));
        assert_eq!(decoder.next().unwrap(), Some((Tag::Err, Vec::new())));
        assert!(decoder.next().unwrap().is_none());
    }

    #[test]
    fn a_status_report_keeps_its_request_id_and_program_apart() {
        let sent = Incoming {
            id: 7,
            request: Request::Status {
                key: "k".into(),
                program: "lf/abc/step".into(),
                state: crate::program_status::State::Working,
                progress: Some(40),
            },
        };
        let json = serde_json::to_vec(&sent).unwrap();
        let received: Incoming = serde_json::from_slice(&json).unwrap();
        assert_eq!(received.id, 7);
        let Request::Status {
            program, progress, ..
        } = received.request
        else {
            panic!("{received:?}");
        };
        assert_eq!(program, "lf/abc/step");
        assert_eq!(progress, Some(40));
    }

    #[test]
    fn events_are_recognized_only_with_the_viewers_nonce() {
        let resize = Event::Resize {
            rows: 40,
            cols: 120,
        };
        let ours = event("a1b2", 7, resize.clone());
        let theirs = event("ffff", 7, resize.clone());
        let mut scanner = Scanner::new("a1b2");
        let mut stream = b"before".to_vec();
        stream.extend(&ours);
        stream.extend(&theirs);
        stream.extend(b"after\x1b_lf1;a1b2;not json\x1b\\");
        let pieces = scanner.scan(&stream);
        assert_eq!(pieces[0], Piece::Bytes(b"before".to_vec()));
        assert_eq!(pieces[1], Piece::Event(7, resize));
        let Piece::Bytes(tail) = &pieces[2] else {
            panic!("{pieces:?}");
        };
        assert!(tail.starts_with(&theirs), "{tail:?}");
        assert!(tail.ends_with(b"not json\x1b\\"), "{tail:?}");
        assert_eq!(pieces.len(), 3);
    }

    #[test]
    fn an_event_split_across_frames_is_reassembled() {
        let exit = Event::ChildExited { code: 3 };
        let bytes = event("0123456789abcdef", 1, exit.clone());
        let mut scanner = Scanner::new("0123456789abcdef");
        let mut pieces = Vec::new();
        for byte in &bytes {
            pieces.extend(scanner.scan(&[*byte]));
        }
        assert_eq!(pieces, vec![Piece::Event(1, exit)]);
        assert_eq!(
            scanner.scan(b"\x1b[0m"),
            vec![Piece::Bytes(b"\x1b[0m".to_vec())]
        );
    }

    #[test]
    fn a_large_event_is_independent_of_read_boundaries() {
        let theme = Event::Theme {
            theme: serde_json::json!({"palette": "x".repeat(2 * super::CHUNK)}),
        };
        let bytes = event("0123456789abcdef", 1, theme.clone());
        let expected = vec![Piece::Event(1, theme)];
        assert_eq!(Scanner::new("0123456789abcdef").scan(&bytes), expected);
        let mut scanner = Scanner::new("0123456789abcdef");
        let pieces: Vec<_> = bytes
            .chunks(8192)
            .flat_map(|chunk| scanner.scan(chunk))
            .collect();
        assert!(
            pieces == expected,
            "a fragmented event must not become terminal output"
        );
    }

    #[test]
    fn a_program_escape_that_merely_starts_like_a_prefix_is_passed_through() {
        let mut scanner = Scanner::new("abcd");
        assert_eq!(scanner.scan(b"\x1b_l"), vec![]);
        assert_eq!(
            scanner.scan(b"x\x1b\\done"),
            vec![Piece::Bytes(b"\x1b_lx\x1b\\done".to_vec())]
        );
    }
}
