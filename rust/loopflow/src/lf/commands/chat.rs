//! `lf chat` — post a message into a wave's thread through its live server.
//!
//! The speech surface of the one-door calling convention: minds, workers,
//! humans, and scripts all emit through the same verb. The message POSTs to
//! the target wave's server as the `say` op — it lands in the thread as an
//! attributed statement AND wakes the mind like any input (queued, coalesced,
//! answered).
//!
//! # Targeting (by CHANNEL name — dots are the tree)
//! - default: the invoking context's channel — `LFD_CHANNEL` env first (set
//!   by dispatch), else `LFD_WAVE_ID`, else the worktree name, which IS the
//!   channel name under the ownership naming (`<repo>.<wave>` → the wave
//!   channel; `<repo>.<wave>.<id>` → that work line's channel — speak
//!   locally). No wave context at all (no env, worktree not wave-shaped)
//!   means there is no subscriber: the publish drops with exit 0 and one
//!   stderr note — correct pubsub semantics, which is what makes the speech
//!   vocabulary safe in every prompt unconditionally.
//! - `--parent`: walk `parent_wave_id` in the registry and post to the parent
//!   wave's live server; its endpoint rides the parent's WaveAgent session
//!   row, so cross-repo parents resolve through the store, not the
//!   filesystem. A root wave errors (the human fall-through arrives with
//!   Decisions).
//! - `--wave <name>`: explicit target. A dotted name (`goals.148e0e02`)
//!   addresses that channel through its FAMILY HEAD's server (the wave
//!   `goals` — the head holds every child channel's pen).
//!
//! # Endpoint resolution
//! Always the family head's: its live WaveAgent session row carries
//! `LF_WAVE_ENDPOINT`; when the store has no row (unregistered server, no
//! registry on this machine), the local `wave/<name>/.wave-endpoint`
//! discovery file is the fallback. A resolvable wave with no live server is
//! a clear error — a dead wave's mail bounces, it doesn't vanish (child
//! channels included: the pen is the parent's, so speech to a work line of a
//! down listener bounces the same way); queuing for offline waves is future
//! work.
//!
//! # Attribution
//! Sender context comes from env: `LFD_SESSION_ID` (the registry session, when
//! there is one) plus a label — `LFD_AGENT_ROLE` when set (workers), `wave
//! <own>` when escalating with `--parent`, else `cli`. `--from <label>`
//! overrides the label outright: machine speech (the webhook gatekeeper's
//! `--from ci` / `--from github`) must arrive attributed, never riding the
//! from-absent human path.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use crate::engine::wave_context::{
    read_endpoint_pointer, resolve_ambient_channel, wave_origin, AmbientWaveRef,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::WaveTargetArgs;
use crate::lfd::types::Wave;
use crate::lfdb::{open_existing_store, SharedStore};
use crate::wave::channel::family_head;
use crate::wave::journal::Attribution;

pub fn run(text_args: &[String], from_label: Option<&str>, target: &WaveTargetArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, text_args, from_label, target).await
    })
}

/// The command body, a function of the detected [`CliContext`]. Resolves the
/// target before touching stdin so a no-wave drop never blocks on a pipe.
pub(crate) async fn run_with_context(
    context: &CliContext,
    text_args: &[String],
    from_label: Option<&str>,
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
    let mut from = sender_attribution(target.parent, resolved.own_name.as_deref());
    if let Some(label) = from_label {
        from.label = label.to_string();
    }
    let mut body = serde_json::json!({ "op": "say", "text": text, "from": from });
    if let Some(channel) = &resolved.channel {
        body["channel"] = serde_json::Value::String(channel.clone());
    }
    post_json(&endpoint, "/messages", &body).await?;
    println!(
        "posted to channel '{}' as [{}]",
        resolved.channel.as_deref().unwrap_or(&resolved.name),
        from.label
    );
    Ok(())
}

