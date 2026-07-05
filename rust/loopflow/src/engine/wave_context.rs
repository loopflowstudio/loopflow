//! Ambient wave context: every `lf` run born inside a wave inherits the
//! wave's recent conversation and curated memory.
//!
//! The wave server is the LISTENER unifying publishers; the two sections
//! assembled here (`<lf:wave-chat-recent>` and `<lf:wave-memory>`) are the
//! unified context flowing back into every publisher at birth. No wave
//! resolves → nothing is added (zero tokens, no headers) — flows stay
//! wave-agnostic.
//!
//! Resolution: explicit `--wave` (the caller passes it) >
//! [`resolve_ambient_wave`] — THE ambient rule, shared with `lf chat`
//! targeting and run registration: `LFD_WAVE_ID` env first (managed sessions
//! and dispatched workers), else worktree-root-guarded worktree/branch name
//! resolution (`ops::util::resolve_wave_name`).
//!
//! Read path — reads only, the wave server stays the single writer:
//! 1. live server: `GET /conversation` at the `wave/<name>/.wave-endpoint`
//!    discovery pointer (the same file `lf wave` publishes);
//! 2. no live server: a read-only fold over the wave's journal
//!    ([`crate::wave::journal::read_events`] — never truncates, never
//!    creates);
//! 3. nothing: empty.
//!
//! Wave state (journal, endpoint pointer, MEMORY.md) lives under the ORIGIN
//! repo — a worktree resolves its main repo first.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::wave::journal::{fold_thread, journal_path, read_events};
use crate::wave::server::endpoint_path;

/// Turns included in `<lf:wave-chat-recent>` (the newest are kept).
pub const WAVE_CHAT_RECENT_TURNS: usize = 12;
/// Hard budget for the rendered chat, in characters. Inherited context obeys
/// the Dumb Zone rule: oldest turns are dropped first, newest survive.
pub const WAVE_CHAT_MAX_CHARS: usize = 4_000;
/// Per-operation timeout for the live-server read (loopback only).
const LIVE_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// Which wave a process is ambiently inside, before any store lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientWaveRef {
    /// `LFD_WAVE_ID` from the env: the id of a wave row in the shared store.
    Id(String),
    /// The worktree/branch-derived wave name of the repo the run executes in.
    Name(String),
}

/// THE ambient-wave rule, in one place — context assembly, `lf chat`/`lf
/// memory` targeting, and run self-registration all resolve through it so a
/// run that is context-visible is also registration-visible.
///
/// `LFD_WAVE_ID` (set by dispatchers on every managed session; trimmed) wins;
/// a bare run falls back to the worktree/branch resolution every workflow op
/// uses — but only when `repo_root` really is the working-tree root. A nested
/// directory handed in as a "repo" (fixture trees, subdir invocations) must
/// not inherit the enclosing checkout's wave.
pub fn resolve_ambient_wave(
    env_wave_id: Option<&str>,
    repo_root: Option<&Path>,
) -> Option<AmbientWaveRef> {
    if let Some(id) = env_wave_id.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(AmbientWaveRef::Id(id.to_string()));
    }
    let repo_root = repo_root?;
    if !repo_git_info(repo_root).is_worktree_root {
        return None;
    }
    crate::ops::util::resolve_wave_name(repo_root, None).map(AmbientWaveRef::Name)
}

/// THE ambient-CHANNEL rule: `LFD_CHANNEL` (set by dispatch on every worker)
/// wins; else the ambient-wave rule — whose worktree-name arm already yields
/// the channel name, because the ownership naming makes the worktree basename
/// minus the repo prefix THE channel name (`repo.goals` → `goals`,
/// `repo.goals.148e0e02` → `goals.148e0e02`). An env wave id names the wave,
/// whose channel is its own name.
pub fn resolve_ambient_channel(
    env_channel: Option<&str>,
    env_wave_id: Option<&str>,
    repo_root: Option<&Path>,
) -> Option<AmbientWaveRef> {
    if let Some(channel) = env_channel.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(AmbientWaveRef::Name(channel.to_string()));
    }
    resolve_ambient_wave(env_wave_id, repo_root)
}

