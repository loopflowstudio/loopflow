//! `lf chat` — post a message into a wave's thread through its live server.
//!
//! The speech surface of the one-door calling convention: minds, workers,
//! humans, and scripts all emit through the same verb. The message POSTs to
//! the target wave's server as the `say` op — it lands in the thread as an
//! attributed statement AND wakes the mind like any input (queued, coalesced,
//! answered).
//!
//! # Targeting
//! - default: the invoking context's wave — `LFD_WAVE_ID` env first, else the
//!   worktree name (`<repo>.<wave>` sibling). No wave context at all (no env,
//!   worktree not wave-shaped) means there is no subscriber: the publish
//!   drops with exit 0 and one stderr note — correct pubsub semantics, which
//!   is what makes the speech vocabulary safe in every prompt unconditionally.
//! - `--parent`: walk `parent_wave_id` in the registry and post to the parent
//!   wave's live server; its endpoint rides the parent's WaveAgent session
//!   row, so cross-repo parents resolve through the store, not the
//!   filesystem. A root wave errors (the human fall-through arrives with
//!   Decisions).
//! - `--wave <name>`: explicit target.
//!
//! # Endpoint resolution
//! The target's live WaveAgent session row carries `LF_WAVE_ENDPOINT`; when
//! the store has no row (unregistered server, no registry on this machine),
//! the local `wave/<name>/.wave-endpoint` discovery file is the fallback. A
//! resolvable wave with no live server is a clear error — a dead wave's mail
//! bounces, it doesn't vanish; queuing for offline waves is future work.
//!
//! # Attribution
//! Sender context comes from env: `LFD_SESSION_ID` (the registry session, when
//! there is one) plus a label — `LFD_AGENT_ROLE` when set (workers), `wave
//! <own>` when escalating with `--parent`, else `cli`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use crate::engine::wave_context::{
    read_endpoint_pointer, resolve_ambient_wave, wave_origin, AmbientWaveRef,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::WaveTargetArgs;
use crate::lfd::store::{open_existing_store, SharedStore};
use crate::lfd::types::{Wave, WAVE_SERVER_ENDPOINT_ENV};
use crate::lfd::wave::journal::Attribution;

pub fn run(text_args: &[String], target: &WaveTargetArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, text_args, target).await
    })
}

/// The command body, a function of the detected [`CliContext`]. Resolves the
/// target before touching stdin so a no-wave drop never blocks on a pipe.
pub(crate) async fn run_with_context(
    context: &CliContext,
    text_args: &[String],
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
    let from = sender_attribution(target.parent, resolved.own_name.as_deref());
    post_json(
        &endpoint,
        "/messages",
        &serde_json::json!({ "op": "say", "text": text, "from": from }),
    )
    .await?;
    println!("posted to wave '{}' as [{}]", resolved.name, from.label);
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
}

impl CliContext {
    pub async fn detect() -> Self {
        Self {
            store: open_existing_store().await.map(Arc::new),
            repo: find_repo_root().ok(),
            env_wave_id: std::env::var(crate::lf::session::WAVE_ID_ENV)
                .ok()
                .filter(|value| !value.is_empty()),
        }
    }
}

/// A resolved target wave: its name, its live endpoint (when one answers via
/// the registry row or the discovery file), the repo root its wave dir lives
/// under (for serverless reads), and the invoking wave's name (for labels).
#[derive(Debug)]
pub(crate) struct ResolvedWave {
    pub name: String,
    pub endpoint: Option<String>,
    pub repo_root: Option<PathBuf>,
    pub own_name: Option<String>,
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

/// Resolve the target wave and its live endpoint. See the module doc for the
/// targeting and endpoint rules. `Ok(None)` is the publish-to-no-subscriber
/// case: default targeting with no wave context anywhere — callers that
/// publish drop the message; callers that read treat it as an error.
pub(crate) async fn resolve_target(
    args: &WaveTargetArgs,
    store: Option<&SharedStore>,
    repo: Option<&Path>,
    env_wave_id: Option<&str>,
) -> Result<Option<ResolvedWave>> {
    let main_repo = repo.map(wave_origin);

    // The invoking context's wave: the shared ambient rule (env LFD_WAVE_ID
    // first, else the worktree name) — the same resolution context assembly
    // and run registration use.
    let mut own_row: Option<Wave> = None;
    let mut own_name: Option<String> = None;
    match resolve_ambient_wave(env_wave_id, repo) {
        Some(AmbientWaveRef::Id(id)) => {
            if let (Some(store), Ok(id)) = (store, id.parse()) {
                own_row = store.get_wave(&id).await?;
            }
            own_name = own_row.as_ref().map(|row| row.name().clone());
        }
        Some(AmbientWaveRef::Name(name)) => {
            if let Some(store) = store {
                own_row = store.get_wave_by_name(&name).await?;
            }
            own_name = Some(name);
        }
        None => {}
    }

    let (target_row, target_name): (Option<Wave>, String) = if let Some(name) = &args.wave {
        let row = match store {
            Some(store) => store.get_wave_by_name(name).await?,
            None => None,
        };
        (row, name.clone())
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
        (Some(parent), name)
    } else {
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
        if let Some(session) = store.live_wave_agent_session(row.id()).await? {
            endpoint = session
                .env
                .get(WAVE_SERVER_ENDPOINT_ENV)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
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
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{
        RepoWork, Session, SessionStatus, SessionUse, WaveMode, WaveStatus, WAVE_SERVER_PID_ENV,
        WAVE_SERVER_SOURCE,
    };
    use crate::lfd::wave::journal::{EventKind, MessageOp};
    use crate::lfd::wave::runtime::{InboxItem, WaveRuntime};
    use crate::lfd::wave::server;

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
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
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
    /// shape `lf wave` registers (see lfd::wave::registry).
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
        tokio::sync::mpsc::UnboundedReceiver<InboxItem>,
    ) {
        let (runtime, inbox_rx) =
            WaveRuntime::open(wave.to_string(), origin.to_path_buf()).expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(runtime.clone());
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
        };

        let resolved = resolve_target(&WaveTargetArgs::default(), None, Some(tmp.path()), None)
            .await
            .expect("resolve");
        assert!(resolved.is_none(), "plain temp dir is not a wave context");

        run_with_context(&context, &["hello".into()], &WaveTargetArgs::default())
            .await
            .expect("dropped publish exits 0");
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
        let (_, events) = crate::lfd::wave::journal::Journal::open(
            &crate::lfd::wave::journal::journal_path(&origin, "goals"),
        )
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
        assert!(message.contains("no live server"), "{message}");
        assert!(message.contains("lf wave ship"), "{message}");
        assert!(message.contains("not implemented yet"), "{message}");
    }
}
