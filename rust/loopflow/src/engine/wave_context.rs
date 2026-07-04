//! Ambient wave context: every `lf` run born inside a wave inherits the
//! wave's recent conversation and curated memory.
//!
//! The wave server is the LISTENER unifying publishers; the two sections
//! assembled here (`<lf:wave-chat-recent>` and `<lf:wave-memory>`) are the
//! unified context flowing back into every publisher at birth. No wave
//! resolves → nothing is added (zero tokens, no headers) — flows stay
//! wave-agnostic.
//!
//! Resolution: explicit `--wave` (the caller passes it) > `LFD_WAVE_ID` env
//! (managed sessions and dispatched workers, mapped to a name through the
//! shared store) > worktree/branch name (`ops::util::resolve_wave_name`).
//!
//! Read path — reads only, the wave server stays the single writer:
//! 1. live server: `GET /conversation` at the `wave/<name>/.wave-endpoint`
//!    discovery pointer (the same file `lf wave` publishes);
//! 2. no live server: a read-only fold over the wave's journal
//!    ([`crate::lfd::wave::journal::read_events`] — never truncates, never
//!    creates);
//! 3. nothing: empty.
//!
//! Wave state (journal, endpoint pointer, MEMORY.md) lives under the ORIGIN
//! repo — a worktree resolves its main repo first.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::engine::worktrees::main_repo_root;
use crate::lfd::conversations::turns::{ChatRole, ChatTurn};
use crate::lfd::conversations::types::{ConversationItem, Lifecycle};
use crate::lfd::wave::journal::{fold_thread, journal_path, read_events};
use crate::lfd::wave::server::endpoint_path;

/// Turns included in `<lf:wave-chat-recent>` (the newest are kept).
pub const WAVE_CHAT_RECENT_TURNS: usize = 12;
/// Hard budget for the rendered chat, in characters. Inherited context obeys
/// the Dumb Zone rule: oldest turns are dropped first, newest survive.
pub const WAVE_CHAT_MAX_CHARS: usize = 4_000;
/// Per-operation timeout for the live-server read (loopback only).
const LIVE_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// The wave a run is ambiently inside, or `None` when no wave resolves.
///
/// `LFD_WAVE_ID` (set by dispatchers on every managed session) wins; a bare
/// run falls back to the worktree/branch resolution every workflow op uses —
/// but only when `repo_root` really is the working-tree root. A nested
/// directory handed in as a "repo" (fixture trees, subdir invocations) must
/// not inherit the enclosing checkout's wave.
pub fn resolve_ambient_wave_name(repo_root: &Path) -> Option<String> {
    if let Some(name) = std::env::var(crate::lf::session::WAVE_ID_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .and_then(|id| wave_name_for_id(id.trim()))
    {
        return Some(name);
    }
    if !is_worktree_root(repo_root) {
        return None;
    }
    crate::ops::util::resolve_wave_name(repo_root, None)
}

/// Whether `repo_root` is the top of a git working tree (not a directory
/// inside one, and not outside git entirely).
fn is_worktree_root(repo_root: &Path) -> bool {
    let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let toplevel = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let toplevel = toplevel.canonicalize().unwrap_or(toplevel);
    let root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    toplevel == root
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
            let store = crate::lfd::store::open_existing_store().await?;
            store.get_wave(&id).await.ok().flatten()
        })
    })
    .join()
    .ok()
    .flatten()
    .map(|wave| wave.name().to_string())
}

/// The origin repo a wave's state lives under: the main checkout when
/// `repo_root` is a worktree root, `repo_root` itself otherwise. A directory
/// that is not itself a working-tree root (fixture trees, plain directories)
/// is its own origin — it must not walk up into an enclosing checkout.
pub fn wave_origin(repo_root: &Path) -> PathBuf {
    if !is_worktree_root(repo_root) {
        return repo_root.to_path_buf();
    }
    main_repo_root(repo_root).unwrap_or_else(|_| repo_root.to_path_buf())
}

/// The wave's recent conversation, rendered compactly, or `None` when the
/// wave has no thread (or no wave state exists at all).
pub fn gather_wave_chat(repo_root: &Path, wave: &str) -> Option<String> {
    let origin = wave_origin(repo_root);
    let turns = live_turns(&origin, wave).or_else(|| journal_turns(&origin, wave))?;
    render_wave_chat(&turns)
}

