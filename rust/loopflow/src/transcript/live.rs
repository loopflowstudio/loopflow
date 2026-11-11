//! A conversation's current or last turn, read from the end of its provider's
//! transcript at the moment someone asks. Only Claude and Codex keep turn
//! boundaries there. A transcript shows a question the agent asked and not a
//! permission prompt, so a turn waiting on one of those reads as running.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{unix_seconds, Run};
use crate::harness::HarnessKind;

/// How much of a transcript's end is read first; the window grows fourfold
/// until it holds the turn's start…
const FIRST: u64 = 128 * 1024;
/// …or this much. A longer turn reads with no start and the usage of its
/// visible part.
const TAIL: u64 = 8 * 1024 * 1024;

/// Rows that can carry a turn boundary, model or usage; others are not parsed.
const MARKERS: [&[u8]; 5] = [
    b"\"type\":\"user\"",
    b"\"type\":\"assistant\"",
    b"\"type\":\"system\"",
    b"\"type\":\"event_msg\"",
    b"\"type\":\"turn_context\"",
];

/// A conversation's current or last turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionTurn {
    pub state: TurnState,
    pub source: TurnSource,
    /// RFC 3339; absent when the turn began before what was read.
    #[serde(deserialize_with = "Option::deserialize")]
    pub started_at: Option<String>,
    /// RFC 3339; absent while it runs.
    #[serde(deserialize_with = "Option::deserialize")]
    pub ended_at: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    /// Tokens across the turn's model calls; absent while it runs.
    #[serde(deserialize_with = "Option::deserialize")]
    pub usage: Option<TurnUsage>,
}

/// `asking` is a turn stopped on a question the agent put to the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Running,
    Asking,
    Ended,
    Interrupted,
}

/// Where a turn reading came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnSource {
    /// A turn boundary Loopflow recorded.
    Hooks,
    /// The provider's own transcript.
    Transcript,
}

/// `input_tokens` counts every input token, cached ones included.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

impl SessionTurn {
    /// What `lf session list` prints for it.
    pub(crate) fn describe(&self, now: i64) -> String {
        let since = |at: &Option<String>| {
            at.as_deref()
                .and_then(|at| unix_seconds(&Value::String(at.to_owned())))
                .map(|at| duration(now.saturating_sub(at)))
        };
        match self.state {
            TurnState::Running => match since(&self.started_at) {
                Some(running) => format!("turn running {running}"),
                None => "turn running".into(),
            },
            TurnState::Asking => "asked a question".into(),
            TurnState::Ended | TurnState::Interrupted => {
                let what = if self.state == TurnState::Ended {
                    "last turn ended"
                } else {
                    "last turn interrupted"
                };
                let mut line = match since(&self.ended_at) {
                    Some(ago) => format!("{what} {ago} ago"),
                    None => what.into(),
                };
                if let Some(usage) = self.usage {
                    line.push_str(&format!(
                        " · {} in ({} cached) · {} out",
                        tokens(usage.input_tokens),
                        tokens(usage.cached_input_tokens),
                        tokens(usage.output_tokens)
                    ));
                }
                line
            }
        }
    }

    fn from_run(run: Run) -> Self {
        Self {
            state: run.state,
            source: TurnSource::Transcript,
            started_at: run.started_at,
            usage: run.ended_at.is_some().then_some(run.usage),
            ended_at: run.ended_at,
            model: run.model,
        }
    }

    /// Whether the conversation waits on a person: the agent handed back, or
    /// asked a question.
    pub(crate) fn waits(&self) -> bool {
        self.state != TurnState::Running
    }

    /// A turn as Loopflow's hooks recorded its boundaries, in Unix seconds.
    pub(crate) fn from_hooks(
        started_at: Option<i64>,
        settled_at: Option<i64>,
        status: Option<&str>,
    ) -> Self {
        let state = match (settled_at, status) {
            (None, _) => TurnState::Running,
            (Some(_), Some(status)) if status.contains("interrupt") => TurnState::Interrupted,
            (Some(_), _) => TurnState::Ended,
        };
        Self {
            state,
            source: TurnSource::Hooks,
            started_at: started_at.and_then(rfc3339),
            ended_at: settled_at.and_then(rfc3339),
            model: None,
            usage: None,
        }
    }
}

fn duration(seconds: i64) -> String {
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3600 if seconds % 60 == 0 || seconds >= 600 => format!("{}m", seconds / 60),
        60..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
        3600..86_400 => format!("{}h", seconds / 3600),
        _ => format!("{}d", seconds / 86_400),
    }
}

