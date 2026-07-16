//! Ambient wave context: every `lf` run born inside a wave inherits the
//! wave's recent conversation and curated memory.
//!
//! The wave server is the LISTENER unifying publishers; the two sections
//! assembled here (`<lf:wave-chat-recent>` and `<lf:wave-memory>`) are the
//! unified context flowing back into every publisher at birth. No wave
//! resolves → nothing is added (zero tokens, no headers) — flows stay
//! wave-agnostic.
//!
//! Resolution: explicit `--wave` (the caller passes it) > `LF_CHANNEL` /
//! `LF_WAVE_ID` from a managed session. Repository location cannot identify a
//! Wave: every Wave and Project operates from the same canonical checkout.
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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Deserialize;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::id::{RunId, WaveId};
use crate::wave::journal::{fold_thread, journal_path, read_events};
use crate::wave::server::endpoint_path;
use crate::wave::Wave;

/// The durable Wave attributed to this process.
pub const WAVE_ID_ENV: &str = "LF_WAVE_ID";
/// The Wave or child channel this process speaks on by default.
pub const CHANNEL_ENV: &str = "LF_CHANNEL";

/// Turns included in `<lf:wave-chat-recent>` (the newest are kept).
pub const WAVE_CHAT_RECENT_TURNS: usize = 12;
/// Hard budget for the rendered chat, in characters. Inherited context obeys
/// the Dumb Zone rule: oldest turns are dropped first, newest survive.
pub const WAVE_CHAT_MAX_CHARS: usize = 4_000;
/// Per-operation timeout for the live-server read (loopback only).
const LIVE_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// Which channel a managed process is ambiently inside, before store lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientChannelRef {
    /// `LF_WAVE_ID` from the env: the id of a wave row in the shared store.
    WaveId(String),
    /// `LF_CHANNEL` from the env: the named channel a managed worker owns.
    Channel(String),
}

/// Resolve the Wave owned by a managed process.
///
/// Humans choose a Wave explicitly. Managed Wave, Project, and Task processes
/// inherit `LF_WAVE_ID` from their launcher.
pub fn resolve_ambient_wave(env_wave_id: Option<&str>) -> Option<String> {
    env_wave_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// THE ambient-CHANNEL rule: `LF_CHANNEL` (set on every worker)
/// wins; otherwise the managed process's Wave owns the channel.
pub fn resolve_ambient_channel(
    env_channel: Option<&str>,
    env_wave_id: Option<&str>,
) -> Option<AmbientChannelRef> {
    if let Some(channel) = env_channel.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(AmbientChannelRef::Channel(channel.to_string()));
    }
    resolve_ambient_wave(env_wave_id).map(AmbientChannelRef::WaveId)
}

/// The channel a run is ambiently inside — [`resolve_ambient_channel`] with
/// this process's env, id-arm resolved through the store. `None` when no
/// wave context resolves anywhere.
pub fn resolve_ambient_channel_name() -> Option<String> {
    let env_channel = std::env::var(CHANNEL_ENV).ok();
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    match resolve_ambient_channel(env_channel.as_deref(), env_wave_id.as_deref())? {
        AmbientChannelRef::WaveId(id) => wave_name_for_id(&id),
        AmbientChannelRef::Channel(name) => Some(name),
    }
}

/// The bus channel owned by a placed run. The worktree carries a timestamp for
/// filesystem freshness; the channel stays stable at `wave.<short-run-id>`.
pub fn placed_channel_name(wave_name: &str, run_id: &RunId) -> String {
    format!(
        "{wave_name}.{}",
        crate::engine::worktrees::short_run_id(run_id.as_str())
    )
}