/// The channel a run is ambiently inside — [`resolve_ambient_channel`] with
/// this process's env, id-arm resolved through the store. `None` when no
/// wave context resolves anywhere.
pub fn resolve_ambient_channel_name(repo_root: &Path) -> Option<String> {
    let env_channel = std::env::var(crate::lf::session::CHANNEL_ENV).ok();
    let env_wave_id = std::env::var(crate::lf::session::WAVE_ID_ENV).ok();
    match resolve_ambient_channel(
        env_channel.as_deref(),
        env_wave_id.as_deref(),
        Some(repo_root),
    )? {
        AmbientWaveRef::Id(id) => wave_name_for_id(&id),
        AmbientWaveRef::Name(name) => Some(name),
    }
}

/// What the ambient-wave paths ask git about a repo root.
#[derive(Debug, Clone)]
struct RepoGitInfo {
    /// `repo_root` is the top of a git working tree (not a directory inside
    /// one, and not outside git entirely).
    is_worktree_root: bool,
    /// The main checkout this working tree belongs to; `repo_root` itself
    /// when it is not a worktree root.
    origin: PathBuf,
}

/// One git resolution per repo root per process. Prompt assembly asks "is
/// this a worktree root?" and "where is the origin?" several times per run
/// (wave resolution, chat, memory — 6-7 git execs before memoization);
/// mirrors the Swift side's memoization of the same resolution.
fn repo_git_info(repo_root: &Path) -> RepoGitInfo {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, RepoGitInfo>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| query_repo_git_info(repo_root))
        .clone()
}

/// The single git call behind [`repo_git_info`]: toplevel and common dir in
/// one `rev-parse`. A directory that is not itself a working-tree root
/// (fixture trees, plain directories) is its own origin — it must not walk
/// up into an enclosing checkout.
fn query_repo_git_info(repo_root: &Path) -> RepoGitInfo {
    let not_a_root = || RepoGitInfo {
        is_worktree_root: false,
        origin: repo_root.to_path_buf(),
    };
    let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ])
        .output()
    else {
        return not_a_root();
    };
    if !output.status.success() {
        return not_a_root();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().map(str::trim);
    let toplevel = PathBuf::from(lines.next().unwrap_or_default());
    let common_dir = PathBuf::from(lines.next().unwrap_or_default());
    let toplevel = toplevel.canonicalize().unwrap_or(toplevel);
    let root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    if toplevel != root {
        return not_a_root();
    }
    RepoGitInfo {
        is_worktree_root: true,
        origin: common_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| repo_root.to_path_buf()),
    }
}

/// Map a wave id from the env to its name through the shared store. The
/// store API is async and context assembly is sync (sometimes already inside
/// a runtime — flow steps), so the lookup runs on a scratch thread. No store
/// or unknown id → `None`, and resolution falls back to the worktree.
fn wave_name_for_id(id: &str) -> Option<String> {
    let id: crate::lfd::id::LfdId = id.parse().ok()?;
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::lfdb::open_existing_store().await?;
            store.get_wave(&id).await.ok().flatten()
        })
    })
    .join()
    .ok()
    .flatten()
    .map(|wave| wave.name().to_string())
}

/// The origin repo a wave's state lives under: the main checkout when
/// `repo_root` is a worktree root, `repo_root` itself otherwise (see
/// [`repo_git_info`] for the guard).
pub fn wave_origin(repo_root: &Path) -> PathBuf {
    repo_git_info(repo_root).origin
}

/// The wave's recent conversation, rendered compactly, or `None` when the
/// wave has no thread (or no wave state exists at all).
pub fn gather_wave_chat(repo_root: &Path, wave: &str) -> Option<String> {
    let origin = wave_origin(repo_root);
    let turns = live_turns(&origin, wave).or_else(|| journal_turns(&origin, wave))?;
    render_wave_chat(&turns)
}

/// Turn/char budget for the parent-wave section of a work line's overlay —
/// the parent rides compactly beside the work line's own thread; the two
/// budgets together stay within [`WAVE_CHAT_MAX_CHARS`].
const PARENT_CHAT_TURNS: usize = 6;
const PARENT_CHAT_MAX_CHARS: usize = 1_500;
/// What the work line's own section may spend when the parent section rides
/// along (`WAVE_CHAT_MAX_CHARS - PARENT_CHAT_MAX_CHARS`).
const CHILD_CHAT_MAX_CHARS: usize = WAVE_CHAT_MAX_CHARS - PARENT_CHAT_MAX_CHARS;

