//! `lf chat` — the human conversing with a served mind's durable thread.
//!
//! One door, `POST /messages` on the wave's server. `--steer` uses the `steer`
//! op, reaching a live steer-capable turn and otherwise queueing for the next
//! one. The default `message` op queues for the loop, unattributed — the same
//! human act journals the same way on every surface (the Mac composer sends
//! the identical op).
//!
//! Agents do not use this verb. Their wire is `lf radio pub` — a broadcast on the
//! shared-store bus, with no server in the path ([`super::radio`]). Two words,
//! two wires: `lf chat` needs a live mind; `lf radio pub` needs nothing.
//!
//! # Targeting
//! - default: the invoking context's wave — `LFD_CHANNEL` env first (set by
//!   dispatch), else `LFD_WAVE_ID`, else the worktree name. A dotted name
//!   resolves to its family head: a hand's channel has no thread to converse
//!   with. No wave context at all (no env, worktree not wave-shaped) drops the
//!   message with exit 0 and one stderr note.
//! - `--parent`: walk `parent_wave_id` in the registry and post to the parent
//!   wave's live server; its endpoint rides the parent's WaveAgent session
//!   row, so cross-repo parents resolve through the store, not the
//!   filesystem. A root wave errors (the human fall-through arrives with
//!   Decisions).
//!
//! # Endpoint resolution
//! The wave's live WaveAgent session row carries `LF_WAVE_ENDPOINT`; when the
//! store has no row (unregistered server, no registry on this machine), the
//! local `wave/<name>/.wave-endpoint` discovery file is the fallback. A
//! resolvable wave with no live server is a clear error — a dead wave's mail
//! bounces, it doesn't vanish; queuing for offline waves is future work.
//!
//! # Attribution
//! None, ever. The thread is unattributed by convention and the server
//! rejects a byline on this door. Machine speech — webhook facts, worker
//! reports, escalations — rides the bus with `lf radio pub --from`, and the
//! listener's bus sweep folds it into the thread attributed.
//!
//! # Following
//! `--follow` composes the same post door with [`super::thread`]'s SSE replay.
//! Typed lines are ordinary messages, or steer requests when `--steer` is set;
//! slash commands stay local to the terminal session.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use crate::engine::wave_context::{
    read_endpoint_pointer, resolve_ambient_channel, wave_origin, AmbientWaveRef,
};
use crate::lf::commands::thread;
use crate::lf::commands::util::{find_repo_root, message_text};
use crate::lf::WaveTargetArgs;
use crate::lfd::types::Wave;
use crate::lfdb::{open_existing_store, SharedStore};
use crate::wave::channel::family_head;
use crate::wave::journal::MessageOp;

pub fn run(text_args: &[String], follow: bool, steer: bool, target: &WaveTargetArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        if follow {
            follow_with_context(&context, steer, target).await
        } else {
            run_with_context(&context, text_args, steer, target).await
        }
    })
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
        context.env_channel.as_deref(),
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
        context.env_channel.as_deref(),
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
    let stream = tokio::spawn(async move { thread::follow(Some(wave_name.as_str()), false).await });

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
    // Machine speech carries a byline and rides the bus (`lf radio pub`).
    let op = if steer {
        MessageOp::Steer
    } else {
        MessageOp::Message
    };
    let body = serde_json::json!({ "op": op, "text": text });
    post_json(endpoint, "/messages", &body).await?;
    Ok(())
}

/// What the process can see: the registry (if this machine has one), the repo
/// the command runs in, and the wave env. Gathered once at the edge so the
/// resolution logic below stays a pure function of its inputs.
pub(crate) struct CliContext {
    pub store: Option<SharedStore>,
    pub repo: Option<PathBuf>,
    pub env_wave_id: Option<String>,
    pub env_channel: Option<String>,
}