/// The run-attribution decision for the current process: the wave name to
/// attribute a run to (if any), plus a classified failure to record when a
/// supplied managed identity failed validation.
///
/// - valid UUID or hand-set name → `wave: Some(name)`, `failure: None`
/// - no managed identity (`NoContext`) → `wave: None`, `failure: None`
///   (worktree inference stays a legitimate fallback for this case alone)
/// - stale UUID / registry read failure → `wave: None`, `failure: Some(...)`
///   naming the stale source and the safe explicit recovery (`--wave <name>`)
///
/// Attribution is non-fatal: a stale identity is never silently re-attributed to
/// a wave inferred from the worktree. The run records `None` and the failure so
/// the stale source stays visible and actionable; see W2-239.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAttribution {
    pub wave: Option<String>,
    pub failure: Option<String>,
}

/// The run-attribution decision for the current process environment. One
/// classification shared by every trace/run attribution site
/// ([`crate::journal::ensure_run_context`] and the `lf` run wrapper).
pub fn run_attribution() -> RunAttribution {
    match resolve_managed_wave_name_sync(None) {
        Ok(name) => RunAttribution {
            wave: Some(name),
            failure: None,
        },
        Err(WaveResolveError::NoContext) => RunAttribution {
            wave: None,
            failure: None,
        },
        Err(error) => RunAttribution {
            wave: None,
            failure: Some(attribution_failure_text(&error)),
        },
    }
}

/// The text recorded for a supplied identity that failed validation. The
/// `StaleIdentity` and `UnknownExplicit` `Display` already name the source and
/// the `--wave <name>` recovery; `Registry` does not, so the recovery hint is
/// appended.
fn attribution_failure_text(error: &WaveResolveError) -> String {
    match error {
        WaveResolveError::Registry(_) => format!("{error}; pass --wave <name> to recover"),
        _ => error.to_string(),
    }
}

/// The Wave a managed run is attributed to, or `None` when there is no valid
/// managed identity. Thin wrapper over [`run_attribution`] for non-attribution
/// callers (`lf home`); trace/run attribution uses [`run_attribution`] directly
/// so a stale identity is propagated, not swallowed.
pub fn resolve_run_wave_name() -> Option<String> {
    run_attribution().wave
}

/// Resolve a top-level `--wave` to the durable Wave row used by prompt,
/// journal, and child-process attribution.
pub fn resolve_explicit_wave(name: &str) -> anyhow::Result<Wave> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow::anyhow!("--wave requires a non-empty wave name"))?;
    let lookup_name = name.clone();
    let lookup = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime always builds");
        runtime.block_on(async move {
            let store = crate::store::open_existing_store()
                .await
                .ok_or_else(|| anyhow::anyhow!("no wave registry on this machine"))?;
            store
                .get_wave_by_name(&lookup_name)
                .await
                .map_err(|error| anyhow::anyhow!("failed to read wave registry: {error}"))
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("failed to resolve explicit wave '{name}'"))??;
    lookup.ok_or_else(|| anyhow::anyhow!("wave '{name}' not found"))
}

/// Why an ambient Wave could not be resolved. The three cases a caller must be
/// able to tell apart: no context to resolve from, a context that named a Wave
/// this machine's registry has never seen (stale), and an explicit `--wave`
/// naming an unknown Wave.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WaveResolveError {
    /// No `--wave` and no `LF_WAVE_ID`: nothing to resolve from.
    #[error("no wave in context; pass --wave <name>")]
    NoContext,
    /// `LF_WAVE_ID` names a Wave (id or name) this machine's registry has no
    /// row for. The env outlived the registry it points into.
    #[error(
        "ambient wave '{0}' (LF_WAVE_ID) is not in this machine's registry; \
         the context is stale — pass --wave <name>"
    )]
    StaleIdentity(String),
    /// `--wave` was given but empty/whitespace after normalization.
    #[error("--wave requires a non-empty wave name")]
    UnknownExplicit(String),
    /// The registry read itself failed (I/O, not a miss).
    #[error("failed to read wave registry: {0}")]
    Registry(String),
}