/// The `<lf:wave-chat-recent>` body for an ambient CHANNEL: the wave channel
/// reads exactly as [`gather_wave_chat`]; a work-line channel reads down the
/// tree — its own thread (folded from the journal in THIS worktree, where
/// the channel lives) plus, compactly, the parent wave channel's recent
/// turns — under the same total cap as a wave-home overlay.
pub fn gather_channel_chat(repo_root: &Path, channel: &str) -> Option<String> {
    // Family membership and the head split are ONE predicate, shared with the
    // server's scoping (`crate::wave::channel::family_head`): a channel with
    // no dot (the wave's own channel) reads as a plain wave chat; a work-line
    // channel reads down the tree from its family head.
    let wave = crate::wave::channel::family_head(channel);
    if wave == channel {
        return gather_wave_chat(repo_root, channel);
    }
    // The work line's own thread: its journal travels with the branch —
    // journal fold only, read-only (the family head's server holds the pen;
    // a vanished/never-opened journal is just an empty section).
    let own = {
        let events = read_events(&journal_path(repo_root, channel));
        let fold = fold_thread(&events);
        let mut turns = fold.turns;
        turns.extend(fold.open);
        render_wave_chat_budget(&turns, WAVE_CHAT_RECENT_TURNS, CHILD_CHAT_MAX_CHARS)
    };
    let parent = {
        let origin = wave_origin(repo_root);
        live_turns(&origin, wave)
            .or_else(|| journal_turns(&origin, wave))
            .and_then(|turns| {
                render_wave_chat_budget(&turns, PARENT_CHAT_TURNS, PARENT_CHAT_MAX_CHARS)
            })
    };
    match (own, parent) {
        (None, None) => None,
        (own, parent) => {
            let mut sections = Vec::new();
            if let Some(own) = own {
                sections.push(format!("## this work line ({channel})\n{own}"));
            }
            if let Some(parent) = parent {
                sections.push(format!("## wave {wave}\n{parent}"));
            }
            Some(sections.join("\n\n"))
        }
    }
}

/// The `wave/<name>/.wave-endpoint` discovery pointer's contents, trimmed.
/// Missing or empty pointer → `None`. Shared by every pointer reader (`lf
/// chat` targeting and the ambient read here); the wave server owns writes.
pub fn read_endpoint_pointer(origin: &Path, wave: &str) -> Option<String> {
    let addr = std::fs::read_to_string(endpoint_path(origin, wave)).ok()?;
    let addr = addr.trim();
    (!addr.is_empty()).then(|| addr.to_string())
}

/// Prefer the live server: the open turn is only there in full fidelity, and
/// the journal is its own persistence. A dead/stale pointer degrades to the
/// journal fold silently. Asks for only the turns the render keeps
/// (`?limit=N` = the last N); a server without limit support returns the
/// full thread, which renders identically.
fn live_turns(origin: &Path, wave: &str) -> Option<Vec<ChatTurn>> {
    #[derive(Debug, Deserialize)]
    struct ConversationBody {
        turns: Vec<ChatTurn>,
    }

    let addr = read_endpoint_pointer(origin, wave)?;
    let body = http_get(format!(
        "http://{addr}/conversation?limit={WAVE_CHAT_RECENT_TURNS}"
    ))?;
    serde_json::from_str::<ConversationBody>(&body)
        .ok()
        .map(|payload| payload.turns)
}

/// Read-only fold over the wave's journal: finalized thread plus any open
/// (crash-tail or in-flight) turns, in order.
fn journal_turns(origin: &Path, wave: &str) -> Option<Vec<ChatTurn>> {
    let events = read_events(&journal_path(origin, wave));
    if events.is_empty() {
        return None;
    }
    let fold = fold_thread(&events);
    let mut turns = fold.turns;
    turns.extend(fold.open);
    Some(turns)
}

/// Blocking GET on a scratch thread: context assembly is sync but sometimes
/// already runs inside a tokio runtime (flow steps), where `reqwest::blocking`
/// panics — the same reason [`wave_name_for_id`] hops threads. Bounded by
/// [`LIVE_READ_TIMEOUT`] per phase (connect, whole request).
fn http_get(url: String) -> Option<String> {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(LIVE_READ_TIMEOUT)
            .timeout(LIVE_READ_TIMEOUT)
            .build()
            .ok()?;
        let response = client.get(url).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.text().ok()
    })
    .join()
    .ok()
    .flatten()
}