/// Message text from the args (joined) or stdin (heredoc-friendly).
fn message_text(args: &[String], mut stdin: impl Read) -> Result<String> {
    let joined = args.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    let mut buffer = String::new();
    stdin.read_to_string(&mut buffer)?;
    let text = buffer.trim().to_string();
    if text.is_empty() {
        bail!("no message text: pass TEXT or pipe it on stdin");
    }
    Ok(text)
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
/// one answers via the registry row or the discovery file), the repo root
/// its wave dir lives under (for serverless reads), the invoking wave's name
/// (for labels), and — when the target is a work-line channel rather than
/// the wave itself — the channel name to address the door with.
#[derive(Debug)]
pub(crate) struct ResolvedWave {
    pub name: String,
    pub endpoint: Option<String>,
    pub repo_root: Option<PathBuf>,
    pub own_name: Option<String>,
    /// `Some` only for a child channel (`goals.148e0e02`); `None` targets
    /// the wave channel itself.
    pub channel: Option<String>,
}

impl ResolvedWave {
    pub fn require_endpoint(&self) -> Result<String> {
        self.endpoint.clone().ok_or_else(|| {
            anyhow!(
                "wave '{name}' has no live server — start one with `lf wave {name}`. \
                 (Queuing for offline waves is not implemented yet.)",
                name = self.name
            )
        })
    }
}

/// Resolve the target channel, its family head wave, and the head's live
/// endpoint. See the module doc for the targeting and endpoint rules.
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
    let mut own_channel: Option<String> = None;
    match resolve_ambient_channel(env_channel, env_wave_id, repo) {
        Some(AmbientWaveRef::Id(id)) => {
            if let (Some(store), Ok(id)) = (store, id.parse()) {
                own_row = store.get_wave(&id).await?;
            }
            own_name = own_row.as_ref().map(|row| row.name().clone());
            own_channel = own_name.clone();
        }
        Some(AmbientWaveRef::Name(name)) => {
            let head = family_head(&name).to_string();
            if let Some(store) = store {
                own_row = store.get_wave_by_name(&head).await?;
            }
            own_name = Some(head);
            own_channel = Some(name);
        }
        None => {}
    }

    let (target_row, target_name, channel): (Option<Wave>, String, Option<String>) =
        if let Some(name) = &args.wave {
            let head = family_head(name).to_string();
            let row = match store {
                Some(store) => store.get_wave_by_name(&head).await?,
                None => None,
            };
            let channel = (*name != head).then(|| name.clone());
            (row, head, channel)
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
            let parent_id = own.parent_wave_id().ok_or_else(|| {
                anyhow!(
                    "wave '{}' has no parent — it is a root wave; the human \
                 fall-through arrives with Decisions",
                    own.name()
                )
            })?;
            let parent = store.get_wave(parent_id).await?.ok_or_else(|| {
                anyhow!(
                    "wave '{}' names parent {parent_id}, but the registry has no such wave",
                    own.name()
                )
            })?;
            let name = parent.name().clone();
            // Escalation targets the parent WAVE's channel, never a work line.
            (Some(parent), name, None)
        } else {
            // Speak locally: the ambient channel, through its family head.
            let channel = own_channel
                .clone()
                .filter(|channel| Some(channel) != own_name.as_ref());
            match own_row {
                Some(row) => {
                    let name = row.name().clone();
                    (Some(row), name, channel)
                }
                None => {
                    // No wave context anywhere: the publish has no subscriber.
                    let Some(name) = own_name.clone() else {
                        return Ok(None);
                    };
                    (None, name, channel)
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
        own_name,
        channel,
    }))
}

