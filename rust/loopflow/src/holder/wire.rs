//! What crosses a holder connection. Requests and answers are one JSON object
//! per line. A `read` turns the connection into frames (`tag u8 · len u32 BE ·
//! payload`); a `write` turns it into raw input bytes.
//!
//! A holder is never restarted for an upgrade, so a newer server speaks this
//! to every older holder still alive. Nothing here has a default: a field is
//! present or the line does not parse.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The protocol this build's holder speaks.
pub const REVISION: u32 = 1;
/// The largest stream frame a holder emits.
pub(super) const CHUNK: usize = 64 * 1024;
/// The largest request line a holder accepts; a `start` carries a whole environment.
pub(super) const MAX_LINE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Hello {
        revision: u32,
    },
    List,
    Start(Start),
    /// The connection becomes `terminal`'s output from byte `from`.
    Read {
        terminal: String,
        from: u64,
    },
    /// The connection becomes `terminal`'s input.
    Write {
        terminal: String,
    },
    Resize {
        terminal: String,
        rows: u16,
        cols: u16,
    },
    /// Put the PTY back in the modes it had before the child ran, after a
    /// program died leaving it raw.
    Restore {
        terminal: String,
    },
    /// Drop an exited terminal's ring once its exit is settled.
    Forget {
        terminal: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    /// Names one PTY for its whole life; never reused.
    pub terminal: String,
    /// Identifies this start: repeating it answers the child it already made.
    pub request: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// The child's whole environment; nothing is inherited from the holder.
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Answer {
    Welcome {
        revision: u32,
        /// SHA-256 of the holder's executable.
        build: String,
        pid: u32,
    },
    /// The asking side is older than this holder can answer.
    Incompatible {
        revision: u32,
    },
    Terminals {
        terminals: Vec<Held>,
    },
    Started {
        pid: u32,
    },
    /// Frames follow.
    Reading,
    /// Input bytes may follow.
    Writing,
    Done,
    Error {
        message: String,
    },
}

/// One PTY as `list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub terminal: String,
    pub request: String,
    pub pid: u32,
    pub rows: u16,
    pub cols: u16,
    /// Bytes the child has written since it started.
    pub written: u64,
    /// Position of the oldest byte still in the ring.
    pub ring_start: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub exit: Option<Exit>,
}

/// How a child ended; also the content of `<terminal>/exit.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exit {
    #[serde(deserialize_with = "Option::deserialize")]
    pub code: Option<i32>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub signal: Option<i32>,
    /// Unix seconds.
    pub at: i64,
    /// The child's final byte count.
    pub written: u64,
}

impl Exit {
    /// The code a shell would report.
    pub fn status(&self) -> u8 {
        self.code.unwrap_or_else(|| 128 + self.signal.unwrap_or(1)) as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Tag {
    /// Raw PTY output continuing the reader's position.
    Out = b'o',
    /// JSON `Gap`: the ring passed the reader's position.
    Gap = b'g',
    /// JSON `Mode`: the PTY's line discipline for the output that follows.
    Mode = b'm',
    /// JSON `Exit`: the last frame of a stream.
    Exit = b'x',
}

impl Tag {
    pub(super) fn parse(byte: u8) -> Option<Self> {
        Some(match byte {
            b'o' => Self::Out,
            b'g' => Self::Gap,
            b'm' => Self::Mode,
            b'x' => Self::Exit,
            _ => return None,
        })
    }
}

/// Output between the reader's position and `ring_start` is gone; the stream
/// continues at `ring_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    pub ring_start: u64,
}

/// Whether the PTY was raw (no line editing) when the following output was
/// read. The server tells a program's screen from its exit text by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub raw: bool,
}

pub(super) fn encode(tag: Tag, payload: &[u8], into: &mut impl Extend<u8>) {
    into.extend([tag as u8]);
    into.extend((payload.len() as u32).to_be_bytes());
    into.extend(payload.iter().copied());
}

pub(super) fn line(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("holder messages serialize");
    bytes.push(b'\n');
    bytes
}

/// One piece of a terminal's output stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Out(Vec<u8>),
    Gap(Gap),
    Mode(Mode),
    Exit(Exit),
}

/// Reassemble a `read` stream's frames.
#[derive(Debug, Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn extend(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn next(&mut self) -> std::io::Result<Option<Frame>> {
        let invalid = |message| std::io::Error::new(std::io::ErrorKind::InvalidData, message);
        if self.buffer.len() < 5 {
            return Ok(None);
        }
        let tag = Tag::parse(self.buffer[0]).ok_or_else(|| invalid("unknown holder frame"))?;
        let length = u32::from_be_bytes(
            self.buffer[1..5]
                .try_into()
                .expect("header holds four length bytes"),
        ) as usize;
        if length > CHUNK {
            return Err(invalid("holder frame too large"));
        }
        if self.buffer.len() < 5 + length {
            return Ok(None);
        }
        let payload = self.buffer[5..5 + length].to_vec();
        self.buffer.drain(..5 + length);
        Ok(Some(match tag {
            Tag::Out => Frame::Out(payload),
            Tag::Gap => Frame::Gap(serde_json::from_slice(&payload)?),
            Tag::Mode => Frame::Mode(serde_json::from_slice(&payload)?),
            Tag::Exit => Frame::Exit(serde_json::from_slice(&payload)?),
        }))
    }
}
