//! Launch-owned authority for provider context callbacks.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The [`SteerAdapter`] of a launch whose hooks report to the Machine server.
pub(crate) const PROVIDER_CONTEXT_ENV: &str = "LF_PROVIDER_CONTEXT";
/// The [`SteerAdapter`] of a bare launch, whose hook claims its steers from
/// the store itself and reports to no server.
pub(crate) const BARE_STEER_ENV: &str = "LF_BARE_STEER";
/// The provider process's seen marker; see [`latest_marker`].
pub(crate) const STEER_SEEN_ENV: &str = "LF_STEER_SEEN";

/// The Session's own marker and, when it belongs to exactly one Task, that
/// Task's; see [`latest_marker`].
pub(crate) const STEER_SESSION_ENV: &str = "LF_STEER_SESSION";
pub(crate) const STEER_TASK_ENV: &str = "LF_STEER_TASK";

/// Touched when a Wave or repository steer is saved; a Task's or a Session's
/// steer touches [`task_marker`] or [`session_marker`] instead, so it starts
/// lf only in the conversations it can reach. A tool-boundary hook skips lf
/// entirely while its provider's seen marker is strictly newer than all three,
/// since no steer arrived after its last check. A managed prompt boundary
/// always checks, which also catches Sessions newly associated with earlier
/// steers.
pub(crate) fn latest_marker(home: &Path) -> PathBuf {
    home.join("steers").join("latest")
}

pub(crate) fn task_marker(home: &Path, task: &str) -> PathBuf {
    home.join("steers").join("task").join(task)
}

pub(crate) fn session_marker(home: &Path, session: &str) -> PathBuf {
    home.join("steers").join("session").join(session)
}

pub(crate) fn seen_marker(home: &Path, process: &crate::id::AgentProcessId) -> PathBuf {
    home.join("steers").join("seen").join(process.as_str())
}

pub(crate) fn touch(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("marker has a parent"))?;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?
        .set_modified(std::time::SystemTime::now())
}

/// Survives attachment-only handoff; replacement creates another capability.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SteerAdapter {
    pub session: String,
    pub agent_process_id: crate::id::AgentProcessId,
    pub capability: String,
}

/// Where a provider context callback fires, which decides whether it may move
/// the Session's selected native conversation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeBoundary<'a> {
    /// The provider declares the conversation it now runs (Claude/Codex
    /// `SessionStart`: startup, resume, clear, compact).
    Select {
        source: &'a str,
    },
    /// Fires on every request, including subagents', so it only fills a
    /// missing selection (OpenCode transforms, Pi).
    Activate {
        source: &'a str,
    },
    Turn,
}

/// A managed conversation whose last boundary was a turn end, and how the
/// Machine server reaches it there.
#[derive(Debug, Clone)]
pub(crate) struct IdleConversation {
    pub adapter: SteerAdapter,
    /// The Session's selected native conversation.
    pub thread: String,
    /// The Session's working directory, where rendered steers are saved.
    pub cwd: std::path::PathBuf,
    pub reach: IdleReach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IdleReach {
    /// The inbox key file Claude publishes beside its session record.
    Claude(std::path::PathBuf),
    /// The Session's private app-server.
    Codex(std::path::PathBuf),
}

impl std::fmt::Debug for SteerAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteerAdapter")
            .field("session", &self.session)
            .field("agent_process_id", &self.agent_process_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Steer {
    pub id: crate::id::SteerId,
    pub sequence: i64,
    pub source: String,
    pub revision: String,
    pub author: crate::durable::Author,
    pub text: String,
}

/// One steer as a Session sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerStatus {
    pub steer: Steer,
    /// When the Session read it, in Unix seconds; null while it waits.
    #[serde(deserialize_with = "Option::deserialize")]
    pub delivered_at: Option<i64>,
}

/// A bounded page of the steers that apply to a Session or that it read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSteerPage {
    pub items: Vec<SessionSteerStatus>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub next_after: Option<i64>,
    pub pending: i64,
}