/// Attribution from env: `LFD_SESSION_ID` when present; label from
/// `LFD_AGENT_ROLE` (workers), else `wave <own>` when escalating, else "cli".
pub(crate) fn sender_attribution(escalating: bool, own_wave: Option<&str>) -> Attribution {
    let session_id = std::env::var(crate::lf::session::SESSION_ID_ENV)
        .ok()
        .filter(|value| !value.is_empty());
    let label = match std::env::var("LFD_AGENT_ROLE")
        .ok()
        .filter(|value| !value.is_empty())
    {
        Some(role) => role,
        None => match (escalating, own_wave) {
            (true, Some(own)) => format!("wave {own}"),
            _ => "cli".to_string(),
        },
    };
    Attribution { session_id, label }
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
                "wave server at {endpoint} is not answering ({err}) — is `lf wave` still running?"
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
                "wave server at {endpoint} is not answering ({err}) — is `lf wave` still running?"
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

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        RepoWork, Session, SessionStatus, SessionUse, WaveStatus, WAVE_SERVER_ENDPOINT_ENV,
        WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
    };
    use crate::lfdb::{open_store, StorageConfig};
    use crate::wave::journal::{EventKind, MessageOp};
    use crate::wave::runtime::{InboxItem, WaveRuntime};
    use crate::wave::server;

    async fn temp_store(dir: &Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(dir.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    fn make_wave(name: &str, repo: &Path, parent: Option<&LfdId>) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.display().to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: parent.cloned(),
        }
    }

    /// A live wave_server WaveAgent row carrying `endpoint` in its env — the
    /// shape `lf wave` registers (see crate::wave::registry).
    fn live_server_session(wave: &Wave, endpoint: &str) -> Session {
        let now = OffsetDateTime::now_utc();
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            step: "mind".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: vec!["lf".to_string(), "wave".to_string(), wave.name().clone()],
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

    /// Boot the HTTP surface over a runtime (the wave/mod.rs harness pattern).
    async fn boot_server(
        origin: &Path,
        wave: &str,
    ) -> (
        String,
        Arc<WaveRuntime>,
        tokio::sync::broadcast::Receiver<InboxItem>,
    ) {
        let runtime =
            WaveRuntime::open(wave.to_string(), origin.to_path_buf()).expect("open runtime");
        let inbox_rx = runtime.subscribe_inbox();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            server::ResidentDoor::new("test-token"),
            server::SubagentDoor::new(),
            None,
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (addr.to_string(), runtime, inbox_rx)
    }

    #[test]
    fn message_text_prefers_args_then_stdin_then_errors() {
        let text = message_text(&["hello".into(), "world".into()], std::io::empty()).unwrap();
        assert_eq!(text, "hello world");

        let text = message_text(&[], std::io::Cursor::new("from stdin\n")).unwrap();
        assert_eq!(text, "from stdin");

        let err = message_text(&[], std::io::empty()).unwrap_err();
        assert!(err.to_string().contains("no message text"));
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
        assert_eq!(resolved.own_name.as_deref(), Some("ship"));
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
            None,
            &WaveTargetArgs::default(),
        )
        .await
        .expect("dropped publish exits 0");
    }

    /// `--from` attributes machine speech: the label lands on the journaled
    /// turn verbatim, overriding the ambient "cli" fallback — webhook execs
    /// (`lf chat --wave x --from ci "CI failed"`) never ride the from-absent
    /// human path.
    #[tokio::test]
    async fn from_flag_overrides_the_attribution_label() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let (addr, runtime, _inbox) = boot_server(&origin, "ship").await;
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
            Some("ci"),
            &WaveTargetArgs {
                wave: Some("ship".into()),
                parent: false,
            },
        )
        .await
        .expect("post with --from");

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "CI failed");
        assert_eq!(thread[0].from.as_deref(), Some("ci"));
    }

    /// `--parent` walks `parent_wave_id` and posts to the parent's live
    /// server: the parent runtime receives the attributed queued message.
    #[tokio::test]
    async fn parent_targeting_posts_an_attributed_message_to_the_parent_server() {
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

        let from = Attribution {
            session_id: Some("sess-child".into()),
            label: "wave concerto".into(),
        };
        post_json(
            &endpoint,
            "/messages",
            &serde_json::json!({ "op": "say", "text": "blocked on the schema", "from": from }),
        )
        .await
        .expect("post say");

        // The parent's thread carries the attributed turn…
        let thread = parent_runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "blocked on the schema");
        assert_eq!(thread[0].from.as_deref(), Some("wave concerto"));
        // …its mind was woken with the same attributed input…
        let InboxItem::Message(msg) = parent_inbox.try_recv().expect("inbox item") else {
            panic!("expected message inbox item");
        };
        assert_eq!(msg.op, MessageOp::Say);
        assert_eq!(
            msg.from.as_ref().map(|f| f.label.as_str()),
            Some("wave concerto")
        );
        // …and the journal row records the attribution durably.
        let (_, events) = crate::wave::journal::Journal::open(&crate::wave::journal::journal_path(
            &origin, "goals",
        ))
        .expect("journal");
        assert!(matches!(
            &events[0].kind,
            EventKind::UserMessage { op: MessageOp::Say, from: Some(from), .. }
                if from.label == "wave concerto" && from.session_id.as_deref() == Some("sess-child")
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

    /// Channel addressing: a dotted name resolves its FAMILY HEAD's endpoint
    /// (the head holds the pen) and rides the wire as the `channel` field —
    /// the message lands in the work line's own journal, in its worktree.
    #[tokio::test]
    async fn dotted_name_targets_the_channel_through_the_family_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        std::fs::create_dir_all(crate::wave::channel::child_worktree_path(
            &origin,
            "ship.148e",
        ))
        .unwrap();
        let (addr, runtime, _inbox) = boot_server(&origin, "ship").await;
        let wave = make_wave("ship", &origin, None);
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_server_session(&wave, &addr))
            .await
            .expect("seed brain");

        // Explicit dotted target: head endpoint, channel on the side.
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
        assert_eq!(resolved.channel.as_deref(), Some("ship.148e"));
        assert_eq!(resolved.endpoint.as_deref(), Some(addr.as_str()));

        // Ambient channel (the dispatch env contract): same resolution.
        let ambient = resolve_target(
            &WaveTargetArgs::default(),
            Some(&store),
            None,
            Some(wave.id().as_str()),
            Some("ship.148e"),
        )
        .await
        .expect("resolve")
        .expect("wave context");
        assert_eq!(ambient.name, "ship");
        assert_eq!(ambient.channel.as_deref(), Some("ship.148e"));

        // The whole door: POST with the channel field lands in the child
        // journal, and a `say` also folds up to the wave thread (the report
        // reaches the mind).
        let mut body = serde_json::json!({
            "op": "say",
            "text": "child-bound",
            "from": Attribution { session_id: None, label: "worker".into() },
        });
        body["channel"] = serde_json::Value::String("ship.148e".into());
        post_json(&addr, "/messages", &body).await.expect("post");
        assert_eq!(
            runtime.thread_snapshot().len(),
            1,
            "the report folded up to the wave thread",
        );
        let events = crate::wave::journal::read_events(&crate::wave::channel::child_journal_path(
            &origin,
            "ship.148e",
        ));
        assert_eq!(events.len(), 1, "the work line's journal has the message");
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
        assert!(message.contains("lf wave ship"), "{message}");
        assert!(message.contains("not implemented yet"), "{message}");
    }
}
