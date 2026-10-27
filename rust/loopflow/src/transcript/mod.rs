//! Conversations as their providers keep them. Each harness's native
//! transcript is read by one reader into the same `Turn` form; Loopflow stores
//! none of the text. Where a transcript is, how long its provider keeps it,
//! and what to say when it is gone are derived here when read.

mod claude;
mod codex;
pub(crate) mod opencode;
mod pi;
mod session;
pub mod summary;

pub(crate) use session::threads;
pub use session::{
    session_transcript, Conversation, Retention, RetentionState, SessionTranscript, ThreadFacts,
    TurnFacts,
};

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chat::types::{ConversationItem, Lifecycle};
use crate::harness::HarnessKind;
use crate::id::AgentSessionId;
use crate::provider_account::activation::native_home;
use crate::provider_auth::Provider;
use crate::store::sqlite::SqliteStore;
use crate::store::ProviderAccountId;

pub(crate) use claude::is_user_input;

/// One request and everything the agent did in answer to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    /// Unix seconds when the request was sent, when the transcript says.
    pub at: Option<i64>,
    /// `None` for what the agent did before any recorded request.
    pub request: Option<Request>,
    pub items: Vec<ConversationItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub text: String,
    pub from: RequestSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestSource {
    /// Something a person typed.
    Person,
    /// A launch turn, hook, notification or instructions Loopflow or the
    /// harness wrote into the conversation.
    Injected,
}

/// Where a recorded conversation's text can be read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Source {
    File(PathBuf),
    /// OpenCode keeps history in its own store, exported by its CLI from the
    /// conversation's directory.
    OpenCode {
        cwd: PathBuf,
        id: String,
    },
}

/// Why a recorded conversation has no readable transcript here.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Gap {
    /// The isolated account whose home held it no longer exists.
    AccountHomeRemoved,
    /// The home is here and the file is not.
    FileMissing { home: PathBuf },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Located {
    /// `home` is the provider home whose settings decide how long it stays.
    Found {
        source: Source,
        home: Option<PathBuf>,
    },
    Gone(Gap),
}

/// The home that holds a recorded conversation: its isolated account's home,
/// else the provider's native one. `None` when the isolated account is gone.
pub(crate) fn conversation_home(
    store: &SqliteStore,
    provider: Provider,
    id: &AgentSessionId,
    account: Option<&ProviderAccountId>,
) -> Result<Option<PathBuf>> {
    if store.provider_session_isolated(provider, id)? != Some(true) {
        return Ok(Some(native_home(provider, None)));
    }
    let account = match account {
        Some(account) => Some(account.clone()),
        None => store.provider_session_account(provider, id)?,
    };
    let Some(account) = account else {
        return Ok(None);
    };
    Ok(store
        .get_provider_account(provider.as_str(), &account)?
        .and_then(|account| account.home))
}

/// Find the transcript of conversation `id`, which ran in `cwd`.
pub(crate) fn locate(
    store: &SqliteStore,
    harness: HarnessKind,
    id: &AgentSessionId,
    account: Option<&ProviderAccountId>,
    cwd: &Path,
) -> Result<Located> {
    let provider = match harness {
        HarnessKind::Claude => Provider::Claude,
        HarnessKind::Codex => Provider::Codex,
        HarnessKind::Pi => {
            // Pi's conversation id is its absolute session file.
            let path = PathBuf::from(id.as_str());
            return Ok(if path.is_file() {
                Located::Found {
                    source: Source::File(path),
                    home: None,
                }
            } else {
                Located::Gone(Gap::FileMissing {
                    home: path.parent().map(Path::to_path_buf).unwrap_or_default(),
                })
            });
        }
        HarnessKind::OpenCode => {
            return Ok(Located::Found {
                source: Source::OpenCode {
                    cwd: cwd.to_path_buf(),
                    id: id.as_str().to_owned(),
                },
                home: None,
            })
        }
    };
    let Some(home) = conversation_home(store, provider, id, account)? else {
        return Ok(Located::Gone(Gap::AccountHomeRemoved));
    };
    Ok(match transcript(harness, &home, id.as_str()) {
        Some(path) => Located::Found {
            source: Source::File(path),
            home: Some(home),
        },
        None => Located::Gone(Gap::FileMissing { home }),
    })
}

/// Every turn of a transcript, oldest first.
pub(crate) fn read(harness: HarnessKind, source: &Source) -> Result<Vec<Turn>> {
    let path = match source {
        Source::OpenCode { cwd, id } => return Ok(opencode::turns(&opencode::export(cwd, id)?)),
        Source::File(path) => path,
    };
    let file =
        fs::File::open(path).with_context(|| format!("open transcript {}", path.display()))?;
    let rows = rows(file).collect::<Vec<_>>();
    Ok(match harness {
        HarnessKind::Claude => claude::turns(&rows),
        HarnessKind::Codex => codex::turns(&rows),
        HarnessKind::Pi => pi::turns(&rows),
        HarnessKind::OpenCode => anyhow::bail!("OpenCode keeps no transcript file"),
    })
}

