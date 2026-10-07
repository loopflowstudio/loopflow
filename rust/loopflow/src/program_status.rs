//! Program Status revision 0.2 (OSC 7501). Reports are display evidence only.
//! Authored from https://www.superlogical.com/rex/docs/build/program-status.

use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::{alphabet, engine::DecodePaddingMode, Engine};
use serde::{Deserialize, Serialize};

const MAX_SEQUENCE: usize = 4096;
const MAX_RECORDS: usize = 64;
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

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

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Error => "error",
            Self::Clear => "clear",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Auth => "auth",
        }
    }
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
    pub fn parse(body: &[u8]) -> Option<Self> {
        // BEL uses eight framing bytes; encoding below checks the longer ST form.
        if body.len() + 8 > MAX_SEQUENCE {
            return None;
        }
        let mut fields = std::collections::BTreeMap::new();
        for raw_pair in body.split(|b| *b == b':') {
            let Ok(pair) = std::str::from_utf8(raw_pair) else {
                continue;
            };
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            if key.len() > 16 {
                return None;
            }
            if key.is_empty()
                || !key.bytes().all(|b| b.is_ascii_lowercase())
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.,+/=-".contains(&b))
            {
                continue;
            }
            // Validate every pair, including values overwritten by a duplicate.
            match key {
                "id" if value.len() > 128
                    || value.split('/').count() > 8
                    || value.split('/').any(|s| s.len() > 32) =>
                {
                    return None
                }
                "app" if value.len() > 32 => return None,
                "msg" => {
                    decode_text(value, 2732, 2048)?;
                }
                "title" => {
                    decode_text(value, 256, 192)?;
                }
                _ => {}
            }
            fields.insert(key, value);
        }
        let state = match *fields.get("state")? {
            "idle" => State::Idle,
            "working" => State::Working,
            "done" => State::Done,
            "blocked" => State::Blocked,
            "error" => State::Error,
            "clear" => State::Clear,
            _ => return None,
        };
        let id = fields.get("id").copied();
        if id.is_some_and(|id| !id.split('/').all(valid_segment)) {
            return None;
        }
        let kind = if state == State::Blocked {
            match fields.get("kind").copied() {
                Some("permission") => Some(Kind::Permission),
                Some("question") => Some(Kind::Question),
                Some("auth") => Some(Kind::Auth),
                _ => None,
            }
        } else {
            None
        };
        let progress = if matches!(state, State::Working | State::Blocked) {
            fields
                .get("progress")
                .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v <= 100)
        } else {
            None
        };
        Some(Self {
            state,
            id: id.map(str::to_owned),
            kind,
            progress,
            app: fields
                .get("app")
                .filter(|v| valid_segment(v))
                .map(|v| (*v).to_owned()),
            title: match fields.get("title") {
                Some(v) => Some(decode_text(v, 256, 192)?),
                None => None,
            },
            msg: match fields.get("msg") {
                Some(v) => Some(decode_text(v, 2732, 2048)?),
                None => None,
            },
        })
    }

    /// Reject invalid locally constructed reports as well as invalid wire reports.
    pub fn encode(&self) -> Option<Vec<u8>> {
        let mut pairs = vec![format!("state={}", self.state.as_str())];
        if let Some(id) = &self.id {
            pairs.push(format!("id={id}"));
        }
        if let Some(kind) = self.kind {
            pairs.push(format!("kind={}", kind.as_str()));
        }
        if let Some(progress) = self.progress {
            pairs.push(format!("progress={progress}"));
        }
        if let Some(app) = &self.app {
            pairs.push(format!("app={app}"));
        }
        if let Some(title) = &self.title {
            pairs.push(format!("title={}", BASE64.encode(title)));
        }
        if let Some(msg) = &self.msg {
            pairs.push(format!("msg={}", BASE64.encode(msg)));
        }
        let body = pairs.join(":");
        if body.len() + 9 > MAX_SEQUENCE {
            return None;
        }
        if Self::parse(body.as_bytes()).as_ref() != Some(self) {
            return None;
        }
        Some(format!("\x1b]7501;{body}\x1b\\").into_bytes())
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
}