/// THE resolver for the ambient Wave's durable name. One rule, shared by every
/// consumer that acts on "the wave I am inside":
///
/// 1. explicit `--wave` always wins — normalized and returned as a name (no
///    registry membership required; PM and file surfaces key off the name).
/// 2. else `LF_WAVE_ID` as a durable registry **UUID** → mapped to its Wave's
///    name through the store. A UUID the registry has no row for is
///    [`WaveResolveError::StaleIdentity`], never silently re-read as a name.
/// 3. else `LF_WAVE_ID` as a hand-set **name** (intentional fallback) → used
///    directly.
/// 4. else [`WaveResolveError::NoContext`].
///
/// The env is only a pointer used to find the durable Wave; identity is the
/// registry row, never the string in the environment.
pub async fn resolve_managed_wave_name(
    store: Option<&crate::store::Store>,
    explicit: Option<&str>,
    env_wave_id: Option<&str>,
) -> Result<String, WaveResolveError> {
    if let Some(raw) = explicit {
        return crate::ops::util::normalize_wave_name(raw)
            .ok_or_else(|| WaveResolveError::UnknownExplicit(raw.to_string()));
    }
    let raw = env_wave_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(WaveResolveError::NoContext)?;
    if let Ok(id) = raw.parse::<WaveId>() {
        let store = store.ok_or_else(|| WaveResolveError::StaleIdentity(raw.to_string()))?;
        return match store.get_wave(&id).await {
            Ok(Some(row)) => Ok(row.name().to_string()),
            Ok(None) => Err(WaveResolveError::StaleIdentity(raw.to_string())),
            Err(error) => Err(WaveResolveError::Registry(error.to_string())),
        };
    }
    // A hand-set name: use it directly. No registry membership required — the
    // name is the durable key file and PM surfaces already use.
    crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::NoContext)
}

/// [`resolve_managed_wave_name`] for sync, store-free call sites (`lf pm …`).
/// Reads `LF_WAVE_ID` from the env. The explicit and hand-set-name arms touch
/// no store; only a UUID needs one, opened on a scratch thread (the
/// [`resolve_explicit_wave`] / [`wave_name_for_id`] idiom — context assembly is
/// sync and sometimes already inside a runtime).
pub fn resolve_managed_wave_name_sync(explicit: Option<&str>) -> Result<String, WaveResolveError> {
    if let Some(raw) = explicit {
        return crate::ops::util::normalize_wave_name(raw)
            .ok_or_else(|| WaveResolveError::UnknownExplicit(raw.to_string()));
    }
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    let raw = env_wave_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(WaveResolveError::NoContext)?;
    // Fast path: a hand-set name needs no registry.
    if raw.parse::<WaveId>().is_err() {
        return crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::NoContext);
    }
    let raw = raw.to_string();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime always builds");
        runtime.block_on(async move {
            let store = crate::store::open_existing_store().await;
            resolve_managed_wave_name(store.as_ref(), None, Some(&raw)).await
        })
    })
    .join()
    .unwrap_or_else(|_| {
        Err(WaveResolveError::Registry(
            "resolver thread panicked".to_string(),
        ))
    })
}

/// Resolve a linked worktree to its canonical checkout once per process.
fn repo_origin(repo_root: &Path) -> PathBuf {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| query_repo_origin(repo_root))
        .clone()
}

/// The single git call behind [`repo_origin`]: toplevel and common dir in
/// one `rev-parse`. A directory that is not itself a working-tree root
/// (fixture trees, plain directories) is its own origin — it must not walk
/// up into an enclosing checkout.
fn query_repo_origin(repo_root: &Path) -> PathBuf {
    let not_a_root = || repo_root.to_path_buf();
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
    common_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root.to_path_buf())
}

/// Map an ambient `LF_WAVE_ID` value to its durable Wave name through the shared
/// [`resolve_managed_wave_name`] rule: a UUID maps through the store, a hand-set
/// name is used directly. The store API is async and context assembly is sync
/// (sometimes already inside a runtime — flow skills), so it runs on a scratch
/// thread. Any resolve error (`StaleIdentity`, registry I/O) → `None`, and
/// resolution falls back to the worktree.
fn wave_name_for_id(id: &str) -> Option<String> {
    let id = id.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::store::open_existing_store().await;
            resolve_managed_wave_name(store.as_ref(), None, Some(&id))
                .await
                .ok()
        })
    })
    .join()
    .ok()
    .flatten()
}

