//! The resident: the wave's flowloop as its own `lf` process.
//!
//! `lf wave <name> --flowloop-only` runs here. The resident owns everything
//! vendor-shaped — the conversations harness, the scheduler
//! ([`crate::flowloop::wave`]), the rendered GOAL.md seed — and NOTHING
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
//! the flowloop never runs in the main checkout — while the listener serves from
//! the origin repo.
//!
//! # Lifecycle
//! Spawned by the listener (default `lf wave <name>`) with the endpoint and
//! token in env, or attached by hand against the discovery files. On listener
//! death the subscription ends and the resident exits cleanly — its keeper is
//! gone; whether anything restarts the pair is the human's arrangement
//! (tmux, systemd). On flowloop failure the resident reports
//! `FlowloopState::Failed` over the wire and exits nonzero — the listener's
//! supervisor owns the respawn ladder.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use tokio::sync::mpsc;

use crate::engine::repo::find_repo_root;
use crate::engine::worktrees::{ensure_wave_worktree, main_repo_root};
use crate::flowloop::wave::{path_for_children, run_flowloop, FlowloopConfig};
use crate::lfd::types::WAVE_SERVER_ENDPOINT_ENV;
use crate::ops::util::resolve_wave_name;
use crate::wave::journal::{MessageId, PendingMessage};
use crate::wave::runtime::InboxItem;
use crate::wave::server;
use crate::wave::subscription::stream_events;
use crate::wave::wire::{
    AttachRequest, AttachResponse, ContextResponse, InboxFrame, PostDeltasRequest, ResidentDelta,
    RESIDENT_TOKEN_ENV, RESIDENT_TOKEN_HEADER,
};

/// `lf wave <name> --flowloop-only`: attach to the wave's live listener and be
/// its flowloop until one of us dies.
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
    // the wave's worktree — the flowloop never runs in the main checkout.
    let flowloop_cwd = wave_worktree(&main_repo, &wave)?;
    if std::env::current_dir().ok().as_deref() != Some(flowloop_cwd.as_path()) {
        std::env::set_current_dir(&flowloop_cwd)?;
    }
    // Every child of this resident — the harness and anything it shells out
    // to — must resolve `lf` to the binary running this resident, not an
    // installed one.
    std::env::set_var("PATH", path_for_children());

    println!(
        "lf wave · {wave} · resident flowloop · listener http://{endpoint} \
         · worktree {}",
        flowloop_cwd.display()
    );

    let config = FlowloopConfig::default();
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(drive(
        ListenerClient::new(endpoint, token),
        flowloop_cwd,
        main_repo,
        wave,
        config,
    ))
}

/// Attach, subscribe, and run the flowloop until the listener disappears (clean
/// end) or the flowloop fails (error — the process exits nonzero and the
/// listener's supervisor takes it from there).
pub async fn drive(
    client: ListenerClient,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: FlowloopConfig,
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
    let result = run_flowloop(client, inbox_rx, cwd, origin_repo, wave, config).await;
    subscription.abort();
    result
}

/// The wave's own worktree — `<repo>.<wave>`, a sibling of the main repo —
/// created on first boot, reused after.
fn wave_worktree(main_repo: &Path, wave: &str) -> Result<PathBuf> {
    Ok(ensure_wave_worktree(main_repo, wave)?.path)
}

/// Where the listener is and how to prove we belong at its resident door:
/// spawn env first (the keeper passed both), the discovery files second
/// (`--flowloop-only` by hand, same trust domain as `.wave-endpoint`).
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

/// Follow the wave's `/events?inbox=true` stream, handing every inbox frame
/// to the flowloop. One connection, no reconnect: when the stream ends the
/// KEEPER is gone (shutdown, crash, or a restart that minted a new token) and
/// this resident's tenancy is over — dropping `inbox_tx` closes the flowloop's
/// inbox, which ends [`run_flowloop`] cleanly.
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

/// Per-attempt bound on resident-door calls. Generous: serving
/// `/resident/context` makes the listener poll the store, which can be slow
/// — a busy listener must not read as a dead one.
const LISTENER_CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Sleeps between transient-failure retries: a slow or briefly-unreachable
/// listener gets three attempts over ~30s of patience before the residency
/// concludes ListenerGone. A listener that ANSWERS with an error (bad token
/// = replaced boot, 404 = wave gone) is fatal on the first attempt —
/// retrying can't fix a replaced keeper.
const LISTENER_RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(10)];

/// One failed listener call, classified for the retry loop.
#[derive(Debug)]
enum CallError {
    /// The listener answered and refused; the residency is over.
    Fatal(anyhow::Error),
    /// Transport trouble (timeout, refused, reset); the listener may merely
    /// be slow — worth retrying before concluding it is gone.
    Transient(anyhow::Error),
}

/// `error_for_status` errors carry the status: the listener is alive and
/// said no. Everything else is transport trouble.
fn classify(err: reqwest::Error) -> CallError {
    if err.status().is_some() {
        CallError::Fatal(err.into())
    } else {
        CallError::Transient(err.into())
    }
}

