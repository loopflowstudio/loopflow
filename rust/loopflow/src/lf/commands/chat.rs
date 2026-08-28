//! `lf chat` — the human conversing with a served mind's durable thread.
//!
//! One door, `POST /messages` on the wave's server. `--steer` uses the `steer`
//! op, reaching a live steer-capable turn and otherwise queueing for the next
//! one. The default `message` op queues for the loop. Local chat commits it to
//! the journal; Discord chat posts the same op through its provider-backed
//! composer and queues the canonical provider echo. The Mac composer uses the
//! identical door.
//!
//! # Targeting
//! - default: the invoking context's wave from `LF_WAVE_ID`.
//!   No managed wave context drops the
//!   message with exit 0 and one stderr note.
//! - `--parent`: walk `parent_wave_id` in the registry and post to the parent
//!   Wave's listener. The parent row resolves its repository and Wave-owned
//!   endpoint pointer. A root Wave errors.
//!
//! # Endpoint resolution
//! The local `wave/<name>/.wave-endpoint` discovery file names the listener. A
//! resolvable wave with no live server is a clear error — a dead wave's mail
//! bounces, it doesn't vanish; queuing for offline waves is future work.
//!
//! # Following
//! `--follow` composes the same post door with [`super::thread`]'s SSE replay.
//! Typed lines are ordinary messages, or steer requests when `--steer` is set;
//! slash commands stay local to the terminal session.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::controller::wave::chat::{
    ChatAction, ChatBacking, ChatHistorySnapshot, ChatHistoryState, ChatMessageSource,
    PostMessageErrorResponse, WaveChatMessage,
};
use crate::controller::wave::journal::{
    fold_thread, journal_path, read_events_with_state, MessageOp, ReadOnlyJournalState,
};
use crate::controller::wave::server::{endpoint_path, HUMAN_THREAD_REPLAY_LIMIT};
use crate::lf::commands::thread;
use crate::lf::commands::util::{find_repo_root, message_text};
use crate::lf::WaveTargetArgs;
use crate::store::{open_existing_store, SharedStore};
use crate::work::wave::context::{resolve_managed_wave, wave_origin, WaveResolveError};
use crate::work::wave::Wave;
use anyhow::{anyhow, bail, Result};

#[derive(Debug, Clone, Copy)]
pub struct ChatOptions<'a> {
    pub follow: bool,
    pub steer: bool,
    pub history: bool,
    pub json: bool,
    pub limit: Option<usize>,
    pub epoch: Option<&'a str>,
}

pub fn run(text_args: &[String], options: ChatOptions<'_>, target: &WaveTargetArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        if options.history {
            if !options.json {
                bail!("--history currently requires --json");
            }
            history_with_context(
                &context,
                target,
                options.limit.unwrap_or(HUMAN_THREAD_REPLAY_LIMIT),
                options.epoch,
            )
            .await
        } else if options.follow {
            follow_with_context(&context, options.steer, target).await
        } else {
            run_with_context(&context, text_args, options.steer, target).await
        }
    })
}

/// Print the bounded durable thread without requiring a live Wave listener.
async fn history_with_context(
    context: &CliContext,
    target: &WaveTargetArgs,
    limit: usize,
    epoch: Option<&str>,
) -> Result<()> {
    if limit == 0 {
        bail!("--limit must be at least 1");
    }
    let Some(resolved) = resolve_target(
        target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
    )
    .await?
    else {
        bail!("no wave here — name one with `lf chat --history --json -w <wave>`");
    };
    if let Some(endpoint) = resolved.endpoint.as_deref() {
        let mut path = format!("/conversation?limit={limit}");
        if let Some(epoch) = epoch {
            path.push_str("&epoch=");
            path.push_str(epoch);
        }
        let value = get_json(endpoint, &path).await?;
        let snapshot: ChatHistorySnapshot = serde_json::from_value(value)
            .map_err(|error| anyhow!("bad Wave Chat history response: {error}"))?;
        println!("{}", serde_json::to_string(&snapshot)?);
        return Ok(());
    }
    let repo_root = resolved.repo_root.ok_or_else(|| {
        anyhow!(
            "wave '{}' has no local origin repository for durable history",
            resolved.name
        )
    })?;
    let snapshot = history_snapshot(&repo_root, &resolved.name, limit, epoch);
    println!("{}", serde_json::to_string(&snapshot)?);
    Ok(())
}