/// Rows of a JSONL file, skipping what does not parse.
fn rows(file: fs::File) -> impl Iterator<Item = Value> {
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
}

/// Rows of a JSONL file; none when unreadable.
pub(crate) fn json_lines(path: &Path) -> impl Iterator<Item = Value> {
    fs::File::open(path).ok().into_iter().flat_map(rows)
}

// -- Transcript file names --

/// The conversation a transcript file name holds. Codex keeps
/// `sessions/<year>/<month>/<day>/rollout-<time>-<id>.jsonl`; Claude keeps
/// `projects/<directory>/<id>.jsonl`.
fn conversation_id(harness: HarnessKind, name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".jsonl")?;
    match harness {
        HarnessKind::Codex => {
            let rest = stem.strip_prefix("rollout-")?;
            let at = rest.len().checked_sub(36)?;
            rest.is_char_boundary(at)
                .then(|| &rest[at..])
                .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        }
        HarnessKind::Claude => Some(stem),
        HarnessKind::Pi | HarnessKind::OpenCode => None,
    }
}

/// The directory under a home holding transcripts, and how deep they are.
fn transcript_root(harness: HarnessKind, home: &Path) -> (PathBuf, usize) {
    match harness {
        HarnessKind::Codex => (home.join("sessions"), 3),
        _ => (home.join("projects"), 1),
    }
}

/// The transcript of conversation `id` in `home`.
pub(crate) fn transcript(harness: HarnessKind, home: &Path, id: &str) -> Option<PathBuf> {
    let (root, depth) = transcript_root(harness, home);
    walk(&root, depth, &mut |entry| {
        let name = entry.file_name();
        (conversation_id(harness, name.to_str()?) == Some(id)).then(|| entry.path())
    })
}

/// Copy conversation `id`'s transcript from home `from` into home `to` at the
/// same place, with Claude's sibling `<id>/` directory, so the provider
/// resumes it there. A copy at `to` at least as new as `from`'s is kept.
/// `None` when `from` holds no transcript of `id`.
pub(crate) fn copy_between(
    harness: HarnessKind,
    id: &str,
    from: &Path,
    to: &Path,
) -> Result<Option<PathBuf>> {
    let Some(source) = transcript(harness, from, id) else {
        return Ok(None);
    };
    let relative = source
        .strip_prefix(from)
        .expect("a transcript is found under its home");
    let target = to.join(relative);
    if modified(&target) >= modified(&source) {
        return Ok(Some(target));
    }
    copy_entry(&source, &target)?;
    let sibling = source.with_extension("");
    if harness == HarnessKind::Claude && sibling.is_dir() {
        copy_entry(&sibling, &target.with_extension(""))?;
    }
    Ok(Some(target))
}

fn copy_entry(source: &Path, target: &Path) -> Result<()> {
    if source.is_dir() {
        fs::create_dir_all(target).with_context(|| format!("create {}", target.display()))?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &target.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::copy(source, target)
        .with_context(|| format!("copy {} to {}", source.display(), target.display()))?;
    Ok(())
}

/// A transcript file and when it was last written, in Unix seconds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Indexed {
    pub path: PathBuf,
    pub modified: i64,
}

/// Every transcript in one home by conversation id, from one directory walk.
pub(crate) fn scan(harness: HarnessKind, home: &Path) -> BTreeMap<String, Indexed> {
    let (root, depth) = transcript_root(harness, home);
    let mut found = BTreeMap::new();
    walk::<()>(&root, depth, &mut |entry| {
        let name = entry.file_name();
        let id = conversation_id(harness, name.to_str()?)?;
        let path = entry.path();
        let modified = modified(&path)?;
        found.insert(id.to_owned(), Indexed { path, modified });
        None
    });
    found
}

/// Visit the files `depth` directories below `directory` until `visit` answers.
fn walk<T>(
    directory: &Path,
    depth: usize,
    visit: &mut dyn FnMut(&fs::DirEntry) -> Option<T>,
) -> Option<T> {
    fs::read_dir(directory).ok()?.flatten().find_map(|entry| {
        if depth > 0 {
            walk(&entry.path(), depth - 1, visit)
        } else {
            visit(&entry)
        }
    })
}

/// When a file was last written, in Unix seconds.
pub(crate) fn modified(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(since.as_secs()).ok()
}

/// The directory the conversation ran in, from the head of its transcript.
pub(crate) fn recorded_cwd(transcript: &Path) -> Option<PathBuf> {
    json_lines(transcript).take(32).find_map(|entry| {
        entry["cwd"]
            .as_str()
            .or_else(|| entry["payload"]["cwd"].as_str())
            .map(PathBuf::from)
    })
}