/// Run `attempt` with in-place backoff on transient errors. Fatal errors and
/// exhausted retries return `Err` — the caller concludes ListenerGone.
async fn with_retries<T, F, Fut>(what: &str, delays: &[Duration], mut attempt: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, CallError>>,
{
    let mut remaining = delays.iter();
    loop {
        match attempt().await {
            Ok(value) => return Ok(value),
            Err(CallError::Fatal(err)) => {
                return Err(err.context(format!("{what}: listener refused")));
            }
            Err(CallError::Transient(err)) => match remaining.next() {
                Some(delay) => {
                    tracing::warn!(
                        error = %format!("{err:#}"),
                        what,
                        delay_secs = delay.as_secs(),
                        "listener call failed; retrying"
                    );
                    tokio::time::sleep(*delay).await;
                }
                None => {
                    return Err(err.context(format!("{what}: listener unreachable after retries")));
                }
            },
        }
    }
}

/// The resident's HTTP client for the listener's resident door. Every call
/// carries this boot's token. Deltas and context calls retry transient
/// transport failures in place (see [`LISTENER_RETRY_DELAYS`]); a refusal
/// (replaced token, vanished wave) or an exhausted retry ladder means the
/// listener is gone — the caller ends cleanly.
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
                .timeout(LISTENER_CALL_TIMEOUT)
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
    /// Transient failures retry in place before the caller may conclude
    /// ListenerGone.
    pub async fn send_deltas(&self, deltas: Vec<ResidentDelta>) -> Result<()> {
        if deltas.is_empty() {
            return Ok(());
        }
        with_retries("send deltas", &LISTENER_RETRY_DELAYS, || {
            let request = PostDeltasRequest {
                deltas: deltas.clone(),
            };
            async move {
                let response = self
                    .http
                    .post(format!("http://{}/resident/deltas", self.endpoint))
                    .header(RESIDENT_TOKEN_HEADER, &self.token)
                    .json(&request)
                    .send()
                    .await
                    .map_err(classify)?;
                response.error_for_status().map_err(classify)?;
                Ok(())
            }
        })
        .await
    }

    /// The pre-turn snapshot (also freshens the listener's store fold).
    /// Retries like [`ListenerClient::send_deltas`] — the listener's poll
    /// can be slow.
    pub async fn context(&self) -> Result<ContextResponse> {
        with_retries("fetch context", &LISTENER_RETRY_DELAYS, || async {
            let response = self
                .http
                .get(format!("http://{}/resident/context", self.endpoint))
                .header(RESIDENT_TOKEN_HEADER, &self.token)
                .send()
                .await
                .map_err(classify)?;
            let response = response.error_for_status().map_err(classify)?;
            response.json().await.map_err(classify)
        })
        .await
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

        // Files only (the --flowloop-only path).
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

    /// The retry ladder: transient errors retry through the delays; fatal
    /// errors (the listener answered and refused) stop on the first attempt;
    /// exhausted transients give up after every rung.
    #[tokio::test]
    async fn with_retries_discriminates_transient_from_fatal() {
        use std::sync::atomic::{AtomicU32, Ordering};

        // Transient twice, then success: the ladder rides it out.
        let calls = AtomicU32::new(0);
        let value = with_retries("test", &[Duration::ZERO, Duration::ZERO], || {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            async move {
                if n < 2 {
                    Err(CallError::Transient(anyhow!("slow listener")))
                } else {
                    Ok(n)
                }
            }
        })
        .await
        .expect("third attempt succeeds");
        assert_eq!(value, 2);

        // Fatal: one attempt, no retry — the listener said no.
        let calls = AtomicU32::new(0);
        let err = with_retries::<(), _, _>("test", &[Duration::ZERO, Duration::ZERO], || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(CallError::Fatal(anyhow!("401 unauthorized"))) }
        })
        .await
        .expect_err("fatal is final");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no retry on refusal");
        assert!(err.to_string().contains("listener refused"), "{err:#}");

        // Transient forever: three attempts (initial + both delays), then gone.
        let calls = AtomicU32::new(0);
        let err = with_retries::<(), _, _>("test", &[Duration::ZERO, Duration::ZERO], || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(CallError::Transient(anyhow!("connection refused"))) }
        })
        .await
        .expect_err("exhausted retries conclude ListenerGone");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "three attempts");
        assert!(err.to_string().contains("after retries"), "{err:#}");
    }

    /// Over the real wire: a listener that ANSWERS with 401 (a replaced
    /// boot's token) is fatal immediately — no retry ladder, no ~30s stall.
    #[tokio::test]
    async fn refused_listener_call_is_fatal_not_retried() {
        use crate::wave::runtime::WaveRuntime;

        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime,
            server::ResidentDoor::new("right-token"),
            server::SubagentDoor::new(),
            None,
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let client = ListenerClient::new(addr.to_string(), "wrong-token".to_string());
        let started = std::time::Instant::now();
        let err = client
            .send_deltas(vec![ResidentDelta::ThreadStarted {
                vendor: "codex".into(),
                thread_id: "t-1".into(),
            }])
            .await
            .expect_err("wrong token is refused");
        assert!(err.to_string().contains("listener refused"), "{err:#}");
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "refusal did not walk the retry ladder"
        );
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
    /// worktree: created on first boot, reused after — the flowloop never runs in
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
