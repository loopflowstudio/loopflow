//! A conversation's current or last turn, read from the end of its provider's
//! transcript at the moment someone asks. Only Claude and Codex keep turn
//! ends there. A transcript shows neither a question the agent asked nor a
//! permission prompt until it is answered, so a turn waiting on one reads as
//! running.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{rfc3339, seconds, Turn};
use crate::harness::HarnessKind;
use crate::store::sqlite::{CurrentConversation, RecordedTurn, SqliteStore};

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

/// A conversation's current or last turn: a transcript `Turn` without what
/// was said in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionTurn {
    /// The provider's name for the turn.
    #[serde(deserialize_with = "Option::deserialize")]
    pub id: Option<String>,
    pub state: TurnState,
    /// RFC 3339; absent when the turn began before what was read.
    #[serde(deserialize_with = "Option::deserialize")]
    pub started_at: Option<String>,
    /// RFC 3339; absent while it runs.
    #[serde(deserialize_with = "Option::deserialize")]
    pub ended_at: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    /// Tokens across the turn's model calls so far; absent when nothing
    /// reports them.
    #[serde(deserialize_with = "Option::deserialize")]
    pub usage: Option<TurnUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Running,
    Ended,
    Interrupted,
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
    /// What `lf session list` and `lf history show` print for it.
    pub(crate) fn describe(&self, now: i64) -> String {
        let since = |at: &Option<String>| seconds(at).map(|at| duration(now.saturating_sub(at)));
        let mut line = match self.state {
            TurnState::Running => match since(&self.started_at) {
                Some(running) => return format!("turn running {running}"),
                None => return "turn running".into(),
            },
            TurnState::Ended => "last turn ended".to_owned(),
            TurnState::Interrupted => "last turn interrupted".to_owned(),
        };
        if let Some(ago) = since(&self.ended_at) {
            line.push_str(&format!(" {ago} ago"));
        }
        if let Some(usage) = self.usage {
            line.push_str(&format!(" · {}", usage.describe()));
        }
        line
    }

    /// Whether the agent handed back.
    pub(crate) fn handed_back(&self) -> bool {
        self.state != TurnState::Running
    }

    /// `None` for a turn of a transcript that keeps no turn ends.
    fn of(turn: Turn) -> Option<Self> {
        Some(Self {
            id: turn.id,
            state: turn.state?,
            started_at: turn.started_at,
            ended_at: turn.ended_at,
            model: turn.model,
            usage: turn.usage,
        })
    }

    /// A turn as Loopflow recorded its boundaries from the provider's live
    /// stream: all there is for a harness whose transcript keeps no turn ends.
    pub(crate) fn from_stream(stream: RecordedTurn) -> Self {
        let mut turn = Self {
            id: stream.id,
            state: TurnState::Running,
            started_at: stream.started_at.and_then(rfc3339),
            ended_at: None,
            model: None,
            usage: None,
        };
        if let Some(at) = stream.settled_at {
            turn.close(at, stream.status.as_deref());
        }
        turn
    }

    /// End a turn the transcript left open at the end Loopflow recorded for
    /// it, in Unix seconds.
    pub(crate) fn close(&mut self, at: i64, status: Option<&str>) {
        self.state = match status {
            Some(status) if status.contains("interrupt") => TurnState::Interrupted,
            _ => TurnState::Ended,
        };
        self.ended_at = rfc3339(at);
    }
}

