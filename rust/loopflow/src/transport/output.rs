//! Retain terminal state or a bounded pipe tail for replacement viewers.

use std::collections::VecDeque;

use anyhow::{Context, Result};

use crate::ghostty_vt;
use crate::program_status::{Report, State};

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
    pending_reports: Vec<u8>,
    directory: Option<Vec<u8>>,
}

impl Output {
    pub fn new(terminal: bool) -> Result<Self> {
        if terminal {
            Ok(Self::Terminal(Box::new(Terminal {
                terminal: ghostty_vt::Terminal::new(80, 24).context("Open the owner's terminal")?,
                pending_reports: Vec::new(),
                directory: None,
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
        if let Some(directory) = &terminal.directory {
            bytes.extend(directory);
        }
        bytes.extend(terminal.terminal.program_status().replay());
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
        let bytes = Report {
            state,
            id: Some(id),
            progress: progress.filter(|p| *p <= 100 && state == State::Working),
            kind: None,
            app: None,
            title: None,
            msg: None,
        }
        .encode();
        if terminal.pending_reports.len() + bytes.len() <= LIMIT {
            terminal.pending_reports.extend(bytes);
        }
    }

    /// Publish the connected provider's cwd through the same serialized stream
    /// as status reports, including when a viewer attaches after the launch.
    pub fn directory(&mut self, cwd: &std::path::Path) {
        let Self::Terminal(terminal) = self else {
            return;
        };
        let host = gethostname::gethostname();
        let Some(bytes) = directory_report(cwd, &host.to_string_lossy()) else {
            return;
        };
        if terminal.directory.as_ref() != Some(&bytes) {
            terminal.pending_reports.extend(&bytes);
            terminal.directory = Some(bytes);
        }
    }

    pub fn take_reports(&mut self) -> Option<Vec<u8>> {
        let Self::Terminal(terminal) = self else {
            return None;
        };
        if terminal.pending_reports.is_empty() || !terminal.terminal.at_ground() {
            return None;
        }
        let reports = std::mem::take(&mut terminal.pending_reports);
        // Reduce lf reports at the same stream boundary where viewers receive
        // them, through the same parser as native provider reports.
        terminal.terminal.write(&reports);
        Some(reports)
    }

    /// The retained terminal, for the snapshot an ending owner leaves behind.
    pub fn terminal(&self) -> Option<&ghostty_vt::Terminal> {
        match self {
            Self::Terminal(terminal) => Some(&terminal.terminal),
            Self::Pipes { .. } => None,
        }
    }
}

fn directory_report(cwd: &std::path::Path, host: &str) -> Option<Vec<u8>> {
    let mut uri = reqwest::Url::from_directory_path(cwd).ok()?;
    // The execution host owns this path. A remote viewer must never mistake it
    // for a directory on the viewer's Machine.
    uri.set_host(Some(host)).ok()?;
    Some(format!("\x1b]7;{uri}\x1b\\").into_bytes())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{directory_report, Output};
    use crate::ghostty_vt::Terminal;
    use crate::program_status::{Kind, Records, Report, State};

    #[test]
    fn directory_report_identifies_execution_host_and_escapes_paths() {
        assert_eq!(
            directory_report(Path::new("/work/research notes/雪"), "remote.example").unwrap(),
            b"\x1b]7;file://remote.example/work/research%20notes/%E9%9B%AA/\x1b\\"
        );
        assert!(directory_report(Path::new("relative"), "remote.example").is_none());
    }

    #[test]
    fn directory_reports_survive_attachment_without_splitting_provider_sequences() {
        let mut output = Output::new(true).unwrap();
        output.process(1, b"\x1b]0;unfinished", false);
        output.directory(Path::new("/work/research notes/雪"));
        assert!(output.take_reports().is_none());
        let snapshot = output.snapshot().unwrap().unwrap();
        let expected = directory_report(
            Path::new("/work/research notes/雪"),
            &gethostname::gethostname().to_string_lossy(),
        )
        .unwrap();
        assert!(snapshot
            .windows(expected.len())
            .any(|part| part == expected));
        assert!(snapshot.ends_with(b"\x1b]0;unfinished"));
        output.process(1, b" title\x07", false);
        assert_eq!(output.take_reports().unwrap(), expected);
        output.directory(Path::new("/work/delivery"));
        let moved = directory_report(
            Path::new("/work/delivery"),
            &gethostname::gethostname().to_string_lossy(),
        )
        .unwrap();
        assert_eq!(output.take_reports().unwrap(), moved);
        let snapshot = output.snapshot().unwrap().unwrap();
        assert!(snapshot.windows(moved.len()).any(|part| part == moved));
        assert!(!snapshot
            .windows(expected.len())
            .any(|part| part == expected));
        output.directory(Path::new("/work/delivery"));
        assert!(output.take_reports().is_none());
        let mut pipes = Output::new(false).unwrap();
        pipes.directory(Path::new("/work/delivery"));
        assert!(pipes.take_reports().is_none());
    }

    fn restored(output: &Output) -> Records {
        let mut viewer = Terminal::new(80, 24).unwrap();
        viewer.write(b"\x1b]7501;id=old:state=working\x1b\\");
        viewer.write(&output.snapshot().unwrap().unwrap());
        viewer.program_status().clone()
    }

    #[test]
    fn program_status_detection_answers_only_while_detached() {
        let mut output = Output::new(true).unwrap();
        assert_eq!(
            output.process(1, b"\x1b]7501;?\x07", true),
            b"\x1b]7501;?\x07"
        );
        assert!(output.process(1, b"\x1b]7501;?\x07", false).is_empty());
        assert_eq!(
            output.process(1, b"\x1b]7501;?\x1b\\", true),
            b"\x1b]7501;?\x1b\\"
        );
    }

    #[test]
    fn program_status_survives_detached_updates_and_clear() {
        let mut output = Output::new(true).unwrap();
        for detached in [false, true] {
            for (state, expected) in [
                ("working", Some(State::Working)),
                ("idle", Some(State::Idle)),
                ("blocked", Some(State::Blocked)),
                ("done", Some(State::Done)),
                ("clear", None),
            ] {
                let report = format!("\x1b]7501;state={state}\x1b\\");
                for byte in report.bytes() {
                    output.process(1, &[byte], detached);
                }
                let status = restored(&output);
                assert_eq!(status, *output.terminal().unwrap().program_status());
                assert_eq!(status.records.len(), usize::from(expected.is_some()));
                assert_eq!(status.records.first().map(|report| report.state), expected);
            }
        }
    }

    #[test]
    fn program_status_replay_keeps_metadata_and_replaces_missing_fields() {
        let mut output = Output::new(true).unwrap();
        let report = Report {
            state: State::Blocked,
            id: Some("worker/job".into()),
            kind: Some(Kind::Permission),
            progress: Some(73),
            app: Some("claude-code".into()),
            title: Some("Révision".into()),
            msg: Some("Apply **literal** changes?".into()),
        };
        // Independent provider bytes exercise both callback decoding and replay encoding.
        output.process(
            1,
            b"\x1b]7501;id=worker/job:state=blocked:kind=permission:progress=73:app=claude-code:title=UsOpdmlzaW9u:msg=QXBwbHkgKipsaXRlcmFsKiogY2hhbmdlcz8=\x1b\\",
            true,
        );
        assert_eq!(restored(&output).records, vec![report]);
        output.process(1, b"\x1b]7501;id=worker/job:state=working\x07", true);
        let current = restored(&output);
        assert_eq!(current.records[0].state, State::Working);
        assert_eq!(current.records[0].msg, None);
        assert_eq!(current.records[0].kind, None);
        assert_eq!(current.records[0].progress, None);
        output.process(1, b"\x1b]7501;state=idle:msg=AA==\x07", true);
        assert_eq!(
            restored(&output),
            current,
            "invalid reports cannot replace evidence"
        );
    }

    #[test]
    fn program_status_clear_prompt_and_reset_apply_to_provider_and_lf_records() {
        let mut output = Output::new(true).unwrap();
        for (id, state) in [
            ("lf/job", "working"),
            ("lf/job/step", "blocked"),
            ("lf/jobs", "done"),
            ("other", "error"),
        ] {
            output.process(
                1,
                format!("\x1b]7501;id={id}:state={state}\x07").as_bytes(),
                true,
            );
        }
        output.report("lf/job".into(), State::Clear, None);
        output.take_reports().unwrap();
        let records = restored(&output).records;
        assert_eq!(
            records
                .iter()
                .map(|r| r.id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["lf/jobs", "other"]
        );

        output.report("lf/active".into(), State::Working, Some(10));
        output.take_reports().unwrap();
        output.process(1, b"\x1b]7501;state=blocked\x07\x1b]133;A\x07", true);
        assert_eq!(
            restored(&output).records,
            records,
            "prompt retires live claims only"
        );
        output.process(1, b"\x1bc", true);
        assert!(restored(&output).records.is_empty());
    }

    #[test]
    fn program_status_replay_preserves_update_order_and_evicts_oldest() {
        let mut output = Output::new(true).unwrap();
        for id in (0..64).chain([0, 64]) {
            output.process(
                1,
                format!("\x1b]7501;id=r{id}:state=working\x07").as_bytes(),
                true,
            );
        }
        let records = restored(&output).records;
        assert_eq!(records.len(), 64);
        assert_eq!(records[0].id.as_deref(), Some("r2"));
        assert_eq!(records[62].id.as_deref(), Some("r0"));
        assert_eq!(records[63].id.as_deref(), Some("r64"));
    }

    #[test]
    fn program_status_lf_reports_wait_for_the_provider_sequence_to_finish() {
        let mut output = Output::new(true).unwrap();
        output.process(1, b"\x1b]7501;state=blocked", true);
        output.report("lf/job".into(), State::Working, None);
        assert!(output.take_reports().is_none());
        let mut viewer = Terminal::new(80, 24).unwrap();
        viewer.write(&output.snapshot().unwrap().unwrap());
        output.process(1, b"\x07", true);
        viewer.write(b"\x07");
        viewer.write(&output.take_reports().unwrap());
        assert_eq!(
            viewer.program_status(),
            output.terminal().unwrap().program_status()
        );
        assert_eq!(restored(&output).records.len(), 2);
    }
}