fn tokens(count: u64) -> String {
    match count {
        ..1000 => count.to_string(),
        1000..1_000_000 => format!("{:.1}k", count as f64 / 1e3),
        _ => format!("{:.1}M", count as f64 / 1e6),
    }
}

fn rfc3339(at: i64) -> Option<String> {
    time::OffsetDateTime::from_unix_timestamp(at)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

/// The current or last turn of the transcript at `path`; `None` when it shows
/// none or cannot be read.
pub(crate) fn read_turn(harness: HarnessKind, path: &Path) -> Option<SessionTurn> {
    let read = match harness {
        HarnessKind::Claude => super::claude::read,
        HarnessKind::Codex => super::codex::read,
        HarnessKind::Pi | HarnessKind::OpenCode => return None,
    };
    let mut limit = FIRST;
    loop {
        let (rows, whole) = tail(path, limit).ok()?;
        let turn = read(&rows).runs.pop().map(SessionTurn::from_run);
        if whole || limit >= TAIL || turn.as_ref().is_some_and(|t| t.started_at.is_some()) {
            return turn;
        }
        limit *= 4;
    }
}

/// Complete rows among a file's last `limit` bytes, and whether that was the
/// whole file: a row cut by the window's start, or still being written at its
/// end, is left out.
fn tail(path: &Path, limit: u64) -> std::io::Result<(Vec<Value>, bool)> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(limit);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(size - start).read_to_end(&mut bytes)?;
    let Some(end) = bytes.iter().rposition(|byte| *byte == b'\n') else {
        return Ok((Vec::new(), start == 0));
    };
    let begin = if start == 0 {
        0
    } else {
        bytes[..end]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(end, |at| at + 1)
    };
    let rows = bytes[begin..end]
        .split(|byte| *byte == b'\n')
        .filter(|line| {
            MARKERS
                .iter()
                .any(|marker| line.windows(marker.len()).any(|window| window == *marker))
        })
        .filter_map(|line| serde_json::from_slice(line).ok())
        .collect();
    Ok((rows, start == 0))
}

// -- Finding a conversation's transcript --

/// Where each conversation's transcript was found; a transcript never moves.
static FOUND: LazyLock<Mutex<HashMap<(String, PathBuf), PathBuf>>> = LazyLock::new(Mutex::default);

/// The transcript of `provider`'s conversation `thread`, which a Session ran
/// in `cwd`; `None` when its harness keeps none or its home is gone.
pub(crate) fn locate(
    store: &crate::store::sqlite::SqliteStore,
    provider: &str,
    thread: &str,
    cwd: &Path,
) -> anyhow::Result<Option<(HarnessKind, PathBuf)>> {
    let Some(harness) = HarnessKind::parse(provider) else {
        return Ok(None);
    };
    let Some(provider) = harness.provider() else {
        return Ok(None);
    };
    let id = crate::id::AgentSessionId::from(thread);
    let Some(home) = super::conversation_home(store, provider, &id)? else {
        return Ok(None);
    };
    Ok(find(harness, &home, thread, cwd).map(|path| (harness, path)))
}

/// Conversation `id`'s transcript in `home`, which ran in `cwd`.
pub(crate) fn find(harness: HarnessKind, home: &Path, id: &str, cwd: &Path) -> Option<PathBuf> {
    let key = (id.to_owned(), home.to_path_buf());
    if let Some(path) = FOUND.lock().expect("transcript locations").get(&key) {
        return Some(path.clone());
    }
    let direct = (harness == HarnessKind::Claude)
        .then(|| claude_project(home, cwd).join(format!("{id}.jsonl")))
        .filter(|path| path.is_file());
    let path = direct.or_else(|| super::transcript(harness, home, id))?;
    FOUND
        .lock()
        .expect("transcript locations")
        .insert(key, path.clone());
    Some(path)
}

