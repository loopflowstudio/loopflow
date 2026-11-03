//! Retain terminal state or a bounded pipe tail for replacement viewers.

use std::collections::{BTreeMap, VecDeque};

use anyhow::{Context, Result};

use crate::ghostty_vt;
use crate::program_status::State;

use super::{terminal, LIMIT};

pub(super) enum Output {
    Terminal(Box<Terminal>),
    Pipes {
        tail: VecDeque<(u8, Vec<u8>)>,
        bytes: usize,
    },
}

pub(super) struct Terminal {
    terminal: ghostty_vt::Terminal,
    reports: BTreeMap<String, Vec<u8>>,
    pending_reports: Vec<u8>,
}

impl Output {
    pub fn new(terminal: bool) -> Result<Self> {
        if terminal {
            Ok(Self::Terminal(Box::new(Terminal {
                terminal: ghostty_vt::Terminal::new(80, 24).context("Open the owner's terminal")?,
                reports: BTreeMap::new(),
                pending_reports: Vec::new(),
            })))
        } else {
            Ok(Self::Pipes {
                tail: VecDeque::new(),
                bytes: 0,
            })
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        if let Self::Terminal(terminal) = self {
            terminal
                .terminal
                .resize(cols, rows)
                .context("Resize the owner's terminal")?;
        }
        Ok(())
    }

    /// The terminal snapshot a fresh viewer's stream begins with; `None` for pipes.
    pub fn snapshot(&self) -> Result<Option<Vec<u8>>> {
        let Self::Terminal(terminal) = self else {
            return Ok(None);
        };
        let mut bytes =
            terminal::repaint(&terminal.terminal).context("Repaint the owner's screen")?;
        for report in terminal.reports.values() {
            bytes.extend(report);
        }
        bytes.extend(
            terminal
                .terminal
                .continuation()
                .context("Provider sequence exceeds snapshot limit; attach after it completes")?,
        );
        Ok(Some(bytes))
    }

    /// The retained pipe tail a fresh viewer's stream begins with.
    pub fn tail(&self) -> impl Iterator<Item = (u8, &[u8])> {
        let tail = match self {
            Self::Pipes { tail, .. } => Some(tail),
            Self::Terminal(_) => None,
        };
        tail.into_iter()
            .flatten()
            .map(|(stream, bytes)| (*stream, bytes.as_slice()))
    }

    /// Whether the stream stands between complete sequences, where an event may go.
    pub fn ready(&self) -> bool {
        match self {
            Self::Terminal(terminal) => terminal.terminal.at_ground(),
            Self::Pipes { .. } => true,
        }
    }

    /// Feed the program's output to the retained state. Returns the replies
    /// its queries are owed while no viewer's terminal is there to answer.
    pub fn process(&mut self, stream: u8, bytes: &[u8], detached: bool) -> Vec<u8> {
        match self {
            Self::Terminal(terminal) => {
                terminal.terminal.write(bytes);
                let replies = terminal.terminal.take_replies();
                if detached {
                    replies
                } else {
                    Vec::new()
                }
            }
            Self::Pipes {
                tail,
                bytes: retained,
            } => {
                tail.push_back((stream, bytes.to_vec()));
                *retained += bytes.len();
                while *retained > LIMIT / 16 {
                    *retained -= tail.pop_front().expect("tail exceeds limit").1.len();
                }
                Vec::new()
            }
        }
    }

    pub fn report(&mut self, id: String, state: State, progress: Option<u8>) {
        let Self::Terminal(terminal) = self else {
            return;
        };
        if !id.starts_with("lf/")
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_.-".contains(&b))
        {
            return;
        }
        let state_name = match state {
            State::Idle => "idle",
            State::Working => "working",
            State::Done => "done",
            State::Blocked => "blocked",
            State::Error => "error",
            State::Clear => "clear",
        };
        let progress = progress
            .filter(|p| *p <= 100 && state == State::Working)
            .map(|p| format!(":progress={p}"))
            .unwrap_or_default();
        let bytes = format!("\x1b]7501;id={id}:state={state_name}{progress}\x1b\\").into_bytes();
        if state == State::Clear {
            terminal.reports.remove(&id);
        } else if terminal.reports.len() < 64 || terminal.reports.contains_key(&id) {
            terminal.reports.insert(id, bytes.clone());
        }
        if terminal.pending_reports.len() + bytes.len() <= LIMIT {
            terminal.pending_reports.extend(bytes);
        }
    }

    pub fn take_reports(&mut self) -> Option<Vec<u8>> {
        let Self::Terminal(terminal) = self else {
            return None;
        };
        if terminal.pending_reports.is_empty() || !terminal.terminal.at_ground() {
            return None;
        }
        Some(std::mem::take(&mut terminal.pending_reports))
    }

    /// The retained terminal, for the snapshot an ending owner leaves behind.
    pub fn terminal(&self) -> Option<&ghostty_vt::Terminal> {
        match self {
            Self::Terminal(terminal) => Some(&terminal.terminal),
            Self::Pipes { .. } => None,
        }
    }
}