fn decode_text(value: &str, encoded: usize, decoded: usize) -> Option<String> {
    if value.len() > encoded {
        return None;
    }
    let text = String::from_utf8(BASE64.decode(value).ok()?).ok()?;
    (text.len() <= decoded && !text.chars().any(char::is_control)).then_some(text)
}

/// Oldest first; replacement moves a record to the end. No heartbeat or clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Records {
    pub seen: bool,
    pub records: Vec<Report>,
}

impl Records {
    pub fn validate(&self) -> bool {
        self.records.len() <= MAX_RECORDS
            && (self.seen || self.records.is_empty())
            && self
                .records
                .iter()
                .all(|r| r.state != State::Clear && r.encode().is_some())
            && self
                .records
                .iter()
                .map(|r| &r.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.records.len()
    }
}

// This command is the sole stdin reader. Polling avoids a blocking stdin worker
// that could survive a replaced observer and keep the process alive indefinitely.
#[cfg(unix)]
pub(crate) fn read_observation_chunk(bytes: &mut [u8]) -> std::io::Result<Option<usize>> {
    let mut fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: fd points to one initialized pollfd for the process's stdin.
    let ready = unsafe { libc::poll(&mut fd, 1, 250) };
    if ready < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if ready == 0 {
        return Ok(None);
    }
    // SAFETY: bytes is writable for its stated length; this is the only reader.
    let count = unsafe { libc::read(fd.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
    if count < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(Some(count as usize))
    }
}
#[cfg(not(unix))]
pub(crate) fn read_observation_chunk(_: &mut [u8]) -> std::io::Result<Option<usize>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "terminal observation requires Unix",
    ))
}

#[cfg(test)]
mod tests {
    use super::{Report, State};

    #[test]
    fn program_status_validates_every_pair_before_replacement() {
        for body in [
            "state=working:msg=Cg==:msg=SGk=",
            "state=working:title=/w==",
            "state=working:id=a//b",
            "state=future",
            "state=working:msg=woc=",
        ] {
            assert!(Report::parse(body.as_bytes()).is_none(), "{body}");
        }
        assert!(
            Report::parse(format!("state=working:app={}:app=ok", "a".repeat(33)).as_bytes())
                .is_none()
        );
        let good = Report::parse(
            b" state = blocked :state=working:kind=unknown:progress=101:future=x:oops:msg=SGk",
        )
        .unwrap();
        assert_eq!(good.state, State::Working);
        assert_eq!(good.msg.as_deref(), Some("Hi"));
        assert_eq!(good.kind, None);
        assert_eq!(good.progress, None);
        let encoded = good.encode().unwrap();
        assert_eq!(Report::parse(&encoded[7..encoded.len() - 2]), Some(good));
        assert_eq!(
            Report::parse(b"state=done:future=\xff").unwrap().state,
            State::Done
        );
    }

    #[test]
    fn program_status_hard_limits_and_optional_padding() {
        use base64::Engine;
        for (key, limit) in [("msg", 2048), ("title", 192)] {
            let text = "a".repeat(limit);
            let encoded = super::BASE64.encode(&text);
            for encoded in [encoded.as_str(), encoded.trim_end_matches('=')] {
                assert!(
                    Report::parse(format!("state=working:{key}={encoded}").as_bytes()).is_some()
                );
            }
            let encoded = super::BASE64.encode(format!("{text}a"));
            assert!(Report::parse(format!("state=working:{key}={encoded}").as_bytes()).is_none());
        }
        for id in [
            "a".repeat(33),
            ["a"; 9].join("/"),
            vec!["a".repeat(32); 4].join("/") + "aa",
        ] {
            assert!(Report::parse(format!("state=working:id={id}").as_bytes()).is_none());
        }
        assert!(Report::parse(b"state=working:abcdefghijklmnopq=x").is_none());
        assert!(
            Report::parse(format!("state=working:x={}", "a".repeat(4096)).as_bytes()).is_none()
        );
    }
}