impl CliContext {
    pub async fn detect() -> Self {
        Self {
            store: open_existing_store().await.map(Arc::new),
            repo: find_repo_root().ok(),
            env_wave_id: std::env::var(crate::lf::session::WAVE_ID_ENV)
                .ok()
                .filter(|value| !value.is_empty()),
            env_channel: std::env::var(crate::lf::session::CHANNEL_ENV)
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
                "wave '{name}' has no live server — serve one with `lf serve {name}`. \
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
    env_channel: Option<&str>,
) -> Result<Option<ResolvedWave>> {
    let main_repo = repo.map(wave_origin);

    // The invoking context's channel: the shared ambient rule (env
    // LFD_CHANNEL first, else LFD_WAVE_ID, else the worktree name — which IS
    // the channel name) — the same resolution context assembly uses. The
    // invoking WAVE is the channel's family head.
    let mut own_row: Option<Wave> = None;
    let mut own_name: Option<String> = None;
    match resolve_ambient_channel(env_channel, env_wave_id, repo) {
        Some(AmbientWaveRef::Id(id)) => {
            if let (Some(store), Ok(id)) = (store, id.parse()) {
                own_row = store.get_wave(&id).await?;
            }
            own_name = own_row.as_ref().map(|row| row.name().clone());
        }
        Some(AmbientWaveRef::Name(name)) => {
            let head = family_head(&name).to_string();
            if let Some(store) = store {
                own_row = store.get_wave_by_name(&head).await?;
            }
            own_name = Some(head);
        }
        None => {}
    }

    let (target_row, target_name): (Option<Wave>, String) = if let Some(name) = &args.wave {
        let head = family_head(name).to_string();
        let row = match store {
            Some(store) => store.get_wave_by_name(&head).await?,
            None => None,
        };
        (row, head)
    } else if args.parent {
        let store = store.ok_or_else(|| {
            anyhow!(
                "--parent needs the run registry to walk the wave tree, \
                 and this machine has none (~/.lf/lfd.db)"
            )
        })?;
        let own = own_row.ok_or_else(|| {
            anyhow!(
                "cannot resolve the invoking wave for --parent: no LFD_WAVE_ID \
                 in env and no registered wave matches this worktree"
            )
        })?;
        let parent = parent_wave(store, &own).await?;
        let name = parent.name().clone();
        (Some(parent), name)
    } else {
        // Speak locally: the ambient wave.
        match own_row {
            Some(row) => {
                let name = row.name().clone();
                (Some(row), name)
            }
            None => {
                // No wave context anywhere: the publish has no subscriber.
                let Some(name) = own_name.clone() else {
                    return Ok(None);
                };
                (None, name)
            }
        }
    };

    // Endpoint: the live WaveAgent session row carries it; the local
    // discovery file is the fallback when the store has no row.
    let mut endpoint = None;
    if let (Some(store), Some(row)) = (store, &target_row) {
        endpoint = crate::wave::registry::wave_server_endpoint(store, row.id()).await?;
    }
    let repo_root = target_row
        .as_ref()
        .map(|row| PathBuf::from(row.repo()))
        .filter(|path| path.is_dir())
        .or(main_repo);
    if endpoint.is_none() {
        if let Some(root) = &repo_root {
            endpoint = read_endpoint_pointer(root, &target_name);
        }
    }

    Ok(Some(ResolvedWave {
        name: target_name,
        endpoint,
        repo_root,
    }))
}

/// Walk one step up the wave tree. Both speech verbs escalate this way —
/// `lf chat --parent` to the parent's thread, `lf radio pub --parent` to its
/// channel — and a root wave is the same clear error for both.
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
                "wave server at {endpoint} is not answering ({err}) — is `lf serve` still running?"
            )
        })?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
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
                "wave server at {endpoint} is not answering ({err}) — is `lf serve` still running?"
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
    use std::collections::BTreeMap;

    use time::OffsetDateTime;

    use crate::lf::commands::fixtures::{boot_server, make_wave, temp_store};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        Session, SessionStatus, SessionUse, WAVE_SERVER_ENDPOINT_ENV, WAVE_SERVER_PID_ENV,
        WAVE_SERVER_SOURCE,
    };
    use crate::wave::journal::{EventKind, MessageOp};
    use crate::wave::runtime::InboxItem;
    use crate::wave::server;

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

    /// A live wave_server WaveAgent row carrying `endpoint` in its env — the
    /// shape `lf serve` registers (see crate::wave::registry).
    fn live_server_session(wave: &Wave, endpoint: &str) -> Session {
        let now = OffsetDateTime::now_utc();
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            skill: "loop".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: vec!["lf".to_string(), "serve".to_string(), wave.name().clone()],
            env: BTreeMap::from([
                (WAVE_SERVER_ENDPOINT_ENV.to_string(), endpoint.to_string()),
                (
                    WAVE_SERVER_PID_ENV.to_string(),
                    std::process::id().to_string(),
                ),
            ]),
            source: WAVE_SERVER_SOURCE.to_string(),
            tmux_name: String::new(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(now),
            completed_at: None,
            created_at: now,
            completion_token: None,
        }
    }

    /// Env-context targeting: LFD_WAVE_ID names the wave; the endpoint comes
    /// off its live WaveAgent session row in the store.
    #[tokio::test]
    async fn resolve_target_uses_env_wave_and_registry_endpoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship", tmp.path(), None);
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_server_session(&wave, "127.0.0.1:4242"))
            .await
            .expect("seed brain");

        let resolved = resolve_target(
            &WaveTargetArgs::default(),
            Some(&store),
            None,
            Some(wave.id().as_str()),
            None,
        )
        .await
        .expect("resolve")
        .expect("wave context");
        assert_eq!(resolved.name, "ship");
        assert_eq!(resolved.endpoint.as_deref(), Some("127.0.0.1:4242"));
    }

    /// Worktree-name fallback: no env, no store — the `<repo>.<wave>` sibling
    /// names the wave and the `.wave-endpoint` discovery file supplies the
    /// endpoint.
    #[tokio::test]
    async fn resolve_target_falls_back_to_worktree_name_and_endpoint_file() {
        let repo = loopflow_test_support::TestRepo::new();
        let (worktree, _branch) =
            crate::lfd::executor::ensure_wave_worktree(repo.path(), "ship").expect("worktree");
        let addr: std::net::SocketAddr = "127.0.0.1:50505".parse().unwrap();
        server::write_endpoint(repo.path(), "ship", addr).expect("pointer");

        let resolved = resolve_target(
            &WaveTargetArgs::default(),
            None,
            Some(Path::new(&worktree)),
            None,
            None,
        )
        .await
        .expect("resolve")
        .expect("wave context");
        assert_eq!(resolved.name, "ship");
        assert_eq!(resolved.endpoint.as_deref(), Some("127.0.0.1:50505"));
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
            env_channel: None,
        };

        let resolved = resolve_target(
            &WaveTargetArgs::default(),
            None,
            Some(tmp.path()),
            None,
            None,
        )
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
        let (addr, runtime, mut inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_server_session(&wave, &addr))
            .await
            .expect("seed brain");

        let context = CliContext {
            store: Some(store),
            repo: None,
            env_wave_id: None,
            env_channel: None,
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
        assert_eq!(thread[0].from, None);
        let InboxItem::Message(message) = inbox.try_recv().expect("steer inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(message.op, MessageOp::Steer);
    }

    /// The same human act journals the same way on every surface: a plain
    /// CLI message is unattributed and op `message`, exactly what the Mac
    /// composer sends. Bylines belong to the bus.
    #[tokio::test]
    async fn plain_chat_is_unattributed_like_the_mac_composer() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (addr, runtime, mut inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_server_session(&wave, &addr))
            .await
            .expect("seed brain");

        let context = CliContext {
            store: Some(store),
            repo: None,
            env_wave_id: None,
            env_channel: None,
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
        assert_eq!(thread[0].from, None, "human turns carry no byline");
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
        store
            .register_session(&live_server_session(&parent, &addr))
            .await
            .expect("seed parent brain");

        let resolved = resolve_target(
            &WaveTargetArgs {
                wave: None,
                parent: true,
            },
            Some(&store),
            None,
            Some(child.id().as_str()),
            None,
        )
        .await
        .expect("resolve parent")
        .expect("wave context");
        assert_eq!(resolved.name, "goals");
        let endpoint = resolved.require_endpoint().expect("live endpoint");
        assert_eq!(endpoint, addr);

        // The thread door takes no bylines: attributed escalation rides the
        // bus and arrives through the parent's bus sweep, not this wire.
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
        assert_eq!(refused.status(), reqwest::StatusCode::BAD_REQUEST);

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
        assert_eq!(thread[0].from, None);
        let InboxItem::Message(msg) = parent_inbox.try_recv().expect("inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(msg.op, MessageOp::Message);
        // The journal row is unattributed too.
        let (_, events) = crate::wave::journal::Journal::open(&crate::wave::journal::journal_path(
            &origin, "goals",
        ))
        .expect("journal");
        assert!(matches!(
            &events[0].kind,
            EventKind::UserMessage {
                op: MessageOp::Message,
                from: None,
                ..
            }
        ));
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
            None,
        )
        .await
        .expect_err("root has no parent");
        assert!(
            err.to_string().contains("wave 'goals' has no parent"),
            "error names the root wave: {err}"
        );
    }

    /// A dotted name still resolves to its family head: a hand's channel is a
    /// bus address, and the bus has no thread to converse with. The mind's
    /// server answers.
    #[tokio::test]
    async fn a_dotted_name_resolves_to_the_family_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (addr, _runtime, _inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_server_session(&wave, &addr))
            .await
            .expect("seed brain");

        let resolved = resolve_target(
            &WaveTargetArgs {
                wave: Some("ship.148e".into()),
                parent: false,
            },
            Some(&store),
            None,
            None,
            None,
        )
        .await
        .expect("resolve")
        .expect("wave context");
        assert_eq!(resolved.name, "ship", "the family head is the wave");
        assert_eq!(resolved.endpoint.as_deref(), Some(addr.as_str()));
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
            None,
        )
        .await
        .expect("resolve")
        .expect("wave context");
        let err = resolved.require_endpoint().expect_err("no server");
        let message = err.to_string();
        assert!(message.contains("no live server"), "{message}");
        // `lf serve` boots a mind; `lf loop` is the batch verb and needs a
        // flow plus a seed — naming it here would name an unspellable command.
        assert!(message.contains("lf serve ship"), "{message}");
        assert!(message.contains("not implemented yet"), "{message}");
    }
}
