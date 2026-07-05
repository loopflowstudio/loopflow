//! The resident: the wave's mind as its own `lf` process.
//!
//! `lf wave <name> --mind-only` runs here. The resident owns everything
//! vendor-shaped — the conversations harness, the scheduler
//! ([`crate::wave::mind`]), the rendered GOAL.md seed — and NOTHING
//! pen-shaped: it never touches a journal file. Its two connections to the
//! listener are the whole protocol (see [`crate::wave::wire`]):
//!
//! - **input**: its own wave's `/events?inbox=true` subscription (the same
//!   SSE machinery `lf sub` uses) — queued messages, steer and interrupt ops,
//!   the pending queue replayed on connect;
//! - **output**: ordered wire deltas through the token-gated resident door.
//!
//! The worktree bootstrap lives HERE now (it moved out of the listener): the
//! resident ensures and enters the wave's `<repo>.<wave>` sibling worktree —
//! the mind never runs in the main checkout — while the listener serves from
//! the origin repo.
//!
//! # Lifecycle
//! Spawned by the listener (default `lf wave <name>`) with the endpoint and
//! token in env, or attached by hand against the discovery files. On listener
//! death the subscription ends and the resident exits cleanly — its keeper is
//! gone; whether anything restarts the pair is the human's arrangement
//! (tmux, systemd). On mind failure the resident reports
//! `MindState::Failed` over the wire and exits nonzero — the listener's
//! supervisor owns the respawn ladder.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use tokio::sync::mpsc;

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::sub::stream_events;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::conversations::harness::{
    canonical_harness, default_create_harness, ApprovalPolicy, Harness,
};
use crate::lfd::conversations::types::ConversationEvent;
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::types::WAVE_SERVER_ENDPOINT_ENV;
use crate::ops::util::resolve_wave_name;
use crate::wave::journal::{MessageId, PendingMessage};
use crate::wave::mind::{path_for_children, run_mind, MindConfig};
use crate::wave::runtime::InboxItem;
use crate::wave::server;
use crate::wave::wire::{
    AttachRequest, AttachResponse, ContextResponse, InboxFrame, PostDeltasRequest, ResidentDelta,
    RESIDENT_TOKEN_ENV, RESIDENT_TOKEN_HEADER,
};

/// `lf wave <name> --mind-only`: attach to the wave's live listener and be
/// its mind until one of us dies.
pub fn run(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let wave = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    let (endpoint, token) = resolve_attachment(
        std::env::var(WAVE_SERVER_ENDPOINT_ENV).ok(),
        std::env::var(RESIDENT_TOKEN_ENV).ok(),
        &main_repo,
        &wave,
    )?;

    // Worktree bootstrap (moved here from the listener): ensure and enter
    // the wave's worktree — the mind never runs in the main checkout.
    let mind_cwd = wave_worktree(&main_repo, &wave)?;
    if std::env::current_dir().ok().as_deref() != Some(mind_cwd.as_path()) {
        std::env::set_current_dir(&mind_cwd)?;
    }
    // Every child of this resident — the harness and anything it shells out
    // to — must resolve `lf` to the binary running this resident, not an
    // installed one.
    std::env::set_var("PATH", path_for_children());

    let vendor = resolve_mind_vendor(&main_repo, &wave)?;
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let harness = default_create_harness(&vendor, ApprovalPolicy::AutoApprove, events_tx)?;
    println!(
        "lf wave · {wave} · resident (vendor {vendor}) · listener http://{endpoint} \
         · worktree {}",
        mind_cwd.display()
    );

    let config = MindConfig {
        vendor,
        ..MindConfig::default()
    };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(drive(
        ListenerClient::new(endpoint, token),
        harness,
        events_rx,
        mind_cwd,
        main_repo,
        wave,
        config,
    ))
}

/// Attach, subscribe, and run the mind until the listener disappears (clean
/// end) or the mind fails (error — the process exits nonzero and the
/// listener's supervisor takes it from there).
pub async fn drive(
    client: ListenerClient,
    harness: Box<dyn Harness>,
    events_rx: mpsc::UnboundedReceiver<ConversationEvent>,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: MindConfig,
) -> Result<()> {
    let attach = client
        .attach(std::process::id())
        .await
        .context("attach to the wave listener")?;
    if attach.wave != wave {
        bail!(
            "listener at {} serves wave '{}', not '{wave}'",
            client.endpoint(),
            attach.wave
        );
    }
    let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
    let subscription = tokio::spawn(follow_inbox(client.endpoint().to_string(), inbox_tx));
    let result = run_mind(
        client,
        inbox_rx,
        harness,
        events_rx,
        cwd,
        origin_repo,
        wave,
        attach.thread_id,
        config,
    )
    .await;
    subscription.abort();
    result
}