// -- How long providers keep transcripts --

/// Claude's default `cleanupPeriodDays`.
const CLAUDE_CLEANUP_DAYS: u32 = 30;

/// How long a provider keeps a transcript in one home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpiryPolicy {
    /// Claude removes conversations idle longer than `cleanupPeriodDays`,
    /// at its next startup. `settings` names the file that set it, if any.
    ClaudeCleanup { days: u32, settings: Option<String> },
    /// The provider keeps transcripts until someone deletes them.
    Kept,
}

impl ExpiryPolicy {
    /// The home-level policy; project and managed settings are not modeled.
    pub(crate) fn read(harness: HarnessKind, home: Option<&Path>) -> Self {
        match harness {
            HarnessKind::Claude => {
                let settings = home.map(|home| home.join("settings.json"));
                let days = settings
                    .as_deref()
                    .and_then(|path| fs::read(path).ok())
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .and_then(|value| value["cleanupPeriodDays"].as_u64())
                    .and_then(|days| u32::try_from(days).ok());
                match days {
                    Some(days) => Self::ClaudeCleanup {
                        days,
                        settings: settings.map(|path| path.display().to_string()),
                    },
                    None => Self::ClaudeCleanup {
                        days: CLAUDE_CLEANUP_DAYS,
                        settings: None,
                    },
                }
            }
            HarnessKind::Codex | HarnessKind::Pi | HarnessKind::OpenCode => Self::Kept,
        }
    }

    /// When a transcript last written at `modified` becomes eligible for removal.
    pub(crate) fn after(&self, modified: i64) -> Option<i64> {
        match self {
            Self::ClaudeCleanup { days, .. } => Some(modified + i64::from(*days) * 86_400),
            Self::Kept => None,
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Self::ClaudeCleanup {
                days,
                settings: Some(settings),
            } => format!(
                "Claude removes conversations idle {days} days; raise cleanupPeriodDays in {settings} to keep more"
            ),
            Self::ClaudeCleanup {
                days,
                settings: None,
            } => format!(
                "Claude removes conversations idle {days} days (default); set cleanupPeriodDays in its settings.json to keep more"
            ),
            Self::Kept => "kept until deleted".into(),
        }
    }
}

// -- Shared reader helpers --

/// A Loopflow launch turn: the skill it invoked and the person's words.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Launch {
    /// The skill's name and instructions.
    pub skill: Option<(String, String)>,
    pub message: Option<String>,
}

