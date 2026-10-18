//! Retain terminal state or a bounded pipe tail for replacement viewers.

use std::collections::{BTreeMap, VecDeque};

use anyhow::{Context, Result};

use crate::program_status::State;

use super::{terminal, Response, Viewer, LIMIT};

pub(super) enum Output {
    Terminal(Box<Terminal>),
    Pipes {
        tail: VecDeque<(u8, Vec<u8>)>,
        bytes: usize,
    },
}

pub(super) struct Terminal {
    parser: vt100::Parser<terminal::Queries>,
    boundary: terminal::Boundary,
    reports: BTreeMap<String, Vec<u8>>,
    pending_reports: Vec<u8>,
}

impl Output {
    pub fn new(terminal: bool) -> Self {
        if terminal {
            Self::Terminal(Box::new(Terminal {
                parser: vt100::Parser::new_with_callbacks(
                    24,
                    80,
                    2000,
                    terminal::Queries::default(),
                ),
                boundary: terminal::Boundary::new(),
                reports: BTreeMap::new(),
                pending_reports: Vec::new(),
            }))
        } else {
            Self::Pipes {
                tail: VecDeque::new(),
                bytes: 0,
            }
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if let Self::Terminal(terminal) = self {
            terminal.parser.screen_mut().set_size(rows, cols);
        }
    }

    pub fn attach(&self, viewer: &mut Viewer) -> Result<()> {
        let bytes = match self {
            Self::Terminal(terminal) => {
                let mut bytes = terminal::snapshot(terminal.parser.screen());
                for report in terminal.reports.values() {
                    bytes.extend(report);
                }
                bytes.extend(terminal.boundary.pending().context(
                    "Provider sequence exceeds snapshot limit; attach after it completes",
                )?);
                bytes
            }
            Self::Pipes { .. } => Vec::new(),
        };
        viewer.queue(&Response::Attached {
            token: viewer.token.clone(),
            terminal: matches!(self, Self::Terminal(_)),
            bytes,
        })?;
        if let Self::Pipes { tail, .. } = self {
            for (stream, bytes) in tail {
                viewer.queue(&Response::Output {
                    stream: *stream,
                    bytes: bytes.clone(),
                })?;
            }
        }
        Ok(())
    }

    pub fn process(&mut self, stream: u8, bytes: &[u8], detached: bool) -> Vec<u8> {
        match self {
            Self::Terminal(terminal) => {
                terminal.parser.callbacks_mut().detached = detached;
                terminal.parser.process(bytes);
                terminal.boundary.advance(bytes);
                std::mem::take(&mut terminal.parser.callbacks_mut().replies)
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

    pub fn take_reports(&mut self) -> Option<Response> {
        let Self::Terminal(terminal) = self else {
            return None;
        };
        if terminal.pending_reports.is_empty() || !terminal.boundary.ready() {
            return None;
        }
        Some(Response::Output {
            stream: 1,
            bytes: std::mem::take(&mut terminal.pending_reports),
        })
    }
}