/// The wave's own worktree — `<repo>.<wave>`, a sibling of the main repo —
/// created on first boot, reused after.
fn wave_worktree(main_repo: &Path, wave: &str) -> Result<PathBuf> {
    let (path, _branch) = ensure_wave_worktree(main_repo, wave)?;
    Ok(PathBuf::from(path))
}

/// Where the listener is and how to prove we belong at its resident door:
/// spawn env first (the keeper passed both), the discovery files second
/// (`--mind-only` by hand, same trust domain as `.wave-endpoint`).
fn resolve_attachment(
    env_endpoint: Option<String>,
    env_token: Option<String>,
    main_repo: &Path,
    wave: &str,
) -> Result<(String, String)> {
    let endpoint = env_endpoint
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            std::fs::read_to_string(server::endpoint_path(main_repo, wave))
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| {
            anyhow!(
                "wave '{wave}' has no live listener (no {env} in env, no \
                 wave/{wave}/{file}) — start one with `lf wave {wave}`",
                env = WAVE_SERVER_ENDPOINT_ENV,
                file = server::ENDPOINT_FILE,
            )
        })?;
    let token = env_token
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| server::read_resident_token(main_repo, wave))
        .ok_or_else(|| {
            anyhow!(
                "no resident token for wave '{wave}' (no {RESIDENT_TOKEN_ENV} in env, no \
                 token file beside the endpoint pointer) — is the listener running?"
            )
        })?;
    Ok((endpoint, token))
}

/// The mind vendor, from `mind:` in the wave's GOAL.md frontmatter; codex
/// when unset. An unknown name is an error, not a silent fallback.
pub fn resolve_mind_vendor(origin: &Path, wave: &str) -> Result<String> {
    let configured = read_wave_config(origin, wave).and_then(|config| config.mind);
    let name = configured.unwrap_or_else(|| "codex".to_string());
    canonical_harness(&name).map(str::to_string).ok_or_else(|| {
        anyhow!(
            "wave '{wave}' GOAL.md names an unknown mind vendor '{name}' \
             (known: codex, claude, opencode)"
        )
    })
}

/// Follow the wave's `/events?inbox=true` stream, handing every inbox frame
/// to the mind. One connection, no reconnect: when the stream ends the
/// KEEPER is gone (shutdown, crash, or a restart that minted a new token) and
/// this resident's tenancy is over — dropping `inbox_tx` closes the mind's
/// inbox, which ends [`run_mind`] cleanly.
pub async fn follow_inbox(endpoint: String, inbox_tx: mpsc::UnboundedSender<InboxItem>) {
    let result = stream_events(&endpoint, "?inbox=true", &mut |frame| {
        if frame.event != "inbox" {
            return;
        }
        match serde_json::from_str::<InboxFrame>(&frame.data) {
            Ok(frame) => {
                let _ = inbox_tx.send(inbox_item(frame));
            }
            Err(err) => {
                tracing::warn!(error = %err, data = frame.data, "unparseable inbox frame; dropped")
            }
        }
    })
    .await;
    match result {
        Ok(()) => tracing::info!("listener closed the event stream; resident tenancy over"),
        Err(err) => tracing::info!(error = %err, "listener unreachable; resident tenancy over"),
    }
}

fn inbox_item(frame: InboxFrame) -> InboxItem {
    match frame.id {
        Some(id) => InboxItem::Message(PendingMessage {
            id: MessageId(id),
            op: frame.op,
            text: frame.text,
            from: frame.from,
        }),
        // Only bare interrupts ride without an id (nothing journaled).
        None => InboxItem::Interrupt,
    }
}

/// The resident's HTTP client for the listener's resident door. Every call
/// carries this boot's token; any transport or auth failure means the
/// listener is gone (or replaced) — the caller ends cleanly.
#[derive(Debug, Clone)]
pub struct ListenerClient {
    endpoint: String,
    token: String,
    http: reqwest::Client,
}