/// Steers claimed for one conversation and rendered for its provider.
#[derive(Debug)]
pub(crate) struct ClaimedSteers {
    pub steers: Vec<crate::id::SteerId>,
    pub text: String,
}

/// Claim the Session's unread steers for the conversation `thread` and render
/// them. The caller prints `text`, then calls [`delivered`]; a caller that
/// dies between the two leaves a claim that lapses, so a steer can arrive
/// twice and is never dropped. `boundary` says whether this callback may
/// select the Session's native conversation.
pub(crate) fn claim(
    store: &crate::store::sqlite::SqliteStore,
    repo: &Path,
    adapter: &SteerAdapter,
    thread: &str,
    boundary: NativeBoundary<'_>,
) -> crate::store::StoreResult<Option<ClaimedSteers>> {
    if !store.has_undelivered_steers(&adapter.session)? {
        return Ok(None);
    }
    store.with_provider_context(adapter, thread, boundary, || {
        let (steers, remaining) = store.claim_session_steers(adapter, thread)?;
        if steers.is_empty() {
            return Ok(None);
        }
        // A render that fails leaves its claim to lapse, like a hook that died.
        let text = render(repo, &adapter.session, &steers, remaining)
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
        let steers = steers.into_iter().map(|steer| steer.id).collect();
        Ok(Some(ClaimedSteers { steers, text }))
    })
}

/// The conversation took the claimed steers.
pub(crate) fn delivered(
    store: &crate::store::sqlite::SqliteStore,
    session: &str,
    claimed: &ClaimedSteers,
) {
    if let Err(error) = store.deliver_session_steers(session, &claimed.steers) {
        tracing::warn!(session, %error, "printed steers not recorded; they will arrive again");
    }
}

fn excerpt(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Each steer's attribution, a pointer to its complete source saved under the
/// checkout, and at most 400 bytes of its text.
fn render(
    repo: &Path,
    session: &str,
    steers: &[Steer],
    remaining: usize,
) -> anyhow::Result<String> {
    let mut text = String::from("<lf:steers>\n");
    for steer in steers {
        let path = crate::prompt::write_prompt_log(
            repo,
            &serde_json::to_string_pretty(steer)?,
            "steer",
            None,
        )?;
        let path = path.strip_prefix(repo).unwrap_or(&path);
        // A Task update is a steer lf wrote about what it saw, not direction.
        let update = steer.source.starts_with("update:");
        let mut item = if update {
            format!(
                "Task update {}. Report this recorded progress briefly. This is observation, not a request or permission to launch work, advance review, publish or merge. Read current Task status before acting. Complete source: {}\n",
                steer.id,
                serde_json::to_string(path)?
            )
        } else {
            format!(
                "Steer {}. Source {} revision {}; author {}. Complete source: {}\n",
                steer.id,
                serde_json::to_string(excerpt(&steer.source, 96))?,
                serde_json::to_string(excerpt(&steer.revision, 64))?,
                serde_json::to_string(excerpt(&steer.author.to_string(), 96))?,
                serde_json::to_string(path)?
            )
        };
        let mut body = excerpt(&steer.text, if update { 600 } else { 400 });
        while !body.is_empty()
            && item.len() + crate::prompt::render_reference(body).len() + 32 > 1024
        {
            body = excerpt(body, body.len() - 1);
        }
        item.push_str(&crate::prompt::render_reference(body));
        if body.len() != steer.text.len() {
            item.push_str(" [excerpt; full source above]");
        }
        item.push('\n');
        anyhow::ensure!(
            item.len() <= 1024,
            "Steer attribution and source pointer exceed the item budget"
        );
        text.push_str(&item);
    }
    text.push_str(&format!(
        "{remaining} pending. Inspect: lf session steers {session} --json\n</lf:steers>\n"
    ));
    Ok(text)
}
