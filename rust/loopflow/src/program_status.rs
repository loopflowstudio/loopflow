//! Program Status revision 0.2 (OSC 7501). Reports are display evidence only.
//! Authored from https://www.superlogical.com/rex/docs/build/program-status.

use base64::Engine;
use serde::{Deserialize, Serialize};

const MAX_RECORDS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub state: State,
    pub id: Option<String>,
    pub kind: Option<Kind>,
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
}

impl Report {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let state = match self.state {
            State::Idle => "idle",
            State::Working => "working",
            State::Done => "done",
            State::Blocked => "blocked",
            State::Error => "error",
            State::Clear => "clear",
        };
        let mut text = "\x1b]7501;".to_owned();
        if let Some(id) = &self.id {
            text.push_str(&format!("id={id}:"));
        }
        text.push_str(&format!("state={state}"));
        if let Some(app) = &self.app {
            text.push_str(&format!(":app={app}"));
        }
        if let Some(kind) = self.kind {
            let kind = match kind {
                Kind::Permission => "permission",
                Kind::Question => "question",
                Kind::Auth => "auth",
            };
            text.push_str(&format!(":kind={kind}"));
        }
        if let Some(progress) = self.progress {
            text.push_str(&format!(":progress={progress}"));
        }
        for (key, value) in [("title", &self.title), ("msg", &self.msg)] {
            if let Some(value) = value {
                let encoded = base64::engine::general_purpose::STANDARD.encode(value);
                text.push_str(&format!(":{key}={encoded}"));
            }
        }
        text.push_str("\x1b\\");
        text.into_bytes()
    }

    // The observer receives decoded records, not terminal bytes. Enforce the
    // spec's field limits directly; together they fit below its sequence cap.
    fn validate(&self) -> bool {
        self.state != State::Clear
            && (self.kind.is_none() || self.state == State::Blocked)
            && self.progress.is_none_or(|progress| {
                progress <= 100 && matches!(self.state, State::Working | State::Blocked)
            })
            && self.id.as_deref().is_none_or(|id| {
                id.len() <= 128 && id.split('/').count() <= 8 && id.split('/').all(valid_segment)
            })
            && self.app.as_deref().is_none_or(valid_segment)
            && self
                .title
                .as_deref()
                .is_none_or(|text| valid_text(text, 192))
            && self
                .msg
                .as_deref()
                .is_none_or(|text| valid_text(text, 2048))
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
}

fn valid_text(text: &str, limit: usize) -> bool {
    text.len() <= limit && !text.chars().any(char::is_control)
}

/// Oldest first; replacement moves a record to the end. No heartbeat or clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Records {
    pub seen: bool,
    pub records: Vec<Report>,
}

impl Records {
    /// Apply a validated terminal report, keeping the newest update last.
    pub fn apply(&mut self, report: Report) {
        self.seen = true;
        if report.state == State::Clear {
            let id = report.id.as_deref().unwrap_or("");
            let children = format!("{id}/");
            self.records.retain(|record| {
                let candidate = record.id.as_deref().unwrap_or("");
                !id.is_empty() && candidate != id && !candidate.starts_with(&children)
            });
        } else {
            self.records.retain(|record| record.id != report.id);
            self.records.push(report);
            if self.records.len() > MAX_RECORDS {
                self.records.remove(0);
            }
        }
    }

    /// A shell prompt or program exit retires live claims, preserving results.
    pub fn clear_live(&mut self) {
        self.records
            .retain(|record| matches!(record.state, State::Done | State::Error));
    }

    /// Replace a viewer's status, including clearing claims from its last attachment.
    pub(crate) fn replay(&self) -> Vec<u8> {
        if !self.seen {
            return Vec::new();
        }
        let mut bytes = b"\x1b]7501;state=clear\x1b\\".to_vec();
        for report in &self.records {
            bytes.extend(report.encode());
        }
        bytes
    }

    pub fn validate(&self) -> bool {
        self.records.len() <= MAX_RECORDS
            && (self.seen || self.records.is_empty())
            && self.records.iter().all(Report::validate)
            && self
                .records
                .iter()
                .map(|r| &r.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.records.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Records, Report, State};

    fn report() -> Report {
        Report {
            state: State::Blocked,
            id: Some("lf/job".into()),
            kind: Some(Kind::Question),
            progress: Some(100),
            app: Some("lf".into()),
            title: Some("é".repeat(96)),
            msg: Some("é".repeat(1024)),
        }
    }

    #[test]
    fn program_status_validates_decoded_text_without_changing_it() {
        let mut value = report();
        assert!(value.validate());
        for text in [
            "a".repeat(2049),
            "bad\ncontrol".into(),
            "bad\u{7f}".into(),
            "bad\u{85}".into(),
        ] {
            value.msg = Some(text);
            assert!(!value.validate());
        }
        value.msg = Some("**Proceed?** \u{202e}".into());
        assert!(value.validate()); // Invisible formatting is removed only at display.
        value.title = Some("é".repeat(97));
        assert!(!value.validate());
    }

    #[test]
    fn program_status_validates_record_identity_and_state_fields() {
        let mut value = report();
        for id in [
            "".into(),
            "a//b".into(),
            "a:b".into(),
            "a".repeat(33),
            ["a"; 9].join("/"),
            vec!["a".repeat(32); 4].join("/") + "aa",
        ] {
            value.id = Some(id);
            assert!(!value.validate());
        }
        value.id = None;
        for app in ["", "a/b", "bad app", &"a".repeat(33)] {
            value.app = Some(app.into());
            assert!(!value.validate());
        }
        value.app = None;
        assert!(value.validate());
        value.progress = Some(101);
        assert!(!value.validate());
        value.progress = Some(100);
        value.state = State::Working;
        assert!(!value.validate()); // Only blocked records carry a kind.
        value.kind = None;
        assert!(value.validate());
        value.state = State::Done;
        assert!(!value.validate()); // Done records cannot carry progress.
        value.progress = None;
        assert!(value.validate());
    }

    #[test]
    fn program_status_snapshots_are_bounded_unique_and_contain_only_records() {
        let mut snapshot = Records {
            seen: true,
            records: vec![report()],
        };
        assert!(snapshot.validate());
        snapshot.seen = false;
        assert!(!snapshot.validate());
        snapshot.seen = true;
        snapshot.records.push(report());
        assert!(!snapshot.validate());
        snapshot.records = (0..64)
            .map(|i| Report {
                id: Some(format!("r{i}")),
                ..report()
            })
            .collect();
        assert!(snapshot.validate());
        snapshot.records.push(Report {
            id: None,
            ..report()
        });
        assert!(!snapshot.validate());
        snapshot.records = vec![Report {
            state: State::Clear,
            kind: None,
            progress: None,
            ..report()
        }];
        assert!(!snapshot.validate());
        snapshot.records.clear();
        assert!(snapshot.validate());
    }
}