impl ListenerClient {
    pub fn new(endpoint: String, token: String) -> Self {
        Self {
            endpoint,
            token,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client always builds"),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn attach(&self, pid: u32) -> Result<AttachResponse> {
        let response = self
            .http
            .post(format!("http://{}/resident/attach", self.endpoint))
            .header(RESIDENT_TOKEN_HEADER, &self.token)
            .json(&AttachRequest { pid })
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    /// Send an ordered batch of deltas. The resident sends serially (awaits
    /// each response before the next batch), so per-turn order is total.
    pub async fn send_deltas(&self, deltas: Vec<ResidentDelta>) -> Result<()> {
        if deltas.is_empty() {
            return Ok(());
        }
        self.http
            .post(format!("http://{}/resident/deltas", self.endpoint))
            .header(RESIDENT_TOKEN_HEADER, &self.token)
            .json(&PostDeltasRequest { deltas })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// The pre-turn snapshot (also freshens the listener's store fold).
    pub async fn context(&self) -> Result<ContextResponse> {
        let response = self
            .http
            .get(format!("http://{}/resident/context", self.endpoint))
            .header(RESIDENT_TOKEN_HEADER, &self.token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_attachment_prefers_env_then_files_then_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");

        // Nothing anywhere: a clear error naming the fix.
        let err = resolve_attachment(None, None, tmp.path(), "ship").expect_err("no listener");
        assert!(err.to_string().contains("lf wave ship"), "{err}");

        // Files only (the --mind-only path).
        let addr: std::net::SocketAddr = "127.0.0.1:50607".parse().unwrap();
        server::write_endpoint(tmp.path(), "ship", addr).expect("pointer");
        server::write_resident_token(tmp.path(), "ship", "tok-file").expect("token");
        let (endpoint, token) =
            resolve_attachment(None, None, tmp.path(), "ship").expect("files resolve");
        assert_eq!(endpoint, "127.0.0.1:50607");
        assert_eq!(token, "tok-file");

        // Env wins over files (the spawned-child path).
        let (endpoint, token) = resolve_attachment(
            Some("127.0.0.1:9".into()),
            Some("tok-env".into()),
            tmp.path(),
            "ship",
        )
        .expect("env resolves");
        assert_eq!(endpoint, "127.0.0.1:9");
        assert_eq!(token, "tok-env");
    }

    #[test]
    fn resolve_mind_vendor_reads_goal_frontmatter() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).unwrap();

        // No GOAL.md at all: codex.
        assert_eq!(resolve_mind_vendor(tmp.path(), "ship").unwrap(), "codex");

        // GOAL.md without mind: codex.
        std::fs::write(dir.join("GOAL.md"), "---\nmode: manual\n---\nShip.\n").unwrap();
        assert_eq!(resolve_mind_vendor(tmp.path(), "ship").unwrap(), "codex");

        // mind: selects the harness (canonicalized).
        std::fs::write(dir.join("GOAL.md"), "---\nmind: Claude\n---\nShip.\n").unwrap();
        assert_eq!(resolve_mind_vendor(tmp.path(), "ship").unwrap(), "claude");

        // An unknown vendor is an error, never a silent fallback.
        std::fs::write(dir.join("GOAL.md"), "---\nmind: hal9000\n---\nShip.\n").unwrap();
        let err = resolve_mind_vendor(tmp.path(), "ship").expect_err("unknown vendor");
        assert!(err.to_string().contains("hal9000"), "{err}");
    }

    #[test]
    fn inbox_frames_map_to_inbox_items() {
        let message = inbox_item(InboxFrame {
            id: Some("msg-3".into()),
            op: crate::wave::journal::MessageOp::Steer,
            text: "focus".into(),
            from: None,
        });
        let InboxItem::Message(message) = message else {
            panic!("expected message");
        };
        assert_eq!(message.id, MessageId("msg-3".into()));
        assert_eq!(message.op, crate::wave::journal::MessageOp::Steer);

        assert!(matches!(
            inbox_item(InboxFrame {
                id: None,
                op: crate::wave::journal::MessageOp::Interrupt,
                text: String::new(),
                from: None,
            }),
            InboxItem::Interrupt
        ));
    }

    /// The resident self-bootstraps the wave's `<repo>.<wave>` sibling
    /// worktree: created on first boot, reused after — the mind never runs in
    /// the main checkout. (This moved here from the listener, which now
    /// serves from the origin repo.)
    #[test]
    fn wave_worktree_creates_and_reuses_the_sibling_tree() {
        let repo = loopflow_test_support::TestRepo::new();

        let created = wave_worktree(repo.path(), "ship").expect("bootstrap worktree");
        let repo_name = repo
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("repo name");
        assert_eq!(
            created.file_name().and_then(|name| name.to_str()),
            Some(format!("{repo_name}.ship").as_str()),
            "wave worktree is the <repo>.<wave> sibling"
        );
        assert!(created.join(".git").exists(), "worktree is a checkout");

        let reused = wave_worktree(repo.path(), "ship").expect("reuse worktree");
        assert_eq!(reused, created, "second boot reuses the same tree");
    }
}