fn history_snapshot(
    repo_root: &Path,
    wave: &str,
    limit: usize,
    selected_epoch_id: Option<&str>,
) -> ChatHistorySnapshot {
    let read = read_events_with_state(&journal_path(repo_root, wave));
    let state = match read.state {
        ReadOnlyJournalState::Available => ChatHistoryState::Available,
        ReadOnlyJournalState::Missing => ChatHistoryState::Missing,
        ReadOnlyJournalState::Partial => ChatHistoryState::Partial,
        ReadOnlyJournalState::Unavailable => ChatHistoryState::Unavailable,
    };
    let mut fold = fold_thread(&read.events);
    fold.turns.append(&mut fold.open);
    let epochs = fold.conversation_epochs.clone();
    let active_epoch = epochs.last().cloned();
    let selected_epoch = match selected_epoch_id {
        Some(id) => epochs.iter().find(|epoch| epoch.id == id).cloned(),
        None => active_epoch.clone(),
    };
    let mut messages = selected_epoch
        .as_ref()
        .filter(|epoch| matches!(epoch.backing, ChatBacking::Local))
        .map(|epoch| {
            let imported_turns = fold.conversation_epoch_turns.get(&epoch.id).cloned();
            let end_seq = epochs
                .iter()
                .find(|candidate| candidate.number == epoch.number + 1)
                .map(|candidate| candidate.journal_seq)
                .unwrap_or(u64::MAX);
            fold.turns
                .into_iter()
                .filter_map(|turn| {
                    let journal_seq = turn.id.strip_prefix("turn-")?.parse::<u64>().ok()?;
                    let belongs = imported_turns.as_ref().map_or_else(
                        || journal_seq > epoch.journal_seq && journal_seq < end_seq,
                        |turn_ids| turn_ids.contains(&turn.id),
                    );
                    belongs.then(|| WaveChatMessage {
                        epoch_id: epoch.id.clone(),
                        source: ChatMessageSource::Local { journal_seq },
                        turn,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let truncated = messages.len() > limit;
    let keep_from = messages.len().saturating_sub(limit);
    messages = messages.split_off(keep_from);
    let (state, detail) =
        if let Some(missing_epoch) = selected_epoch_id.filter(|_| selected_epoch.is_none()) {
            (
                ChatHistoryState::Unavailable,
                Some(format!("Unknown Wave Chat epoch '{missing_epoch}'.")),
            )
        } else if selected_epoch
            .as_ref()
            .is_some_and(|epoch| matches!(epoch.backing, ChatBacking::Discord { .. }))
        {
            (
                ChatHistoryState::Unavailable,
                Some("Discord history requires the active Wave listener.".to_string()),
            )
        } else {
            (state, read.detail)
        };
    ChatHistorySnapshot {
        epochs,
        selected_epoch_id: selected_epoch.map(|epoch| epoch.id),
        state,
        detail,
        messages,
        truncated,
    }
}

/// The command body, a function of the detected [`CliContext`]. Resolves the
/// target before touching stdin so a no-wave drop never blocks on a pipe.
pub(crate) async fn run_with_context(
    context: &CliContext,
    text_args: &[String],
    steer: bool,
    target: &WaveTargetArgs,
) -> Result<()> {
    let Some(resolved) = resolve_target(
        target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
    )
    .await?
    else {
        eprintln!("no wave here; message dropped");
        return Ok(());
    };
    let text = message_text(text_args, std::io::stdin())?;
    let endpoint = resolved.require_endpoint()?;
    post_message(&endpoint, &text, steer).await?;
    println!(
        "sent to '{}' ({})",
        resolved.name,
        if steer {
            "steer live, otherwise queue"
        } else {
            "queued for the next turn"
        }
    );
    Ok(())
}

/// Replay and follow the resolved thread while stdin supplies human speech.
async fn follow_with_context(
    context: &CliContext,
    steer: bool,
    target: &WaveTargetArgs,
) -> Result<()> {
    let Some(resolved) = resolve_target(
        target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
    )
    .await?
    else {
        bail!("no wave here — name one with `lf chat --follow -w <wave>`");
    };
    follow_thread(&resolved, steer).await
}

async fn follow_thread(resolved: &ResolvedWave, steer: bool) -> Result<()> {
    let endpoint = resolved.require_endpoint()?;
    println!(
        "chat: {} @ {endpoint}   (/help, Ctrl-D to leave)",
        resolved.name
    );

    // The stream replays on connect. Use the resolved family-head name so an
    // ambient hand or --parent follows the same wave that receives speech.
    let wave_name = resolved.name.clone();
    let stream = tokio::spawn(async move { thread::follow(Some(wave_name.as_str())).await });

    // stdin blocks, so it reads on its own thread and hands lines to the loop.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(16);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.blocking_send(line).is_err() {
                break;
            }
        }
    });

    loop {
        let line = tokio::select! {
            line = rx.recv() => line,
            _ = tokio::signal::ctrl_c() => None,
        };
        // EOF (Ctrl-D) or Ctrl-C leaves the chat; the wave keeps running.
        let Some(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(command) = line.strip_prefix('/') {
            if !handle_command(command, &endpoint).await? {
                break;
            }
            continue;
        }
        post_message(&endpoint, line, steer).await?;
    }

    stream.abort();
    Ok(())
}

/// A steering verb typed inside an interactive chat rather than speech.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Quit,
    Status,
    Help,
    Unknown,
}

fn parse_command(command: &str) -> Command {
    match command.trim() {
        "q" | "quit" | "exit" => Command::Quit,
        "status" => Command::Status,
        "help" | "?" => Command::Help,
        _ => Command::Unknown,
    }
}

/// Returns false when the interactive chat should end.
async fn handle_command(command: &str, endpoint: &str) -> Result<bool> {
    match parse_command(command) {
        Command::Quit => return Ok(false),
        Command::Status => {
            let health = get_json(endpoint, "/health").await?;
            println!("{}", serde_json::to_string_pretty(&health)?);
        }
        Command::Help => println!(
            "  /status   the wave's loop state\n  \
               /quit     leave (Ctrl-D also works)\n  \
             anything else is spoken into the thread"
        ),
        Command::Unknown => eprintln!("unknown command '/{}' — try /help", command.trim()),
    }
    Ok(true)
}

/// Post one unattributed human act, shared by one-shot and followed chat.
async fn post_message(endpoint: &str, text: &str, steer: bool) -> Result<()> {
    let op = if steer {
        MessageOp::Steer
    } else {
        MessageOp::Message
    };
    let body = serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "op": op,
        "text": text,
    });
    post_json(endpoint, "/messages", &body).await?;
    Ok(())
}