impl TurnUsage {
    pub(crate) fn describe(&self) -> String {
        format!(
            "{} in ({} cached) · {} out",
            tokens(self.input_tokens),
            tokens(self.cached_input_tokens),
            tokens(self.output_tokens)
        )
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

/// The current or last turn of the transcript at `path`; `None` when it
/// shows none, keeps no turn ends, or cannot be read.
pub(crate) fn read_turn(harness: HarnessKind, path: &Path) -> Option<SessionTurn> {
    last_turns(harness, path).pop().and_then(SessionTurn::of)
}

/// The turns at the end of the transcript at `path`, through its last one:
/// as few as the window that holds that turn's start shows.
pub(super) fn last_turns(harness: HarnessKind, path: &Path) -> Vec<Turn> {
    let Some(read) = super::ended_reader(harness) else {
        return Vec::new();
    };
    let mut limit = FIRST;
    loop {
        let Ok((rows, whole)) = tail(path, limit) else {
            return Vec::new();
        };
        let turns = read(&rows);
        if whole || limit >= TAIL || turns.last().is_some_and(|t| t.started_at.is_some()) {
            return turns;
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

/// The transcript of `conversation`, which a Session ran in `cwd`; `None`
/// when its harness keeps none or its home is gone.
pub(crate) fn locate(
    store: &SqliteStore,
    conversation: &CurrentConversation,
    cwd: &Path,
) -> anyhow::Result<Option<(HarnessKind, PathBuf)>> {
    let Some(harness) = HarnessKind::parse(&conversation.provider) else {
        return Ok(None);
    };
    let Some(provider) = harness.provider() else {
        return Ok(None);
    };
    let thread = &conversation.thread;
    let Some(home) = super::conversation_home(store, provider, thread)? else {
        return Ok(None);
    };
    Ok(find(harness, &home, thread.as_str(), cwd).map(|path| (harness, path)))
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

/// Claude keeps a directory's conversations under its absolute path, every
/// character other than a letter or digit replaced by `-`.
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
                id: Some("u1".into()),
                state: TurnState::Ended,
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
        assert_eq!(turn.ended_at, None);
        assert_eq!(turn.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(turn.describe(1_794_327_937 + 8), "turn running 8s");
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
        assert_eq!(turn.id.as_deref(), Some("turn-2"));
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
        assert_eq!(turn.usage.unwrap().input_tokens, 25000);
    }

    #[test]
    fn a_window_opening_mid_turn_counts_only_what_it_shows() {
        let text = fs::read_to_string(fixture("codex_rollout.jsonl")).unwrap();
        // Start inside the second turn's first token report.
        let from = text.find("\"total_tokens\":45900").unwrap();
        let (rows, _) = tail(&fixture("codex_rollout.jsonl"), (text.len() - from) as u64).unwrap();
        let turn = super::super::codex::read(&rows).pop().unwrap();
        assert_eq!(turn.started_at, None);
        assert_eq!(turn.state, Some(TurnState::Ended));
        assert_eq!(turn.usage.unwrap().input_tokens, 25000);
    }

    #[test]
    fn a_transcript_with_no_turn_ends_gives_no_current_turn() {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), "{\"type\":\"message\",\"id\":\"a\"}\n").unwrap();
        assert_eq!(read_turn(HarnessKind::Pi, file.path()), None);
    }

    #[test]
    fn a_recorded_turn_reads_its_boundaries_and_an_open_one_closes_at_its_recorded_end() {
        let recorded = |settled_at, status: Option<&str>| RecordedTurn {
            id: Some("t".into()),
            started_at: Some(1_794_327_900),
            settled_at,
            status: status.map(str::to_owned),
        };
        let mut turn = SessionTurn::from_stream(recorded(None, None));
        assert_eq!(turn.state, TurnState::Running);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:00Z"));
        assert_eq!(turn.describe(1_794_327_900 + 134), "turn running 2m14s");
        assert!(!turn.handed_back());
        turn.close(1_794_327_960, Some("completed"));
        assert_eq!(turn.state, TurnState::Ended);
        assert_eq!(turn.describe(1_794_327_960 + 5), "last turn ended 5s ago");
        assert!(turn.handed_back());

        let interrupted = SessionTurn::from_stream(recorded(Some(2), Some("interrupted")));
        assert_eq!(interrupted.state, TurnState::Interrupted);
        assert_eq!(
            (tokens(999), tokens(41_200), tokens(3_859_200)),
            ("999".into(), "41.2k".into(), "3.9M".into())
        );
    }
}