/// The origin repo a wave's state lives under: the main checkout when
/// `repo_root` is a worktree root, `repo_root` itself otherwise (see
/// [`repo_origin`] for the guard).
pub fn wave_origin(repo_root: &Path) -> PathBuf {
    repo_origin(repo_root)
}

/// The wave's recent conversation, rendered compactly, or `None` when the
/// wave has no thread (or no wave state exists at all).
pub fn gather_wave_chat(repo_root: &Path, wave: &str) -> Option<String> {
    let origin = wave_origin(repo_root);
    let turns = live_turns(&origin, wave).or_else(|| journal_turns(&origin, wave))?;
    render_wave_chat(&turns)
}

/// The wave's prompt memory: recent stream facts layered above the compiled
/// `MEMORY.md` base. Stream facts are newest first for prompt recency; the
/// `lf memory log` command still prints oldest to newest.
pub fn gather_wave_memory(repo_root: &Path, wave: &str) -> Option<String> {
    let origin = wave_origin(repo_root);
    let chain = memory_wave_chain(wave).unwrap_or_else(|| vec![wave.to_string()]);
    gather_memory_chain(&origin, &chain)
}

/// Resolve lexical memory scope through the registry. Chat intentionally does
/// not call this: a child wave inherits facts, never its parent's mailbox.
fn memory_wave_chain(wave: &str) -> Option<Vec<String>> {
    let wave = wave.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::store::open_existing_store().await?;
            memory_wave_chain_from_store(&store, &wave).await
        })
    })
    .join()
    .ok()
    .flatten()
}

async fn memory_wave_chain_from_store(
    store: &crate::store::Store,
    wave: &str,
) -> Option<Vec<String>> {
    let mut current = store.get_wave_by_name(wave).await.ok().flatten()?;
    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    loop {
        if !seen.insert(current.id().clone()) {
            tracing::warn!(
                wave,
                "cycle in parent_wave_id; using the acyclic memory prefix"
            );
            break;
        }
        chain.push(current.name().to_string());
        let Some(parent) = current.parent_wave_id() else {
            break;
        };
        current = match store.get_wave(parent).await.ok().flatten() {
            Some(parent) => parent,
            None => {
                tracing::warn!(wave, parent = %parent, "missing parent wave in memory scope");
                break;
            }
        };
    }
    chain.reverse();
    Some(chain)
}