/// Nudge a live Wave server to drain its authoritative child-observation
/// outbox. A stopped Wave needs no direct journal writer: its store observer
/// catches up from the durable outbox on the next serve.
pub(crate) async fn nudge_child_observations(wave: &str) -> Result<bool> {
    let context = CliContext::detect().await;
    let target = WaveTargetArgs {
        wave: Some(wave.to_string()),
        parent: false,
    };
    let resolved = resolve_target(
        &target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
    )
    .await?
    .ok_or_else(|| anyhow!("wave {wave:?} cannot be resolved"))?;
    let Some(endpoint) = resolved.endpoint else {
        return Ok(false);
    };
    post_json(&endpoint, "/observations", &serde_json::json!({})).await?;
    Ok(true)
}

/// What the process can see: the registry (if this machine has one), the repo
/// the command runs in, and the wave env. Gathered once at the edge so the
/// resolution logic below stays a pure function of its inputs.
pub(crate) struct CliContext {
    pub store: Option<SharedStore>,
    pub repo: Option<PathBuf>,
    pub env_wave_id: Option<String>,
}

impl CliContext {
    pub async fn detect() -> Self {
        Self {
            store: open_existing_store().await.map(Arc::new),
            repo: find_repo_root().ok(),
            env_wave_id: std::env::var(crate::work::wave::context::WAVE_ID_ENV)
                .ok()
                .filter(|value| !value.is_empty()),
        }
    }
}