/// Prefer the live server: the open turn is only there in full fidelity, and
/// the journal is its own persistence. A dead/stale pointer degrades to the
/// journal fold silently.
fn live_turns(origin: &Path, wave: &str) -> Option<Vec<ChatTurn>> {
    #[derive(Debug, Deserialize)]
    struct ConversationBody {
        turns: Vec<ChatTurn>,
    }

    let addr = std::fs::read_to_string(endpoint_path(origin, wave)).ok()?;
    let addr = addr.trim();
    if addr.is_empty() {
        return None;
    }
    let body = http_get(addr, "/conversation")?;
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

/// Blocking loopback HTTP GET over a raw socket. Deliberately not reqwest:
/// context assembly is sync but sometimes runs inside a tokio runtime (flow
/// steps), where `reqwest::blocking` panics. A raw socket blocks a thread
/// that already blocks on child processes, bounded by [`LIVE_READ_TIMEOUT`].
fn http_get(addr: &str, path: &str) -> Option<String> {
    let socket_addr: std::net::SocketAddr = addr.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&socket_addr, LIVE_READ_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(LIVE_READ_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(LIVE_READ_TIMEOUT)).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    // Read until the response is complete (Content-Length satisfied or the
    // chunked terminator seen) rather than to EOF: a peer that resets after
    // writing everything must not void a fully-received body.
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if response_complete(&raw) {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => raw.extend_from_slice(&buf[..read]),
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status_ok = head
        .lines()
        .next()
        .is_some_and(|line| line.split_whitespace().nth(1) == Some("200"));
    if !status_ok {
        return None;
    }
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return Some(dechunk(body));
    }
    Some(body.to_string())
}

/// Whether `raw` holds a complete HTTP response: headers plus either the
/// declared Content-Length of body or the chunked terminator. Malformed or
/// header-incomplete input is "not complete" — the read loop then continues
/// to EOF/timeout and the parser decides.
fn response_complete(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let head_lower = head.to_ascii_lowercase();
    if head_lower.contains("transfer-encoding: chunked") {
        return body.contains("0\r\n\r\n");
    }
    let Some(length) = head_lower
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return false;
    };
    body.len() >= length
}

/// Decode a chunked transfer body, tolerating a truncated tail.
fn dechunk(mut body: &str) -> String {
    let mut out = String::new();
    while let Some(size_end) = body.find("\r\n") {
        let Ok(size) = usize::from_str_radix(body[..size_end].trim(), 16) else {
            break;
        };
        let start = size_end + 2;
        if size == 0 || body.len() < start + size {
            break;
        }
        out.push_str(&body[start..start + size]);
        body = &body[(start + size + 2).min(body.len())..];
    }
    out
}

/// Render the last [`WAVE_CHAT_RECENT_TURNS`] turns compactly, newest last,
/// within [`WAVE_CHAT_MAX_CHARS`]: `speaker: text`, tool items summarized to
/// a count, non-completed status noted. Oldest lines are dropped first; a
/// single oversized newest turn is clipped rather than dropped.
pub fn render_wave_chat(turns: &[ChatTurn]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0usize;
    for turn in turns.iter().rev().take(WAVE_CHAT_RECENT_TURNS) {
        let mut line = render_turn_line(turn);
        if line.is_empty() {
            continue;
        }
        let cost = line.len() + usize::from(!lines.is_empty());
        if used + cost > WAVE_CHAT_MAX_CHARS {
            if lines.is_empty() {
                truncate_on_char_boundary(&mut line, WAVE_CHAT_MAX_CHARS);
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
    use crate::lfd::wave::journal::{EventKind, Journal, MessageId, MessageOp, Usage};

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
        crate::lfd::wave::server::write_endpoint(tmp.path(), "goals", addr).expect("endpoint");

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
        crate::lfd::wave::server::write_endpoint(tmp.path(), "goals", dead_addr).expect("endpoint");

        let chat = gather_wave_chat(tmp.path(), "goals").expect("journal fallback");
        assert!(chat.contains("from the journal"));
    }
}