/// Render the last [`WAVE_CHAT_RECENT_TURNS`] turns compactly, newest last,
/// within [`WAVE_CHAT_MAX_CHARS`]: `speaker: text`, tool items summarized to
/// a count, non-completed status noted. Oldest lines are dropped first; a
/// single oversized newest turn is clipped rather than dropped.
pub fn render_wave_chat(turns: &[ChatTurn]) -> Option<String> {
    render_wave_chat_budget(turns, WAVE_CHAT_RECENT_TURNS, WAVE_CHAT_MAX_CHARS)
}

/// [`render_wave_chat`] under an explicit turn/char budget (the work-line
/// overlay splits the default budget between its two sections).
fn render_wave_chat_budget(
    turns: &[ChatTurn],
    max_turns: usize,
    max_chars: usize,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0usize;
    for turn in turns.iter().rev().take(max_turns) {
        let mut line = render_turn_line(turn);
        if line.is_empty() {
            continue;
        }
        let cost = line.len() + usize::from(!lines.is_empty());
        if used + cost > max_chars {
            if lines.is_empty() {
                truncate_on_char_boundary(&mut line, max_chars);
                lines.push(line);
            }
            break;
        }
        used += cost;
        lines.push(line);
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    Some(lines.join("\n"))
}

fn render_turn_line(turn: &ChatTurn) -> String {
    let speaker = turn.from.as_deref().unwrap_or(match turn.role {
        ChatRole::User => "user",
        ChatRole::Assistant => "wave",
    });
    let text = turn.text.trim();
    let tool_items = turn
        .items
        .iter()
        .filter(|item| !matches!(item, ConversationItem::Message { .. }))
        .count();
    if text.is_empty() && tool_items == 0 && turn.status == Lifecycle::Completed {
        return String::new();
    }
    let mut line = format!("{speaker}: {text}");
    if tool_items > 0 {
        let plural = if tool_items == 1 { "" } else { "s" };
        line.push_str(&format!(" ({tool_items} tool item{plural})"));
    }
    match turn.status {
        Lifecycle::Failed => line.push_str(" [failed]"),
        Lifecycle::Interrupted => line.push_str(" [interrupted]"),
        Lifecycle::Running | Lifecycle::Pending => line.push_str(" [in progress]"),
        Lifecycle::Completed => {}
    }
    line
}

fn truncate_on_char_boundary(value: &mut String, mut max: usize) {
    if value.len() <= max {
        return;
    }
    while max > 0 && !value.is_char_boundary(max) {
        max -= 1;
    }
    value.truncate(max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wave::journal::{EventKind, Journal, MessageId, MessageOp, Usage};
    use std::io::{Read, Write};

    /// The one rule the three ambient call sites (context assembly, `lf chat`
    /// targeting, run registration) share: env id first (trimmed), else
    /// worktree-root-guarded name resolution.
    #[test]
    fn ambient_rule_env_id_wins_and_is_trimmed() {
        assert_eq!(
            resolve_ambient_wave(Some(" wave-1 "), None),
            Some(AmbientWaveRef::Id("wave-1".to_string()))
        );
        // Blank env falls through to the (absent) repo.
        assert_eq!(resolve_ambient_wave(Some("  "), None), None);
        assert_eq!(resolve_ambient_wave(None, None), None);
    }

    #[test]
    fn ambient_rule_resolves_wave_worktree_but_not_nested_or_bare_dirs() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_wave_worktree("ship");
        assert_eq!(
            resolve_ambient_wave(None, Some(&worktree)),
            Some(AmbientWaveRef::Name("ship".to_string()))
        );

        // A nested directory handed in as a "repo" must not inherit the
        // enclosing checkout's wave.
        let nested = worktree.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(resolve_ambient_wave(None, Some(&nested)), None);

        // A bare directory outside git resolves nothing.
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(resolve_ambient_wave(None, Some(tmp.path())), None);

        // Env still wins over the worktree.
        assert_eq!(
            resolve_ambient_wave(Some("wave-1"), Some(&worktree)),
            Some(AmbientWaveRef::Id("wave-1".to_string()))
        );
    }

    /// The ambient-channel rule: LFD_CHANNEL wins; a work-line worktree's
    /// name IS its channel; env wave id still resolves the wave channel.
    #[test]
    fn ambient_channel_env_wins_then_worktree_name() {
        assert_eq!(
            resolve_ambient_channel(Some(" ship.148e "), Some("wave-1"), None),
            Some(AmbientWaveRef::Name("ship.148e".to_string()))
        );
        assert_eq!(
            resolve_ambient_channel(Some(""), Some("wave-1"), None),
            Some(AmbientWaveRef::Id("wave-1".to_string()))
        );

        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_wave_worktree("ship.148e");
        assert_eq!(
            resolve_ambient_channel(None, None, Some(&worktree)),
            Some(AmbientWaveRef::Name("ship.148e".to_string())),
            "the work-line worktree's name is its channel"
        );
    }

    /// A run inside a work-line worktree reads down the tree: its own
    /// channel's thread plus, compactly, the parent wave channel's — both
    /// sections labeled; a run at the wave home reads the wave channel alone,
    /// exactly as before.
    #[test]
    fn channel_chat_in_a_work_line_carries_child_and_parent_sections() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_wave_worktree("goals.148e");

        // Parent wave channel: journal at the origin.
        seed_journal(repo.path(), "goals", "wave-level question?");
        // The work line's own channel: journal in ITS worktree.
        let (mut child, _) =
            Journal::open(&journal_path(&worktree, "goals.148e")).expect("child journal");
        child.append(|seq| EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op: MessageOp::Say,
            text: "child-level report".to_string(),
            from: None,
        });
        drop(child);

        let overlay =
            gather_channel_chat(&worktree, "goals.148e").expect("work-line overlay renders");
        assert!(overlay.contains("## this work line (goals.148e)"));
        assert!(overlay.contains("child-level report"));
        assert!(overlay.contains("## wave goals"));
        assert!(overlay.contains("wave-level question?"));
        let child_pos = overlay.find("child-level report").unwrap();
        let parent_pos = overlay.find("wave-level question?").unwrap();
        assert!(child_pos < parent_pos, "own channel first, parent after");

        // A wave-channel gather stays the plain single-section render.
        let wave_only = gather_channel_chat(repo.path(), "goals").expect("wave chat");
        assert!(
            !wave_only.contains("## "),
            "no section headers at the wave home"
        );

        // A work line with no thread yet still inherits the parent section.
        let bare = repo.create_wave_worktree("goals.bare0");
        let overlay = gather_channel_chat(&bare, "goals.bare0").expect("parent-only overlay");
        assert!(!overlay.contains("## this work line"));
        assert!(overlay.contains("## wave goals"));
    }

    #[test]
    fn wave_origin_of_a_worktree_is_the_main_checkout() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_wave_worktree("origin-check");
        let origin = wave_origin(&worktree);
        assert_eq!(
            origin.canonicalize().unwrap(),
            repo.path().canonicalize().unwrap()
        );

        // A plain directory is its own origin.
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(wave_origin(tmp.path()), tmp.path());
    }

    fn turn(role: ChatRole, text: &str) -> ChatTurn {
        ChatTurn {
            id: "turn-0".to_string(),
            role,
            text: text.to_string(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: "1970-01-01T00:00:00Z".to_string(),
            from: None,
        }
    }

    #[test]
    fn render_notes_status_tools_and_attribution() {
        let mut worker = turn(ChatRole::User, "worker report: PR landed");
        worker.from = Some("worker-1".to_string());
        let mut failed = turn(ChatRole::Assistant, "tried a build");
        failed.status = Lifecycle::Failed;
        failed.items.push(ConversationItem::Tool {
            id: "t-0".to_string(),
            name: "Bash".to_string(),
            status: Lifecycle::Completed,
            input: None,
            output: None,
        });

        let rendered = render_wave_chat(&[worker, failed]).expect("chat renders");
        assert_eq!(
            rendered,
            "worker-1: worker report: PR landed\nwave: tried a build (1 tool item) [failed]"
        );
    }

    #[test]
    fn render_budget_drops_oldest_first_and_keeps_newest() {
        let mut turns = Vec::new();
        for i in 0..30 {
            turns.push(turn(
                ChatRole::Assistant,
                &format!("turn {i} {}", "x".repeat(600)),
            ));
        }
        let rendered = render_wave_chat(&turns).expect("chat renders");
        assert!(rendered.len() <= WAVE_CHAT_MAX_CHARS);
        assert!(rendered.contains("turn 29"), "newest turn survives");
        assert!(!rendered.contains("turn 0 "), "oldest turns are dropped");
        // Newest last: what survives reads in chronological order.
        let first_kept = rendered.lines().next().unwrap();
        let last_kept = rendered.lines().last().unwrap();
        assert!(last_kept.contains("turn 29"));
        assert!(first_kept < last_kept);
    }

    #[test]
    fn render_caps_turn_count() {
        let turns: Vec<ChatTurn> = (0..40)
            .map(|i| turn(ChatRole::User, &format!("m{i}")))
            .collect();
        let rendered = render_wave_chat(&turns).expect("chat renders");
        assert_eq!(rendered.lines().count(), WAVE_CHAT_RECENT_TURNS);
        assert!(rendered.ends_with("user: m39"));
        assert!(rendered.starts_with(&format!("user: m{}", 40 - WAVE_CHAT_RECENT_TURNS)));
    }

    #[test]
    fn render_clips_a_single_oversized_newest_turn() {
        let huge = turn(ChatRole::Assistant, &"y".repeat(WAVE_CHAT_MAX_CHARS * 2));
        let rendered = render_wave_chat(&[huge]).expect("chat renders");
        assert_eq!(rendered.len(), WAVE_CHAT_MAX_CHARS);
    }

    #[test]
    fn render_empty_thread_is_none() {
        assert!(render_wave_chat(&[]).is_none());
        // A turn with nothing to say contributes nothing.
        assert!(render_wave_chat(&[turn(ChatRole::Assistant, "  ")]).is_none());
    }

    fn seed_journal(origin: &Path, wave: &str, text: &str) {
        let (mut journal, _) = Journal::open(&journal_path(origin, wave)).expect("open journal");
        journal.append(|seq| EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op: MessageOp::Message,
            text: text.to_string(),
            from: None,
        });
        journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![MessageId("msg-1".to_string())],
        });
        journal.append(|_| EventKind::TurnItem {
            turn_id: "turn-2".to_string(),
            item: ConversationItem::Message {
                id: "text-0".to_string(),
                text: "on it".to_string(),
                phase: None,
            },
        });
        journal.append(|_| EventKind::TurnFinished {
            turn_id: "turn-2".to_string(),
            status: Lifecycle::Completed,
            usage: Usage::empty(),
        });
    }

    #[test]
    fn journal_fold_feeds_chat_when_no_server_answers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_journal(tmp.path(), "goals", "how goes the build?");

        let chat = gather_wave_chat(tmp.path(), "goals").expect("journal-backed chat");
        assert!(chat.contains("user: how goes the build?"));
        assert!(chat.contains("wave: on it"));
    }

    #[test]
    fn no_wave_state_yields_no_chat() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(gather_wave_chat(tmp.path(), "ghost").is_none());
    }

    /// Canned one-shot HTTP server: the "existing test-server pattern"
    /// shrunk to a std thread, since this read path is deliberately sync.
    fn spawn_canned_conversation(turn_text: &str) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let body = serde_json::json!({
            "turns": [{
                "id": "turn-1",
                "role": "assistant",
                "text": turn_text,
                "status": "completed",
                "items": [],
                "created_at": "1970-01-01T00:00:00Z",
                "from": null,
            }]
        })
        .to_string();
        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                // Read the whole request before answering; dropping a socket
                // with unread bytes RSTs the peer and can void its receive
                // buffer mid-read.
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match socket.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buf[..read]),
                    }
                }
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.shutdown(std::net::Shutdown::Write);
                // Wait for the client to finish before dropping the socket.
                let _ = socket.read(&mut buf);
            }
        });
        addr
    }

    #[test]
    fn live_server_is_preferred_over_the_journal_fold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_journal(tmp.path(), "goals", "from the journal");

        let addr = spawn_canned_conversation("from the live server");
        crate::wave::server::write_endpoint(tmp.path(), "goals", addr).expect("endpoint");

        let chat = gather_wave_chat(tmp.path(), "goals").expect("live chat");
        assert!(chat.contains("from the live server"));
        assert!(!chat.contains("from the journal"));
    }

    #[test]
    fn dead_endpoint_falls_back_to_the_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        seed_journal(tmp.path(), "goals", "from the journal");

        // A dead address: bind, learn the port, drop the listener.
        let dead = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let dead_addr = dead.local_addr().expect("addr");
        drop(dead);
        crate::wave::server::write_endpoint(tmp.path(), "goals", dead_addr).expect("endpoint");

        let chat = gather_wave_chat(tmp.path(), "goals").expect("journal fallback");
        assert!(chat.contains("from the journal"));
    }
}