/// A resolved target: the family-head wave's name, its live endpoint (when
/// one answers via the registry row or the discovery file), and the repo
/// root its wave dir lives under (for serverless reads).
#[derive(Debug)]
pub(crate) struct ResolvedWave {
    pub name: String,
    pub endpoint: Option<String>,
    pub repo_root: Option<PathBuf>,
}

impl ResolvedWave {
    pub fn require_endpoint(&self) -> Result<String> {
        self.endpoint.clone().ok_or_else(|| {
            anyhow!(
                "wave '{name}' has no live listener — start one with `lf wave {name}`. \
                 (Queuing for offline waves is not implemented yet.)",
                name = self.name
            )
        })
    }
}

/// Resolve the target wave and its live endpoint. See the module doc for the
/// targeting and endpoint rules.
/// `Ok(None)` is the publish-to-no-subscriber case: default targeting with
/// no wave context anywhere — callers that publish drop the message; callers
/// that read treat it as an error.
pub(crate) async fn resolve_target(
    args: &WaveTargetArgs,
    store: Option<&SharedStore>,
    repo: Option<&Path>,
    env_wave_id: Option<&str>,
) -> Result<Option<ResolvedWave>> {
    let main_repo = repo.map(wave_origin);

    // The invoking context's Wave comes from LF_WAVE_ID. It is only resolved when the target
    // is the ambient wave (default or `--parent`); an explicit `--wave` always
    // wins, so its stale/mis-set env is never consulted.
    let mut own_row: Option<Wave> = None;
    if args.wave.is_none() {
        if let Some(id) = env_wave_id {
            // The shared ambient-Wave rule: a durable UUID maps through the
            // registry; a hand-set name resolves inside this repository. Stale
            // context is a loud error, never a silent drop.
            let row = resolve_managed_wave(
                store.map(|store| &**store),
                main_repo.as_deref(),
                None,
                Some(id),
            )
            .await
            .map_err(|err| anyhow!("{err}"))?;
            own_row = Some(row);
        }
    }

    let (target_row, target_name): (Option<Wave>, String) = if let Some(name) = &args.wave {
        let name = name.to_string();
        let store = store.ok_or_else(|| {
            anyhow!(
                "{}",
                WaveResolveError::Registry("no wave registry on this machine".to_string())
            )
        })?;
        let row = resolve_managed_wave(Some(&**store), main_repo.as_deref(), Some(&name), None)
            .await
            .map_err(|err| anyhow!("{err}"))?;
        (Some(row), name)
    } else if args.parent {
        let store = store.ok_or_else(|| {
            anyhow!(
                "--parent needs the run registry to walk the wave tree, \
                 and this machine has none (~/.lf/loopflow.db)"
            )
        })?;
        let own = own_row.ok_or_else(|| {
            anyhow!("cannot resolve the invoking wave for --parent: no LF_WAVE_ID in env")
        })?;
        let parent = parent_wave(store, &own).await?;
        let name = parent.name().to_string();
        (Some(parent), name)
    } else {
        // Speak locally: the ambient wave.
        match own_row {
            Some(row) => {
                let name = row.name().to_string();
                (Some(row), name)
            }
            None => return Ok(None),
        }
    };

    // Listener presence is the Wave-owned discovery pointer.
    let mut endpoint = None;
    let repo_root = target_row
        .as_ref()
        .map(|row| PathBuf::from(row.repo()))
        .filter(|path| path.is_dir())
        .or(main_repo);
    if endpoint.is_none() {
        if let Some(root) = &repo_root {
            endpoint = std::fs::read_to_string(endpoint_path(root, &target_name))
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
    }

    Ok(Some(ResolvedWave {
        name: target_name,
        endpoint,
        repo_root,
    }))
}

/// Walk one step up the wave tree for `lf chat --parent`.
pub(crate) async fn parent_wave(store: &SharedStore, own: &Wave) -> Result<Wave> {
    let parent_id = own.parent_wave_id().ok_or_else(|| {
        anyhow!(
            "wave '{}' has no parent — it is a root wave; the human \
             fall-through arrives with Decisions",
            own.name()
        )
    })?;
    store.get_wave(parent_id).await?.ok_or_else(|| {
        anyhow!(
            "wave '{}' names parent {parent_id}, but the registry has no such wave",
            own.name()
        )
    })
}

/// POST a JSON body to the wave server; connection failure and non-2xx are
/// clear errors (the pointer/registry row can outlive a crashed server).
pub(crate) async fn post_json(
    endpoint: &str,
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{endpoint}{path}"))
        .json(body)
        .send()
        .await
        .map_err(|err| {
            anyhow!(
                "wave listener at {endpoint} is not answering ({err}) — is `lf wave` still running?"
            )
        })?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        if let Ok(rejection) = serde_json::from_str::<PostMessageErrorResponse>(&text) {
            if let ChatBacking::Discord {
                open: ChatAction::OpenDiscord { label, url },
                ..
            } = rejection.epoch.backing
            {
                bail!("{label}: {url}");
            }
        }
        bail!("wave server rejected the request ({status}): {text}");
    }
    Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
}