impl Launch {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let skill = text.find("<lf:skill:").and_then(|open| {
            let name_start = open + "<lf:skill:".len();
            let name_end = text[name_start..].find('>')? + name_start;
            let name = &text[name_start..name_end];
            let close = format!("</lf:skill:{name}>");
            let body_end = text[name_end..].find(&close)? + name_end;
            Some((
                name.to_owned(),
                text[name_end + 1..body_end].trim().to_owned(),
            ))
        });
        let message = text.find("<lf:message>").and_then(|open| {
            let start = open + "<lf:message>".len();
            let end = text[start..].find("</lf:message>")? + start;
            Some(text[start..end].trim().to_owned())
        });
        (skill.is_some() || message.is_some()).then_some(Self { skill, message })
    }

    /// The skill's full instructions and the person's words.
    pub(crate) fn text(&self) -> String {
        let skill = self
            .skill
            .as_ref()
            .map(|(name, body)| format!("Skill {name}:\n\n{body}"));
        [skill, self.message.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Text of a string, or of the text blocks in a content array.
fn text_of(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_owned();
    }
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A Unix time as its UTC date.
pub(crate) fn date(at: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(at)
        .map(|at| at.date().to_string())
        .unwrap_or_else(|_| at.to_string())
}

fn unix_seconds(timestamp: &Value) -> Option<i64> {
    let at = time::OffsetDateTime::parse(
        timestamp.as_str()?,
        &time::format_description::well_known::Rfc3339,
    )
    .ok()?;
    Some(at.unix_timestamp())
}

/// Builds turns in transcript order: each request opens a turn.
#[derive(Debug, Default)]
struct Turns(Vec<Turn>);

impl Turns {
    fn request(&mut self, at: Option<i64>, text: String, from: RequestSource) {
        self.0.push(Turn {
            at,
            request: Some(Request { text, from }),
            items: Vec::new(),
        });
    }

    fn item(&mut self, item: ConversationItem) {
        if self.0.is_empty() {
            self.0.push(Turn {
                at: None,
                request: None,
                items: Vec::new(),
            });
        }
        self.0
            .last_mut()
            .expect("a turn was just ensured")
            .items
            .push(item);
    }

    fn message(&mut self, id: &str, text: &str) {
        if !text.trim().is_empty() {
            self.item(ConversationItem::Message {
                id: id.to_owned(),
                text: text.to_owned(),
                phase: None,
            });
        }
    }

    fn thought(&mut self, id: &str, text: &str) {
        if !text.trim().is_empty() {
            self.item(ConversationItem::Thought {
                id: id.to_owned(),
                text: text.to_owned(),
            });
        }
    }

    fn tool(&mut self, id: Option<&str>, name: Option<&str>, input: Value) {
        self.item(ConversationItem::Tool {
            id: id.unwrap_or_default().to_owned(),
            name: name.unwrap_or("unknown").to_owned(),
            status: Lifecycle::Running,
            input: Some(input),
            output: None,
        });
    }

    /// Settle the latest call `call` with its result.
    fn resolve(&mut self, call: &str, failed: bool, text: String) {
        let found = self
            .0
            .iter_mut()
            .rev()
            .flat_map(|turn| turn.items.iter_mut().rev())
            .find(|item| matches!(item, ConversationItem::Tool { id, .. } if id == call));
        if let Some(ConversationItem::Tool { status, output, .. }) = found {
            *status = if failed {
                Lifecycle::Failed
            } else {
                Lifecycle::Completed
            };
            *output = Some(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0199a213-81c0-7800-8aa1-bbab2a035a53";

    #[test]
    fn a_codex_rollout_is_found_by_its_conversation_id_with_its_directory() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/10/02");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join(format!("rollout-2026-10-02T10-00-00-{ID}.jsonl")),
            r#"{"type":"session_meta","payload":{"id":"x","cwd":"/work/repo"}}"#,
        )
        .unwrap();

        let found = transcript(HarnessKind::Codex, home.path(), ID).unwrap();
        assert_eq!(recorded_cwd(&found), Some(PathBuf::from("/work/repo")));
        assert!(transcript(HarnessKind::Codex, home.path(), &ID.replace('3', "4")).is_none());
        assert!(transcript(HarnessKind::Claude, home.path(), ID).is_none());
        let index = scan(HarnessKind::Codex, home.path());
        assert_eq!(index.keys().collect::<Vec<_>>(), [ID]);
        assert_eq!(index[ID].path, found);
    }

    #[test]
    fn a_claude_transcript_is_found_by_its_conversation_id() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("projects/-work-repo");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join(format!("{ID}.jsonl")),
            "{\"type\":\"summary\"}\n{\"type\":\"user\",\"cwd\":\"/work/repo\"}\n",
        )
        .unwrap();

        let found = transcript(HarnessKind::Claude, home.path(), ID).unwrap();
        assert_eq!(recorded_cwd(&found), Some(PathBuf::from("/work/repo")));
        assert_eq!(scan(HarnessKind::Claude, home.path())[ID].path, found);
    }

    #[test]
    fn a_copied_transcript_lands_at_the_same_place_and_never_replaces_a_newer_one() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let project = from.path().join("projects/-work-repo");
        fs::create_dir_all(project.join(format!("{ID}/subagents"))).unwrap();
        fs::write(project.join(format!("{ID}.jsonl")), "{\"type\":\"user\"}\n").unwrap();
        fs::write(project.join(format!("{ID}/subagents/a.jsonl")), "{}\n").unwrap();

        let copied = copy_between(HarnessKind::Claude, ID, from.path(), to.path())
            .unwrap()
            .unwrap();
        assert_eq!(
            copied,
            to.path().join(format!("projects/-work-repo/{ID}.jsonl"))
        );
        assert!(to
            .path()
            .join(format!("projects/-work-repo/{ID}/subagents/a.jsonl"))
            .is_file());

        fs::write(&copied, "newer\n").unwrap();
        copy_between(HarnessKind::Claude, ID, from.path(), to.path()).unwrap();
        assert_eq!(fs::read_to_string(&copied).unwrap(), "newer\n");
        assert!(copy_between(HarnessKind::Codex, ID, from.path(), to.path())
            .unwrap()
            .is_none());
    }

    #[test]
    fn claude_cleanup_reads_the_home_setting_or_states_the_default() {
        let home = tempfile::tempdir().unwrap();
        let default = ExpiryPolicy::read(HarnessKind::Claude, Some(home.path()));
        assert_eq!(
            default,
            ExpiryPolicy::ClaudeCleanup {
                days: 30,
                settings: None
            }
        );
        assert_eq!(default.after(0), Some(30 * 86_400));
        fs::write(
            home.path().join("settings.json"),
            r#"{"cleanupPeriodDays":90}"#,
        )
        .unwrap();
        let set = ExpiryPolicy::read(HarnessKind::Claude, Some(home.path()));
        assert_eq!(set.after(0), Some(90 * 86_400));
        assert!(set.describe().contains("settings.json"));
        assert_eq!(ExpiryPolicy::read(HarnessKind::Codex, None).after(0), None);
    }
}