/// Claude keeps a directory's conversations under its path with every `/`
/// and `.` turned into `-`.
pub(crate) fn claude_project(home: &Path, cwd: &Path) -> PathBuf {
    let name = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    home.join("projects").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/transcript/testdata")
            .join(name)
    }

    /// The fixture's first `rows` lines, then `partial` bytes of the next.
    fn cut(name: &str, rows: usize, partial: usize) -> tempfile::NamedTempFile {
        let text = fs::read_to_string(fixture(name)).unwrap();
        let mut lines = text.split_inclusive('\n');
        let mut kept = lines.by_ref().take(rows).collect::<String>();
        if let Some(next) = lines.next() {
            kept.push_str(&next[..partial.min(next.len())]);
        }
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), kept).unwrap();
        file
    }

    #[test]
    fn a_finished_claude_turn_reads_its_end_model_and_usage_once_per_message() {
        let turn = read_turn(HarnessKind::Claude, &fixture("claude_turn.jsonl")).unwrap();
        assert_eq!(
            turn,
            SessionTurn {
                state: TurnState::Ended,
                source: TurnSource::Transcript,
                started_at: Some("2026-11-10T16:25:37.000Z".into()),
                ended_at: Some("2026-11-10T16:27:51.000Z".into()),
                model: Some("claude-opus-5-5".into()),
                usage: Some(TurnUsage {
                    input_tokens: 2 + 1000 + 18000 + 2 + 2196 + 20000,
                    cached_input_tokens: 38000,
                    output_tokens: 1100,
                }),
            }
        );
        assert_eq!(
            turn.describe(1_794_328_071 + 180),
            "last turn ended 3m ago · 41.2k in (38.0k cached) · 1.1k out"
        );
    }

    #[test]
    fn a_claude_turn_waiting_on_a_tool_runs_even_with_its_last_line_half_written() {
        // Through the tool result, then half of the final answer's row.
        let file = cut("claude_turn.jsonl", 6, 120);
        let turn = read_turn(HarnessKind::Claude, file.path()).unwrap();
        assert_eq!(turn.state, TurnState::Running);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:37.000Z"));
        assert_eq!((turn.ended_at, turn.usage), (None, None));
        assert_eq!(turn.model.as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn an_interrupted_claude_turn_says_so() {
        let file = cut("claude_turn.jsonl", 6, 0);
        let mut text = fs::read_to_string(file.path()).unwrap();
        text.push_str(r#"{"type":"user","userType":"external","timestamp":"2026-11-10T16:26:00.000Z","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}"#);
        text.push('\n');
        fs::write(file.path(), text).unwrap();
        let turn = read_turn(HarnessKind::Claude, file.path()).unwrap();
        assert_eq!(turn.state, TurnState::Interrupted);
        assert_eq!(turn.ended_at.as_deref(), Some("2026-11-10T16:26:00.000Z"));
    }

    #[test]
    fn a_codex_turn_counts_the_growth_of_its_running_total() {
        let turn = read_turn(HarnessKind::Codex, &fixture("codex_rollout.jsonl")).unwrap();
        assert_eq!(turn.state, TurnState::Ended);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:00.000Z"));
        assert_eq!(turn.ended_at.as_deref(), Some("2026-11-10T16:25:09.000Z"));
        assert_eq!(turn.model.as_deref(), Some("gpt-6-astra"));
        assert_eq!(
            turn.usage,
            Some(TurnUsage {
                input_tokens: 25000,
                cached_input_tokens: 18000,
                output_tokens: 500,
            })
        );
    }

    #[test]
    fn a_codex_turn_with_its_last_line_half_written_is_running() {
        let file = cut("codex_rollout.jsonl", 9, 40);
        let turn = read_turn(HarnessKind::Codex, file.path()).unwrap();
        assert_eq!(turn.state, TurnState::Running);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:00.000Z"));
        assert_eq!(turn.usage, None);
    }

    #[test]
    fn a_window_opening_mid_turn_counts_only_what_it_shows() {
        let text = fs::read_to_string(fixture("codex_rollout.jsonl")).unwrap();
        // Start inside the second turn's first token report.
        let from = text.find("\"total_tokens\":45900").unwrap();
        let (rows, _) = tail(&fixture("codex_rollout.jsonl"), (text.len() - from) as u64).unwrap();
        let run = super::super::codex::read(&rows).runs.pop().unwrap();
        assert_eq!(run.started_at, None);
        assert_eq!(run.state, TurnState::Ended);
        assert_eq!(run.usage.input_tokens, 25000);
    }

    #[test]
    fn hooks_turns_read_their_boundaries() {
        let turn = SessionTurn::from_hooks(Some(1_794_327_900), None, None);
        assert_eq!(turn.state, TurnState::Running);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:00Z"));
        assert_eq!(turn.describe(1_794_327_900 + 134), "turn running 2m14s");
        assert_eq!(
            (tokens(999), tokens(41_200), tokens(3_859_200)),
            ("999".into(), "41.2k".into(), "3.9M".into())
        );
        let asking = SessionTurn {
            state: TurnState::Asking,
            ..turn.clone()
        };
        assert_eq!(asking.describe(0), "asked a question");
        assert!(asking.waits() && !turn.waits());
        let ended = SessionTurn::from_hooks(Some(1), Some(2), Some("interrupted"));
        assert_eq!(ended.state, TurnState::Interrupted);
    }
}