/// Render each wave's memory oldest-ancestor first. A lone wave reads as its
/// own memory, unheadered; an inherited chain labels who owns what.
fn gather_memory_chain(origin: &Path, chain: &[String]) -> Option<String> {
    let leaf = chain.last()?;
    let scoped = chain
        .iter()
        .filter_map(|wave| {
            let base = crate::wave::memory::Memory::for_wave(origin, wave).read();
            let adds = live_memory_adds(origin, wave)
                .or_else(|| journal_memory_adds(origin, wave))
                .unwrap_or_default();
            let memory = render_wave_memory(adds, &base)?;
            if chain.len() == 1 {
                return Some(memory);
            }
            let ownership = if wave == leaf {
                "owned by"
            } else {
                "inherited from"
            };
            Some(format!("## Memory {ownership} {wave}\n\n{memory}"))
        })
        .collect::<Vec<_>>();
    (!scoped.is_empty()).then(|| scoped.join("\n\n"))
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

fn live_memory_adds(origin: &Path, wave: &str) -> Option<Vec<String>> {
    #[derive(Debug, Deserialize)]
    struct MemoryLogBody {
        facts: Vec<String>,
    }

    let addr = read_endpoint_pointer(origin, wave)?;
    let body = http_get(format!("http://{addr}/memory/log"))?;
    serde_json::from_str::<MemoryLogBody>(&body)
        .ok()
        .map(|payload| payload.facts)
}

fn journal_memory_adds(origin: &Path, wave: &str) -> Option<Vec<String>> {
    let events = read_events(&journal_path(origin, wave));
    if events.is_empty() {
        return None;
    }
    Some(fold_thread(&events).memory_adds)
}

fn render_wave_memory(mut adds: Vec<String>, base: &str) -> Option<String> {
    let base = base.trim();
    let has_base = !base.is_empty();
    adds.retain(|fact| !fact.trim().is_empty());
    if adds.is_empty() && !has_base {
        return None;
    }

    let mut sections = Vec::new();
    if !adds.is_empty() {
        adds.reverse();
        sections.push(
            adds.into_iter()
                .map(|fact| format!("- {}", fact.trim()))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if has_base {
        sections.push(base.to_string());
    }
    Some(sections.join("\n\n"))
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
/// already runs inside a tokio runtime (flow skills), where `reqwest::blocking`
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

    /// The one rule the ambient call sites share: managed identity is explicit
    /// in the process environment.
    #[test]
    fn ambient_rule_env_id_wins_and_is_trimmed() {
        assert_eq!(
            resolve_ambient_wave(Some(" wave-1 ")),
            Some("wave-1".to_string())
        );
        assert_eq!(resolve_ambient_wave(Some("  ")), None);
        assert_eq!(resolve_ambient_wave(None), None);
    }

    /// LF_CHANNEL wins; otherwise LF_WAVE_ID identifies the Wave channel.
    #[test]
    fn ambient_channel_env_wins_then_wave_id() {
        assert_eq!(
            resolve_ambient_channel(Some(" ship.148e "), Some("wave-1")),
            Some(AmbientChannelRef::Channel("ship.148e".to_string()))
        );
        assert_eq!(
            resolve_ambient_channel(Some(""), Some("wave-1")),
            Some(AmbientChannelRef::WaveId("wave-1".to_string()))
        );
        assert_eq!(resolve_ambient_channel(None, None), None);
    }

    /// Trace attribution and `lf home` read the ambient wave through
    /// [`resolve_run_wave_name`]. A hand-set `LF_WAVE_ID=<name>` (not a UUID)
    /// now resolves to that name — the fix — while an absent env is `None` so
    /// the caller falls back to the worktree. The name arm touches no store.
    #[test]
    fn run_wave_name_resolves_a_hand_set_name_and_none_without_context() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var(WAVE_ID_ENV).ok();

        std::env::set_var(WAVE_ID_ENV, "product");
        assert_eq!(resolve_run_wave_name(), Some("product".to_string()));

        std::env::remove_var(WAVE_ID_ENV);
        assert_eq!(resolve_run_wave_name(), None);

        match previous {
            Some(value) => std::env::set_var(WAVE_ID_ENV, value),
            None => std::env::remove_var(WAVE_ID_ENV),
        }
    }

    /// `run_attribution` keeps the classified failure instead of swallowing it.
    /// Absent context is `(None, None)` — worktree inference stays a legitimate
    /// fallback for it alone. A hand-set name, even one no registry row backs, is
    /// durable and never stale: it attributes to itself with no failure (a
    /// "stale name" is not a state the resolver produces — only UUIDs go stale).
    /// The name arm touches no store.
    #[test]
    fn run_attribution_classifies_absent_context_and_hand_set_names() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var(WAVE_ID_ENV).ok();

        std::env::remove_var(WAVE_ID_ENV);
        let absent = run_attribution();
        assert_eq!(absent.wave, None);
        assert_eq!(absent.failure, None);

        std::env::set_var(WAVE_ID_ENV, "product");
        let named = run_attribution();
        assert_eq!(named.wave.as_deref(), Some("product"));
        assert_eq!(named.failure, None);

        std::env::set_var(WAVE_ID_ENV, "ghost");
        let unregistered = run_attribution();
        assert_eq!(unregistered.wave.as_deref(), Some("ghost"));
        assert_eq!(unregistered.failure, None);

        match previous {
            Some(value) => std::env::set_var(WAVE_ID_ENV, value),
            None => std::env::remove_var(WAVE_ID_ENV),
        }
    }

    #[test]
    fn placed_channel_uses_the_wave_and_registry_run_id() {
        let run_id = RunId::parse("a1b2c3d4-1111-4111-8111-111111111111").unwrap();
        assert_eq!(placed_channel_name("ship", &run_id), "ship.a1b2c3d4");
    }

    /// A hand lives inside its wave's mind: a run in a work-line worktree reads
    /// the WAVE's thread — the journal lives at the origin, and the worktree
    /// carries none of its own — byte-identical to a run at the wave home.
    #[test]
    fn a_work_line_worktree_reads_the_waves_thread() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_named_worktree("goals.148e");
        seed_journal(repo.path(), "goals", "wave-level question?");

        let from_work_line =
            gather_wave_chat(&worktree, "goals").expect("work line reads its wave");
        assert!(from_work_line.contains("wave-level question?"));
        assert_eq!(
            from_work_line,
            gather_wave_chat(repo.path(), "goals").expect("wave chat"),
            "the wave home reads exactly the same thread"
        );
    }

    #[test]
    fn wave_origin_of_a_worktree_is_the_main_checkout() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_named_worktree("origin-check");
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
            body: None,
            activity: None,
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
            body: None,
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
            termination_reason: None,
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

    #[test]
    fn render_wave_memory_layers_recent_above_base_newest_first() {
        let rendered = render_wave_memory(
            vec![
                "first fact".to_string(),
                " ".to_string(),
                "second fact".to_string(),
            ],
            "# Memory\n\ncompiled base\n",
        )
        .expect("memory renders");
        assert_eq!(
            rendered,
            "- second fact\n- first fact\n\n# Memory\n\ncompiled base"
        );
    }

    #[test]
    fn gather_wave_memory_uses_journal_delta_since_update() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime =
            crate::wave::runtime::WaveRuntime::open("goals".to_string(), tmp.path().to_path_buf())
                .expect("runtime");
        runtime
            .update_memory("# Goals\n\ncompiled\n", "compiled")
            .expect("update");
        runtime.append_memory("oldest", vec![]).expect("append");
        runtime.append_memory("newest", vec![]).expect("append");

        let memory = gather_wave_memory(tmp.path(), "goals").expect("memory");
        assert_eq!(memory, "- newest\n- oldest\n\n# Goals\n\ncompiled");
    }

    #[tokio::test]
    async fn child_memory_walks_parent_scope_while_chat_stays_local() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
            tmp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let parent = crate::wave::Wave::new(
            WaveId::new(),
            "platform".into(),
            tmp.path().display().to_string(),
        );
        let child = crate::wave::Wave::new(
            WaveId::new(),
            "release".into(),
            tmp.path().display().to_string(),
        )
        .with_parent(parent.id().clone());
        store.create_wave(&parent).await.unwrap();
        store.create_wave(&child).await.unwrap();
        std::fs::create_dir_all(tmp.path().join("wave/platform")).unwrap();
        std::fs::create_dir_all(tmp.path().join("wave/release")).unwrap();
        std::fs::write(
            tmp.path().join("wave/platform/MEMORY.md"),
            "Parent constraint.",
        )
        .unwrap();
        std::fs::write(tmp.path().join("wave/release/MEMORY.md"), "Child decision.").unwrap();

        let chain = memory_wave_chain_from_store(&store, "release")
            .await
            .expect("scope resolves");
        assert_eq!(chain, ["platform", "release"]);
        let memory = gather_memory_chain(tmp.path(), &chain).expect("memory renders");
        assert!(memory.contains("## Memory inherited from platform\n\nParent constraint."));
        assert!(memory.contains("## Memory owned by release\n\nChild decision."));
        assert!(
            memory.find("Parent constraint.").unwrap() < memory.find("Child decision.").unwrap()
        );

        seed_journal(tmp.path(), "platform", "parent-only chat");
        seed_journal(tmp.path(), "release", "child-only chat");
        let chat = gather_wave_chat(tmp.path(), "release").expect("child chat");
        assert!(chat.contains("child-only chat"));
        assert!(!chat.contains("parent-only chat"));
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