/// GET a JSON body from the wave server.
pub(crate) async fn get_json(endpoint: &str, path: &str) -> Result<serde_json::Value> {
    let response = reqwest::get(format!("http://{endpoint}{path}"))
        .await
        .map_err(|err| {
            anyhow!(
                "wave listener at {endpoint} is not answering ({err}) — is `lf wave` still running?"
            )
        })?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("wave server rejected the request ({status}): {text}");
    }
    serde_json::from_str(&text).map_err(|err| anyhow!("bad response from wave server: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::controller::wave::journal::{EventKind, MessageOp};
    use crate::controller::wave::runtime::InboxItem;
    use crate::lf::commands::fixtures::{boot_server, make_wave, temp_store};

    #[test]
    fn durable_history_is_bounded_and_does_not_need_a_listener() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = crate::controller::wave::runtime::WaveRuntime::open(
            "ship".to_string(),
            tmp.path().to_path_buf(),
        )
        .expect("open wave journal");
        for index in 0..15 {
            runtime
                .deliver(MessageOp::Message, format!("message {index}"))
                .expect("append message");
        }

        let snapshot = history_snapshot(tmp.path(), "ship", 12, None);
        assert_eq!(snapshot.state, ChatHistoryState::Available);
        assert_eq!(snapshot.messages.len(), 12);
        assert!(snapshot.truncated);
        assert_eq!(snapshot.messages.first().unwrap().turn.text, "message 3");
        assert_eq!(snapshot.messages.last().unwrap().turn.text, "message 14");
    }

    #[test]
    fn durable_history_distinguishes_missing_evidence_from_an_empty_thread() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let snapshot = history_snapshot(tmp.path(), "ship", 12, None);
        assert_eq!(snapshot.state, ChatHistoryState::Missing);
        assert!(snapshot.messages.is_empty());
        assert!(!snapshot.truncated);
        assert!(snapshot.detail.is_some());
    }

    #[test]
    fn serverless_history_keeps_the_imported_local_epoch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        let (mut journal, _) =
            crate::controller::wave::journal::Journal::open(&path).expect("legacy journal");
        journal.append(|_| EventKind::UserMessage {
            id: crate::controller::wave::journal::MessageId("legacy-message".into()),
            op: MessageOp::Message,
            text: "read me after migration".into(),
        });
        drop(journal);
        drop(
            crate::controller::wave::runtime::WaveRuntime::open(
                "ship".into(),
                tmp.path().to_path_buf(),
            )
            .expect("migrate journal"),
        );

        let snapshot = history_snapshot(tmp.path(), "ship", 12, Some("chat-epoch-legacy-1"));
        assert_eq!(snapshot.state, ChatHistoryState::Available);
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].turn.text, "read me after migration");
        assert!(matches!(
            snapshot.messages[0].source,
            ChatMessageSource::Local { .. }
        ));
    }

    #[test]
    fn serverless_history_keeps_discord_epoch_without_shadow_transcript() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let local = crate::controller::wave::runtime::WaveRuntime::open(
            "ship".into(),
            tmp.path().to_path_buf(),
        )
        .expect("open local epoch");
        local
            .try_deliver_authored(MessageOp::Message, "local history".into())
            .expect("write local message");
        drop(local);

        let binding = crate::controller::wave::journal::DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let discord = crate::controller::wave::runtime::WaveRuntime::open_with_backing(
            "ship".into(),
            tmp.path().to_path_buf(),
            ChatBacking::discord(&binding),
        )
        .expect("open Discord epoch");
        let discord_epoch = discord.active_conversation_epoch();
        drop(discord);

        let local_again = crate::controller::wave::runtime::WaveRuntime::open(
            "ship".into(),
            tmp.path().to_path_buf(),
        )
        .expect("open next local epoch");
        assert_eq!(local_again.active_conversation_epoch().number, 3);
        drop(local_again);

        let snapshot = history_snapshot(tmp.path(), "ship", 12, Some(&discord_epoch.id));
        assert_eq!(snapshot.state, ChatHistoryState::Unavailable);
        assert_eq!(snapshot.epochs.len(), 3);
        assert_eq!(
            snapshot.selected_epoch_id.as_deref(),
            Some(discord_epoch.id.as_str())
        );
        let selected = snapshot
            .epochs
            .iter()
            .find(|epoch| epoch.id == discord_epoch.id)
            .expect("selected Discord epoch");
        assert_eq!(selected.id, discord_epoch.id);
        assert_eq!(selected.backing, discord_epoch.backing);
        assert!(selected.ended_at.is_some());
        assert!(matches!(
            snapshot.epochs.last().map(|epoch| &epoch.backing),
            Some(&ChatBacking::Local)
        ));
        assert!(snapshot.messages.is_empty());
        assert_eq!(
            snapshot.detail.as_deref(),
            Some("Discord history requires the active Wave listener.")
        );
    }

    #[test]
    fn chat_history_fixture_round_trips() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/chat_history.json"
        ));
        let snapshot: ChatHistorySnapshot =
            serde_json::from_str(fixture).expect("decode chat history fixture");
        assert_eq!(snapshot.state, ChatHistoryState::Partial);
        assert_eq!(snapshot.messages.len(), 1);
        assert!(snapshot.truncated);
        let encoded = serde_json::to_string(&snapshot).expect("encode chat history fixture");
        let decoded: ChatHistorySnapshot =
            serde_json::from_str(&encoded).expect("re-decode chat history fixture");
        assert_eq!(decoded, snapshot);
    }

    #[tokio::test]
    async fn discord_chat_cli_surfaces_the_open_action() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("address");
        let rejection: PostMessageErrorResponse = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/post_message_error_response.json"
        )))
        .expect("decode rejection fixture");
        let app = axum::Router::new().route(
            "/messages",
            axum::routing::post(move || {
                let rejection = rejection.clone();
                async move { (reqwest::StatusCode::CONFLICT, axum::Json(rejection)) }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let error = post_message(&address.to_string(), "shadow", false)
            .await
            .expect_err("Discord mode rejects CLI compose");
        assert_eq!(
            error.to_string(),
            "Open in Discord: https://discord.com/channels/guild-1/channel-1"
        );
        server.abort();
    }

    #[test]
    fn interactive_commands_parse_and_everything_else_is_speech() {
        assert_eq!(parse_command("quit"), Command::Quit);
        assert_eq!(parse_command(" q "), Command::Quit);
        assert_eq!(parse_command("exit"), Command::Quit);
        assert_eq!(parse_command("status"), Command::Status);
        assert_eq!(parse_command("help"), Command::Help);
        assert_eq!(parse_command("?"), Command::Help);
        assert_eq!(parse_command("deploy"), Command::Unknown);
    }

    /// Env-context targeting: LF_WAVE_ID names the Wave; its endpoint comes
    /// from the Wave-owned discovery pointer.
    #[tokio::test]
    async fn resolve_target_uses_env_wave_and_discovery_endpoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship", tmp.path(), None);
        store.create_wave(&wave).await.expect("seed wave");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).expect("wave dir");
        std::fs::write(
            crate::controller::wave::server::endpoint_path(tmp.path(), "ship"),
            "127.0.0.1:4242",
        )
        .expect("endpoint pointer");

        let resolved = resolve_target(
            &WaveTargetArgs::default(),
            Some(&store),
            None,
            Some(wave.id().as_str()),
        )
        .await
        .expect("resolve")
        .expect("wave context");
        assert_eq!(resolved.name, "ship");
        assert_eq!(resolved.endpoint.as_deref(), Some("127.0.0.1:4242"));
    }

    /// A hand-set `LF_WAVE_ID=<name>` (not a UUID) resolves to that wave —
    /// before the shared resolver this fell through `id.parse()` and dropped
    /// the message silently.
    #[tokio::test]
    async fn resolve_target_uses_a_hand_set_name_env() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (addr, _runtime, _inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");

        let resolved = resolve_target(&WaveTargetArgs::default(), Some(&store), None, Some("ship"))
            .await
            .expect("resolve")
            .expect("hand-set name is a wave context");
        assert_eq!(resolved.name, "ship");
        assert_eq!(resolved.endpoint.as_deref(), Some(addr.as_str()));
    }

    /// A stale `LF_WAVE_ID=<uuid>` (registry has no such row) is a loud error,
    /// not a silent drop — the context is wrong, not absent. Reads surface it;
    /// publishes no longer succeed-as-drop on it.
    #[tokio::test]
    async fn resolve_target_errors_on_a_stale_wave_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let stale = crate::id::WaveId::new().to_string();

        let err = resolve_target(&WaveTargetArgs::default(), Some(&store), None, Some(&stale))
            .await
            .expect_err("stale id is a loud error");
        let message = err.to_string();
        assert!(message.contains("stale"), "{message}");
        assert!(message.contains(&stale), "{message}");
    }

    /// Publish-to-no-subscriber: no env wave, no registry, and a repo that is
    /// not wave-shaped resolve to no target at all — `lf chat` drops the
    /// message and exits 0 instead of erroring.
    #[tokio::test]
    async fn no_wave_context_drops_the_message_with_exit_zero() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let context = CliContext {
            store: None,
            repo: Some(tmp.path().to_path_buf()),
            env_wave_id: None,
        };

        let resolved = resolve_target(&WaveTargetArgs::default(), None, Some(tmp.path()), None)
            .await
            .expect("resolve");
        assert!(resolved.is_none(), "plain temp dir is not a wave context");

        run_with_context(
            &context,
            &["hello".into()],
            false,
            &WaveTargetArgs::default(),
        )
        .await
        .expect("dropped publish exits 0");
    }

    /// `--steer` uses the same wire op as the Mac composer and carries no
    /// attributed byline.
    #[tokio::test]
    async fn steer_flag_requests_live_steering() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (_addr, runtime, mut inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");

        let context = CliContext {
            store: Some(store),
            repo: None,
            env_wave_id: None,
        };
        run_with_context(
            &context,
            &["skip".into(), "the".into(), "migration".into()],
            true,
            &WaveTargetArgs {
                wave: Some("ship".into()),
                parent: false,
            },
        )
        .await
        .expect("post human message");

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "skip the migration");
        let InboxItem::Message(message) = inbox.try_recv().expect("steer inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(message.op, MessageOp::Steer);
    }

    /// The same human act journals the same way on every surface: a plain
    /// CLI message is unattributed and op `message`, exactly what the Mac
    /// composer sends.
    #[tokio::test]
    async fn plain_chat_is_unattributed_like_the_mac_composer() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (_addr, runtime, mut inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");

        let context = CliContext {
            store: Some(store),
            repo: None,
            env_wave_id: None,
        };
        run_with_context(
            &context,
            &["CI".into(), "failed".into()],
            false,
            &WaveTargetArgs {
                wave: Some("ship".into()),
                parent: false,
            },
        )
        .await
        .expect("post plain message");

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "CI failed");
        let InboxItem::Message(message) = inbox.try_recv().expect("inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(message.op, MessageOp::Message);
    }

    /// `--parent` walks `parent_wave_id` and posts to the parent's live
    /// server. The thread door refuses bylines; a human turn arrives plain.
    #[tokio::test]
    async fn parent_targeting_reaches_the_parent_server_unattributed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("parent-repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (addr, parent_runtime, mut parent_inbox) = boot_server(&origin, "goals").await;

        let parent = make_wave("goals", &origin, None);
        store.create_wave(&parent).await.expect("seed parent");
        let child = make_wave("concerto", tmp.path(), Some(parent.id()));
        store.create_wave(&child).await.expect("seed child");

        let resolved = resolve_target(
            &WaveTargetArgs {
                wave: None,
                parent: true,
            },
            Some(&store),
            None,
            Some(child.id().as_str()),
        )
        .await
        .expect("resolve parent")
        .expect("wave context");
        assert_eq!(resolved.name, "goals");
        let endpoint = resolved.require_endpoint().expect("live endpoint");
        assert_eq!(endpoint, addr);

        // The thread door's schema has no bylines or machine-only operations.
        // Axum rejects the retired enum value before the handler runs.
        let refused = reqwest::Client::new()
            .post(format!("http://{endpoint}/messages"))
            .json(&serde_json::json!({
                "op": "say",
                "text": "blocked on the schema",
                "from": "wave concerto",
            }))
            .send()
            .await
            .expect("post");
        assert_eq!(refused.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);

        // A human standing in the child steers the parent: unattributed, like
        // every human turn.
        post_json(
            &endpoint,
            "/messages",
            &serde_json::json!({ "op": "message", "text": "blocked on the schema" }),
        )
        .await
        .expect("post message");
        let thread = parent_runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "blocked on the schema");
        let InboxItem::Message(msg) = parent_inbox.try_recv().expect("inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(msg.op, MessageOp::Message);
        // The journal row is unattributed too.
        let (_, events) = crate::controller::wave::journal::Journal::open(
            &crate::controller::wave::journal::journal_path(&origin, "goals"),
        )
        .expect("journal");
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            EventKind::UserMessage {
                op: MessageOp::Message,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn parent_of_a_root_wave_is_a_clear_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let root = make_wave("goals", tmp.path(), None);
        store.create_wave(&root).await.expect("seed root");

        let err = resolve_target(
            &WaveTargetArgs {
                wave: None,
                parent: true,
            },
            Some(&store),
            None,
            Some(root.id().as_str()),
        )
        .await
        .expect_err("root has no parent");
        assert!(
            err.to_string().contains("wave 'goals' has no parent"),
            "error names the root wave: {err}"
        );
    }

    /// No live server anywhere (no registry row, no discovery file): the
    /// error says so and names the fix.
    #[tokio::test]
    async fn no_live_server_is_a_clear_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship", tmp.path(), None);
        store.create_wave(&wave).await.expect("seed wave");

        let resolved = resolve_target(
            &WaveTargetArgs::default(),
            Some(&store),
            None,
            Some(wave.id().as_str()),
        )
        .await
        .expect("resolve")
        .expect("wave context");
        let err = resolved.require_endpoint().expect_err("no server");
        let message = err.to_string();
        assert!(message.contains("no live listener"), "{message}");
        // `lf wave` is the public Wave lifecycle entrypoint.
        assert!(message.contains("lf wave ship"), "{message}");
        assert!(message.contains("not implemented yet"), "{message}");
    }
}
